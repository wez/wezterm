pub type RepeatCount = usize;

#[derive(Debug, Clone, Copy)]
pub enum Movement {
    BackwardChar(RepeatCount),
    ForwardChar(RepeatCount),
    StartOfLine,
    EndOfLine,
}

#[derive(Debug, Clone)]
pub enum Action {
    AcceptLine,
    InsertChar(RepeatCount, char),
    InsertText(RepeatCount, String),
    Repaint,
    Move(Movement),
    Kill(Movement),
}
