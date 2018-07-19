use cell::CellAttributes;
use failure;
use screen::Change;
use terminal::Terminal;

/// The `Renderer` trait defines a way to translate a sequence
/// of `Change`s into an output stream.  This is typically a
/// sequence of ANSI or other escape sequences suitable for the
/// active terminal.
/// This interface is suitable for the unix pty interface but is
/// likely to fall short for the classic windows console API.
pub trait Renderer {
    /// Given a starting set of attributes, successively consider each
    /// of the entries in `changes` and emit the appropriate sequence
    /// of data to `out` such that an associated pty would render the
    /// information described by `changes`.  Returns the attribute
    /// value of the terminal at the end of the stream of changes.
    ///
    /// The intent is that you'd set `starting_attr` to
    /// `CellAttributes::default()` on the first call, then feed the return
    /// value in on subsequent calls to maintain the running view of the
    /// attributes.
    fn render_to(
        &self,
        starting_attr: &CellAttributes,
        changes: &[Change],
        out: &mut Terminal,
    ) -> Result<CellAttributes, failure::Error>;
}

pub mod terminfo;
