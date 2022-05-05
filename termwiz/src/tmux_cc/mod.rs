use anyhow::Context;
use parser::Rule;
use pest::iterators::{Pair, Pairs};
use pest::Parser as _;

pub type TmuxWindowId = u64;
pub type TmuxPaneId = u64;
pub type TmuxSessionId = u64;

mod parser {
    use pest_derive::Parser;
    #[derive(Parser)]
    #[grammar = "tmux_cc/tmux.pest"]
    pub struct TmuxParser;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guarded {
    pub error: bool,
    pub timestamp: i64,
    pub number: u64,
    pub flags: i64,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowLayout {
    pub layout_id: String,
    pub width: u64,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Begin {
        timestamp: i64,
        number: u64,
        flags: i64,
    },
    End {
        timestamp: i64,
        number: u64,
        flags: i64,
    },
    Error {
        timestamp: i64,
        number: u64,
        flags: i64,
    },
    Guarded(Guarded),
    Output {
        pane: TmuxPaneId,
        text: String,
    },
    Exit {
        reason: Option<String>,
    },
    SessionsChanged,
    SessionChanged {
        session: TmuxSessionId,
        name: String,
    },
    SessionRenamed {
        name: String,
    },
    SessionWindowChanged {
        session: TmuxSessionId,
        window: TmuxWindowId,
    },
    ClientSessionChanged {
        client_name: String,
        session: TmuxSessionId,
        session_name: String,
    },
    ClientDetached {
        client_name: String,
    },
    PaneModeChanged {
        pane: TmuxPaneId,
    },
    WindowAdd {
        window: TmuxWindowId,
    },
    WindowClose {
        window: TmuxWindowId,
    },
    WindowPaneChanged {
        window: TmuxWindowId,
        pane: TmuxPaneId,
    },
    WindowRenamed {
        window: TmuxWindowId,
        name: String,
    },
    LayoutChange {
        window: TmuxWindowId,
        layout: WindowLayout,
        visible_layout: Option<WindowLayout>,
        raw_flags: Option<String>,
    },
}

fn parse_pane_id(pair: Pair<Rule>) -> anyhow::Result<TmuxPaneId> {
    match pair.as_rule() {
        Rule::pane_id => {
            let mut pairs = pair.into_inner();
            pairs
                .next()
                .unwrap()
                .as_str()
                .parse()
                .context("pane_id is somehow not digits")
        }
        _ => anyhow::bail!("parse_pane_id can only parse Rule::pane_id, got {:?}", pair),
    }
}

fn parse_window_id(pair: Pair<Rule>) -> anyhow::Result<TmuxWindowId> {
    match pair.as_rule() {
        Rule::window_id => {
            let mut pairs = pair.into_inner();
            pairs
                .next()
                .unwrap()
                .as_str()
                .parse()
                .context("window_id is somehow not digits")
        }
        _ => anyhow::bail!(
            "parse_window_id can only parse Rule::window_id, got {:?}",
            pair
        ),
    }
}

fn parse_session_id(pair: Pair<Rule>) -> anyhow::Result<TmuxSessionId> {
    match pair.as_rule() {
        Rule::session_id => {
            let mut pairs = pair.into_inner();
            pairs
                .next()
                .unwrap()
                .as_str()
                .parse()
                .context("session_id is somehow not digits")
        }
        _ => anyhow::bail!(
            "parse_session_id can only parse Rule::session_id, got {:?}",
            pair
        ),
    }
}

/// Parses a %begin, %end, %error guard line tuple
fn parse_guard(mut pairs: Pairs<Rule>) -> anyhow::Result<(i64, u64, i64)> {
    let timestamp = pairs.next().unwrap().as_str().parse::<i64>()?;
    let number = pairs.next().unwrap().as_str().parse::<u64>()?;
    let flags = pairs.next().unwrap().as_str().parse::<i64>()?;
    Ok((timestamp, number, flags))
}

/// Parses a window_layout line, for example "b25d,80x24,0,0,0"
fn parse_window_layout(pair: Pair<Rule>) -> Option<WindowLayout> {
    match pair.as_rule() {
        Rule::window_layout => {
            let mut pairs = pair.into_inner();
            let layout_id_option = pairs.next()?.as_str().parse::<String>().ok();
            let width_option = pairs.next()?.as_str().parse::<u64>().ok();
            let height_option = pairs.next()?.as_str().parse::<u64>().ok();
            if let (Some(layout_id), Some(width), Some(height)) =
                (layout_id_option, width_option, height_option)
            {
                return Some(WindowLayout {
                    layout_id,
                    width,
                    height,
                });
            }
            return None;
        }
        _ => None,
    }
}

fn parse_line(line: &str) -> anyhow::Result<Event> {
    let mut pairs = parser::TmuxParser::parse(Rule::line_entire, line)?;
    let pair = pairs.next().ok_or_else(|| anyhow::anyhow!("no pairs!?"))?;
    match pair.as_rule() {
        Rule::begin => {
            let (timestamp, number, flags) = parse_guard(pair.into_inner())?;
            Ok(Event::Begin {
                timestamp,
                number,
                flags,
            })
        }
        Rule::end => {
            let (timestamp, number, flags) = parse_guard(pair.into_inner())?;
            Ok(Event::End {
                timestamp,
                number,
                flags,
            })
        }
        Rule::error => {
            let (timestamp, number, flags) = parse_guard(pair.into_inner())?;
            Ok(Event::Error {
                timestamp,
                number,
                flags,
            })
        }
        Rule::exit => {
            let mut pairs = pair.into_inner();
            let reason = pairs.next().map(|pair| pair.as_str().to_owned());
            Ok(Event::Exit { reason })
        }
        Rule::sessions_changed => Ok(Event::SessionsChanged),
        Rule::pane_mode_changed => {
            let mut pairs = pair.into_inner();
            let pane = parse_pane_id(pairs.next().unwrap())?;
            Ok(Event::PaneModeChanged { pane })
        }
        Rule::window_add => {
            let mut pairs = pair.into_inner();
            let window = parse_window_id(pairs.next().unwrap())?;
            Ok(Event::WindowAdd { window })
        }
        Rule::window_close => {
            let mut pairs = pair.into_inner();
            let window = parse_window_id(pairs.next().unwrap())?;
            Ok(Event::WindowClose { window })
        }
        Rule::window_pane_changed => {
            let mut pairs = pair.into_inner();
            let window = parse_window_id(pairs.next().unwrap())?;
            let pane = parse_pane_id(pairs.next().unwrap())?;
            Ok(Event::WindowPaneChanged { window, pane })
        }
        Rule::window_renamed => {
            let mut pairs = pair.into_inner();
            let window = parse_window_id(pairs.next().unwrap())?;
            let name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::WindowRenamed { window, name })
        }
        Rule::output => {
            let mut pairs = pair.into_inner();
            let pane = parse_pane_id(pairs.next().unwrap())?;
            let text = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::Output { pane, text })
        }
        Rule::session_changed => {
            let mut pairs = pair.into_inner();
            let session = parse_session_id(pairs.next().unwrap())?;
            let name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::SessionChanged { session, name })
        }
        Rule::client_session_changed => {
            let mut pairs = pair.into_inner();
            let client_name = unvis(pairs.next().unwrap().as_str())?;
            let session = parse_session_id(pairs.next().unwrap())?;
            let session_name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::ClientSessionChanged {
                client_name,
                session,
                session_name,
            })
        }
        Rule::client_detached => {
            let mut pairs = pair.into_inner();
            let client_name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::ClientDetached { client_name })
        }
        Rule::session_renamed => {
            let mut pairs = pair.into_inner();
            let name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::SessionRenamed { name })
        }
        Rule::session_window_changed => {
            let mut pairs = pair.into_inner();
            let session = parse_session_id(pairs.next().unwrap())?;
            let window = parse_window_id(pairs.next().unwrap())?;
            Ok(Event::SessionWindowChanged { session, window })
        }
        Rule::layout_change => {
            let mut pairs = pair.into_inner();
            let window = parse_window_id(pairs.next().unwrap())?;
            let layout = pairs.next().and_then(parse_window_layout).unwrap();
            let visible_layout = pairs.next().and_then(parse_window_layout);
            let raw_flags = pairs.next().map(|r| r.as_str().to_owned());
            Ok(Event::LayoutChange {
                window,
                layout,
                visible_layout,
                raw_flags,
            })
        }
        Rule::pane_id
        | Rule::word
        | Rule::client_name
        | Rule::window_id
        | Rule::session_id
        | Rule::window_layout
        | Rule::any_text
        | Rule::line
        | Rule::line_entire
        | Rule::EOI
        | Rule::number => unreachable!(),
    }
}

/// Decode OpenBSD `vis` encoded strings
/// See: https://github.com/tmux/tmux/blob/486ce9b09855ae30a2bf5e576cb6f7ad37792699/compat/unvis.c
pub fn unvis(s: &str) -> anyhow::Result<String> {
    enum State {
        Ground,
        Start,
        Meta,
        Meta1,
        Ctrl(u8),
        Octal2(u8),
        Octal3(u8),
    }

    let mut state = State::Ground;
    let mut result: Vec<u8> = vec![];
    let mut bytes = s.as_bytes().iter();

    fn is_octal(b: u8) -> bool {
        b >= b'0' && b <= b'7'
    }

    fn unvis_byte(b: u8, state: &mut State, result: &mut Vec<u8>) -> anyhow::Result<bool> {
        match state {
            State::Ground => {
                if b == b'\\' {
                    *state = State::Start;
                } else {
                    result.push(b);
                }
            }

            State::Start => {
                match b {
                    b'\\' => {
                        result.push(b'\\');
                        *state = State::Ground;
                    }
                    b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' => {
                        let value = b - b'0';
                        *state = State::Octal2(value);
                    }
                    b'M' => {
                        *state = State::Meta;
                    }
                    b'^' => {
                        *state = State::Ctrl(0);
                    }
                    b'n' => {
                        result.push(b'\n');
                        *state = State::Ground;
                    }
                    b'r' => {
                        result.push(b'\r');
                        *state = State::Ground;
                    }
                    b'b' => {
                        result.push(b'\x08');
                        *state = State::Ground;
                    }
                    b'a' => {
                        result.push(b'\x07');
                        *state = State::Ground;
                    }
                    b'v' => {
                        result.push(b'\x0b');
                        *state = State::Ground;
                    }
                    b't' => {
                        result.push(b'\t');
                        *state = State::Ground;
                    }
                    b'f' => {
                        result.push(b'\x0c');
                        *state = State::Ground;
                    }
                    b's' => {
                        result.push(b' ');
                        *state = State::Ground;
                    }
                    b'E' => {
                        result.push(b'\x1b');
                        *state = State::Ground;
                    }
                    b'\n' => {
                        // Hidden newline
                        // result.push(b'\n');
                        *state = State::Ground;
                    }
                    b'$' => {
                        // Hidden marker
                        *state = State::Ground;
                    }
                    _ => {
                        // Invalid syntax
                        anyhow::bail!("Invalid \\ escape: {}", b);
                    }
                }
            }

            State::Meta => {
                if b == b'-' {
                    *state = State::Meta1;
                } else if b == b'^' {
                    *state = State::Ctrl(0o200);
                } else {
                    anyhow::bail!("invalid \\M escape: {}", b);
                }
            }

            State::Meta1 => {
                result.push(b | 0o200);
                *state = State::Ground;
            }

            State::Ctrl(c) => {
                if b == b'?' {
                    result.push(*c | 0o177);
                } else {
                    result.push((b & 0o37) | *c);
                }
                *state = State::Ground;
            }

            State::Octal2(prior) => {
                if is_octal(b) {
                    // It's the second in a 2 or 3 byte octal sequence
                    let value = (*prior << 3) + (b - b'0');
                    *state = State::Octal3(value);
                } else {
                    // Prior character was a single octal value
                    result.push(*prior);
                    *state = State::Ground;
                    // re-process the current byte
                    return Ok(true);
                }
            }

            State::Octal3(prior) => {
                if is_octal(b) {
                    // It's the third in a 3 byte octal sequence
                    let value = (*prior << 3) + (b - b'0');
                    result.push(value);
                    *state = State::Ground;
                } else {
                    // Prior was a 2-byte octal sequence
                    result.push(*prior);
                    *state = State::Ground;
                    // re-process the current byte
                    return Ok(true);
                }
            }
        }
        // Don't process this byte again
        Ok(false)
    }

    while let Some(&b) = bytes.next() {
        let again = unvis_byte(b, &mut state, &mut result)?;
        if again {
            unvis_byte(b, &mut state, &mut result)?;
        }
    }

    String::from_utf8(result)
        .map_err(|err| anyhow::anyhow!("Unescaped string is not valid UTF8: {}", err))
}

pub struct Parser {
    buffer: Vec<u8>,
    begun: Option<Guarded>,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            buffer: vec![],
            begun: None,
        }
    }

    pub fn advance_byte(&mut self, c: u8) -> anyhow::Result<Option<Event>> {
        if c == b'\n' {
            self.process_line()
        } else {
            self.buffer.push(c);
            Ok(None)
        }
    }

    pub fn advance_string(&mut self, s: &str) -> anyhow::Result<Vec<Event>> {
        self.advance_bytes(s.as_bytes())
    }

    pub fn advance_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<Vec<Event>> {
        let mut events = vec![];
        for (i, &b) in bytes.iter().enumerate() {
            match self.advance_byte(b) {
                Ok(option_event) => {
                    if let Some(e) = option_event {
                        events.push(e);
                    }
                }
                Err(err) => {
                    // concat remained bytes after digested bytes
                    return Err(anyhow::anyhow!(format!(
                        "{}{}",
                        err,
                        String::from_utf8_lossy(&bytes[i..])
                    )));
                }
            }
        }
        Ok(events)
    }

    fn process_guarded_line(&mut self, line: String) -> anyhow::Result<Option<Event>> {
        let result = match parse_line(&line) {
            Ok(Event::End {
                timestamp,
                number,
                flags,
            }) => {
                if let Some(begun) = self.begun.take() {
                    if begun.timestamp == timestamp
                        && begun.number == number
                        && begun.flags == flags
                    {
                        Some(Event::Guarded(begun))
                    } else {
                        log::error!("mismatched %end; expected {:?} but got {}", begun, line);
                        None
                    }
                } else {
                    log::error!("unexpected %end with no %begin ({})", line);
                    None
                }
            }
            Ok(Event::Error {
                timestamp,
                number,
                flags,
            }) => {
                if let Some(mut begun) = self.begun.take() {
                    if begun.timestamp == timestamp
                        && begun.number == number
                        && begun.flags == flags
                    {
                        begun.error = true;
                        Some(Event::Guarded(begun))
                    } else {
                        log::error!("mismatched %error; expected {:?} but got {}", begun, line);
                        None
                    }
                } else {
                    log::error!("unexpected %error with no %begin ({})", line);
                    None
                }
            }
            _ => {
                let begun = self.begun.as_mut().unwrap();
                begun.output.push_str(&line);
                begun.output.push('\n');
                None
            }
        };
        self.buffer.clear();
        return Ok(result);
    }

    fn process_line(&mut self) -> anyhow::Result<Option<Event>> {
        if self.buffer.last() == Some(&b'\r') {
            self.buffer.pop();
        }
        let result = match std::str::from_utf8(&self.buffer) {
            Ok(line) => {
                if self.begun.is_some() {
                    let line = line.to_owned();
                    return self.process_guarded_line(line);
                }
                match parse_line(line) {
                    Ok(Event::Begin {
                        timestamp,
                        number,
                        flags,
                    }) => {
                        if self.begun.is_some() {
                            log::error!("expected %end or %error before %begin ({})", line);
                        }
                        self.begun.replace(Guarded {
                            timestamp,
                            number,
                            flags,
                            error: false,
                            output: String::new(),
                        });
                        None
                    }
                    Ok(event) => Some(event),
                    Err(err) => {
                        log::error!("Unrecognized tmux cc line: {}", err);
                        return Err(anyhow::anyhow!(line.to_owned()));
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to parse line from tmux: {}", err);
                None
            }
        };
        self.buffer.clear();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k9::assert_equal as assert_eq;

    #[test]
    fn test_parse_line() {
        let _ = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        assert_eq!(
            Event::Begin {
                timestamp: 12345,
                number: 321,
                flags: 0,
            },
            parse_line("%begin 12345 321 0").unwrap()
        );

        assert_eq!(
            Event::End {
                timestamp: 12345,
                number: 321,
                flags: 0,
            },
            parse_line("%end 12345 321 0").unwrap()
        );
    }

    #[test]
    fn test_parse_sequence() {
        let input = b"%sessions-changed
%pane-mode-changed %0
%begin 1604279270 310 0
stuff
in
here
%end 1604279270 310 0
%window-add @1
%sessions-changed
%session-changed $1 1
%client-session-changed /dev/pts/5 $1 home
%client-detached /dev/pts/10
%layout-change @1 b25d,80x24,0,0,0
%layout-change @1 cafd,120x29,0,0,0 cafd,120x29,0,0,0 *
%output %1 \\033[1m\\033[7m%\\033[27m\\033[1m\\033[0m    \\015 \\015
%output %1 \\033kwez@cube-localdomain:~\\033\\134\\033]2;wez@cube-localdomain:~\\033\\134
%output %1 \\033]7;file://cube-localdomain/home/wez\\033\\134
%output %1 \\033[K\\033[?2004h
%exit
%exit I said so
";

        let mut p = Parser::new();
        let events = p.advance_bytes(input).unwrap();
        assert_eq!(
            vec![
                Event::SessionsChanged,
                Event::PaneModeChanged { pane: 0 },
                Event::Guarded(Guarded {
                    timestamp: 1604279270,
                    number: 310,
                    flags: 0,
                    error: false,
                    output: "stuff\nin\nhere\n".to_owned()
                }),
                Event::WindowAdd { window: 1 },
                Event::SessionsChanged,
                Event::SessionChanged {
                    session: 1,
                    name: "1".to_owned(),
                },
                Event::ClientSessionChanged {
                    client_name: "/dev/pts/5".to_owned(),
                    session: 1,
                    session_name: "home".to_owned()
                },
                Event::ClientDetached {
                    client_name: "/dev/pts/10".to_owned()
                },
                Event::LayoutChange {
                    window: 1,
                    layout: WindowLayout {
                        layout_id: "b25d".to_owned(),
                        width: 80,
                        height: 24
                    },
                    visible_layout: None,
                    raw_flags: None
                },
                Event::LayoutChange {
                    window: 1,
                    layout: WindowLayout {
                        layout_id: "cafd".to_owned(),
                        width: 120,
                        height: 29
                    },
                    visible_layout: Some(WindowLayout {
                        layout_id: "cafd".to_owned(),
                        width: 120,
                        height: 29
                    }),
                    raw_flags: Some("*".to_owned())
                },
                Event::Output {
                    pane: 1,
                    text: "\x1b[1m\x1b[7m%\x1b[27m\x1b[1m\x1b[0m    \r \r".to_owned()
                },
                Event::Output {
                    pane: 1,
                    text: "\x1bkwez@cube-localdomain:~\x1b\\\x1b]2;wez@cube-localdomain:~\x1b\\"
                        .to_owned()
                },
                Event::Output {
                    pane: 1,
                    text: "\x1b]7;file://cube-localdomain/home/wez\x1b\\".to_owned(),
                },
                Event::Output {
                    pane: 1,
                    text: "\x1b[K\x1b[?2004h".to_owned(),
                },
                Event::Exit { reason: None },
                Event::Exit {
                    reason: Some("I said so".to_owned())
                },
            ],
            events
        );
    }
}
