use base64;
use failure::{self, err_msg, Error};
pub use hyperlink::Hyperlink;
use num;
use ordered_float::NotNaN;
use std::collections::HashMap;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystemCommand {
    SetIconNameAndWindowTitle(String),
    SetWindowTitle(String),
    SetIconName(String),
    SetHyperlink(Option<Hyperlink>),
    ClearSelection(Selection),
    QuerySelection(Selection),
    SetSelection(Selection, String),
    SystemNotification(String),
    ITermProprietary(ITermProprietary),

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
            SystemNotification => single_string!(SystemNotification),
            ITermProprietary => self::ITermProprietary::parse(osc)
                .map(|p| OperatingSystemCommand::ITermProprietary(p)),

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
                }
            }
            ClearSelection(s) => write!(f, "52;{}", s)?,
            QuerySelection(s) => write!(f, "52;{};?", s)?,
            SetSelection(s, val) => write!(f, "52;{};{}", s, base64::encode(val))?,
            SystemNotification(s) => write!(f, "9;{}", s)?,
            ITermProprietary(i) => i.fmt(f)?,
        };
        write!(f, "\x07")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ITermProprietary {
    /// The "Set Mark" command allows you to record a location and then jump back to it later
    SetMark,
    /// To bring iTerm2 to the foreground
    StealFocus,
    /// To erase the scrollback history
    ClearScrollback,
    /// To inform iTerm2 of the current directory to help semantic history
    CurrentDir(String),
    /// To change the session's profile on the fly
    SetProfile(String),
    /// Currently defined values for the string parameter are "rule", "find", "font"
    /// or an empty string.  iTerm2 will go into paste mode until EndCopy is received.
    CopyToClipboard(String),
    /// Ends CopyToClipboard mode in iTerm2.
    EndCopy,
    /// The boolean should be yes or no. This shows or hides the cursor guide
    HighlightCursorLine(bool),
    /// Request that the terminal send a ReportCellSize response
    RequestCellSize,
    /// The response to RequestCellSize.  The height and width are the dimensions
    /// of a cell measured in points
    ReportCellSize {
        height_points: NotNaN<f32>,
        width_points: NotNaN<f32>,
    },
    /// Place a string in the systems pasteboard
    Copy(String),
    /// Each iTerm2 session has internal variables (as described in
    /// <https://www.iterm2.com/documentation-badges.html>). This escape sequence reports
    /// a variable's value.  The response is another ReportVariable.
    ReportVariable(String),
    /// User-defined variables may be set with the following escape sequence
    SetUserVar {
        name: String,
        value: String,
    },
    SetBadgeFormat(String),
    /// Download file data from the application.
    File(Box<ITermFileData>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ITermFileData {
    /// file name
    name: Option<String>,
    /// size of the data in bytes; this is used by iterm to show progress
    /// while waiting for the rest of the payload
    size: Option<u64>,
    /// width to render
    width: ITermDimension,
    /// height to render
    height: ITermDimension,
    /// if true, preserve aspect ratio when fitting to width/height
    preserve_aspect_ratio: bool,
    /// if true, attempt to display in the terminal rather than downloading to
    /// the users download directory
    inline: bool,
    /// The data to transfer
    data: Vec<u8>,
}

impl ITermFileData {
    fn parse(osc: &[&[u8]]) -> Result<Self, Error> {
        let mut params = HashMap::new();

        // Unfortunately, the encoding for the file download data is
        // awkward to fit in the conventional OSC data that our parser
        // expects at a higher level.
        // We have a mix of '=', ';' and ':' separated keys and values,
        // and a number of them are optional.
        // ESC ] 1337 ; File = [optional arguments] : base-64 encoded file contents ^G

        let mut data = None;

        let last = osc.len() - 1;
        for (idx, s) in osc.iter().enumerate().skip(1) {
            let param = if idx == 1 {
                // skip over File=
                &s[5..]
            } else {
                s
            };

            let param = if idx == last {
                // The final argument contains `:base64`, so look for that
                if let Some(colon) = param.iter().position(|c| *c == b':') {
                    data = Some(base64::decode(&param[colon + 1..])?);
                    &param[..colon]
                } else {
                    // If we don't find the colon in the last piece, we've
                    // got nothing useful
                    bail!("failed to parse file data; no colon found");
                }
            } else {
                param
            };

            // look for k=v in param
            if let Some(equal) = param.iter().position(|c| *c == b'=') {
                let key = &param[..equal];
                let value = &param[equal + 1..];
                params.insert(str::from_utf8(key)?, str::from_utf8(value)?);
            } else if idx != last {
                bail!("failed to parse file data; no equals found");
            }
        }

        let name = params
            .get("name")
            .and_then(|s| base64::decode(s).ok())
            .and_then(|b| String::from_utf8(b).ok());
        let size = params.get("size").and_then(|s| s.parse().ok());
        let width = params
            .get("width")
            .and_then(|s| ITermDimension::parse(s).ok())
            .unwrap_or(ITermDimension::Automatic);
        let height = params
            .get("height")
            .and_then(|s| ITermDimension::parse(s).ok())
            .unwrap_or(ITermDimension::Automatic);
        let preserve_aspect_ratio = params
            .get("preserveAspectRatio")
            .map(|s| if *s == "0" { false } else { true })
            .unwrap_or(true);
        let inline = params
            .get("inline")
            .map(|s| if *s == "0" { false } else { true })
            .unwrap_or(false);
        let data = data.ok_or_else(|| err_msg("didn't set data"))?;
        Ok(Self {
            name,
            size,
            width,
            height,
            preserve_aspect_ratio,
            inline,
            data,
        })
    }
}

impl Display for ITermFileData {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "File")?;
        let mut sep = "=";
        let emit_sep = |sep, f: &mut Formatter| {
            write!(f, "{}", sep)?;
            Ok(";")
        };
        if let Some(size) = self.size {
            sep = emit_sep(sep, f)?;
            write!(f, "size={}", size)?;
        }
        if let Some(ref name) = self.name {
            sep = emit_sep(sep, f)?;
            write!(f, "name={}", base64::encode(name))?;
        }
        if self.width != ITermDimension::Automatic {
            sep = emit_sep(sep, f)?;
            write!(f, "width={}", self.width)?;
        }
        if self.height != ITermDimension::Automatic {
            sep = emit_sep(sep, f)?;
            write!(f, "height={}", self.height)?;
        }
        if !self.preserve_aspect_ratio {
            sep = emit_sep(sep, f)?;
            write!(f, "preserveAspectRatio=0")?;
        }
        if self.inline {
            sep = emit_sep(sep, f)?;
            write!(f, "inline=1")?;
        }
        // Ensure that we emit a sep if we didn't already.
        // It will still be set to '=' in that case.
        if sep == "=" {
            write!(f, "=")?;
        }
        write!(f, ":{}", base64::encode(&self.data))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ITermDimension {
    Automatic,
    Cells(i64),
    Pixels(i64),
    Percent(i64),
}

impl Display for ITermDimension {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        use self::ITermDimension::*;
        match self {
            Automatic => write!(f, "auto"),
            Cells(n) => write!(f, "{}", n),
            Pixels(n) => write!(f, "{}px", n),
            Percent(n) => write!(f, "{}%", n),
        }
    }
}

impl ITermDimension {
    fn parse(s: &str) -> Result<Self, Error> {
        if s == "auto" {
            Ok(ITermDimension::Automatic)
        } else if s.ends_with("px") {
            let s = &s[..s.len() - 2];
            let num = s.parse()?;
            Ok(ITermDimension::Pixels(num))
        } else if s.ends_with("%") {
            let s = &s[..s.len() - 1];
            let num = s.parse()?;
            Ok(ITermDimension::Percent(num))
        } else {
            let num = s.parse()?;
            Ok(ITermDimension::Cells(num))
        }
    }
}

impl ITermProprietary {
    fn parse(osc: &[&[u8]]) -> Result<Self, Error> {
        // iTerm has a number of different styles of OSC parameter
        // encodings, which makes this section of code a bit gnarly.
        ensure!(osc.len() > 1, "not enough args");

        let param = String::from_utf8_lossy(osc[1]);

        let mut iter = param.splitn(2, '=');
        let keyword = iter.next().ok_or_else(|| err_msg("bad params"))?;
        let p1 = iter.next();

        macro_rules! single {
            ($variant:ident, $text:expr) => {
                if osc.len() == 2 && keyword == $text && p1.is_none() {
                    return Ok(ITermProprietary::$variant);
                }
            };
        };

        macro_rules! one_str {
            ($variant:ident, $text:expr) => {
                if osc.len() == 2 && keyword == $text {
                    if let Some(p1) = p1 {
                        return Ok(ITermProprietary::$variant(p1.into()));
                    }
                }
            };
        };
        macro_rules! const_arg {
            ($variant:ident, $text:expr, $value:expr, $res:expr) => {
                if osc.len() == 2 && keyword == $text {
                    if let Some(p1) = p1 {
                        if p1 == $value {
                            return Ok(ITermProprietary::$variant($res));
                        }
                    }
                }
            };
        }

        single!(SetMark, "SetMark");
        single!(StealFocus, "StealFocus");
        single!(ClearScrollback, "ClearScrollback");
        single!(EndCopy, "EndCopy");
        single!(RequestCellSize, "ReportCellSize");
        const_arg!(HighlightCursorLine, "HighlightCursorLine", "yes", true);
        const_arg!(HighlightCursorLine, "HighlightCursorLine", "no", false);
        one_str!(CurrentDir, "CurrentDir");
        one_str!(SetProfile, "SetProfile");
        one_str!(CopyToClipboard, "CopyToClipboard");

        let p1_empty = match p1 {
            Some(p1) if p1 == "" => true,
            None => true,
            _ => false,
        };

        if osc.len() == 3 && keyword == "Copy" && p1_empty {
            return Ok(ITermProprietary::Copy(String::from_utf8(base64::decode(
                osc[2],
            )?)?));
        }
        if osc.len() == 3 && keyword == "SetBadgeFormat" && p1_empty {
            return Ok(ITermProprietary::SetBadgeFormat(String::from_utf8(
                base64::decode(osc[2])?,
            )?));
        }

        if osc.len() == 3 && keyword == "ReportCellSize" && p1.is_some() {
            if let Some(p1) = p1 {
                return Ok(ITermProprietary::ReportCellSize {
                    height_points: NotNaN::new(p1.parse()?)?,
                    width_points: NotNaN::new(String::from_utf8_lossy(osc[2]).parse()?)?,
                });
            }
        }

        if osc.len() == 2 && keyword == "SetUserVar" {
            if let Some(p1) = p1 {
                let mut iter = p1.splitn(2, '=');
                let p1 = iter.next();
                let p2 = iter.next();

                match (p1, p2) {
                    (Some(k), Some(v)) => {
                        return Ok(ITermProprietary::SetUserVar {
                            name: k.to_string(),
                            value: String::from_utf8(base64::decode(v)?)?,
                        });
                    }
                    _ => {}
                }
            }
        }

        if keyword == "File" {
            return Ok(ITermProprietary::File(Box::new(ITermFileData::parse(osc)?)));
        }

        bail!("ITermProprietary {:?}", osc);
    }
}

impl Display for ITermProprietary {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "1337;")?;
        use self::ITermProprietary::*;
        match self {
            SetMark => write!(f, "SetMark")?,
            StealFocus => write!(f, "StealFocus")?,
            ClearScrollback => write!(f, "ClearScrollback")?,
            CurrentDir(s) => write!(f, "CurrentDir={}", s)?,
            SetProfile(s) => write!(f, "SetProfile={}", s)?,
            CopyToClipboard(s) => write!(f, "CopyToClipboard={}", s)?,
            EndCopy => write!(f, "EndCopy")?,
            HighlightCursorLine(yes) => {
                write!(f, "HighlightCursorLine={}", if *yes { "yes" } else { "no" })?
            }
            RequestCellSize => write!(f, "ReportCellSize")?,
            ReportCellSize {
                height_points,
                width_points,
            } => write!(f, "ReportCellSize={};{}", height_points, width_points)?,
            Copy(s) => write!(f, "Copy=;{}", base64::encode(s))?,
            ReportVariable(s) => write!(f, "ReportVariable={}", base64::encode(s))?,
            SetUserVar { name, value } => {
                write!(f, "SetUserVar={}={}", name, base64::encode(value))?
            }
            SetBadgeFormat(s) => write!(f, "SetBadgeFormat={}", base64::encode(s))?,
            File(file) => file.fmt(f)?,
        }
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

    #[test]
    fn iterm() {
        assert_eq!(
            parse(&["1337", "SetMark"], "\x1b]1337;SetMark\x07"),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::SetMark)
        );

        assert_eq!(
            parse(
                &["1337", "CurrentDir=woot"],
                "\x1b]1337;CurrentDir=woot\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::CurrentDir("woot".into()))
        );

        assert_eq!(
            parse(
                &["1337", "HighlightCursorLine=yes"],
                "\x1b]1337;HighlightCursorLine=yes\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::HighlightCursorLine(true))
        );

        assert_eq!(
            parse(
                &["1337", "Copy=", "aGVsbG8="],
                "\x1b]1337;Copy=;aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::Copy("hello".into()))
        );

        assert_eq!(
            parse(
                &["1337", "SetUserVar=foo=aGVsbG8="],
                "\x1b]1337;SetUserVar=foo=aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::SetUserVar {
                name: "foo".into(),
                value: "hello".into()
            })
        );

        assert_eq!(
            parse(
                &["1337", "SetBadgeFormat=", "aGVsbG8="],
                "\x1b]1337;SetBadgeFormat=aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::SetBadgeFormat(
                "hello".into()
            ))
        );

        assert_eq!(
            parse(
                &["1337", "ReportCellSize=12.0", "15.5"],
                "\x1b]1337;ReportCellSize=12;15.5\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::ReportCellSize {
                height_points: NotNaN::new(12.0).unwrap(),
                width_points: NotNaN::new(15.5).unwrap()
            })
        );

        assert_eq!(
            parse(&["1337", "File=:aGVsbG8="], "\x1b]1337;File=:aGVsbG8=\x07"),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: None,
                    size: None,
                    width: ITermDimension::Automatic,
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &["1337", "File=name=bXluYW1l:aGVsbG8="],
                "\x1b]1337;File=name=bXluYW1l:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: None,
                    width: ITermDimension::Automatic,
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &["1337", "File=size=123", "name=bXluYW1l:aGVsbG8="],
                "\x1b]1337;File=size=123;name=bXluYW1l:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(123),
                    width: ITermDimension::Automatic,
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &["1337", "File=name=bXluYW1l", "size=234:aGVsbG8="],
                "\x1b]1337;File=size=234;name=bXluYW1l:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(234),
                    width: ITermDimension::Automatic,
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &[
                    "1337",
                    "File=name=bXluYW1l",
                    "width=auto",
                    "size=234:aGVsbG8="
                ],
                "\x1b]1337;File=size=234;name=bXluYW1l:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(234),
                    width: ITermDimension::Automatic,
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &["1337", "File=name=bXluYW1l", "width=5", "size=234:aGVsbG8="],
                "\x1b]1337;File=size=234;name=bXluYW1l;width=5:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(234),
                    width: ITermDimension::Cells(5),
                    height: ITermDimension::Automatic,
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &[
                    "1337",
                    "File=name=bXluYW1l",
                    "width=5",
                    "height=10%",
                    "size=234:aGVsbG8="
                ],
                "\x1b]1337;File=size=234;name=bXluYW1l;width=5;height=10%:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(234),
                    width: ITermDimension::Cells(5),
                    height: ITermDimension::Percent(10),
                    preserve_aspect_ratio: true,
                    inline: false,
                    data: b"hello".to_vec()
                }
            )))
        );

        assert_eq!(
            parse(
                &["1337", "File=name=bXluYW1l", "preserveAspectRatio=0", "width=5", "inline=1", "height=10px","size=234:aGVsbG8="],
                "\x1b]1337;File=size=234;name=bXluYW1l;width=5;height=10px;preserveAspectRatio=0;inline=1:aGVsbG8=\x07"
            ),
            OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
                ITermFileData {
                    name: Some("myname".into()),
                    size: Some(234),
                    width: ITermDimension::Cells(5),
                    height: ITermDimension::Pixels(10),
                    preserve_aspect_ratio: false,
                    inline: true,
                    data: b"hello".to_vec()
                }
            )))
        );
    }
}
