//! Property metadata after model loading.
//!
//! Same idea as `declaration.rs`: take the chatty metamodel JSON shape and turn
//! it into something the validator can work with without re-parsing JSON nodes
//! over and over.

use crate::error::ConcertoError;

/// One property declared on a class-like type.
///
/// This is the flattened version after loading, not the raw metamodel JSON node.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Property name from the model.
    pub name: String,
    /// Namespace of the declaration that introduced this property.
    pub declaring_namespace: String,
    /// Whether the property can be absent.
    pub is_optional: bool,
    /// Whether the property is an array.
    pub is_array: bool,
    /// Semantic property type plus any attached validator.
    pub property_type: PropertyType,
}

/// Semantic type of a property.
///
/// Concerto has more source-level surface area than this, but these are the
/// variants the validator actually needs once parsing is done.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyType {
    /// String with optional validators.
    String(Option<StringValidator>),
    /// Boolean value.
    Boolean,
    /// Integer with optional range limits.
    Integer(Option<NumericValidator<i64>>),
    /// Long with optional range limits.
    Long(Option<NumericValidator<i64>>),
    /// Double with optional range limits.
    Double(Option<NumericValidator<f64>>),
    /// RFC 3339 date-time string.
    DateTime,
    /// Relationship URI that points at another declaration.
    Relationship {
        /// Referenced target type.
        type_ref: String,
    },
    /// Nested object reference.
    ObjectRef {
        /// Referenced target type.
        type_ref: String,
    },
}

impl PropertyType {
    /// Returns a short human-readable label used in diagnostics and CLI output.
    pub fn describe(&self) -> String {
        match self {
            PropertyType::String(_) => "String".to_string(),
            PropertyType::Boolean => "Boolean".to_string(),
            PropertyType::Integer(_) => "Integer".to_string(),
            PropertyType::Long(_) => "Long".to_string(),
            PropertyType::Double(_) => "Double".to_string(),
            PropertyType::DateTime => "DateTime".to_string(),
            PropertyType::Relationship { type_ref } => format!("Relationship<{type_ref}>"),
            PropertyType::ObjectRef { type_ref } => format!("Object<{type_ref}>"),
        }
    }
}

/// String-specific validation rules.
///
/// Regex and length live together here because they both end up attached to
/// the same string property in practice.
#[derive(Debug, Clone, PartialEq)]
pub struct StringValidator {
    /// Optional regex, compiled when the model loads.
    pub regex: Option<StringRegex>,
    /// Optional inclusive minimum length.
    pub min_length: Option<usize>,
    /// Optional inclusive maximum length.
    pub max_length: Option<usize>,
}

/// A `regex=` validator, compiled once when the model loads.
///
/// Concerto patterns are JavaScript regular expressions, so they're compiled with `regress`,
/// which follows ECMAScript semantics: flags, ASCII-only `\d`, `\w` and `\b`, lookaround and
/// backreferences all behave as they do in concerto-core. Like the JS `RegExp` that
/// concerto-core uses by default, it backtracks, so a pathological pattern can be slow on some
/// inputs. Patterns come from the model, so treat models as trusted input.
#[derive(Debug, Clone)]
pub struct StringRegex {
    /// The pattern as written in the model, without the surrounding slashes.
    pub pattern: String,
    /// The JavaScript flags, such as `"i"`.
    pub flags: String,
    regex: regress::Regex,
}

impl PartialEq for StringRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.flags == other.flags
    }
}

impl StringRegex {
    /// Compiles `pattern` with JavaScript `flags`, rejecting what `new RegExp` would reject.
    ///
    /// # Errors
    ///
    /// Returns `ConcertoError::Semantic` if the flags or the pattern are invalid.
    pub fn new(pattern: &str, flags: &str) -> Result<Self, ConcertoError> {
        // same rules as `new RegExp`: known flags only, each at most once, not both `u` and `v`
        let mut seen = String::new();
        for flag in flags.chars() {
            if !"dgimsuvy".contains(flag) || seen.contains(flag) {
                return Err(ConcertoError::Semantic(format!(
                    "bad regex flags '{flags}'"
                )));
            }
            seen.push(flag);
        }
        if seen.contains('u') && seen.contains('v') {
            return Err(ConcertoError::Semantic(format!(
                "bad regex flags '{flags}'"
            )));
        }

        let regex = regress::Regex::with_flags(pattern, flags)
            .map_err(|error| ConcertoError::Semantic(format!("bad regex '{pattern}': {error}")))?;

        Ok(Self {
            pattern: pattern.to_string(),
            flags: flags.to_string(),
            regex,
        })
    }

    /// Tests `value` the way concerto-core's `RegExp.test` does.
    pub fn is_match(&self, value: &str) -> bool {
        let found = if value.is_ascii() {
            // ASCII reads the same as UTF-8, UTF-16 and UCS-2, so skip the conversion
            self.regex.find(value)
        } else {
            // JS strings are UTF-16, and without `u` or `v` a regex sees each code unit on
            // its own, so for example `.` matches only half of an emoji
            let text: Vec<u16> = value.encode_utf16().collect();
            if self.flags.contains(['u', 'v']) {
                self.regex.find_from_utf16(&text, 0).next()
            } else {
                self.regex.find_from_ucs2(&text, 0).next()
            }
        };

        // concerto-core resets `lastIndex` before every test, so `g` changes nothing and a
        // sticky (`y`) regex has to match at the very start
        found.is_some_and(|m| !self.flags.contains('y') || m.start() == 0)
    }
}

/// Numeric lower/upper bound validator.
///
/// Used for Integer, Long, and Double with inclusive bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericValidator<T> {
    /// Inclusive lower bound.
    pub lower: Option<T>,
    /// Inclusive upper bound.
    pub upper: Option<T>,
}
