use crate::color::ColorPalette;
use termwiz::hyperlink::Rule as HyperlinkRule;
use termwiz::hyperfile::Rule as HyperfileRule;

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

    /// Returns the current generation and its associated hyperfile rules.
    /// hyperfile rules are used to recognize and automatically generate
    /// hyperfile attributes for runs of text that match the provided rules.
    fn hyperfile_rules(&self) -> (usize, Vec<HyperfileRule>) {
        (self.generation(), vec![])
    }

    /// Returns the default color palette for the application.
    /// Various escape sequences can dynamically modify the effective
    /// color palette for a terminal instance at runtime, but this method
    /// defines the initial palette.
    fn color_palette(&self) -> ColorPalette;

    /// Return true if a resize operation should consider rows that have
    /// made it to scrollback as being immutable.
    /// When immutable, the resize operation will pad out the screen height
    /// with additional blank rows and due to implementation details means
    /// that the user will need to scroll back the scrollbar post-resize
    /// than they would otherwise.
    ///
    /// When mutable, resizing the window taller won't add extra rows;
    /// instead the resize will tend to have "bottom gravity" meaning that
    /// making the window taller will reveal more history than in the other
    /// mode.
    ///
    /// mutable is generally speaking a nicer experience.
    ///
    /// On Windows, the PTY layer doesn't play well with a mutable scrollback,
    /// frequently moving the cursor up to high and erasing portions of the
    /// screen.
    ///
    /// This behavior only happens with the windows pty layer; it doesn't
    /// manifest when using eg: ssh directly to a remote unix system.
    ///
    /// Ideally we'd have this return `true` only for the native windows
    /// pty layer, but for the sake of simplicity, we make this conditional
    /// on being a windows build.
    fn resize_preserves_scrollback(&self) -> bool {
        cfg!(windows)
    }
}
