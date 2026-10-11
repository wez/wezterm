use std::path::{Path, PathBuf};
use wezterm_dynamic::{FromDynamic, FromDynamicOptions, ToDynamic, Value};

/// Resolve a possibly-relative path against a base directory, leaving
/// absolute paths unchanged.
trait ResolveRelative {
    fn join_relative_to(&self, base: &Path) -> PathBuf;
}

impl ResolveRelative for PathBuf {
    fn join_relative_to(&self, base: &Path) -> PathBuf {
        if self.is_absolute() {
            self.clone()
        } else {
            base.join(self)
        }
    }
}

/// A strong-typed path to a Ghostty/shadertoy convention GLSL shader.
/// Wraps a PathBuf so the type system distinguishes it from native WGSL paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GhosttyPathBuf(PathBuf);

impl GhosttyPathBuf {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Return a new instance with the path resolved relative to `base`.
    /// If the path is already absolute, returns a clone unchanged.
    pub fn join_relative_to(&self, base: &Path) -> Self {
        Self(self.0.join_relative_to(base))
    }
}

impl std::ops::Deref for GhosttyPathBuf {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl From<PathBuf> for GhosttyPathBuf {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

/// An imported (non-native) shader path that requires cross-compilation to WGSL.
/// Each variant corresponds to a supported foreign shader convention.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImportedShaderPathBuf {
    Ghostty(GhosttyPathBuf),
}

impl ImportedShaderPathBuf {
    pub fn as_path(&self) -> &Path {
        match self {
            ImportedShaderPathBuf::Ghostty(p) => p.as_path(),
        }
    }

    /// Return a new instance with the path resolved relative to `base`.
    pub fn join_relative_to(&self, base: &Path) -> Self {
        match self {
            ImportedShaderPathBuf::Ghostty(p) => {
                ImportedShaderPathBuf::Ghostty(p.join_relative_to(base))
            }
        }
    }
}

impl std::ops::Deref for ImportedShaderPathBuf {
    type Target = Path;
    fn deref(&self) -> &Path {
        self.as_path()
    }
}

/// A shader path in the `custom_shaders` config list.
/// `Native` paths are WGSL and need no translation.
/// `Imported` paths are foreign formats that get cross-compiled to WGSL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShaderPathBuf {
    Native(PathBuf),
    Imported(ImportedShaderPathBuf),
}

impl ShaderPathBuf {
    pub fn as_path(&self) -> &Path {
        match self {
            ShaderPathBuf::Native(p) => p,
            ShaderPathBuf::Imported(p) => p.as_path(),
        }
    }

    /// Return a new instance with the path resolved relative to `base`.
    pub fn join_relative_to(&self, base: &Path) -> Self {
        match self {
            ShaderPathBuf::Native(p) => ShaderPathBuf::Native(p.join_relative_to(base)),
            ShaderPathBuf::Imported(p) => ShaderPathBuf::Imported(p.join_relative_to(base)),
        }
    }
}

impl std::ops::Deref for ShaderPathBuf {
    type Target = Path;
    fn deref(&self) -> &Path {
        self.as_path()
    }
}

impl FromDynamic for ShaderPathBuf {
    fn from_dynamic(
        value: &Value,
        _options: FromDynamicOptions,
    ) -> Result<Self, wezterm_dynamic::Error> {
        match value {
            // Bare string → native WGSL path (backwards compatible)
            Value::String(s) => Ok(ShaderPathBuf::Native(s.clone().into())),
            // Tagged object → imported shader, dispatch on "format" field
            Value::Object(obj) => {
                let format = obj.get_by_str("format").ok_or_else(|| {
                    wezterm_dynamic::Error::Message(
                        "missing `format` field in shader config".to_string(),
                    )
                })?;
                let path_value = obj.get_by_str("path").ok_or_else(|| {
                    wezterm_dynamic::Error::Message(
                        "missing `path` field in shader config".to_string(),
                    )
                })?;
                let path: PathBuf =
                    PathBuf::from_dynamic(path_value, FromDynamicOptions::default())?;

                match format {
                    Value::String(f) if f == "Ghostty" => Ok(ShaderPathBuf::Imported(
                        ImportedShaderPathBuf::Ghostty(GhosttyPathBuf(path)),
                    )),
                    other => Err(wezterm_dynamic::Error::Message(format!(
                        "invalid shader format `{}`; expected `Ghostty`",
                        other.variant_name(),
                    ))),
                }
            }
            other => Err(wezterm_dynamic::Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "ShaderPathBuf",
            }),
        }
    }
}

impl ToDynamic for ShaderPathBuf {
    fn to_dynamic(&self) -> Value {
        match self {
            ShaderPathBuf::Native(p) => Value::String(p.to_string_lossy().to_string()),
            ShaderPathBuf::Imported(imported) => {
                let mut obj = wezterm_dynamic::Object::default();
                let (format, path) = match imported {
                    ImportedShaderPathBuf::Ghostty(p) => ("Ghostty", p.as_path()),
                };
                obj.insert(
                    Value::String("format".to_string()),
                    Value::String(format.to_string()),
                );
                obj.insert(
                    Value::String("path".to_string()),
                    Value::String(path.to_string_lossy().to_string()),
                );
                Value::Object(obj)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wezterm_dynamic::Value;

    #[test]
    fn test_bare_string_to_native() {
        let value = Value::String("/path/to/shader.wgsl".to_string());
        let shader = ShaderPathBuf::from_dynamic(&value, FromDynamicOptions::default()).unwrap();
        assert_eq!(
            shader,
            ShaderPathBuf::Native(PathBuf::from("/path/to/shader.wgsl"))
        );
    }

    #[test]
    fn test_tagged_object_to_ghostty() {
        let mut obj = wezterm_dynamic::Object::default();
        obj.insert(
            Value::String("format".to_string()),
            Value::String("Ghostty".to_string()),
        );
        obj.insert(
            Value::String("path".to_string()),
            Value::String("/path/to/crt.glsl".to_string()),
        );
        let value = Value::Object(obj);
        let shader = ShaderPathBuf::from_dynamic(&value, FromDynamicOptions::default()).unwrap();
        assert_eq!(
            shader,
            ShaderPathBuf::Imported(ImportedShaderPathBuf::Ghostty(GhosttyPathBuf(
                PathBuf::from("/path/to/crt.glsl")
            )))
        );
    }

    #[test]
    fn test_invalid_format_rejected() {
        let mut obj = wezterm_dynamic::Object::default();
        obj.insert(
            Value::String("format".to_string()),
            Value::String("Unknown".to_string()),
        );
        obj.insert(
            Value::String("path".to_string()),
            Value::String("/path/to/shader.glsl".to_string()),
        );
        let value = Value::Object(obj);
        let result = ShaderPathBuf::from_dynamic(&value, FromDynamicOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_format_rejected() {
        let mut obj = wezterm_dynamic::Object::default();
        obj.insert(
            Value::String("path".to_string()),
            Value::String("/path/to/shader.glsl".to_string()),
        );
        let value = Value::Object(obj);
        let result = ShaderPathBuf::from_dynamic(&value, FromDynamicOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_native_roundtrip() {
        let original = ShaderPathBuf::Native(PathBuf::from("/path/to/shader.wgsl"));
        let dynamic = original.to_dynamic();
        let roundtrip =
            ShaderPathBuf::from_dynamic(&dynamic, FromDynamicOptions::default()).unwrap();
        assert_eq!(original, roundtrip);
    }

    #[test]
    fn test_ghostty_roundtrip() {
        let original = ShaderPathBuf::Imported(ImportedShaderPathBuf::Ghostty(GhosttyPathBuf(
            PathBuf::from("/path/to/crt.glsl"),
        )));
        let dynamic = original.to_dynamic();
        let roundtrip =
            ShaderPathBuf::from_dynamic(&dynamic, FromDynamicOptions::default()).unwrap();
        assert_eq!(original, roundtrip);
    }
}
