use crate::config::validate_domain_name;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Default, Debug, Clone, FromDynamic, ToDynamic)]
pub struct SerialDomain {
    /// The name of this specific domain.  Must be unique amongst
    /// all types of domain in the configuration file.
    #[dynamic(validate = "validate_domain_name")]
    pub name: String,

    /// Specifies the serial device name.
    /// On Windows systems this can be a name like `COM0`.
    /// On posix systems this will be something like `/dev/ttyUSB0`.
    /// If omitted, the name will be interpreted as the port.
    pub port: Option<String>,

    /// Set the baud rate.  The default is 9600 baud.
    pub baud: Option<usize>,
}
