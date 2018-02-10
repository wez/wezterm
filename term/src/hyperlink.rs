//! Handling hyperlinks.
//! This gist describes an escape sequence for explicitly managing hyperlinks:
//! https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5fedaA
//! We use that as the foundation of our hyperlink support, and the game
//! plan is to then implicitly enable the hyperlink attribute for a cell
//! as we recognize linkable input text during print() processing.

use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hyperlink {
    /// The target
    pub url: String,
    /// The identifier.  This can be used by the render layer to determine
    /// which cells to underline on hover.  This is for the usecase where
    /// an application has drawn windows in the window and the URL has
    /// wrapped lines within such a window.
    pub id: String,
}

impl Hyperlink {
    pub fn new(url: &str, params: &HashMap<&str, &str>) -> Self {
        let id = params.get("id").unwrap_or(&"");
        Self {
            url: url.into(),
            id: (**id).into(),
        }
    }
}

/// The spec says that the escape sequence is of the form:
/// OSC 8 ; params ; URI BEL|ST
/// params is an optional list of key=value assignments,
/// separated by the : character. Example: id=xyz123:foo=bar:baz=quux.
/// This function parses such a string and returns the mapping
/// of key to value.  Malformed input causes subsequent key/value pairs
/// to be skipped, returning the data successfully parsed out so far.
pub fn parse_link_params(params: &str) -> HashMap<&str, &str> {
    let mut map = HashMap::new();
    for kv in params.split(":") {
        let mut iter = kv.splitn(2, "=");
        let key = iter.next();
        let value = iter.next();
        match (key, value) {
            (Some(key), Some(value)) => map.insert(key, value),
            _ => break,
        };
    }

    map
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_link() {
        assert_eq!(parse_link_params(""), hashmap!{});
        assert_eq!(parse_link_params("foo"), hashmap!{});
        assert_eq!(
            parse_link_params("foo=bar=baz"),
            hashmap!{"foo" => "bar=baz"}
        );
        assert_eq!(parse_link_params("foo=bar"), hashmap!{"foo" => "bar"});

        assert_eq!(
            parse_link_params("id=1234:foo=bar"),
            hashmap!{
                "id" => "1234",
                "foo" => "bar"
            }
        );
        assert_eq!(
            parse_link_params("id=1234:foo=bar:"),
            hashmap!{
                "id" => "1234",
                "foo" => "bar"
            }
        );
    }
}
