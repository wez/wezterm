//! Handling hyperfiles.
//! This gist describes an escape sequence for explicitly managing hyperfiles:
//! <https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5fedaA>
//! We use that as the foundation of our hyperfile support, and the game
//! plan is to then implicitly enable the hyperfile attribute for a cell
//! as we recognize fileable input text during print() processing.
use anyhow::{anyhow, ensure, Error};
use regex::{Captures, Regex};
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::ops::Range;
use std::sync::Arc;

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperfile {
    params: HashMap<String, String>,
    uri: String,
    /// If the file was produced by an implicit or matching rule,
    /// this field will be set to true.
    implicit: bool,
}

impl Hyperfile {
    pub fn uri(&self) -> &str {
        &self.uri
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

    pub fn parse(osc: &[&[u8]]) -> Result<Option<Hyperfile>, Error> {
        ensure!(osc.len() == 3, "wrong param count");
        if osc[1].is_empty() && osc[2].is_empty() {
            // Clearing current hyperfile
            Ok(None)
        } else {
            let param_str = String::from_utf8(osc[1].to_vec())?;
            let uri = String::from_utf8(osc[2].to_vec())?;

            let mut params = HashMap::new();
            if !param_str.is_empty() {
                for pair in param_str.split(':') {
                    let mut iter = pair.splitn(2, '=');
                    let key = iter.next().ok_or_else(|| anyhow!("bad params"))?;
                    let value = iter.next().ok_or_else(|| anyhow!("bad params"))?;
                    params.insert(key.to_owned(), value.to_owned());
                }
            }

            Ok(Some(Hyperfile::new_with_params(uri, params)))
        }
    }
}

impl Display for Hyperfile {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "8;")?;
        for (idx, (k, v)) in self.params.iter().enumerate() {
            // TODO: protect against k, v containing : or =
            if idx > 0 {
                write!(f, ":")?;
            }
            write!(f, "{}={}", k, v)?;
        }
        // TODO: ensure that file.uri doesn't contain characters
        // outside the range 32-126.  Need to pull in a URI/URL
        // crate to help with this.
        write!(f, ";{}", self.uri)?;

        Ok(())
    }
}

/// In addition to handling explicit escape sequences to enable
/// hyperfiles, we also support defining rules that match text
/// from screen lines and generate implicit hyperfiles.  This
/// can be used both for making http URLs clickable and also to
/// make other text clickable.  For example, you might define
/// a rule that makes bug or issue numbers expand to the corresponding
/// URL to view the details for that issue.
/// The Rule struct is configuration that is passed to the terminal
/// and is evaluated when processing mouse hover events.
#[cfg_attr(feature = "use_serde", derive(Deserialize))]
#[derive(Debug, Clone)]
pub struct Rule {
    /// The compiled regex for the rule.  This is used to match
    /// against a line of text from the screen (typically the line
    /// over which the mouse is hovering).
    #[cfg_attr(feature = "use_serde", serde(deserialize_with = "deserialize_regex"))]
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

#[cfg(feature = "use_serde")]
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
    /// Holds the created Hyperfile object that should be associated
    /// the cells that correspond to the span.
    pub file: Arc<Hyperfile>,
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
    pub fn match_hyperfiles(line: &str, rules: &[Rule]) -> Vec<RuleMatch> {
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
                let file = Arc::new(Hyperfile::new_implicit(url));
                RuleMatch {
                    file,
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
            Rule::new(r"^\s*[a-zA-Z0-9\/\_\-\.\ ]+\.?[a-zA-Z0-9]+\:[0-9]+", "$0").unwrap(),
        ];

        assert_eq!(
            Rule::match_hyperfiles("/Users/user/.bash_history:10", &rules),
            vec![RuleMatch {
                range: 2..20,
                file: Arc::new(Hyperfile::new_implicit("/Users/user/.bash_history:10")),
            }]
        );

    }
}
