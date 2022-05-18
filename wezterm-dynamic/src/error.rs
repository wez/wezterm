use crate::fromdynamic::{FromDynamicOptions, UnknownFieldAction};
use crate::value::Value;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("`{}` is not a valid {} variant. {}", .variant_name, .type_name, Self::possible_matches(.variant_name, &.possible))]
    InvalidVariantForType {
        variant_name: String,
        type_name: &'static str,
        possible: &'static [&'static str],
    },
    #[error("`{}` is not a valid {} field. {}", .field_name, .type_name, Self::possible_matches(.field_name, &.possible))]
    UnknownFieldForStruct {
        field_name: String,
        type_name: &'static str,
        possible: &'static [&'static str],
    },
    #[error("{}", .0)]
    Message(String),
    #[error("Cannot coerce vec of size {} to array of size {}", .vec_size, .array_size)]
    ArraySizeMismatch { vec_size: usize, array_size: usize },
    #[error("Cannot convert `{}` to `{}`", .source_type, .dest_type)]
    NoConversion {
        source_type: String,
        dest_type: &'static str,
    },
    #[error("Expected char to be a string with a single character")]
    CharFromWrongSizedString,
    #[error("Expected a valid `{}` variant name as single key in object, but there are {} keys", .type_name, .num_keys)]
    IncorrectNumberOfEnumKeys {
        type_name: &'static str,
        num_keys: usize,
    },
    #[error("Error in {}::{}: {:#}", .type_name, .field_name, .error)]
    ErrorInField {
        type_name: &'static str,
        field_name: &'static str,
        error: String,
    },
    #[error("`{}` is not a valid type to use as a field name in `{}`", .key_type, .type_name)]
    InvalidFieldType {
        type_name: &'static str,
        key_type: String,
    },
}

impl Error {
    fn compute_unknown_fields(
        type_name: &'static str,
        object: &crate::Object,
        possible: &'static [&'static str],
    ) -> Vec<Self> {
        let mut errors = vec![];

        for key in object.keys() {
            match key {
                Value::String(s) => {
                    if !possible.contains(&s.as_str()) {
                        errors.push(Self::UnknownFieldForStruct {
                            field_name: s.to_string(),
                            type_name,
                            possible: possible.clone(),
                        });
                    }
                }
                other => {
                    errors.push(Self::InvalidFieldType {
                        type_name,
                        key_type: other.variant_name().to_string(),
                    });
                }
            }
        }

        errors
    }

    pub fn raise_unknown_fields(
        options: FromDynamicOptions,
        type_name: &'static str,
        object: &crate::Object,
        possible: &'static [&'static str],
    ) -> Result<(), Self> {
        if options.unknown_fields == UnknownFieldAction::Ignore {
            return Ok(());
        }

        let errors = Self::compute_unknown_fields(type_name, object, possible);
        if errors.is_empty() {
            return Ok(());
        }

        let show_warning = options.unknown_fields == UnknownFieldAction::Warn || errors.len() > 1;

        if show_warning {
            for err in &errors {
                log::warn!("{:#}", err);
            }
        }

        if options.unknown_fields == UnknownFieldAction::Deny {
            for err in errors {
                return Err(err);
            }
        }

        Ok(())
    }

    fn possible_matches(used: &str, possible: &'static [&'static str]) -> String {
        // Produce similar field name list
        let mut candidates: Vec<(f64, &str)> = possible
            .iter()
            .map(|&name| (strsim::jaro_winkler(used, name), name))
            .filter(|(confidence, _)| *confidence > 0.8)
            .collect();
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let suggestions: Vec<&str> = candidates.into_iter().map(|(_, name)| name).collect();

        // Filter the suggestions out of the allowed field names
        // and sort what remains.
        let mut fields: Vec<&str> = possible
            .iter()
            .filter(|&name| !suggestions.iter().any(|candidate| candidate == name))
            .copied()
            .collect();
        fields.sort_unstable();

        let mut message = String::new();

        match suggestions.len() {
            0 => {}
            1 => message.push_str(&format!("Did you mean `{}`?", suggestions[0])),
            _ => {
                message.push_str("Did you mean one of ");
                for (idx, candidate) in suggestions.iter().enumerate() {
                    if idx > 0 {
                        message.push_str(", ");
                    }
                    message.push('`');
                    message.push_str(candidate);
                    message.push('`');
                }
                message.push('?');
            }
        }
        if !fields.is_empty() {
            if suggestions.is_empty() {
                message.push_str("Possible items are ");
            } else {
                message.push_str(" Other possible items are ");
            }
            let limit = 5;
            for (idx, candidate) in fields.iter().enumerate() {
                if idx > 0 {
                    message.push_str(", ");
                }
                message.push('`');
                message.push_str(candidate);
                message.push('`');

                if idx > limit {
                    break;
                }
            }
            if fields.len() > limit {
                message.push_str(&format!(" and {} others", fields.len() - limit));
            }
            message.push('.');
        }

        message
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error::Message(s)
    }
}
