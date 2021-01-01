use pulldown_cmark::{Event, Options, Parser, Tag};
use std::io::Read;
use std::sync::Arc;
use termwiz::cell::*;
use termwiz::color::AnsiColor;
use termwiz::surface::Change;
use termwiz::terminal::ScreenSize;
use unicode_segmentation::UnicodeSegmentation;

pub struct RenderState {
    screen_size: ScreenSize,
    changes: Vec<Change>,
    current_list_item: Option<u64>,
    current_indent: Option<usize>,
    x_pos: usize,
    wrap_width: usize,
}

fn is_whitespace_word(word: &str) -> bool {
    word.chars().any(|c| c.is_whitespace())
}

impl RenderState {
    pub fn into_changes(self) -> Vec<Change> {
        self.changes
    }

    pub fn new(wrap_width: usize, screen_size: ScreenSize) -> Self {
        Self {
            changes: vec![],
            current_list_item: None,
            current_indent: None,
            x_pos: 0,
            wrap_width,
            screen_size,
        }
    }

    fn emit_indent(&mut self) {
        if let Some(indent) = self.current_indent {
            let mut s = String::new();
            for _ in 0..indent {
                s.push(' ');
            }
            self.changes.push(s.into());
            self.x_pos += indent;
        }
    }

    fn newline(&mut self) {
        self.changes.push("\r\n".into());
        self.x_pos = 0;
    }

    fn wrap_text(&mut self, text: &str) {
        for word in text.split_word_bounds() {
            let len = unicode_column_width(word);
            if self.x_pos + len < self.wrap_width {
                if !(self.x_pos == 0 && is_whitespace_word(word)) {
                    self.changes.push(word.into());
                    self.x_pos += len;
                }
            } else if len < self.wrap_width {
                self.newline();
                self.emit_indent();
                if !is_whitespace_word(word) {
                    self.changes.push(word.into());
                    self.x_pos += len;
                }
            } else {
                self.newline();
                self.emit_indent();
                self.changes.push(word.into());
                self.newline();
                self.emit_indent();
                self.x_pos = len;
            }
        }
    }

    fn apply_event(&mut self, event: Event) {
        match event {
            Event::Start(Tag::Paragraph) => {}
            Event::End(Tag::Paragraph) => {
                self.newline();
            }

            Event::Start(Tag::BlockQuote) => {}
            Event::End(Tag::BlockQuote) => {
                self.newline();
            }

            Event::Start(Tag::CodeBlock(_)) => {}
            Event::End(Tag::CodeBlock(_)) => {
                self.newline();
            }

            Event::Start(Tag::List(first_idx)) => {
                self.current_list_item = first_idx;
                self.newline();
            }
            Event::End(Tag::List(_)) => {
                self.newline();
            }

            Event::Start(Tag::Item) => {
                let list_item_prefix = if let Some(idx) = self.current_list_item.take() {
                    self.current_list_item.replace(idx + 1);
                    format!("  {}. ", idx)
                } else {
                    "  * ".to_owned()
                };
                let indent_width = unicode_column_width(&list_item_prefix);
                self.current_indent.replace(indent_width);
                self.changes.push(list_item_prefix.into());
                self.x_pos += indent_width;
            }
            Event::End(Tag::Item) => {
                self.newline();
                self.current_indent.take();
            }

            Event::Start(Tag::Heading(_)) => {
                self.newline();
                self.changes
                    .push(AttributeChange::Intensity(Intensity::Bold).into());
            }
            Event::End(Tag::Heading(_)) => {
                self.changes
                    .push(AttributeChange::Intensity(Intensity::Normal).into());
                self.newline();
            }

            Event::Start(Tag::Strikethrough) => {
                self.changes
                    .push(AttributeChange::StrikeThrough(true).into());
            }
            Event::End(Tag::Strikethrough) => {
                self.changes
                    .push(AttributeChange::StrikeThrough(false).into());
            }

            Event::Start(Tag::Emphasis) => {
                self.changes.push(AttributeChange::Italic(true).into());
            }
            Event::End(Tag::Emphasis) => {
                self.changes.push(AttributeChange::Italic(false).into());
            }

            Event::Start(Tag::Link(_linktype, url, _title)) => {
                self.changes.push(
                    AttributeChange::Hyperlink(Some(Arc::new(Hyperlink::new(url.into_string()))))
                        .into(),
                );
                self.changes
                    .push(AttributeChange::Underline(Underline::Single).into());
            }
            Event::End(Tag::Link(..)) => {
                self.changes.push(AttributeChange::Hyperlink(None).into());
                self.changes
                    .push(AttributeChange::Underline(Underline::None).into());
            }
            
            Event::Start(Tag::Link(_filetype, url, _title)) => {
                self.changes.push(
                    AttributeChange::Hyperfile(Some(Arc::new(Hyperfile::new(url.into_string()))))
                        .into(),
                );
                self.changes
                    .push(AttributeChange::Underline(Underline::Single).into());
            }
            Event::End(Tag::Link(..)) => {
                self.changes.push(AttributeChange::Hyperfile(None).into());
                self.changes
                    .push(AttributeChange::Underline(Underline::None).into());
            }

            Event::Start(Tag::Image(_linktype, img_url, _title)) => {
                use image::GenericImageView;
                use termwiz::image::TextureCoordinate;

                let url: &str = img_url.as_ref();
                if let Ok(mut f) = std::fs::File::open(url) {
                    let mut data = vec![];
                    if let Ok(_len) = f.read_to_end(&mut data) {
                        if let Ok(decoded_image) = image::load_from_memory(&data) {
                            let image = Arc::new(termwiz::image::ImageData::with_raw_data(data));

                            let scale = self.wrap_width as f32 / decoded_image.width() as f32;

                            let aspect_ratio =
                                if self.screen_size.xpixel == 0 || self.screen_size.ypixel == 0 {
                                    // Guess: most monospace fonts are twice as tall as they are wide
                                    2.0
                                } else {
                                    let cell_height = self.screen_size.ypixel as f32
                                        / self.screen_size.rows as f32;
                                    let cell_width = self.screen_size.xpixel as f32
                                        / self.screen_size.cols as f32;
                                    cell_height / cell_width
                                };

                            let height = decoded_image.height() as f32 * scale / aspect_ratio;

                            self.newline();
                            self.changes.push(termwiz::surface::Change::Image(
                                termwiz::surface::Image {
                                    width: self.wrap_width,
                                    height: height as usize,
                                    top_left: TextureCoordinate::new_f32(0., 0.),
                                    bottom_right: TextureCoordinate::new_f32(1., 1.),
                                    image,
                                },
                            ));
                            self.newline();
                        }
                    }
                }
            }
            Event::End(Tag::Image(_linktype, _img_url, _title)) => {}

            Event::Start(Tag::Strong) => {
                self.changes
                    .push(AttributeChange::Intensity(Intensity::Bold).into());
            }
            Event::End(Tag::Strong) => {
                self.changes
                    .push(AttributeChange::Intensity(Intensity::Normal).into());
            }

            Event::Start(Tag::FootnoteDefinition(_label)) => {}
            Event::End(Tag::FootnoteDefinition(_)) => {}

            Event::Start(Tag::Table(_alignment)) => {}
            Event::End(Tag::Table(_)) => {}

            Event::Start(Tag::TableHead) => {}
            Event::End(Tag::TableHead) => {}

            Event::Start(Tag::TableRow) => {}
            Event::End(Tag::TableRow) => {}

            Event::Start(Tag::TableCell) => {}
            Event::End(Tag::TableCell) => {}

            Event::FootnoteReference(s) | Event::Text(s) | Event::Html(s) => {
                self.wrap_text(&s);
            }

            Event::Code(s) => {
                self.changes
                    .push(AttributeChange::Foreground(AnsiColor::Fuschia.into()).into());
                self.wrap_text(&s);
                self.changes
                    .push(AttributeChange::Foreground(Default::default()).into());
            }

            Event::SoftBreak => {
                self.wrap_text(" ");
            }

            Event::HardBreak => {
                self.newline();
                self.emit_indent();
            }

            Event::Rule => {
                self.changes.push("---".into());
                self.newline();
            }

            Event::TaskListMarker(true) => {
                self.changes.push("[x]".into());
            }

            Event::TaskListMarker(false) => {
                self.changes.push("[ ]".into());
            }
        }
    }

    pub fn parse_str(&mut self, s: &str) {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(s, options);

        for event in parser {
            self.apply_event(event);
        }
    }
}
