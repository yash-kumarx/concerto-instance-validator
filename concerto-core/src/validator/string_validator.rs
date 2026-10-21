//! String validation helpers.
//!
//! Small file, still worth having. Keeps the regex and length logic out of the
//! main validator walk. Regexes are compiled once, when the model loads.

use crate::property::StringValidator;
use crate::validator::instance_validator::{ErrorKind, ValidationError};

pub(crate) fn validate_string(
    value: &str,
    validator: &Option<StringValidator>,
    path: &str,
    property_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let string_validator = match validator {
        Some(string_validator) => string_validator,
        None => return errors,
    };

    if let Some(regex) = &string_validator.regex {
        if !regex.is_match(value) {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!(
                    "'{}' doesn't match /{}/{}",
                    value, regex.pattern, regex.flags
                ),
                error_type: ErrorKind::PatternMismatch {
                    pattern: regex.pattern.clone(),
                },
            });
        }
    }

    // .len() counts bytes and .chars() counts code points, but concerto-core checks
    // javascript's `value.length`, which counts utf-16 code units. those only differ
    // outside the BMP: an emoji has length 2 there
    let length = value.encode_utf16().count();

    if let Some(min_length) = string_validator.min_length {
        if length < min_length {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{property_name}' is too short: {length} < {min_length}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("minLength={min_length}"),
                },
            });
        }
    }

    if let Some(max_length) = string_validator.max_length {
        if length > max_length {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{property_name}' is too long: {length} > {max_length}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("maxLength={max_length}"),
                },
            });
        }
    }

    errors
}
