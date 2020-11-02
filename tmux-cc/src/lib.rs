use parser::Rule;
use pest::Parser as _;

mod parser {
    use pest_derive::Parser;
    #[derive(Parser)]
    #[grammar = "tmux.pest"]
    pub struct TmuxParser;
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
    Error(String),
    Output {
        pane: u64,
        text: String,
    },
    Exit,
    SessionsChanged,
    SessionChanged {
        session: u64,
        name: String,
    },
    PaneModeChanged {
        pane: u64,
    },
    WindowAdd {
        window: u64,
    },
}

fn parse_line(line: &str) -> anyhow::Result<Event> {
    let mut pairs = parser::TmuxParser::parse(Rule::line_entire, line)?;
    let pair = pairs.next().ok_or_else(|| anyhow::anyhow!("no pairs!?"))?;
    match pair.as_rule() {
        Rule::begin => {
            let mut pairs = pair.into_inner();
            let timestamp = pairs.next().unwrap().as_str().parse::<i64>()?;
            let number = pairs.next().unwrap().as_str().parse::<u64>()?;
            let flags = pairs.next().unwrap().as_str().parse::<i64>()?;
            Ok(Event::Begin {
                timestamp,
                number,
                flags,
            })
        }
        Rule::end => {
            let mut pairs = pair.into_inner();
            let timestamp = pairs.next().unwrap().as_str().parse::<i64>()?;
            let number = pairs.next().unwrap().as_str().parse::<u64>()?;
            let flags = pairs.next().unwrap().as_str().parse::<i64>()?;
            Ok(Event::End {
                timestamp,
                number,
                flags,
            })
        }
        Rule::error => {
            let mut pairs = pair.into_inner();
            Ok(Event::Error(unvis(pairs.next().unwrap().as_str())?))
        }
        Rule::exit => Ok(Event::Exit),
        Rule::sessions_changed => Ok(Event::SessionsChanged),
        Rule::pane_mode_changed => {
            let mut pairs = pair.into_inner();
            let pane = pairs.next().unwrap().as_str().parse()?;
            Ok(Event::PaneModeChanged { pane })
        }
        Rule::window_add => {
            let mut pairs = pair.into_inner();
            let window = pairs.next().unwrap().as_str().parse()?;
            Ok(Event::WindowAdd { window })
        }
        Rule::output => {
            let mut pairs = pair.into_inner();
            let pane = pairs.next().unwrap().as_str().parse()?;
            let text = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::Output { pane, text })
        }
        Rule::session_changed => {
            let mut pairs = pair.into_inner();
            let session = pairs.next().unwrap().as_str().parse()?;
            let name = unvis(pairs.next().unwrap().as_str())?;
            Ok(Event::SessionChanged { session, name })
        }
        Rule::any_text | Rule::line | Rule::line_entire | Rule::EOI | Rule::number => {
            unreachable!()
        }
    }
}

/// Decode OpenBSD `vis` encoded strings
/// See: https://github.com/tmux/tmux/blob/486ce9b09855ae30a2bf5e576cb6f7ad37792699/compat/unvis.c
fn unvis(s: &str) -> anyhow::Result<String> {
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
                    *state = State::Ctrl(0200);
                } else {
                    anyhow::bail!("invalid \\M escape: {}", b);
                }
            }

            State::Meta1 => {
                result.push(b | 0200);
                *state = State::Ground;
            }

            State::Ctrl(c) => {
                if b == b'?' {
                    result.push(*c | 0177);
                } else {
                    result.push((b & 037) | *c);
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
}

impl Parser {
    pub fn new() -> Self {
        Self { buffer: vec![] }
    }

    pub fn advance_byte(&mut self, c: u8) -> Option<Event> {
        if c == b'\n' {
            self.process_line()
        } else {
            self.buffer.push(c);
            None
        }
    }

    pub fn advance_string(&mut self, s: &str) -> Vec<Event> {
        self.advance_bytes(s.as_bytes())
    }

    pub fn advance_bytes(&mut self, bytes: &[u8]) -> Vec<Event> {
        let mut events = vec![];
        for &b in bytes {
            if let Some(event) = self.advance_byte(b) {
                events.push(event);
            }
        }
        events
    }

    fn process_line(&mut self) -> Option<Event> {
        if self.buffer.last() == Some(&b'\r') {
            self.buffer.pop();
        }
        let result = match std::str::from_utf8(&self.buffer) {
            Ok(line) => match parse_line(line) {
                Ok(event) => Some(event),
                Err(err) => {
                    log::error!("Unrecognized tmux cc line: {}", err);
                    None
                }
            },
            Err(err) => {
                log::error!("Failed to parse line from tmux: {}", err);
                None
            }
        };
        self.buffer.clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_line() {
        let _ = pretty_env_logger::formatted_builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        assert_eq!(
            Event::Error("doh".to_owned()),
            parse_line("%error doh").unwrap()
        );

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
%end 1604279270 310 0
%window-add @1
%sessions-changed
%session-changed $1 1
%output %1 \\033[1m\\033[7m%\\033[27m\\033[1m\\033[0m    \\015 \\015
%output %1 \\033kwez@cube-localdomain:~\\033\\134\\033]2;wez@cube-localdomain:~\\033\\134
%output %1 \\033]7;file://cube-localdomain/home/wez\\033\\134
%output %1 \\033[K\\033[?2004h
%exit
";

        let mut p = Parser::new();
        let events = p.advance_bytes(input);
        assert_eq!(
            vec![
                Event::SessionsChanged,
                Event::PaneModeChanged { pane: 0 },
                Event::Begin {
                    timestamp: 1604279270,
                    number: 310,
                    flags: 0
                },
                Event::End {
                    timestamp: 1604279270,
                    number: 310,
                    flags: 0
                },
                Event::WindowAdd { window: 1 },
                Event::SessionsChanged,
                Event::SessionChanged {
                    session: 1,
                    name: "1".to_owned(),
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
                Event::Exit,
            ],
            events
        );
    }
}
