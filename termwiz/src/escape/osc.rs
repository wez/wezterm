use base64;
use failure::{self, Error};
use num;
use std::fmt::{Display, Error as FmtError, Formatter};
pub use hyperlink::Hyperlink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystemCommand {
    SetIconNameAndWindowTitle(String),
    SetWindowTitle(String),
    SetIconName(String),
    SetHyperlink(Option<Hyperlink>),
    ClearSelection(Selection),
    QuerySelection(Selection),
    SetSelection(Selection, String),

    Unspecified(Vec<Vec<u8>>),
}

bitflags! {
pub struct Selection :u16{
    const NONE = 0;
    const CLIPBOARD = 1<<1;
    const PRIMARY=1<<2;
    const SELECT=1<<3;
    const CUT0=1<<4;
    const CUT1=1<<5;
    const CUT2=1<<6;
    const CUT3=1<<7;
    const CUT4=1<<8;
    const CUT5=1<<9;
    const CUT6=1<<10;
    const CUT7=1<<11;
    const CUT8=1<<12;
    const CUT9=1<<13;
}
}

impl Selection {
    fn try_parse(buf: &[u8]) -> Result<Selection, Error> {
        if buf == b"" {
            Ok(Selection::SELECT | Selection::CUT0)
        } else {
            let mut s = Selection::NONE;
            for c in buf {
                s |= match c {
                    b'c' => Selection::CLIPBOARD,
                    b'p' => Selection::PRIMARY,
                    b's' => Selection::SELECT,
                    b'0' => Selection::CUT0,
                    b'1' => Selection::CUT1,
                    b'2' => Selection::CUT2,
                    b'3' => Selection::CUT3,
                    b'4' => Selection::CUT4,
                    b'5' => Selection::CUT5,
                    b'6' => Selection::CUT6,
                    b'7' => Selection::CUT7,
                    b'8' => Selection::CUT8,
                    b'9' => Selection::CUT9,
                    _ => bail!("invalid selection {:?}", buf),
                }
            }
            Ok(s)
        }
    }
}

impl Display for Selection {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        macro_rules! item {
            ($variant:ident, $s:expr) => {
                if (*self & Selection::$variant) != Selection::NONE {
                    write!(f, $s)?;
                }
            };
        };

        item!(CLIPBOARD, "c");
        item!(PRIMARY, "p");
        item!(SELECT, "s");
        item!(CUT0, "0");
        item!(CUT1, "1");
        item!(CUT2, "2");
        item!(CUT3, "3");
        item!(CUT4, "4");
        item!(CUT5, "5");
        item!(CUT6, "6");
        item!(CUT7, "7");
        item!(CUT8, "8");
        item!(CUT9, "9");
        Ok(())
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

    fn parse_selection(osc: &[&[u8]]) -> Result<Self, Error> {
        if osc.len() == 2 {
            Selection::try_parse(osc[1]).map(|s| OperatingSystemCommand::ClearSelection(s))
        } else if osc.len() == 3 && osc[2] == b"?" {
            Selection::try_parse(osc[1]).map(|s| OperatingSystemCommand::QuerySelection(s))
        } else if osc.len() == 3 {
            let sel = Selection::try_parse(osc[1])?;
            let bytes = base64::decode(osc[2])?;
            let s = String::from_utf8(bytes)?;
            Ok(OperatingSystemCommand::SetSelection(sel, s))
        } else {
            bail!("unhandled OSC 52: {:?}", osc);
        }
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
            ManipulateSelectionData => Self::parse_selection(osc),

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

impl Display for OperatingSystemCommand {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "\x1b]")?;

        macro_rules! single_string {
            ($variant:ident, $s:expr) => {
                write!(f, "{};{}", OperatingSystemCommandCode::$variant as u8, $s)?
            };
        };

        use self::OperatingSystemCommand::*;
        match self {
            SetIconNameAndWindowTitle(title) => single_string!(SetIconNameAndWindowTitle, title),
            SetWindowTitle(title) => single_string!(SetWindowTitle, title),
            SetIconName(title) => single_string!(SetIconName, title),
            SetHyperlink(Some(link)) => link.fmt(f)?,
            SetHyperlink(None) => write!(f, "8;;")?,
            Unspecified(v) => {
                for (idx, item) in v.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ";")?;
                    }
                    f.write_str(&String::from_utf8_lossy(item))?;
                    //f.write_all(item.as_slice())?;
                }
            }
            ClearSelection(s) => write!(f, "52;{}", s)?,
            QuerySelection(s) => write!(f, "52;{};?", s)?,
            SetSelection(s, val) => write!(f, "52;{};{}", s, base64::encode(val))?,
        };
        write!(f, "\x07")?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn encode(osc: &OperatingSystemCommand) -> String {
        format!("{}", osc)
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

        assert_eq!(
            Hyperlink::parse(&[b"8", b"", b"x"]).unwrap(),
            Some(Hyperlink::new("x"))
        );
    }
}
