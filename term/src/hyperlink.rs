//! Handling hyperlinks.
//! This gist describes an escape sequence for explicitly managing hyperlinks:
//! <https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5fedaA>
//! We use that as the foundation of our hyperlink support, and the game
//! plan is to then implicitly enable the hyperlink attribute for a cell
//! as we recognize linkable input text during print() processing.

use failure::Error;
use regex::{Captures, Regex};
use serde::{self, Deserialize, Deserializer};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hyperlink {
    /// The target
    pub url: String,
    /// The identifier.  This can be used by the render layer to determine
    /// which cells to underline on hover.  This is for the usecase where
    /// an application has drawn windows in the window and the URL has
    /// wrapped lines within such a window.
    pub id: String,
    /// If the link was produced by an implicit or matching rule,
    /// this field will be set to true.
    pub implicit: bool,
}

impl Hyperlink {
    pub fn new(url: &str, params: &HashMap<&str, &str>) -> Self {
        let id = params.get("id").unwrap_or(&"");
        Self {
            url: url.into(),
            id: (**id).into(),
            implicit: false,
        }
    }

    pub fn with_id(url: &str, id: &str) -> Self {
        Self {
            url: url.into(),
            id: id.into(),
            implicit: false,
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
    for kv in params.split(':') {
        let mut iter = kv.splitn(2, '=');
        let key = iter.next();
        let value = iter.next();
        match (key, value) {
            (Some(key), Some(value)) => map.insert(key, value),
            _ => break,
        };
    }

    map
}

/// In addition to handling explicit escape sequences to enable
/// hyperlinks, we also support defining rules that match text
/// from screen lines and generate implicit hyperlinks.  This
/// can be used both for making http URLs clickable and also to
/// make other text clickable.  For example, you might define
/// a rule that makes bug or issue numbers expand to the corresponding
/// URL to view the details for that issue.
/// The Rule struct is configuration that is passed to the terminal
/// and is evaluated when processing mouse hover events.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    /// The compiled regex for the rule.  This is used to match
    /// against a line of text from the screen (typically the line
    /// over which the mouse is hovering).
    #[serde(deserialize_with = "deserialize_regex")]
    regex: Regex,
    /// The format string that defines how to transform the matched
    /// text into a URL.  For example, a format string of `$0` expands
    /// to the entire matched text, whereas `mailto:$0` expands to
    /// the matched text with a `mailto:` prefix.  More formally,
    /// each instance of `$N` (where N is a number) in the `format`
    /// string is replaced by the capture number N from the regex.
    /// The replacements are carried out in reverse order, starting
    /// with the highest numbered capture first.  This avoids issues
    /// with ambiguous replacement of `$11` vs `$1` in the case of
    /// more complex regexes.
    format: String,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Regex::new(&s).map_err(|e| serde::de::Error::custom(format!("{:?}", e)))
}

/// Holds a resolved rule match.
#[derive(Debug, PartialEq)]
pub struct RuleMatch {
    /// Holds the span (measured in bytes) of the matched text
    pub range: Range<usize>,
    /// Holds the created Hyperlink object that should be associated
    /// the cells that correspond to the span.
    pub link: Rc<Hyperlink>,
}

/// An internal intermediate match result
struct Match<'t> {
    rule: &'t Rule,
    captures: Captures<'t>,
}

impl<'t> Match<'t> {
    /// Returns the length of the matched text in bytes (not cells!)
    fn len(&self) -> usize {
        let c0 = self.captures.get(0).unwrap();
        c0.end() - c0.start()
    }

    /// Returns the span of the matched text, measured in bytes (not cells!)
    fn range(&self) -> Range<usize> {
        let c0 = self.captures.get(0).unwrap();
        c0.start()..c0.end()
    }

    /// Expand replacements in the format string to yield the URL
    /// The replacement is as described on Rule::format.
    fn expand(&self) -> String {
        let mut result = self.rule.format.clone();
        // Start with the highest numbered capture and decrement.
        // This avoids ambiguity when replacing $11 vs $1.
        for n in (0..self.captures.len()).rev() {
            let search = format!("${}", n);
            result = result.replace(&search, self.captures.get(n).unwrap().as_str());
        }
        result
    }
}

impl Rule {
    /// Construct a new rule.  It may fail if the regex is invalid.
    pub fn new(regex: &str, format: &str) -> Result<Self, Error> {
        Ok(Self {
            regex: Regex::new(regex)?,
            format: format.to_owned(),
        })
    }

    /// Given a line of text from the terminal screen, and a set of
    /// rules, return the set of RuleMatches.
    pub fn match_hyperlinks(line: &str, rules: &[Rule]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();
        for rule in rules.iter() {
            for captures in rule.regex.captures_iter(line) {
                matches.push(Match { rule, captures });
            }
        }
        // Sort the matches by descending match length.
        // This is to avoid confusion if multiple rules match the
        // same sections of text.
        matches.sort_by(|a, b| b.len().cmp(&a.len()));

        matches
            .into_iter()
            .map(|m| {
                let url = m.expand();
                let link = Rc::new(Hyperlink {
                    url,
                    id: "".to_owned(),
                    implicit: true,
                });
                RuleMatch {
                    link,
                    range: m.range(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_implicit() {
        let rules = vec![
            Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap(),
            Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
        ];

        assert_eq!(
            Rule::match_hyperlinks("  http://example.com", &rules),
            vec![
                RuleMatch {
                    range: 2..20,
                    link: Rc::new(Hyperlink {
                        url: "http://example.com".to_owned(),
                        id: "".to_owned(),
                        implicit: true,
                    }),
                },
            ]
        );

        assert_eq!(
            Rule::match_hyperlinks("  foo@example.com woot@example.com", &rules),
            vec![
                // Longest match first
                RuleMatch {
                    range: 18..34,
                    link: Rc::new(Hyperlink {
                        url: "mailto:woot@example.com".to_owned(),
                        id: "".to_owned(),
                        implicit: true,
                    }),
                },
                RuleMatch {
                    range: 2..17,
                    link: Rc::new(Hyperlink {
                        url: "mailto:foo@example.com".to_owned(),
                        id: "".to_owned(),
                        implicit: true,
                    }),
                },
            ]
        );
    }

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
