use escape::EncodeEscape;
use failure;
use num;
use std;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystemCommand {
    SetIconNameAndWindowTitle(String),
    SetWindowTitle(String),
    SetIconName(String),
    SetHyperlink(Option<Hyperlink>),

    Unspecified(Vec<Vec<u8>>),
    #[doc(hidden)]
    __Nonexhaustive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    params: HashMap<String, String>,
    uri: String,
}

impl Hyperlink {
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
        }
    }

    pub fn new_with_id<S: Into<String>, S2: Into<String>>(uri: S, id: S2) -> Self {
        let mut params = HashMap::new();
        params.insert("id".into(), id.into());
        Self {
            uri: uri.into(),
            params,
        }
    }

    pub fn new_with_params<S: Into<String>>(uri: S, params: HashMap<String, String>) -> Self {
        Self {
            uri: uri.into(),
            params,
        }
    }

    pub fn parse(osc: &[&[u8]]) -> Result<Option<Hyperlink>, failure::Error> {
        ensure!(osc.len() == 3, "wrong param count");
        if osc[1].len() == 0 && osc[2].len() == 0 {
            // Clearing current hyperlink
            Ok(None)
        } else {
            let param_str = String::from_utf8(osc[1].to_vec())?;
            let uri = String::from_utf8(osc[2].to_vec())?;

            let mut params = HashMap::new();
            for pair in param_str.split(':') {
                let mut iter = pair.splitn(2, '=');
                let key = iter.next().ok_or_else(|| failure::err_msg("bad params"))?;
                let value = iter.next().ok_or_else(|| failure::err_msg("bad params"))?;
                params.insert(key.to_owned(), value.to_owned());
            }

            Ok(Some(Hyperlink::new_with_params(uri, params)))
        }
    }
}

impl EncodeEscape for Option<Hyperlink> {
    fn encode_escape<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        match self {
            None => write!(w, "8;;"),
            Some(link) => {
                write!(w, "8;")?;
                for (idx, (k, v)) in link.params.iter().enumerate() {
                    // TODO: protect against k, v containing : or =
                    if idx > 0 {
                        write!(w, ":")?;
                    }
                    write!(w, "{}={}", k, v)?;
                }
                // TODO: ensure that link.uri doesn't contain characters
                // outside the range 32-126.  Need to pull in a URI/URL
                // crate to help with this.
                write!(w, ";{}", link.uri)?;

                Ok(())
            }
        }
    }
}

impl OperatingSystemCommand {
    pub fn parse(osc: &[&[u8]]) -> Self {
        Self::internal_parse(osc).unwrap_or_else(|_| {
            let mut vec = Vec::new();
            for slice in osc {
                vec.push(slice.to_vec());
            }
            OperatingSystemCommand::Unspecified(vec)
        })
    }
    fn internal_parse(osc: &[&[u8]]) -> Result<Self, failure::Error> {
        ensure!(osc.len() > 0, "no params");
        let p1str = String::from_utf8_lossy(osc[0]);
        let code: i64 = p1str.parse()?;
        let osc_code: OperatingSystemCommandCode =
            num::FromPrimitive::from_i64(code).ok_or_else(|| failure::err_msg("unknown code"))?;

        macro_rules! single_string {
            ($variant:ident) => {{
                if osc.len() != 2 {
                    bail!("wrong param count");
                }
                let s = String::from_utf8(osc[1].to_vec())?;

                Ok(OperatingSystemCommand::$variant(s))
            }};
        };

        use self::OperatingSystemCommandCode::*;
        match osc_code {
            SetIconNameAndWindowTitle => single_string!(SetIconNameAndWindowTitle),
            SetWindowTitle => single_string!(SetWindowTitle),
            SetIconName => single_string!(SetIconName),
            SetHyperlink => Ok(OperatingSystemCommand::SetHyperlink(Hyperlink::parse(osc)?)),

            _ => bail!("not impl"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive)]
pub enum OperatingSystemCommandCode {
    SetIconNameAndWindowTitle = 0,
    SetIconName = 1,
    SetWindowTitle = 2,
    SetXWindowProperty = 3,
    ChangeColorNumber = 4,
    /// iTerm2
    ChangeTitleTabColor = 6,
    SetCurrentWorkingDirectory = 7,
    /// See https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
    SetHyperlink = 8,
    /// iTerm2
    SystemNotification = 9,
    SetTextForegroundColor = 10,
    SetTextBackgroundColor = 11,
    SetTextCursorColor = 12,
    SetMouseForegroundColor = 13,
    SetMouseBackgroundColor = 14,
    SetTektronixForegroundColor = 15,
    SetTektronixBackgroundColor = 16,
    SetHighlightColor = 17,
    SetTektronixCursorColor = 18,
    SetLogFileName = 46,
    SetFont = 50,
    EmacsShell = 51,
    ManipulateSelectionData = 52,
    RxvtProprietary = 777,
    ITermProprietary = 1337,
}

impl EncodeEscape for OperatingSystemCommand {
    fn encode_escape<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        write!(w, "\x1b]")?;

        macro_rules! single_string {
            ($variant:ident, $s:expr) => {
                write!(w, "{};{}", OperatingSystemCommandCode::$variant as u8, $s)?
            };
        };

        use self::OperatingSystemCommand::*;
        match self {
            SetIconNameAndWindowTitle(title) => single_string!(SetIconNameAndWindowTitle, title),
            SetWindowTitle(title) => single_string!(SetWindowTitle, title),
            SetIconName(title) => single_string!(SetIconName, title),
            SetHyperlink(link) => link.encode_escape(w)?,
            Unspecified(v) => {
                for (idx, item) in v.iter().enumerate() {
                    if idx > 0 {
                        write!(w, ";")?;
                    }
                    w.write_all(item.as_slice())?;
                }
            }
            __Nonexhaustive => {}
        };
        write!(w, "\x07")?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn encode(osc: &OperatingSystemCommand) -> String {
        let mut res = Vec::new();
        osc.encode_escape(&mut res).unwrap();
        String::from_utf8(res).unwrap()
    }

    fn parse(osc: &[&str], expected: &str) -> OperatingSystemCommand {
        let mut v = Vec::new();
        for s in osc {
            v.push(s.as_bytes());
        }
        let result = OperatingSystemCommand::parse(&v);

        assert_eq!(encode(&result), expected);

        result
    }

    #[test]
    fn title() {
        assert_eq!(
            parse(&["0", "hello"], "\x1b]0;hello\x07"),
            OperatingSystemCommand::SetIconNameAndWindowTitle("hello".into())
        );

        // Missing title parameter
        assert_eq!(
            parse(&["0"], "\x1b]0\x07"),
            OperatingSystemCommand::Unspecified(vec![b"0".to_vec()])
        );

        // too many params
        assert_eq!(
            parse(&["0", "1", "2"], "\x1b]0;1;2\x07"),
            OperatingSystemCommand::Unspecified(vec![b"0".to_vec(), b"1".to_vec(), b"2".to_vec()])
        );
    }

    #[test]
    fn hyperlink() {
        assert_eq!(
            parse(
                &["8", "id=foo", "http://example.com"],
                "\x1b]8;id=foo;http://example.com\x07"
            ),
            OperatingSystemCommand::SetHyperlink(Some(Hyperlink::new_with_id(
                "http://example.com",
                "foo"
            )))
        );

        assert_eq!(
            parse(&["8", "", ""], "\x1b]8;;\x07"),
            OperatingSystemCommand::SetHyperlink(None)
        );

        // too many params
        assert_eq!(
            parse(&["8", "1", "2"], "\x1b]8;1;2\x07"),
            OperatingSystemCommand::Unspecified(vec![b"8".to_vec(), b"1".to_vec(), b"2".to_vec()])
        );
    }
}
