use crate::color::ColorPalette;
use termwiz::hyperlink::Rule as HyperlinkRule;

/// TerminalConfiguration allows for the embedding application to pass configuration
/// information to the Terminal.
/// The configuration can be changed at runtime; provided that the implementation
/// increments the generation counter appropriately, the changes will be detected
/// and applied at the next appropriate opportunity.
pub trait TerminalConfiguration: std::fmt::Debug {
    /// Returns a generation counter for the active
    /// configuration.  If the implementation may be
    /// changed at runtime, it must increment the generation
    /// number with each change so that any caches maintained
    /// by the terminal can be flushed.
    fn generation(&self) -> usize {
        0
    }

    /// Returns the size of the scrollback in terms of the number of rows.
    fn scrollback_size(&self) -> usize {
        3500
    }

    /// Return true if the embedding application wants to use CSI-u encoding
    /// for keys that would otherwise be ambiguous.
    /// <http://www.leonerd.org.uk/hacks/fixterms/>
    fn enable_csi_u_key_encoding(&self) -> bool {
        false
    }

    /// Returns the current generation and its associated hyperlink rules.
    /// hyperlink rules are used to recognize and automatically generate
    /// hyperlink attributes for runs of text that match the provided rules.
    fn hyperlink_rules(&self) -> (usize, Vec<HyperlinkRule>) {
        (self.generation(), vec![])
    }

    /// Returns the default color palette for the application.
    /// Various escape sequences can dynamically modify the effective
    /// color palette for a terminal instance at runtime, but this method
    /// defines the initial palette.
    fn color_palette(&self) -> ColorPalette;
}
