/// The shader type a uniform field maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformType {
    Vec2,
    Float,
    UInt,
    Vec4,
}

/// A single field of a shader uniform buffer, as reflected from a Rust
/// struct by the `UniformBuffer` derive.
#[derive(Debug, Clone, Copy)]
pub struct UniformField {
    pub name: &'static str,
    pub ty: UniformType,
}
