pub trait TerminalConfiguration: std::fmt::Debug {
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

    //    fn hyperlink_rules(&self) -> &Vec<HyperlinkRule>;
}
