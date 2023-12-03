use crate::fromdynamic::{FromDynamicOptions, UnknownFieldAction};
use crate::object::Object;
use crate::value::Value;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

pub trait WarningCollector {
    fn warn(&self, message: String);
}

thread_local! {
    static WARNING_COLLECTOR: RefCell<Option<Box<dyn WarningCollector>>> = RefCell::new(None);
}

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
    #[error("Error processing {}::{}: {:#}", .type_name, .field_name, .error)]
    ErrorInField {
        type_name: &'static str,
        field_name: &'static str,
        error: String,
    },
    #[error("Error processing {} (types: {}) {:#}", .field_name.join("."), .type_name.join(", "), .error)]
    ErrorInNestedField {
        type_name: Vec<&'static str>,
        field_name: Vec<&'static str>,
        error: String,
    },
    #[error("`{}` is not a valid type to use as a field name in `{}`", .key_type, .type_name)]
    InvalidFieldType {
        type_name: &'static str,
        key_type: String,
    },
    #[error("{}::{} is deprecated: {}", .type_name, .field_name, .reason)]
    DeprecatedField {
        type_name: &'static str,
        field_name: &'static str,
        reason: &'static str,
    },
}

impl Error {
    /// Log a warning; if a warning collector is set for the current thread,
    /// use it, otherwise, log a regular warning message.
    pub fn warn(message: String) {
        WARNING_COLLECTOR.with(|collector| {
            let collector = collector.borrow();
            if let Some(collector) = collector.as_ref() {
                collector.warn(message);
            } else {
                log::warn!("{message}");
            }
        });
    }

    pub fn capture_warnings<F: FnOnce() -> T, T>(f: F) -> (T, Vec<String>) {
        let warnings = Rc::new(RefCell::new(vec![]));

        struct Collector {
            warnings: Rc<RefCell<Vec<String>>>,
        }

        impl WarningCollector for Collector {
            fn warn(&self, message: String) {
                self.warnings.borrow_mut().push(message);
            }
        }

        Self::set_warning_collector(Collector {
            warnings: Rc::clone(&warnings),
        });
        let result = f();
        Self::clear_warning_collector();
        let warnings = match Rc::try_unwrap(warnings) {
            Ok(warnings) => warnings.into_inner(),
            Err(warnings) => (*warnings).clone().into_inner(),
        };
        (result, warnings)
    }

    /// Replace the warning collector for the current thread
    fn set_warning_collector<T: WarningCollector + 'static>(c: T) {
        WARNING_COLLECTOR.with(|collector| {
            collector.borrow_mut().replace(Box::new(c));
        });
    }

    /// Clear the warning collector for the current thread
    fn clear_warning_collector() {
        WARNING_COLLECTOR.with(|collector| {
            collector.borrow_mut().take();
        });
    }

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
                            possible,
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

    pub fn raise_deprecated_fields(
        options: FromDynamicOptions,
        type_name: &'static str,
        field_name: &'static str,
        reason: &'static str,
    ) -> Result<(), Self> {
        if options.deprecated_fields == UnknownFieldAction::Ignore {
            return Ok(());
        }
        let err = Self::DeprecatedField {
            type_name,
            field_name,
            reason,
        };

        match options.deprecated_fields {
            UnknownFieldAction::Deny => Err(err),
            UnknownFieldAction::Warn => {
                Self::warn(format!("{:#}", err));
                Ok(())
            }
            UnknownFieldAction::Ignore => unreachable!(),
        }
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
                Self::warn(format!("{:#}", err));
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
            let limit = 5;
            if fields.len() > limit {
                message.push_str(
                    " There are too many alternatives to list here; consult the documentation!",
                );
            } else {
                if suggestions.is_empty() {
                    message.push_str("Possible alternatives are ");
                } else if suggestions.len() == 1 {
                    message.push_str(" The other option is ");
                } else {
                    message.push_str(" Other alternatives are ");
                }
                for (idx, candidate) in fields.iter().enumerate() {
                    if idx > 0 {
                        message.push_str(", ");
                    }
                    message.push('`');
                    message.push_str(candidate);
                    message.push('`');
                }
            }
        }

        message
    }

    pub fn field_context(
        self,
        type_name: &'static str,
        field_name: &'static str,
        obj: &Object,
    ) -> Self {
        let is_leaf = !matches!(self, Self::ErrorInField { .. });
        fn add_obj_context(is_leaf: bool, obj: &Object, message: String) -> String {
            if is_leaf {
                // Show the object as context.
                // However, some objects, like the main config, are very large and
                // it isn't helpful to show that, so only include it when the context
                // is more reasonable.
                let obj_str = format!("{:#?}", obj);
                if obj_str.len() > 128 || obj_str.lines().count() > 10 {
                    message
                } else {
                    format!("{}.\n{}", message, obj_str)
                }
            } else {
                message
            }
        }

        match self {
            Self::NoConversion { source_type, .. } if source_type == "Null" => Self::ErrorInField {
                type_name,
                field_name,
                error: add_obj_context(is_leaf, obj, format!("missing field `{}`", field_name)),
            },
            Self::ErrorInField {
                type_name: child_type,
                field_name: child_field,
                error,
            } => Self::ErrorInNestedField {
                type_name: vec![type_name, child_type],
                field_name: vec![field_name, child_field],
                error,
            },
            Self::ErrorInNestedField {
                type_name: mut child_type,
                field_name: mut child_field,
                error,
            } => Self::ErrorInNestedField {
                type_name: {
                    child_type.insert(0, type_name);
                    child_type
                },
                field_name: {
                    child_field.insert(0, field_name);
                    child_field
                },
                error,
            },
            _ => Self::ErrorInField {
                type_name,
                field_name,
                error: add_obj_context(is_leaf, obj, format!("{:#}", self)),
            },
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error::Message(s)
    }
}
