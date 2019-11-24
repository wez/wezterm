use termwiz::hyperlink::Rule as HyperlinkRule;

pub trait TerminalConfiguration: std::fmt::Debug {
    /// Returns a generation counter for the active
    /// configuration.  If the implementation may be
    /// changed at runtime, it must increment the generation
    /// number with each change so that any caches maintained
    /// by the terminal can be flushed.
    fn generation(&self) -> usize {
        0
    }

    fn scrollback_size(&self) -> usize {
        3500
    }

    // TODO: expose is_double_click_word in config file
    fn is_double_click_word(&self, s: &str) -> bool {
        if s.len() > 1 {
            true
        } else if s.len() == 1 {
            match s.chars().nth(0).unwrap() {
                ' ' | '\t' | '\n' | '{' | '[' | '}' | ']' | '(' | ')' | '"' | '\'' => false,
                _ => true,
            }
        } else {
            false
        }
    }

    // TODO: expose scroll_to_bottom_on_key_input in config file
    fn scroll_to_bottom_on_key_input(&self) -> bool {
        true
    }

    /// Returns the current generation and its associated hyperlink rules.
    fn hyperlink_rules(&self) -> (usize, Vec<HyperlinkRule>) {
        (self.generation(), vec![])
    }
}
