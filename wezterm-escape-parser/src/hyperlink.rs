use crate::{Result, ensure, format_err};
use core::hash::{Hash, Hasher};
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use wezterm_dynamic::{FromDynamic, ToDynamic};

use crate::allocate::*;

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
        ensure!(osc.len() >= 3, "wrong param count");
        // Take everything after the params, so any further `;…` are counted as part of the URI
        // e.g. `https://example.com/a;b;c=d` would have been split in 3 parts by the OSC splitter.
        let uri = osc[2..].join(&b';');
        if osc[1].is_empty() && uri.is_empty() {
            // Clearing current hyperlink
            Ok(None)
        } else {
            let param_str = String::from_utf8(osc[1].to_vec())?;
            let uri = String::from_utf8(uri)?;

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

impl core::fmt::Display for Hyperlink {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
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
