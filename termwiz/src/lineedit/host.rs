use crate::cell::{AttributeChange, CellAttributes};
use crate::lineedit::{BasicHistory, History};
use crate::surface::Change;

/// The `OutputElement` type allows returning graphic attribute changes
/// as well as textual output.
pub enum OutputElement {
    /// Change a single attribute
    Attribute(AttributeChange),
    /// Change all possible attributes to the given set of values
    AllAttributes(CellAttributes),
    /// Printable text.
    /// Control characters are rendered inert by transforming them
    /// to space.  CR and LF characters are interpreted by moving
    /// the cursor position.  CR moves the cursor to the start of
    /// the line and LF moves the cursor down to the next line.
    /// You typically want to use both together when sending in
    /// a line break.
    Text(String),
}

impl Into<Change> for OutputElement {
    fn into(self) -> Change {
        match self {
            OutputElement::Attribute(a) => Change::Attribute(a),
            OutputElement::AllAttributes(a) => Change::AllAttributes(a),
            OutputElement::Text(t) => Change::Text(t),
        }
    }
}

/// The `LineEditorHost` trait allows an embedding application to influence
/// how the line editor functions.
/// A concrete implementation of the host with neutral defaults is provided
/// as `NopLineEditorHost`.
pub trait LineEditorHost {
    /// Given a prompt string, return the rendered form of the prompt as
    /// a sequence of `OutputElement` instances.
    /// The implementation is free to interpret the prompt string however
    /// it chooses; for instance, the application can opt to expand its own
    /// application specific escape sequences as it sees fit.
    /// The `OutputElement` type allows returning graphic attribute changes
    /// as well as textual output.
    /// The default implementation returns the prompt as-is with no coloring
    /// and no textual transformation.
    fn render_prompt(&self, prompt: &str) -> Vec<OutputElement> {
        vec![OutputElement::Text(prompt.to_owned())]
    }

    /// Given a reference to the current line being edited and the position
    /// of the cursor, return the rendered form of the line as a sequence
    /// of `OutputElement` instances.
    /// While this interface technically allows returning arbitrary Text sequences,
    /// the application should preserve the column positions of the graphemes,
    /// otherwise the terminal cursor position won't match up to the correct
    /// location.
    /// The `OutputElement` type allows returning graphic attribute changes
    /// as well as textual output.
    /// The default implementation returns the line as-is with no coloring.
    fn highlight_line(&self, line: &str, _cursor_position: usize) -> Vec<OutputElement> {
        vec![OutputElement::Text(line.to_owned())]
    }

    /// Returns the history implementation
    fn history(&mut self) -> &mut dyn History;

    /// Tab completion support.
    /// The line and current cursor position are provided and it is up to the
    /// embedding application to produce a list of completion candidates.
    /// The default implementation is an empty list.
    fn complete(&self, _line: &str, _cursor_position: usize) -> Vec<CompletionCandidate> {
        vec![]
    }
}

/// A candidate for tab completion.
/// If the line and cursor look like "why he<CURSOR>" and if "hello" is a valid
/// completion of "he" in that context, then the corresponding CompletionCandidate
/// would have its range set to [4..6] (the "he" slice range) and its text
/// set to "hello".
pub struct CompletionCandidate {
    /// The section of the input line to be replaced
    pub range: std::ops::Range<usize>,
    /// The replacement text
    pub text: String,
}

/// A concrete implementation of `LineEditorHost` that uses the default behaviors.
#[derive(Default)]
pub struct NopLineEditorHost {
    history: BasicHistory,
}
impl LineEditorHost for NopLineEditorHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }
}
