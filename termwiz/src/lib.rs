//! # Terminal Wizardry
//!
//! This is a rust crate that provides a number of support functions
//! for applications interested in either displaying data to a terminal
//! or in building a terminal emulator.
//!
//! It is currently in active development and subject to fairly wild
//! sweeping changes.
//!
//! Included functionality:
//!
//! * `Surface` models a terminal display and its component `Cell`s
//! * Terminal attributes are aware of modern features such as
//!   True Color, [Hyperlinks](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
//!   and will also support sixel and iterm style terminal graphics display.
//! * `Surface`s include a log of `Change`s and an API for consuming
//!   and applying deltas.  This is a powerful building block for
//!   synchronizing screen instances.
//! * Escape sequence parser decodes inscrutable escape sequences
//!   and gives them semantic meaning, making the code that uses
//!   them clearer.  The decoded escapes can be re-encoded, allowing
//!   applications to start with the semantic meaning and emit
//!   the appropriate escape sequence without embedding obscure
//!   binary bytes.
//! * `Capabilities` allows probing for terminal capabilities
//!   that may not be included in the system terminfo database,
//!   and overriding them in an embedding application.
//! * `Terminal` trait provides an abstraction over unix style ttys
//!   and Windows style console APIs.  `Change`s from `Surface`
//!   can be rendered to `Terminal`s.  `Terminal`s allow decoding
//!   mouse and keyboard inputs in both blocking or non-blocking
//!   mode.
//! * `Widget` trait allows composition of UI elements at a higher
//!   level.
//! * `LineEditor` provides line editing facilities similar to those
//!   in the unix shell.
//!
//! ## Features
//!
//! * `widgets` - enables the widget layout and related traits
//! * `use_serde` - makes a number of structs serde serializable

pub mod caps;
pub use wezterm_cell as cell;
pub use wezterm_cell::color;
pub use wezterm_surface::cellcluster;
pub mod error;
#[cfg(feature = "use_image")]
pub use wezterm_cell::image;
pub use wezterm_surface::hyperlink;
pub mod input;
pub mod istty;
pub mod keymap;
pub mod lineedit;
mod macros;
pub use wezterm_char_props::nerdfonts;
mod readbuf;
pub mod render;
pub use wezterm_surface as surface;
pub mod terminal;
#[cfg(feature = "tmux_cc")]
pub use wezterm_escape_parser::tmux_cc;
#[cfg(feature = "widgets")]
pub mod widgets;

pub use error::{Context, Error, Result};

pub use wezterm_escape_parser as escape;
