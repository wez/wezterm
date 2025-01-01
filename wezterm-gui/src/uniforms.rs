use ::window::glium::uniforms::{AsUniformValue, UniformValue, Uniforms};
use std::borrow::Cow;

/// Builds up the list of name/values that we will pass to the shader program.
/// glium provides a uniform! macro that builds an aggretated UniformStorage
/// type that makes it awkward to pass things that are more complex than primitive
/// types, because everything must be known statically.
/// The builder works a bit more dynamically by accumulating references to
/// the names and values.
/// When binding structs to the shader, the name strings are dynamically
/// allocated because glsl expects to bind the the `bar` field of struct `foo`
/// using a name like "foo.bar".
/// A companion trait `UniformStruct` is used to aid in defining structs
/// that can be passed as uniforms.
#[derive(Default)]
pub struct UniformBuilder<'a> {
    entries: Vec<(Cow<'a, str>, UniformValue<'a>)>,
}

/// Implement this trait on a struct that you wish to pass to a shader
/// program as a uniform.
/// In your add_fields impl, you should call builder.add_struct_field
/// for each of the fields in your struct, passing through the struct_name.
pub trait UniformStruct<'a> {
    fn add_fields(&'a self, struct_name: &str, builder: &mut UniformBuilder<'a>);
}

impl<'a> UniformBuilder<'a> {
    /// Add a simple named uniform to the shader
    pub fn add<V: AsUniformValue>(&mut self, name: &'a str, v: &'a V) {
        self.entries
            .push((Cow::Borrowed(name), v.as_uniform_value()));
    }

    /// Add a struct uniform to the shader
    pub fn add_struct<S: UniformStruct<'a>>(&mut self, struct_name: &str, s: &'a S) {
        s.add_fields(struct_name, self);
    }

    /// Implementations of UniformStruct should call this for each
    /// struct field they are adding
    pub fn add_struct_field<V: AsUniformValue>(&mut self, struct_name: &str, name: &str, v: &'a V) {
        self.entries.push((
            Cow::Owned(format!("{struct_name}.{name}")),
            v.as_uniform_value(),
        ));
    }
}

/// This is the glue that allows glium to bind the uniforms
impl<'a> Uniforms for UniformBuilder<'a> {
    fn visit_values<'b, F: FnMut(&str, UniformValue<'b>)>(&'b self, mut output: F) {
        for (name, value) in &self.entries {
            output(name, *value);
        }
    }
}
