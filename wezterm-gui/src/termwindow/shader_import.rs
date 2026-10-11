use config::ImportedShaderPathBuf;
use std::path::Path;
use wezterm_shader_types::{UniformField, UniformType};

use crate::termwindow::webgpu::{ResolvedShader, ShaderSource};

/// Errors that can occur during shader import (cross-compilation from
/// a foreign format to WGSL).
#[derive(Debug, thiserror::Error)]
pub enum ShaderImportError {
    #[error("failed to read shader file {path}: {error}")]
    ReadError { path: String, error: std::io::Error },
    #[error("shader file {path} is not valid UTF-8: {error}")]
    InvalidUtf8 {
        path: String,
        error: std::str::Utf8Error,
    },
    #[error("shader file {path} is empty")]
    EmptyShader { path: String },
    #[error("glslang compilation error in {path}: {error}")]
    GlslangError {
        path: String,
        error: glslang::error::GlslangError,
    },
    #[error("SPIR-V parse error in {path}: {error}")]
    SpvParseError {
        path: String,
        error: naga::front::spv::Error,
    },
    #[error("validation error in {path}: {error}")]
    ValidationError {
        path: String,
        error: naga::WithSpan<naga::valid::ValidationError>,
    },
    #[error("WGSL emit error in {path}: {error}")]
    EmitError {
        path: String,
        error: naga::back::wgsl::Error,
    },
}

/// Patched ghostty prefix with a `{{UNIFORM_BLOCK}}` placeholder, rendered
/// at runtime from the reflected `UNIFORM_FIELDS` and cached.
const GHOSTTY_SHADERTOY_PREFIX: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/ghostty_shadertoy_prefix_patched.glsl"
));

fn uniform_type_glsl(ty: UniformType) -> &'static str {
    match ty {
        UniformType::Vec2 => "vec2",
        UniformType::Float => "float",
        UniformType::UInt => "uint",
        UniformType::Vec4 => "vec4",
    }
}

/// Render the wezterm uniform block GLSL declaration from the reflected
/// field list.
fn render_glsl_uniform_block(fields: &[UniformField]) -> String {
    let mut out =
        String::from("layout(set = 1, binding = 0, std140) uniform PostProcessUniform {\n");
    for field in fields {
        out.push_str(&format!(
            "    {} {};\n",
            uniform_type_glsl(field.ty),
            field.name
        ));
    }
    out.push_str("};\n");
    out
}

/// The full ghostty shader prefix: patched template with the uniform block
/// rendered in. Rendered once and cached.
fn ghostty_prefix() -> &'static str {
    static PREFIX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PREFIX.get_or_init(|| {
        GHOSTTY_SHADERTOY_PREFIX.replace(
            "{{UNIFORM_BLOCK}}",
            &render_glsl_uniform_block(crate::termwindow::webgpu::UNIFORM_FIELDS),
        )
    })
}

const GHOSTTY_FULLSCREEN_VERTEX: &str = include_str!("shaders/ghostty_fullscreen_vertex.wgsl");

/// Import (cross-compile) a foreign shader to a resolved WGSL shader.
pub fn import_shader(shader: &ImportedShaderPathBuf) -> Result<ResolvedShader, ShaderImportError> {
    match shader {
        ImportedShaderPathBuf::Ghostty(path) => import_ghostty(path.as_path()),
    }
}

fn import_ghostty(path: &Path) -> Result<ResolvedShader, ShaderImportError> {
    let path_str = path.display().to_string();

    let raw_bytes = std::fs::read(path).map_err(|e| ShaderImportError::ReadError {
        path: path_str.clone(),
        error: e,
    })?;

    // Skip a potential BOM that Windows software may have placed in the file.
    let source_str = std::str::from_utf8(&raw_bytes)
        .map_err(|e| ShaderImportError::InvalidUtf8 {
            path: path_str.clone(),
            error: e,
        })?
        .trim_start_matches('\u{FEFF}');

    if source_str.trim().is_empty() {
        return Err(ShaderImportError::EmptyShader { path: path_str });
    }

    compile_ghostty(source_str, &path_str)
}

fn compile_ghostty(
    shader_source: &str,
    source_label: &str,
) -> Result<ResolvedShader, ShaderImportError> {
    let full_source = format!("{}\n{}", ghostty_prefix(), shader_source);

    let spirv_bytes = compile_glsl_to_spirv(&full_source, source_label)?;

    let spv_options = naga::front::spv::Options::default();
    let mut module = naga::front::spv::parse_u8_slice(&spirv_bytes, &spv_options).map_err(|e| {
        ShaderImportError::SpvParseError {
            path: source_label.to_string(),
            error: e,
        }
    })?;

    rename_entry_point(&mut module);

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator
        .validate(&module)
        .map_err(|e| ShaderImportError::ValidationError {
            path: source_label.to_string(),
            error: e,
        })?;

    let wgsl =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .map_err(|e| ShaderImportError::EmitError {
                path: source_label.to_string(),
                error: e,
            })?;

    let path = std::path::PathBuf::from(source_label);
    Ok(ResolvedShader::new(
        std::sync::Arc::new(ShaderSource::new(
            GHOSTTY_FULLSCREEN_VERTEX.to_string(),
            path.clone(),
        )),
        std::sync::Arc::new(ShaderSource::new(wgsl, path)),
    ))
}

fn compile_glsl_to_spirv(source: &str, path_str: &str) -> Result<Vec<u8>, ShaderImportError> {
    let compiler = glslang::Compiler::acquire().ok_or_else(|| ShaderImportError::GlslangError {
        path: path_str.to_string(),
        error: glslang::error::GlslangError::NoLanguageTarget,
    })?;

    let shader_source = glslang::ShaderSource::from(source.to_string());
    let options = glslang::CompilerOptions {
        source_language: glslang::SourceLanguage::GLSL,
        target: glslang::Target::Vulkan {
            version: glslang::VulkanVersion::Vulkan1_2,
            spirv_version: glslang::SpirvVersion::SPIRV1_5,
        },
        version_profile: None,
        messages: glslang::ShaderMessage::DEFAULT,
    };
    let defines: Option<&[(&str, Option<&str>)]> = None;
    let input = glslang::ShaderInput::new(
        &shader_source,
        glslang::ShaderStage::Fragment,
        &options,
        defines,
        None,
    )
    .map_err(|e| ShaderImportError::GlslangError {
        path: path_str.to_string(),
        error: e,
    })?;

    let shader = compiler
        .create_shader(input)
        .map_err(|e| ShaderImportError::GlslangError {
            path: path_str.to_string(),
            error: e,
        })?;

    shader
        .compile()
        .map(|words: Vec<u32>| words.iter().flat_map(|w| w.to_le_bytes()).collect())
        .map_err(|e| ShaderImportError::GlslangError {
            path: path_str.to_string(),
            error: e,
        })
}

fn rename_entry_point(module: &mut naga::Module) {
    for ep in module.entry_points.iter_mut() {
        if ep.name == "main" {
            ep.name = "fs_postprocess".to_string();
            if let Some(ref mut name) = ep.function.name {
                if name == "main" {
                    *name = "fs_postprocess".to_string();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_import_error_missing_file() {
        let result = import_ghostty(&PathBuf::from("/nonexistent/shader.glsl"));
        assert!(matches!(result, Err(ShaderImportError::ReadError { .. })));
    }

    #[test]
    fn test_import_error_empty_shader() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), b"   \n  \t  ").unwrap();
        let result = import_ghostty(temp.path());
        assert!(matches!(result, Err(ShaderImportError::EmptyShader { .. })));
    }

    #[test]
    fn test_import_simple_shader() {
        let glsl = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    fragColor = texture(iChannel0, uv);
}
"#;
        let result = compile_ghostty(glsl, "simple.glsl");
        assert!(
            result.is_ok(),
            "Simple shader should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_crt() {
        let result = compile_ghostty(include_str!("shaders/test_fixtures/crt.glsl"), "crt.glsl");
        assert!(result.is_ok(), "crt.glsl should import: {:?}", result.err());
    }

    #[test]
    fn test_import_real_bloom() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/bloom.glsl"),
            "bloom.glsl",
        );
        assert!(
            result.is_ok(),
            "bloom.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_dither() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/dither.glsl"),
            "dither.glsl",
        );
        assert!(
            result.is_ok(),
            "dither.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_negative() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/negative.glsl"),
            "negative.glsl",
        );
        assert!(
            result.is_ok(),
            "negative.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_vhs() {
        let result = compile_ghostty(include_str!("shaders/test_fixtures/vhs.glsl"), "vhs.glsl");
        assert!(result.is_ok(), "vhs.glsl should import: {:?}", result.err());
    }

    #[test]
    fn test_import_real_starfield() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/starfield.glsl"),
            "starfield.glsl",
        );
        assert!(
            result.is_ok(),
            "starfield.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_cursor_uniforms() {
        let glsl = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel0, uv);
    vec2 currentCenter = iCurrentCursor.xy + vec2(iCurrentCursor.z * 0.5, -iCurrentCursor.w * 0.5);
    vec2 previousCenter = iPreviousCursor.xy + vec2(iPreviousCursor.z * 0.5, -iPreviousCursor.w * 0.5);
    float age = clamp((iTime - iTimeCursorChange) / 0.14, 0.0, 1.0);
    vec3 color = mix(iCurrentCursorColor.rgb, iPreviousCursorColor.rgb, age);
    fragColor = vec4(mix(terminal.rgb, color, age), terminal.a);
}
"#;
        let result = compile_ghostty(glsl, "cursor.glsl");
        assert!(
            result.is_ok(),
            "Cursor-uniform shader should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_cursor_blaze() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/cursor_blaze.glsl"),
            "cursor_blaze.glsl",
        );
        assert!(
            result.is_ok(),
            "cursor_blaze.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_cursor_lightning() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/cursor_lightning.glsl"),
            "cursor_lightning.glsl",
        );
        assert!(
            result.is_ok(),
            "cursor_lightning.glsl should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_real_in_game_crt_cursor() {
        let result = compile_ghostty(
            include_str!("shaders/test_fixtures/in-game-crt-cursor.glsl"),
            "in-game-crt-cursor.glsl",
        );
        assert!(
            result.is_ok(),
            "in-game-crt-cursor.glsl should import: {:?}",
            result.err()
        );
    }
}
