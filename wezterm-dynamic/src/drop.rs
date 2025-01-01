use crate::Value;

/// Non-recursive drop implementation.
/// This is taken from dtolnay's miniserde library
/// and is reproduced here under the terms of its
/// MIT license
pub fn safely(value: Value) {
    match value {
        Value::Array(_) | Value::Object(_) => {}
        _ => return,
    }

    let mut stack = Vec::new();
    stack.push(value);
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(vec) => {
                for child in vec {
                    stack.push(child);
                }
            }
            Value::Object(map) => {
                for (_, child) in map {
                    stack.push(child);
                }
            }
            _ => {}
        }
    }
}
