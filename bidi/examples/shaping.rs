use wezterm_bidi::{BidiContext, Direction, ParagraphDirectionHint};

fn main() {
    // The UBA is strongly coupled with codepoints and indices into the
    // original text, and that fans out to our API here.
    //
    // paragraph is a Vec<char>.
    let paragraph = vec!['א', 'ב', 'ג', 'a', 'b', 'c'];

    let mut context = BidiContext::new();

    // Leave it to the algorithm to determine the paragraph direction.
    // If you have some higher level understanding or override for the
    // direction, you can set `direction` accordingly.
    let hint = ParagraphDirectionHint::AutoLeftToRight;

    // Resolve the embedding levels for our paragraph.
    context.resolve_paragraph(&paragraph, hint);

    /// In order to layout the text, we need to feed information to a shaper.
    /// For the purposes of example, we're sketching out a stub shaper interface
    /// here, which is essentially compatible with eg: Harfbuzz's buffer data type.
    struct ShaperBuffer {}
    impl ShaperBuffer {
        pub fn add_codepoint(&mut self, codepoint: char) {
            let _ = codepoint;
            // could call hb_buffer_add_codepoints() here
        }
        pub fn set_direction(&mut self, direction: Direction) {
            let _ = direction;
            // could call hb_buffer_set_direction() here
        }
        pub fn reset(&mut self) {}
        pub fn shape(&mut self) {}
    }

    let mut buffer = ShaperBuffer {};
    for run in context.runs() {
        buffer.reset();
        buffer.set_direction(run.direction);
        for idx in run.indices() {
            buffer.add_codepoint(paragraph[idx]);
        }

        buffer.shape();

        // Now it is up to you to use the information from the shaper
        // to decide whether and how the paragraph should be wrapped
        // into lines
    }
}
