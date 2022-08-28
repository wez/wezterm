//! Handling hyperlinks.
//! This gist describes an escape sequence for explicitly managing hyperlinks:
//! <https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5fedaA>
//! We use that as the foundation of our hyperlink support, and the game
//! plan is to then implicitly enable the hyperlink attribute for a cell
//! as we recognize linkable input text during print() processing.
use crate::{ensure, format_err, Result};
use regex::{Captures, Regex};
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use wezterm_dynamic::{FromDynamic, FromDynamicOptions, ToDynamic, Value};

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, FromDynamic, ToDynamic)]
pub struct Hyperlink {
    params: HashMap<String, String>,
    uri: String,
    /// If the link was produced by an implicit or matching rule,
    /// this field will be set to true.
    implicit: bool,
}

impl Hyperlink {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn compute_shape_hash<H: Hasher>(&self, hasher: &mut H) {
        self.uri.hash(hasher);
        for (k, v) in &self.params {
            k.hash(hasher);
            v.hash(hasher);
        }
        self.implicit.hash(hasher);
    }

    pub fn params(&self) -> &HashMap<String, String> {
        &self.params
    }

    pub fn new<S: Into<String>>(uri: S) -> Self {
        Self {
            uri: uri.into(),
            params: HashMap::new(),
            implicit: false,
        }
    }

    #[inline]
    pub fn is_implicit(&self) -> bool {
        self.implicit
    }

    pub fn new_implicit<S: Into<String>>(uri: S) -> Self {
        Self {
            uri: uri.into(),
            params: HashMap::new(),
            implicit: true,
        }
    }

    pub fn new_with_id<S: Into<String>, S2: Into<String>>(uri: S, id: S2) -> Self {
        let mut params = HashMap::new();
        params.insert("id".into(), id.into());
        Self {
            uri: uri.into(),
            params,
            implicit: false,
        }
    }

    pub fn new_with_params<S: Into<String>>(uri: S, params: HashMap<String, String>) -> Self {
        Self {
            uri: uri.into(),
            params,
            implicit: false,
        }
    }

    pub fn parse(osc: &[&[u8]]) -> Result<Option<Hyperlink>> {
        ensure!(osc.len() == 3, "wrong param count");
        if osc[1].is_empty() && osc[2].is_empty() {
            // Clearing current hyperlink
            Ok(None)
        } else {
            let param_str = String::from_utf8(osc[1].to_vec())?;
            let uri = String::from_utf8(osc[2].to_vec())?;

            let mut params = HashMap::new();
            if !param_str.is_empty() {
                for pair in param_str.split(':') {
                    let mut iter = pair.splitn(2, '=');
                    let key = iter.next().ok_or_else(|| format_err!("bad params"))?;
                    let value = iter.next().ok_or_else(|| format_err!("bad params"))?;
                    params.insert(key.to_owned(), value.to_owned());
                }
            }

            Ok(Some(Hyperlink::new_with_params(uri, params)))
        }
    }
}

impl Display for Hyperlink {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), FmtError> {
        write!(f, "8;")?;
        for (idx, (k, v)) in self.params.iter().enumerate() {
            // TODO: protect against k, v containing : or =
            if idx > 0 {
                write!(f, ":")?;
            }
            write!(f, "{}={}", k, v)?;
        }
        // TODO: ensure that link.uri doesn't contain characters
        // outside the range 32-126.  Need to pull in a URI/URL
        // crate to help with this.
        write!(f, ";{}", self.uri)?;

        Ok(())
    }
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
#[cfg_attr(feature = "use_serde", derive(Deserialize, Serialize))]
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct Rule {
    /// The compiled regex for the rule.  This is used to match
    /// against a line of text from the screen (typically the line
    /// over which the mouse is hovering).
    #[cfg_attr(
        feature = "use_serde",
        serde(
            deserialize_with = "deserialize_regex",
            serialize_with = "serialize_regex"
        )
    )]
    #[dynamic(into = "RegexWrap", try_from = "RegexWrap")]
    pub regex: Regex,
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
    pub format: String,
}

struct RegexWrap(Regex);

impl FromDynamic for RegexWrap {
    fn from_dynamic(
        value: &Value,
        options: FromDynamicOptions,
    ) -> std::result::Result<RegexWrap, wezterm_dynamic::Error> {
        let s = String::from_dynamic(value, options)?;
        Ok(RegexWrap(Regex::new(&s).map_err(|e| e.to_string())?))
    }
}

impl From<&Regex> for RegexWrap {
    fn from(regex: &Regex) -> RegexWrap {
        RegexWrap(regex.clone())
    }
}

impl Into<Regex> for RegexWrap {
    fn into(self) -> Regex {
        self.0
    }
}

impl ToDynamic for RegexWrap {
    fn to_dynamic(&self) -> Value {
        self.0.to_string().to_dynamic()
    }
}

#[cfg(feature = "use_serde")]
fn deserialize_regex<'de, D>(deserializer: D) -> std::result::Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Regex::new(&s).map_err(|e| serde::de::Error::custom(format!("{:?}", e)))
}

#[cfg(feature = "use_serde")]
fn serialize_regex<S>(regex: &Regex, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = regex.to_string();
    s.serialize(serializer)
}

/// Holds a resolved rule match.
#[derive(Debug, PartialEq)]
pub struct RuleMatch {
    /// Holds the span (measured in bytes) of the matched text
    pub range: Range<usize>,
    /// Holds the created Hyperlink object that should be associated
    /// the cells that correspond to the span.
    pub link: Arc<Hyperlink>,
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
            if let Some(rep) = self.captures.get(n) {
                result = result.replace(&search, rep.as_str());
            } else {
                result = result.replace(&search, "");
            }
        }
        result
    }
}

impl Rule {
    /// Construct a new rule.  It may fail if the regex is invalid.
    pub fn new(regex: &str, format: &str) -> Result<Self> {
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
                let link = Arc::new(Hyperlink::new_implicit(url));
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
            vec![RuleMatch {
                range: 2..20,
                link: Arc::new(Hyperlink::new_implicit("http://example.com")),
            }]
        );

        assert_eq!(
            Rule::match_hyperlinks("  foo@example.com woot@example.com", &rules),
            vec![
                // Longest match first
                RuleMatch {
                    range: 18..34,
                    link: Arc::new(Hyperlink::new_implicit("mailto:woot@example.com")),
                },
                RuleMatch {
                    range: 2..17,
                    link: Arc::new(Hyperlink::new_implicit("mailto:foo@example.com")),
                },
            ]
        );
    }
}
