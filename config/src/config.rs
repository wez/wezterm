use crate::background::{BackgroundLayer, Gradient};
use crate::bell::{AudibleBell, EasingFunction, VisualBell};
use crate::color::{
    ColorSchemeFile, HsbTransform, Palette, SrgbaTuple, TabBarStyle, WindowFrameConfig,
};
use crate::daemon::DaemonOptions;
use crate::exec_domain::ExecDomain;
use crate::font::{
    AllowSquareGlyphOverflow, DisplayPixelGeometry, FontLocatorSelection, FontRasterizerSelection,
    FontShaperSelection, FreeTypeLoadFlags, FreeTypeLoadTarget, StyleRule, TextStyle,
};
use crate::frontend::FrontEndSelection;
use crate::keyassignment::{
    KeyAssignment, KeyTable, KeyTableEntry, KeyTables, MouseEventTrigger, SpawnCommand,
};
use crate::keys::{Key, LeaderKey, Mouse};
use crate::lua::make_lua_context;
use crate::ssh::{SshBackend, SshDomain};
use crate::tls::{TlsDomainClient, TlsDomainServer};
use crate::units::Dimension;
use crate::unix::UnixDomain;
use crate::wsl::WslDomain;
use crate::{
    default_config_with_overrides_applied, default_one_point_oh, default_one_point_oh_f64,
    default_true, default_win32_acrylic_accent_color, GpuInfo, IntegratedTitleButtonColor,
    KeyMapPreference, LoadedConfig, MouseEventTriggerMods, RgbaColor, SerialDomain, SystemBackdrop,
    WebGpuPowerPreference, CONFIG_DIRS, CONFIG_FILE_OVERRIDE, CONFIG_OVERRIDES, CONFIG_SKIP,
    HOME_DIR,
};
use anyhow::Context;
use luahelper::impl_lua_conversion_dynamic;
use mlua::FromLua;
use portable_pty::CommandBuilder;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use termwiz::hyperlink;
use termwiz::surface::CursorShape;
use wezterm_bidi::ParagraphDirectionHint;
use wezterm_config_derive::ConfigMeta;
use wezterm_dynamic::{FromDynamic, ToDynamic};
use wezterm_input_types::{
    IntegratedTitleButton, IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle, Modifiers,
    UIKeyCapRendering, WindowDecorations,
};
use wezterm_term::TerminalSize;

#[derive(Debug, Clone, FromDynamic, ToDynamic, ConfigMeta)]
pub struct Config {
    /// The font size, measured in points
    #[dynamic(default = "default_font_size")]
    pub font_size: f64,

    #[dynamic(
        default = "default_one_point_oh_f64",
        validate = "validate_line_height"
    )]
    pub line_height: f64,

    #[dynamic(default = "default_one_point_oh_f64")]
    pub cell_width: f64,

    #[dynamic(try_from = "crate::units::OptPixelUnit", default)]
    pub cursor_thickness: Option<Dimension>,

    #[dynamic(try_from = "crate::units::OptPixelUnit", default)]
    pub underline_thickness: Option<Dimension>,

    #[dynamic(try_from = "crate::units::OptPixelUnit", default)]
    pub underline_position: Option<Dimension>,

    #[dynamic(try_from = "crate::units::OptPixelUnit", default)]
    pub strikethrough_position: Option<Dimension>,

    #[dynamic(default)]
    pub allow_square_glyphs_to_overflow_width: AllowSquareGlyphOverflow,

    #[dynamic(default)]
    pub window_decorations: WindowDecorations,

    #[dynamic(default = "default_integrated_title_buttons")]
    pub integrated_title_buttons: Vec<IntegratedTitleButton>,

    #[dynamic(default)]
    pub log_unknown_escape_sequences: bool,

    #[dynamic(default)]
    pub integrated_title_button_alignment: IntegratedTitleButtonAlignment,

    #[dynamic(default)]
    pub integrated_title_button_style: IntegratedTitleButtonStyle,

    #[dynamic(default)]
    pub integrated_title_button_color: IntegratedTitleButtonColor,

    /// When using FontKitXXX font systems, a set of directories to
    /// search ahead of the standard font locations for fonts.
    /// Relative paths are taken to be relative to the directory
    /// from which the config was loaded.
    #[dynamic(default)]
    pub font_dirs: Vec<PathBuf>,

    #[dynamic(default)]
    pub color_scheme_dirs: Vec<PathBuf>,

    /// The DPI to assume
    pub dpi: Option<f64>,

    #[dynamic(default)]
    pub dpi_by_screen: HashMap<String, f64>,

    /// The baseline font to use
    #[dynamic(default)]
    pub font: TextStyle,

    /// An optional set of style rules to select the font based
    /// on the cell attributes
    #[dynamic(default)]
    pub font_rules: Vec<StyleRule>,

    /// When true (the default), PaletteIndex 0-7 are shifted to
    /// bright when the font intensity is bold.  The brightening
    /// doesn't apply to text that is the default color.
    #[dynamic(default)]
    pub bold_brightens_ansi_colors: BoldBrightening,

    /// The color palette
    pub colors: Option<Palette>,

    #[dynamic(default)]
    pub switch_to_last_active_tab_when_closing_tab: bool,

    /// When true, launching a new wezterm instance will prefer
    /// to spawn a new tab into an existing instance.
    /// Otherwise, it will spawn a new window.
    #[dynamic(default)]
    pub prefer_to_spawn_tabs: bool,

    #[dynamic(default)]
    pub window_frame: WindowFrameConfig,

    #[dynamic(default = "default_char_select_font_size")]
    pub char_select_font_size: f64,

    #[dynamic(default = "default_char_select_fg_color")]
    pub char_select_fg_color: RgbaColor,

    #[dynamic(default = "default_char_select_bg_color")]
    pub char_select_bg_color: RgbaColor,

    #[dynamic(default = "default_command_palette_font_size")]
    pub command_palette_font_size: f64,

    pub command_palette_rows: Option<usize>,
    #[dynamic(default = "default_command_palette_fg_color")]
    pub command_palette_fg_color: RgbaColor,

    #[dynamic(default = "default_command_palette_bg_color")]
    pub command_palette_bg_color: RgbaColor,

    #[dynamic(default = "default_pane_select_font_size")]
    pub pane_select_font_size: f64,

    #[dynamic(default = "default_pane_select_fg_color")]
    pub pane_select_fg_color: RgbaColor,

    #[dynamic(default = "default_pane_select_bg_color")]
    pub pane_select_bg_color: RgbaColor,

    #[dynamic(default)]
    pub tab_bar_style: TabBarStyle,

    #[dynamic(default)]
    pub resolved_palette: Palette,

    /// Use a named color scheme rather than the palette specified
    /// by the colors setting.
    pub color_scheme: Option<String>,

    /// Named color schemes
    #[dynamic(default)]
    pub color_schemes: HashMap<String, Palette>,

    /// How many lines of scrollback you want to retain
    #[dynamic(
        default = "default_scrollback_lines",
        validate = "validate_scrollback_lines"
    )]
    pub scrollback_lines: usize,

    /// If no `prog` is specified on the command line, use this
    /// instead of running the user's shell.
    /// For example, to have `wezterm` always run `top` by default,
    /// you'd use this:
    ///
    /// ```toml
    /// default_prog = ["top"]
    /// ```
    ///
    /// `default_prog` is implemented as an array where the 0th element
    /// is the command to run and the rest of the elements are passed
    /// as the positional arguments to that command.
    pub default_prog: Option<Vec<String>>,

    #[dynamic(default = "default_gui_startup_args")]
    pub default_gui_startup_args: Vec<String>,

    /// Specifies the default current working directory if none is specified
    /// through configuration or OSC 7 (see docs for `default_cwd` for more
    /// info!)
    pub default_cwd: Option<PathBuf>,

    #[dynamic(default)]
    pub exit_behavior: ExitBehavior,

    #[dynamic(default)]
    pub exit_behavior_messaging: ExitBehaviorMessaging,

    #[dynamic(default = "default_clean_exits")]
    pub clean_exit_codes: Vec<u32>,

    #[dynamic(default = "default_true")]
    pub detect_password_input: bool,

    /// Specifies a map of environment variables that should be set
    /// when spawning commands in the local domain.
    /// This is not used when working with remote domains.
    #[dynamic(default)]
    pub set_environment_variables: HashMap<String, String>,

    /// Specifies the height of a new window, expressed in character cells.
    #[dynamic(default = "default_initial_rows", validate = "validate_row_or_col")]
    pub initial_rows: u16,

    #[dynamic(default = "default_true")]
    pub enable_kitty_graphics: bool,
    #[dynamic(default)]
    pub enable_kitty_keyboard: bool,

    /// Whether the terminal should respond to requests to read the
    /// title string.
    /// Disabled by default for security concerns with shells that might
    /// otherwise attempt to execute the response.
    /// <https://marc.info/?l=bugtraq&m=104612710031920&w=2>
    #[dynamic(default)]
    pub enable_title_reporting: bool,

    /// Specifies the width of a new window, expressed in character cells
    #[dynamic(default = "default_initial_cols", validate = "validate_row_or_col")]
    pub initial_cols: u16,

    #[dynamic(default = "default_hyperlink_rules")]
    pub hyperlink_rules: Vec<hyperlink::Rule>,

    /// What to set the TERM variable to
    #[dynamic(default = "default_term")]
    pub term: String,

    #[dynamic(default)]
    pub font_locator: FontLocatorSelection,
    #[dynamic(default)]
    pub font_rasterizer: FontRasterizerSelection,
    #[dynamic(default = "default_colr_rasterizer")]
    pub font_colr_rasterizer: FontRasterizerSelection,
    #[dynamic(default)]
    pub font_shaper: FontShaperSelection,

    #[dynamic(default)]
    pub display_pixel_geometry: DisplayPixelGeometry,
    #[dynamic(default)]
    pub freetype_load_target: FreeTypeLoadTarget,
    #[dynamic(default)]
    pub freetype_render_target: Option<FreeTypeLoadTarget>,
    #[dynamic(default)]
    pub freetype_load_flags: Option<FreeTypeLoadFlags>,

    /// Selects the freetype interpret version to use.
    /// Likely values are 35, 38 and 40 which have different
    /// characteristics with respective to subpixel hinting.
    /// See https://freetype.org/freetype2/docs/subpixel-hinting.html
    pub freetype_interpreter_version: Option<u32>,

    #[dynamic(default)]
    pub freetype_pcf_long_family_names: bool,

    /// Specify the features to enable when using harfbuzz for font shaping.
    /// There is some light documentation here:
    /// <https://harfbuzz.github.io/shaping-opentype-features.html>
    /// but it boils down to allowing opentype feature names to be specified
    /// using syntax similar to the CSS font-feature-settings options:
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-feature-settings>.
    /// The OpenType spec lists a number of features here:
    /// <https://docs.microsoft.com/en-us/typography/opentype/spec/featurelist>
    ///
    /// Options of likely interest will be:
    ///
    /// * `calt` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-calt>
    /// * `clig` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-clig>
    ///
    /// If you want to disable ligatures in most fonts, then you may want to
    /// use a setting like this:
    ///
    /// ```toml
    /// harfbuzz_features = ["calt=0", "clig=0", "liga=0"]
    /// ```
    ///
    /// Some fonts make available extended options via stylistic sets.
    /// If you use the [Fira Code font](https://github.com/tonsky/FiraCode),
    /// it lists available stylistic sets here:
    /// <https://github.com/tonsky/FiraCode/wiki/How-to-enable-stylistic-sets>
    ///
    /// and you can set them in wezterm:
    ///
    /// ```toml
    /// # Use this for a zero with a dot rather than a line through it
    /// # when using the Fira Code font
    /// harfbuzz_features = ["zero"]
    /// ```
    #[dynamic(default = "default_harfbuzz_features")]
    pub harfbuzz_features: Vec<String>,

    #[dynamic(default)]
    pub front_end: FrontEndSelection,

    /// Whether to select the higher powered discrete GPU when
    /// the system has a choice of integrated or discrete.
    /// Defaults to low power.
    #[dynamic(default)]
    pub webgpu_power_preference: WebGpuPowerPreference,

    #[dynamic(default)]
    pub webgpu_force_fallback_adapter: bool,

    #[dynamic(default)]
    pub webgpu_preferred_adapter: Option<GpuInfo>,

    #[dynamic(default)]
    pub wsl_domains: Option<Vec<WslDomain>>,

    #[dynamic(default)]
    pub exec_domains: Vec<ExecDomain>,

    #[dynamic(default)]
    pub serial_ports: Vec<SerialDomain>,

    /// The set of unix domains
    #[dynamic(default = "UnixDomain::default_unix_domains")]
    pub unix_domains: Vec<UnixDomain>,

    #[dynamic(default)]
    pub ssh_domains: Option<Vec<SshDomain>>,

    #[dynamic(default)]
    pub ssh_backend: SshBackend,

    /// When running in server mode, defines configuration for
    /// each of the endpoints that we'll listen for connections
    #[dynamic(default)]
    pub tls_servers: Vec<TlsDomainServer>,

    /// The set of tls domains that we can connect to as a client
    #[dynamic(default)]
    pub tls_clients: Vec<TlsDomainClient>,

    /// Constrains the rate at which the multiplexer client will
    /// speculatively fetch line data.
    /// This helps to avoid saturating the link between the client
    /// and server if the server is dumping a large amount of output
    /// to the client.
    #[dynamic(default = "default_ratelimit_line_prefetches_per_second")]
    pub ratelimit_mux_line_prefetches_per_second: u32,

    /// The buffer size used by parse_buffered_data in the mux module.
    /// This should not be too large, otherwise the processing cost
    /// of applying a batch of actions to the terminal will be too
    /// high and the user experience will be laggy and less responsive.
    #[dynamic(default = "default_mux_output_parser_buffer_size")]
    pub mux_output_parser_buffer_size: usize,

    #[dynamic(default = "default_true")]
    pub mux_enable_ssh_agent: bool,

    #[dynamic(default)]
    pub default_ssh_auth_sock: Option<String>,

    /// How many ms to delay after reading a chunk of output
    /// in order to try to coalesce fragmented writes into
    /// a single bigger chunk of output and reduce the chances
    /// observing "screen tearing" with un-synchronized output
    #[dynamic(default = "default_mux_output_parser_coalesce_delay_ms")]
    pub mux_output_parser_coalesce_delay_ms: u64,

    #[dynamic(default = "default_mux_env_remove")]
    pub mux_env_remove: Vec<String>,

    #[dynamic(default)]
    pub keys: Vec<Key>,
    #[dynamic(default)]
    pub key_tables: HashMap<String, Vec<Key>>,

    #[dynamic(default = "default_bypass_mouse_reporting_modifiers")]
    pub bypass_mouse_reporting_modifiers: Modifiers,

    #[dynamic(default)]
    pub debug_key_events: bool,

    #[dynamic(default)]
    pub normalize_output_to_unicode_nfc: bool,

    #[dynamic(default)]
    pub disable_default_key_bindings: bool,
    pub leader: Option<LeaderKey>,

    #[dynamic(default)]
    pub disable_default_quick_select_patterns: bool,
    #[dynamic(default)]
    pub quick_select_patterns: Vec<String>,
    #[dynamic(default = "default_alphabet")]
    pub quick_select_alphabet: String,

    #[dynamic(default)]
    pub mouse_bindings: Vec<Mouse>,
    #[dynamic(default)]
    pub disable_default_mouse_bindings: bool,

    #[dynamic(default)]
    pub daemon_options: DaemonOptions,

    #[dynamic(default)]
    pub send_composed_key_when_left_alt_is_pressed: bool,

    #[dynamic(default = "default_true")]
    pub send_composed_key_when_right_alt_is_pressed: bool,

    #[dynamic(default = "default_macos_forward_mods")]
    pub macos_forward_to_ime_modifier_mask: Modifiers,

    #[dynamic(default)]
    pub treat_left_ctrlalt_as_altgr: bool,

    /// If true, the `Backspace` and `Delete` keys generate `Delete` and `Backspace`
    /// keypresses, respectively, rather than their normal keycodes.
    /// On macOS the default for this is true because its Backspace key
    /// is labeled as Delete and things are backwards.
    #[dynamic(default = "default_swap_backspace_and_delete")]
    pub swap_backspace_and_delete: bool,

    /// If true, display the tab bar UI at the top of the window.
    /// The tab bar shows the titles of the tabs and which is the
    /// active tab.  Clicking on a tab activates it.
    #[dynamic(default = "default_true")]
    pub enable_tab_bar: bool,
    #[dynamic(default = "default_true")]
    pub use_fancy_tab_bar: bool,

    #[dynamic(default)]
    pub tab_bar_at_bottom: bool,

    #[dynamic(default = "default_true")]
    pub mouse_wheel_scrolls_tabs: bool,

    /// If true, tab bar titles are prefixed with the tab index
    #[dynamic(default = "default_true")]
    pub show_tab_index_in_tab_bar: bool,

    #[dynamic(default = "default_true")]
    pub show_tabs_in_tab_bar: bool,

    #[dynamic(default = "default_true")]
    pub show_new_tab_button_in_tab_bar: bool,

    #[dynamic(default = "default_true")]
    pub show_close_tab_button_in_tabs: bool,

    /// If true, show_tab_index_in_tab_bar uses a zero-based index.
    /// The default is false and the tab shows a one-based index.
    #[dynamic(default)]
    pub tab_and_split_indices_are_zero_based: bool,

    /// Specifies the maximum width that a tab can have in the
    /// tab bar.  Defaults to 16 glyphs in width.
    #[dynamic(default = "default_tab_max_width")]
    pub tab_max_width: usize,

    /// If true, hide the tab bar if the window only has a single tab.
    #[dynamic(default)]
    pub hide_tab_bar_if_only_one_tab: bool,

    #[dynamic(default)]
    pub enable_scroll_bar: bool,

    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_half_cell")]
    pub min_scroll_bar_height: Dimension,

    /// If false, do not try to use a Wayland protocol connection
    /// when starting the gui frontend, and instead use X11.
    /// This option is only considered on X11/Wayland systems and
    /// has no effect on macOS or Windows.
    /// The default is true.
    #[dynamic(default = "default_true")]
    pub enable_wayland: bool,
    #[dynamic(default)]
    pub enable_zwlr_output_manager: bool,

    /// Whether to prefer EGL over other GL implementations.
    /// EGL on Windows has jankier resize behavior than WGL (which
    /// is used if EGL is unavailable), but EGL survives graphics
    /// driver updates without breaking and losing your work.
    #[dynamic(default = "default_prefer_egl")]
    pub prefer_egl: bool,

    #[dynamic(default = "default_true")]
    pub custom_block_glyphs: bool,
    #[dynamic(default = "default_true")]
    pub anti_alias_custom_block_glyphs: bool,

    /// Controls the amount of padding to use around the terminal cell area
    #[dynamic(default)]
    pub window_padding: WindowPadding,

    /// Specifies the path to a background image attachment file.
    /// The file can be any image format that the rust `image`
    /// crate is able to identify and load.
    /// A window background image is rendered into the background
    /// of the window before any other content.
    ///
    /// The image will be scaled to fit the window.
    #[dynamic(default)]
    pub window_background_image: Option<PathBuf>,
    #[dynamic(default)]
    pub window_background_gradient: Option<Gradient>,
    #[dynamic(default)]
    pub window_background_image_hsb: Option<HsbTransform>,
    #[dynamic(default)]
    pub foreground_text_hsb: HsbTransform,

    #[dynamic(default)]
    pub background: Vec<BackgroundLayer>,

    /// Only works on MacOS
    #[dynamic(default)]
    pub macos_window_background_blur: i64,

    /// Only works on Windows
    #[dynamic(default)]
    pub win32_system_backdrop: SystemBackdrop,

    #[dynamic(default = "default_win32_acrylic_accent_color")]
    pub win32_acrylic_accent_color: RgbaColor,

    /// Specifies the alpha value to use when rendering the background
    /// of the window.  The background is taken either from the
    /// window_background_image, or if there is none, the background
    /// color of the cell in the current position.
    /// The default is 1.0 which is 100% opaque.  Setting it to a number
    /// between 0.0 and 1.0 will allow for the screen behind the window
    /// to "shine through" to varying degrees.
    /// This only works on systems with a compositing window manager.
    /// Setting opacity to a value other than 1.0 can impact render
    /// performance.
    #[dynamic(default = "default_one_point_oh")]
    pub window_background_opacity: f32,

    /// inactive_pane_hue, inactive_pane_saturation and
    /// inactive_pane_brightness allow for transforming the color
    /// of inactive panes.
    /// The pane colors are converted to HSV values and multiplied
    /// by these values before being converted back to RGB to
    /// use in the display.
    ///
    /// The default is 1.0 which leaves the values as-is.
    ///
    /// Modifying the hue changes the hue of the color by rotating
    /// it through the color wheel.  It is not as useful as the
    /// other components, but is available "for free" as part of
    /// the colorspace conversion.
    ///
    /// Modifying the saturation can add or reduce the amount of
    /// "colorfulness".  Making the value smaller can make it appear
    /// more washed out.
    ///
    /// Modifying the brightness can be used to dim or increase
    /// the perceived amount of light.
    ///
    /// The range of these values is 0.0 and up; they are used to
    /// multiply the existing values, so the default of 1.0
    /// preserves the existing component, whilst 0.5 will reduce
    /// it by half, and 2.0 will double the value.
    ///
    /// A subtle dimming effect can be achieved by setting:
    /// inactive_pane_saturation = 0.9
    /// inactive_pane_brightness = 0.8
    #[dynamic(default = "default_inactive_pane_hsb")]
    pub inactive_pane_hsb: HsbTransform,

    #[dynamic(default = "default_one_point_oh")]
    pub text_background_opacity: f32,

    /// Specifies how often a blinking cursor transitions between visible
    /// and invisible, expressed in milliseconds.
    /// Setting this to 0 disables blinking.
    /// Note that this value is approximate due to the way that the system
    /// event loop schedulers manage timers; non-zero values will be at
    /// least the interval specified with some degree of slop.
    #[dynamic(default = "default_cursor_blink_rate")]
    pub cursor_blink_rate: u64,
    #[dynamic(default = "linear_ease")]
    pub cursor_blink_ease_in: EasingFunction,
    #[dynamic(default = "linear_ease")]
    pub cursor_blink_ease_out: EasingFunction,

    #[dynamic(default = "default_anim_fps")]
    pub animation_fps: u8,

    #[dynamic(default)]
    pub force_reverse_video_cursor: bool,

    /// Specifies the default cursor style.  various escape sequences
    /// can override the default style in different situations (eg:
    /// an editor can change it depending on the mode), but this value
    /// controls how the cursor appears when it is reset to default.
    /// The default is `SteadyBlock`.
    /// Acceptable values are `SteadyBlock`, `BlinkingBlock`,
    /// `SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
    /// and `BlinkingBar`.
    #[dynamic(default)]
    pub default_cursor_style: DefaultCursorStyle,

    /// Specifies how often blinking text (normal speed) transitions
    /// between visible and invisible, expressed in milliseconds.
    /// Setting this to 0 disables slow text blinking.  Note that this
    /// value is approximate due to the way that the system event loop
    /// schedulers manage timers; non-zero values will be at least the
    /// interval specified with some degree of slop.
    #[dynamic(default = "default_text_blink_rate")]
    pub text_blink_rate: u64,
    #[dynamic(default = "linear_ease")]
    pub text_blink_ease_in: EasingFunction,
    #[dynamic(default = "linear_ease")]
    pub text_blink_ease_out: EasingFunction,

    /// Specifies how often blinking text (rapid speed) transitions
    /// between visible and invisible, expressed in milliseconds.
    /// Setting this to 0 disables rapid text blinking.  Note that this
    /// value is approximate due to the way that the system event loop
    /// schedulers manage timers; non-zero values will be at least the
    /// interval specified with some degree of slop.
    #[dynamic(default = "default_text_blink_rate_rapid")]
    pub text_blink_rate_rapid: u64,
    #[dynamic(default = "linear_ease")]
    pub text_blink_rapid_ease_in: EasingFunction,
    #[dynamic(default = "linear_ease")]
    pub text_blink_rapid_ease_out: EasingFunction,

    /// If true, the mouse cursor will be hidden while typing.
    /// This option is true by default.
    #[dynamic(default = "default_true")]
    pub hide_mouse_cursor_when_typing: bool,

    /// If non-zero, specifies the period (in seconds) at which various
    /// statistics are logged.  Note that there is a minimum period of
    /// 10 seconds.
    #[dynamic(default)]
    pub periodic_stat_logging: u64,

    /// If false, do not scroll to the bottom of the terminal when
    /// you send input to the terminal.
    /// The default is to scroll to the bottom when you send input
    /// to the terminal.
    #[dynamic(default = "default_true")]
    pub scroll_to_bottom_on_input: bool,

    #[dynamic(default = "default_true")]
    pub use_ime: bool,
    #[dynamic(default)]
    pub xim_im_name: Option<String>,
    #[dynamic(default)]
    pub ime_preedit_rendering: ImePreeditRendering,

    #[dynamic(default)]
    pub notification_handling: NotificationHandling,

    #[dynamic(default = "default_true")]
    pub use_dead_keys: bool,

    #[dynamic(default)]
    pub launch_menu: Vec<SpawnCommand>,

    #[dynamic(default)]
    pub use_box_model_render: bool,

    /// When true, watch the config file and reload it automatically
    /// when it is detected as changing.
    #[dynamic(default = "default_true")]
    pub automatically_reload_config: bool,

    #[dynamic(default = "default_check_for_updates")]
    pub check_for_updates: bool,
    #[dynamic(
        default,
        deprecated = "this option no longer does anything and will be removed in a future release"
    )]
    pub show_update_window: bool,

    #[dynamic(default = "default_update_interval")]
    pub check_for_updates_interval_seconds: u64,

    /// When set to true, use the CSI-U encoding scheme as described
    /// in http://www.leonerd.org.uk/hacks/fixterms/
    /// This is off by default because @wez and @jsgf find the shift-space
    /// mapping annoying in vim :-p
    #[dynamic(default)]
    pub enable_csi_u_key_encoding: bool,

    #[dynamic(default)]
    pub window_close_confirmation: WindowCloseConfirmation,

    #[dynamic(default)]
    pub native_macos_fullscreen_mode: bool,

    #[dynamic(default = "default_word_boundary")]
    pub selection_word_boundary: String,

    #[dynamic(default = "default_enq_answerback")]
    pub enq_answerback: String,

    #[dynamic(default)]
    pub adjust_window_size_when_changing_font_size: Option<bool>,

    #[dynamic(default = "default_tiling_desktop_environments")]
    pub tiling_desktop_environments: Vec<String>,

    #[dynamic(default)]
    pub use_resize_increments: bool,

    #[dynamic(default = "default_alternate_buffer_wheel_scroll_speed")]
    pub alternate_buffer_wheel_scroll_speed: u8,

    #[dynamic(default = "default_status_update_interval")]
    pub status_update_interval: u64,

    #[dynamic(default)]
    pub experimental_pixel_positioning: bool,

    #[dynamic(default)]
    pub ignore_svg_fonts: bool,

    #[dynamic(default)]
    pub bidi_enabled: bool,

    #[dynamic(default)]
    pub bidi_direction: ParagraphDirectionHint,

    #[dynamic(default = "default_stateless_process_list")]
    pub skip_close_confirmation_for_processes_named: Vec<String>,

    #[dynamic(default = "default_true")]
    pub quit_when_all_windows_are_closed: bool,

    #[dynamic(default = "default_true")]
    pub warn_about_missing_glyphs: bool,

    #[dynamic(default)]
    pub sort_fallback_fonts_by_coverage: bool,

    #[dynamic(default)]
    pub search_font_dirs_for_fallback: bool,

    #[dynamic(default)]
    pub use_cap_height_to_scale_fallback_fonts: bool,

    #[dynamic(default)]
    pub swallow_mouse_click_on_pane_focus: bool,

    #[dynamic(default = "default_swallow_mouse_click_on_window_focus")]
    pub swallow_mouse_click_on_window_focus: bool,

    #[dynamic(default)]
    pub pane_focus_follows_mouse: bool,

    #[dynamic(default = "default_true")]
    pub unzoom_on_switch_pane: bool,

    #[dynamic(default = "default_max_fps")]
    pub max_fps: u8,

    #[dynamic(default = "default_shape_cache_size")]
    pub shape_cache_size: usize,
    #[dynamic(default = "default_line_state_cache_size")]
    pub line_state_cache_size: usize,
    #[dynamic(default = "default_line_quad_cache_size")]
    pub line_quad_cache_size: usize,
    #[dynamic(default = "default_line_to_ele_shape_cache_size")]
    pub line_to_ele_shape_cache_size: usize,
    #[dynamic(default = "default_glyph_cache_image_cache_size")]
    pub glyph_cache_image_cache_size: usize,

    #[dynamic(default)]
    pub visual_bell: VisualBell,

    #[dynamic(default)]
    pub audible_bell: AudibleBell,

    #[dynamic(default)]
    pub canonicalize_pasted_newlines: Option<NewlineCanon>,

    #[dynamic(default = "default_unicode_version")]
    pub unicode_version: u8,

    #[dynamic(default)]
    pub treat_east_asian_ambiguous_width_as_wide: bool,

    #[dynamic(default = "default_true")]
    pub allow_download_protocols: bool,

    #[dynamic(default = "default_true")]
    pub allow_win32_input_mode: bool,

    #[dynamic(default)]
    pub default_domain: Option<String>,

    #[dynamic(default)]
    pub default_mux_server_domain: Option<String>,

    #[dynamic(default)]
    pub default_workspace: Option<String>,

    #[dynamic(default)]
    pub xcursor_theme: Option<String>,

    #[dynamic(default)]
    pub xcursor_size: Option<u32>,

    #[dynamic(default)]
    pub key_map_preference: KeyMapPreference,

    #[dynamic(default)]
    pub quote_dropped_files: DroppedFileQuoting,

    #[dynamic(default)]
    pub ui_key_cap_rendering: UIKeyCapRendering,

    #[dynamic(default = "default_one")]
    pub palette_max_key_assigments_for_action: usize,

    #[dynamic(default = "default_ulimit_nofile")]
    pub ulimit_nofile: u64,

    #[dynamic(default = "default_ulimit_nproc")]
    pub ulimit_nproc: u64,
}
impl_lua_conversion_dynamic!(Config);

fn default_one() -> usize {
    1
}

fn default_ulimit_nofile() -> u64 {
    2048
}

fn default_ulimit_nproc() -> u64 {
    2048
}

impl Default for Config {
    fn default() -> Self {
        // Ask FromDynamic to provide the defaults based on the attributes
        // specified in the struct so that we don't have to repeat
        // the same thing in a different form down here
        Config::from_dynamic(
            &wezterm_dynamic::Value::Object(Default::default()),
            Default::default(),
        )
        .unwrap()
    }
}

impl Config {
    pub fn load() -> LoadedConfig {
        Self::load_with_overrides(&wezterm_dynamic::Value::default())
    }

    /// It is relatively expensive to parse all the ssh config files,
    /// so we defer producing the default list until someone explicitly
    /// asks for it
    pub fn ssh_domains(&self) -> Vec<SshDomain> {
        if let Some(domains) = &self.ssh_domains {
            domains.clone()
        } else {
            SshDomain::default_domains()
        }
    }

    pub fn wsl_domains(&self) -> Vec<WslDomain> {
        if let Some(domains) = &self.wsl_domains {
            domains.clone()
        } else {
            WslDomain::default_domains()
        }
    }

    pub fn update_ulimit(&self) -> anyhow::Result<()> {
        #[cfg(unix)]
        {
            use nix::sys::resource::{getrlimit, rlim_t, setrlimit, Resource};
            use std::convert::TryInto;

            let (no_file_soft, no_file_hard) = getrlimit(Resource::RLIMIT_NOFILE)?;

            let ulimit_nofile: rlim_t = self.ulimit_nofile.try_into().with_context(|| {
                format!(
                    "ulimit_nofile value {} is out of range for this system",
                    self.ulimit_nofile
                )
            })?;

            if no_file_soft < ulimit_nofile {
                setrlimit(
                    Resource::RLIMIT_NOFILE,
                    ulimit_nofile.min(no_file_hard),
                    no_file_hard,
                )
                .with_context(|| {
                    format!(
                        "raise RLIMIT_NOFILE from {no_file_soft} to ulimit_nofile {}",
                        ulimit_nofile
                    )
                })?;
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use nix::sys::resource::{getrlimit, rlim_t, setrlimit, Resource};
            use std::convert::TryInto;

            let (nproc_soft, nproc_hard) = getrlimit(Resource::RLIMIT_NPROC)?;

            let ulimit_nproc: rlim_t = self.ulimit_nproc.try_into().with_context(|| {
                format!(
                    "ulimit_nproc value {} is out of range for this system",
                    self.ulimit_nproc
                )
            })?;

            if nproc_soft < ulimit_nproc {
                setrlimit(
                    Resource::RLIMIT_NPROC,
                    ulimit_nproc.min(nproc_hard),
                    nproc_hard,
                )
                .with_context(|| {
                    format!(
                        "raise RLIMIT_NPROC from {nproc_soft} to ulimit_nproc {}",
                        ulimit_nproc
                    )
                })?;
            }
        }

        Ok(())
    }

    pub fn load_with_overrides(overrides: &wezterm_dynamic::Value) -> LoadedConfig {
        // Note that the directories crate has methods for locating project
        // specific config directories, but only returns one of them, not
        // multiple.  In addition, it spawns a lot of subprocesses,
        // so we do this bit "by-hand"

        let mut paths = vec![PathPossibility::optional(HOME_DIR.join(".wezterm.lua"))];
        for dir in CONFIG_DIRS.iter() {
            paths.push(PathPossibility::optional(dir.join("wezterm.lua")))
        }

        if cfg!(windows) {
            // On Windows, a common use case is to maintain a thumb drive
            // with a set of portable tools that don't need to be installed
            // to run on a target system.  In that scenario, the user would
            // like to run with the config from their thumbdrive because
            // either the target system won't have any config, or will have
            // the config of another user.
            // So we prioritize that here: if there is a config in the same
            // dir as the executable that will take precedence.
            if let Ok(exe_name) = std::env::current_exe() {
                if let Some(exe_dir) = exe_name.parent() {
                    paths.insert(0, PathPossibility::optional(exe_dir.join("wezterm.lua")));
                }
            }
        }
        if let Some(path) = std::env::var_os("WEZTERM_CONFIG_FILE") {
            log::trace!("Note: WEZTERM_CONFIG_FILE is set in the environment");
            paths.insert(0, PathPossibility::required(path.into()));
        }

        if let Some(path) = CONFIG_FILE_OVERRIDE.lock().unwrap().as_ref() {
            log::trace!("Note: config file override is set");
            paths.insert(0, PathPossibility::required(path.clone()));
        }

        for path_item in &paths {
            if CONFIG_SKIP.load(Ordering::Relaxed) {
                break;
            }

            match Self::try_load(path_item, overrides) {
                Err(err) => {
                    return LoadedConfig {
                        config: Err(err),
                        file_name: Some(path_item.path.clone()),
                        lua: None,
                        warnings: vec![],
                    }
                }
                Ok(None) => continue,
                Ok(Some(loaded)) => return loaded,
            }
        }

        // We didn't find (or were asked to skip) a wezterm.lua file, so
        // update the environment to make it simpler to understand this
        // state.
        std::env::remove_var("WEZTERM_CONFIG_FILE");
        std::env::remove_var("WEZTERM_CONFIG_DIR");

        match Self::try_default() {
            Err(err) => LoadedConfig {
                config: Err(err),
                file_name: None,
                lua: None,
                warnings: vec![],
            },
            Ok(cfg) => cfg,
        }
    }

    pub fn try_default() -> anyhow::Result<LoadedConfig> {
        let (config, warnings) =
            wezterm_dynamic::Error::capture_warnings(|| -> anyhow::Result<Config> {
                Ok(default_config_with_overrides_applied()?.compute_extra_defaults(None))
            });

        Ok(LoadedConfig {
            config: Ok(config?),
            file_name: None,
            lua: Some(make_lua_context(Path::new(""))?),
            warnings,
        })
    }

    fn try_load(
        path_item: &PathPossibility,
        overrides: &wezterm_dynamic::Value,
    ) -> anyhow::Result<Option<LoadedConfig>> {
        let p = path_item.path.as_path();
        log::trace!("consider config: {}", p.display());
        let mut file = match std::fs::File::open(p) {
            Ok(file) => file,
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound if !path_item.is_required => return Ok(None),
                _ => anyhow::bail!("Error opening {}: {}", p.display(), err),
            },
        };

        let mut s = String::new();
        file.read_to_string(&mut s)?;
        let lua = make_lua_context(p)?;

        let (config, warnings) =
            wezterm_dynamic::Error::capture_warnings(|| -> anyhow::Result<Config> {
                let cfg: Config;

                let config: mlua::Value = smol::block_on(
                    // Skip a potential BOM that Windows software may have placed in the
                    // file. Note that we can't catch this happening for files that are
                    // imported via the lua require function.
                    lua.load(s.trim_start_matches('\u{FEFF}'))
                        .set_name(p.to_string_lossy())
                        .eval_async(),
                )?;
                let config = Config::apply_overrides_to(&lua, config)?;
                let config = Config::apply_overrides_obj_to(&lua, config, overrides)?;
                cfg = Config::from_lua(config, &lua).with_context(|| {
                    format!(
                        "Error converting lua value returned by script {} to Config struct",
                        p.display()
                    )
                })?;
                cfg.check_consistency()?;

                // Compute but discard the key bindings here so that we raise any
                // problems earlier than we use them.
                let _ = cfg.key_bindings();

                std::env::set_var("WEZTERM_CONFIG_FILE", p);
                if let Some(dir) = p.parent() {
                    std::env::set_var("WEZTERM_CONFIG_DIR", dir);
                }
                Ok(cfg)
            });
        let cfg = config?;

        Ok(Some(LoadedConfig {
            config: Ok(cfg.compute_extra_defaults(Some(p))),
            file_name: Some(p.to_path_buf()),
            lua: Some(lua),
            warnings,
        }))
    }

    pub(crate) fn apply_overrides_obj_to<'l>(
        lua: &'l mlua::Lua,
        mut config: mlua::Value<'l>,
        overrides: &wezterm_dynamic::Value,
    ) -> anyhow::Result<mlua::Value<'l>> {
        // config may be a table, or it may be a config builder.
        // We'll leave it up to lua to call the appropriate
        // index function as managing that from Rust is a PITA.
        let setter: mlua::Function = lua
            .load(
                r#"
                    return function(config, key, value)
                        config[key] = value;
                        return config;
                    end
                    "#,
            )
            .eval()?;

        match overrides {
            wezterm_dynamic::Value::Object(obj) => {
                for (key, value) in obj {
                    let key = luahelper::dynamic_to_lua_value(lua, key.clone())?;
                    let value = luahelper::dynamic_to_lua_value(lua, value.clone())?;
                    config = setter.call((config, key, value))?;
                }
                Ok(config)
            }
            _ => Ok(config),
        }
    }

    pub(crate) fn apply_overrides_to<'l>(
        lua: &'l mlua::Lua,
        mut config: mlua::Value<'l>,
    ) -> anyhow::Result<mlua::Value<'l>> {
        let overrides = CONFIG_OVERRIDES.lock().unwrap();
        for (key, value) in &*overrides {
            if value == "nil" {
                // Literal nil as the value is the same as not specifying the value.
                // We special case this here as we want to explicitly check for
                // the value evaluating as nil, as can happen in the case where the
                // user specifies something like: `--config term=xterm`.
                // The RHS references a global that doesn't exist and evaluates as
                // nil. We want to raise this as an error.
                continue;
            }
            let literal = value.escape_debug();
            let code = format!(
                r#"
                local wezterm = require 'wezterm';
                local value = {value};
                if value == nil then
                    error("{literal} evaluated as nil. Check for missing quotes or other syntax issues")
                end
                config.{key} = value;
                return config;
                "#,
            );
            let chunk = lua.load(&code);
            let chunk = chunk.set_name(format!("--config {}={}", key, value));
            lua.globals().set("config", config.clone())?;
            log::debug!("Apply {}={} to config", key, value);
            config = chunk.eval()?;
        }
        Ok(config)
    }

    /// Check for logical conflicts in the config
    pub fn check_consistency(&self) -> anyhow::Result<()> {
        self.check_domain_consistency()?;
        Ok(())
    }

    fn check_domain_consistency(&self) -> anyhow::Result<()> {
        let mut domains = HashMap::new();

        let mut check_domain = |name: &str, kind: &str| {
            if let Some(exists) = domains.get(name) {
                anyhow::bail!(
                    "{kind} with name \"{name}\" conflicts with \
                     another existing {exists} with the same name"
                );
            }
            domains.insert(name.to_string(), kind.to_string());
            Ok(())
        };

        for d in &self.unix_domains {
            check_domain(&d.name, "unix domain")?;
        }
        if let Some(domains) = &self.ssh_domains {
            for d in domains {
                check_domain(&d.name, "ssh domain")?;
            }
        }
        for d in &self.exec_domains {
            check_domain(&d.name, "exec domain")?;
        }
        if let Some(domains) = &self.wsl_domains {
            for d in domains {
                check_domain(&d.name, "wsl domain")?;
            }
        }
        for d in &self.tls_clients {
            check_domain(&d.name, "tls domain")?;
        }
        Ok(())
    }

    pub fn default_config() -> Self {
        Self::default().compute_extra_defaults(None)
    }

    pub fn key_bindings(&self) -> KeyTables {
        let mut tables = KeyTables::default();

        for k in &self.keys {
            let (key, mods) = k
                .key
                .key
                .resolve(self.key_map_preference)
                .normalize_shift(k.key.mods);
            tables.default.insert(
                (key, mods),
                KeyTableEntry {
                    action: k.action.clone(),
                },
            );
        }

        for (name, keys) in &self.key_tables {
            let mut table = KeyTable::default();
            for k in keys {
                let (key, mods) = k
                    .key
                    .key
                    .resolve(self.key_map_preference)
                    .normalize_shift(k.key.mods);
                table.insert(
                    (key, mods),
                    KeyTableEntry {
                        action: k.action.clone(),
                    },
                );
            }
            tables.by_name.insert(name.to_string(), table);
        }

        tables
    }

    pub fn mouse_bindings(
        &self,
    ) -> HashMap<(MouseEventTrigger, MouseEventTriggerMods), KeyAssignment> {
        let mut map = HashMap::new();

        for m in &self.mouse_bindings {
            map.insert((m.event.clone(), m.mods), m.action.clone());
        }

        map
    }

    /// In some cases we need to compute expanded values based
    /// on those provided by the user.  This is where we do that.
    pub fn compute_extra_defaults(&self, config_path: Option<&Path>) -> Self {
        let mut cfg = self.clone();

        // Convert any relative font dirs to their config file relative locations
        if let Some(config_dir) = config_path.as_ref().and_then(|p| p.parent()) {
            for font_dir in &mut cfg.font_dirs {
                if !font_dir.is_absolute() {
                    let dir = config_dir.join(&font_dir);
                    *font_dir = dir;
                }
            }

            if let Some(path) = &self.window_background_image {
                if !path.is_absolute() {
                    cfg.window_background_image.replace(config_dir.join(path));
                }
            }
        }

        // Add some reasonable default font rules
        let reduced = self.font.reduce_first_font_to_family();

        let italic = reduced.make_italic();

        let bold = reduced.make_bold();
        let bold_italic = bold.make_italic();

        let half_bright = reduced.make_half_bright();
        let half_bright_italic = half_bright.make_italic();

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Half),
            font: half_bright_italic,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(false),
            intensity: Some(wezterm_term::Intensity::Half),
            font: half_bright,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(false),
            intensity: Some(wezterm_term::Intensity::Bold),
            font: bold,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Bold),
            font: bold_italic,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Normal),
            font: italic,
            ..Default::default()
        });

        // Load any additional color schemes into the color_schemes map
        cfg.load_color_schemes(&cfg.compute_color_scheme_dirs())
            .ok();

        if let Some(scheme) = cfg.color_scheme.as_ref() {
            match cfg.resolve_color_scheme() {
                None => {
                    log::error!(
                        "Your configuration specifies color_scheme=\"{}\" \
                        but that scheme was not found",
                        scheme
                    );
                }
                Some(p) => {
                    cfg.resolved_palette = p.clone();
                }
            }
        }

        if let Some(colors) = &cfg.colors {
            cfg.resolved_palette = cfg.resolved_palette.overlay_with(colors);
        }

        if let Some(bg) = BackgroundLayer::with_legacy(self) {
            cfg.background.insert(0, bg);
        }

        cfg
    }

    fn compute_color_scheme_dirs(&self) -> Vec<PathBuf> {
        let mut paths = self.color_scheme_dirs.clone();
        for dir in CONFIG_DIRS.iter() {
            paths.push(dir.join("colors"));
        }
        if cfg!(windows) {
            // See commentary re: portable tools above!
            if let Ok(exe_name) = std::env::current_exe() {
                if let Some(exe_dir) = exe_name.parent() {
                    paths.insert(0, exe_dir.join("colors"));
                }
            }
        }
        paths
    }

    fn load_color_schemes(&mut self, paths: &[PathBuf]) -> anyhow::Result<()> {
        fn extract_scheme_name(name: &str) -> Option<&str> {
            if name.ends_with(".toml") {
                let len = name.len();
                Some(&name[..len - 5])
            } else {
                None
            }
        }

        fn load_scheme(path: &Path) -> anyhow::Result<ColorSchemeFile> {
            let s = std::fs::read_to_string(path)?;
            ColorSchemeFile::from_toml_str(&s).context("parsing TOML")
        }

        for colors_dir in paths {
            if let Ok(dir) = std::fs::read_dir(colors_dir) {
                for entry in dir {
                    if let Ok(entry) = entry {
                        if let Some(name) = entry.file_name().to_str() {
                            if let Some(scheme_name) = extract_scheme_name(name) {
                                if self.color_schemes.contains_key(scheme_name) {
                                    // This scheme has already been defined
                                    continue;
                                }

                                let path = entry.path();
                                match load_scheme(&path) {
                                    Ok(scheme) => {
                                        let name = scheme
                                            .metadata
                                            .name
                                            .unwrap_or_else(|| scheme_name.to_string());
                                        log::trace!(
                                            "Loaded color scheme `{}` from {}",
                                            name,
                                            path.display()
                                        );
                                        self.color_schemes.insert(name, scheme.colors);
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "Color scheme in `{}` failed to load: {:#}",
                                            path.display(),
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn resolve_color_scheme(&self) -> Option<&Palette> {
        let scheme_name = self.color_scheme.as_ref()?;

        if let Some(palette) = self.color_schemes.get(scheme_name) {
            Some(palette)
        } else {
            crate::COLOR_SCHEMES.get(scheme_name)
        }
    }

    pub fn initial_size(&self, dpi: u32, cell_pixel_dims: Option<(usize, usize)>) -> TerminalSize {
        // If we aren't passed the actual values, guess at a plausible
        // default set of pixel dimensions.
        // This is based on "typical" 10 point font at "normal"
        // pixel density.
        // This will get filled in by the gui layer, but there is
        // an edge case where we emit an iTerm image escape in
        // the software update banner through the mux layer before
        // the GUI has had a chance to update the pixel dimensions
        // when running under X11.
        // This is a bit gross.
        let (cell_pixel_width, cell_pixel_height) = cell_pixel_dims.unwrap_or((8, 16));

        TerminalSize {
            rows: self.initial_rows as usize,
            cols: self.initial_cols as usize,
            pixel_width: cell_pixel_width * self.initial_cols as usize,
            pixel_height: cell_pixel_height * self.initial_rows as usize,
            dpi,
        }
    }

    pub fn build_prog(
        &self,
        prog: Option<Vec<&OsStr>>,
        default_prog: Option<&Vec<String>>,
        default_cwd: Option<&PathBuf>,
    ) -> anyhow::Result<CommandBuilder> {
        let mut cmd = match prog {
            Some(args) => {
                let mut args = args.iter();
                let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                cmd.args(args);
                cmd
            }
            None => {
                if let Some(prog) = default_prog {
                    let mut args = prog.iter();
                    let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                    cmd.args(args);
                    cmd
                } else {
                    CommandBuilder::new_default_prog()
                }
            }
        };

        self.apply_cmd_defaults(&mut cmd, default_cwd);

        Ok(cmd)
    }

    pub fn apply_cmd_defaults(&self, cmd: &mut CommandBuilder, default_cwd: Option<&PathBuf>) {
        // Apply `default_cwd` only if `cwd` is not already set, allows `--cwd`
        // option to take precedence
        if let (None, Some(cwd)) = (cmd.get_cwd(), default_cwd) {
            cmd.cwd(cwd);
        }

        // Augment WSLENV so that TERM related environment propagates
        // across the win32/wsl boundary
        let mut wsl_env = std::env::var("WSLENV").ok();

        // If we are running as an appimage, we will have "$APPIMAGE"
        // and "$APPDIR" set in the wezterm process. These will be
        // propagated to the child processes. Since some apps (including
        // wezterm) use these variables to detect if they are running in
        // an appimage, those child processes will be misconfigured.
        // Ensure that they are unset.
        // https://docs.appimage.org/packaging-guide/environment-variables.html#id2
        cmd.env_remove("APPIMAGE");
        cmd.env_remove("APPDIR");
        cmd.env_remove("OWD");

        for (k, v) in &self.set_environment_variables {
            if k == "WSLENV" {
                wsl_env.replace(v.clone());
            } else {
                cmd.env(k, v);
            }
        }

        if wsl_env.is_some() || cfg!(windows) || crate::version::running_under_wsl() {
            let mut wsl_env = wsl_env.unwrap_or_default();
            if !wsl_env.is_empty() {
                wsl_env.push(':');
            }
            wsl_env.push_str("TERM:COLORTERM:TERM_PROGRAM:TERM_PROGRAM_VERSION");
            cmd.env("WSLENV", wsl_env);
        }

        #[cfg(unix)]
        cmd.umask(umask::UmaskSaver::saved_umask());
        cmd.env("TERM", &self.term);
        cmd.env("COLORTERM", "truecolor");
        // TERM_PROGRAM and TERM_PROGRAM_VERSION are an emerging
        // de-facto standard for identifying the terminal.
        cmd.env("TERM_PROGRAM", "WezTerm");
        cmd.env("TERM_PROGRAM_VERSION", crate::wezterm_version());
    }
}

fn default_check_for_updates() -> bool {
    cfg!(not(feature = "distro-defaults"))
}

fn default_pane_select_fg_color() -> RgbaColor {
    SrgbaTuple(0.75, 0.75, 0.75, 1.0).into()
}

fn default_pane_select_bg_color() -> RgbaColor {
    SrgbaTuple(0., 0., 0., 0.5).into()
}

fn default_pane_select_font_size() -> f64 {
    36.0
}

fn default_integrated_title_buttons() -> Vec<IntegratedTitleButton> {
    use IntegratedTitleButton::*;
    vec![Hide, Maximize, Close]
}

fn default_char_select_font_size() -> f64 {
    18.0
}

fn default_char_select_fg_color() -> RgbaColor {
    SrgbaTuple(0.75, 0.75, 0.75, 1.0).into()
}

fn default_char_select_bg_color() -> RgbaColor {
    (0x33, 0x33, 0x33).into()
}

fn default_command_palette_font_size() -> f64 {
    14.0
}

fn default_command_palette_fg_color() -> RgbaColor {
    SrgbaTuple(0.75, 0.75, 0.75, 1.0).into()
}

fn default_command_palette_bg_color() -> RgbaColor {
    (0x33, 0x33, 0x33).into()
}

fn default_swallow_mouse_click_on_window_focus() -> bool {
    cfg!(target_os = "macos")
}

fn default_mux_output_parser_coalesce_delay_ms() -> u64 {
    3
}

fn default_mux_output_parser_buffer_size() -> usize {
    128 * 1024
}

fn default_ratelimit_line_prefetches_per_second() -> u32 {
    50
}

fn default_cursor_blink_rate() -> u64 {
    800
}

fn default_text_blink_rate() -> u64 {
    500
}

fn default_text_blink_rate_rapid() -> u64 {
    250
}

fn default_swap_backspace_and_delete() -> bool {
    // cfg!(target_os = "macos")
    // See: https://github.com/wez/wezterm/issues/88
    false
}

fn default_scrollback_lines() -> usize {
    3500
}

const MAX_SCROLLBACK_LINES: usize = 999_999_999;
fn validate_scrollback_lines(value: &usize) -> Result<(), String> {
    if *value > MAX_SCROLLBACK_LINES {
        return Err(format!(
            "Illegal value {value} for scrollback_lines; it must be <= {MAX_SCROLLBACK_LINES}!"
        ));
    }
    Ok(())
}

fn default_initial_rows() -> u16 {
    24
}

fn default_initial_cols() -> u16 {
    80
}

pub fn default_hyperlink_rules() -> Vec<hyperlink::Rule> {
    vec![
        // First handle URLs wrapped with punctuation (i.e. brackets)
        // e.g. [http://foo] (http://foo) <http://foo>
        hyperlink::Rule::with_highlight(r"\((\w+://\S+)\)", "$1", 1).unwrap(),
        hyperlink::Rule::with_highlight(r"\[(\w+://\S+)\]", "$1", 1).unwrap(),
        hyperlink::Rule::with_highlight(r"<(\w+://\S+)>", "$1", 1).unwrap(),
        // Then handle URLs not wrapped in brackets
        // and include terminating ), / or - characters, if any
        hyperlink::Rule::new(r"\b\w+://\S+[)/a-zA-Z0-9-]+", "$0").unwrap(),
        // implicit mailto link
        hyperlink::Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
    ]
}

fn default_harfbuzz_features() -> Vec<String> {
    ["kern", "liga", "clig"]
        .iter()
        .map(|&s| s.to_string())
        .collect()
}

fn default_term() -> String {
    "xterm-256color".into()
}

fn default_font_size() -> f64 {
    12.0
}

pub(crate) fn compute_cache_dir() -> anyhow::Result<PathBuf> {
    if let Some(runtime) = dirs_next::cache_dir() {
        return Ok(runtime.join("wezterm"));
    }

    Ok(crate::HOME_DIR.join(".local/share/wezterm"))
}

pub(crate) fn compute_data_dir() -> anyhow::Result<PathBuf> {
    if let Some(runtime) = dirs_next::data_dir() {
        return Ok(runtime.join("wezterm"));
    }

    Ok(crate::HOME_DIR.join(".local/share/wezterm"))
}

pub(crate) fn compute_runtime_dir() -> anyhow::Result<PathBuf> {
    if let Some(runtime) = dirs_next::runtime_dir() {
        return Ok(runtime.join("wezterm"));
    }

    Ok(crate::HOME_DIR.join(".local/share/wezterm"))
}

pub fn pki_dir() -> anyhow::Result<PathBuf> {
    compute_runtime_dir().map(|d| d.join("pki"))
}

pub fn default_read_timeout() -> Duration {
    Duration::from_secs(60)
}

pub fn default_write_timeout() -> Duration {
    Duration::from_secs(60)
}

pub fn default_local_echo_threshold_ms() -> Option<u64> {
    Some(100)
}

fn default_bypass_mouse_reporting_modifiers() -> Modifiers {
    Modifiers::SHIFT
}

fn default_gui_startup_args() -> Vec<String> {
    vec!["start".to_string()]
}

// Coupled with term/src/config.rs:TerminalConfiguration::unicode_version
fn default_unicode_version() -> u8 {
    9
}

fn default_mux_env_remove() -> Vec<String> {
    vec![
        "SSH_AUTH_SOCK".to_string(),
        "SSH_CLIENT".to_string(),
        "SSH_CONNECTION".to_string(),
    ]
}

fn default_anim_fps() -> u8 {
    10
}

fn default_max_fps() -> u8 {
    60
}

fn default_tiling_desktop_environments() -> Vec<String> {
    [
        "X11 LG3D",
        "X11 Qtile",
        "X11 awesome",
        "X11 bspwm",
        "X11 dwm",
        "X11 i3",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_stateless_process_list() -> Vec<String> {
    [
        "bash",
        "sh",
        "zsh",
        "fish",
        "tmux",
        "nu",
        "nu.exe",
        "cmd.exe",
        "pwsh.exe",
        "powershell.exe",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_status_update_interval() -> u64 {
    1_000
}

fn default_alternate_buffer_wheel_scroll_speed() -> u8 {
    3
}

fn default_alphabet() -> String {
    "asdfqwerzxcvjklmiuopghtybn".to_string()
}

fn default_word_boundary() -> String {
    " \t\n{[}]()\"'`".to_string()
}

fn default_enq_answerback() -> String {
    "".to_string()
}

fn default_tab_max_width() -> usize {
    16
}

fn default_update_interval() -> u64 {
    86400
}

fn default_prefer_egl() -> bool {
    !cfg!(windows)
}

fn default_clean_exits() -> Vec<u32> {
    vec![]
}

fn default_inactive_pane_hsb() -> HsbTransform {
    HsbTransform {
        brightness: 0.8,
        saturation: 0.9,
        hue: 1.0,
    }
}

#[derive(FromDynamic, ToDynamic, Clone, Copy, Debug, Default)]
pub enum DefaultCursorStyle {
    BlinkingBlock,
    #[default]
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}

impl DefaultCursorStyle {
    pub fn effective_shape(self, shape: CursorShape) -> CursorShape {
        match shape {
            CursorShape::Default => match self {
                Self::BlinkingBlock => CursorShape::BlinkingBlock,
                Self::SteadyBlock => CursorShape::SteadyBlock,
                Self::BlinkingUnderline => CursorShape::BlinkingUnderline,
                Self::SteadyUnderline => CursorShape::SteadyUnderline,
                Self::BlinkingBar => CursorShape::BlinkingBar,
                Self::SteadyBar => CursorShape::SteadyBar,
            },
            _ => shape,
        }
    }
}

const fn linear_ease() -> EasingFunction {
    EasingFunction::Linear
}

const fn default_one_cell() -> Dimension {
    Dimension::Cells(1.)
}

const fn default_half_cell() -> Dimension {
    Dimension::Cells(0.5)
}

#[derive(FromDynamic, ToDynamic, Clone, Copy, Debug)]
pub struct WindowPadding {
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_one_cell")]
    pub left: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_half_cell")]
    pub top: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_one_cell")]
    pub right: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_half_cell")]
    pub bottom: Dimension,
}

impl Default for WindowPadding {
    fn default() -> Self {
        Self {
            left: default_one_cell(),
            right: default_one_cell(),
            top: default_half_cell(),
            bottom: default_half_cell(),
        }
    }
}

#[derive(FromDynamic, ToDynamic, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineCanon {
    // FIXME: also allow deserialziing from bool
    None,
    LineFeed,
    CarriageReturn,
    CarriageReturnAndLineFeed,
}

#[derive(FromDynamic, ToDynamic, Clone, Copy, Debug, Default)]
pub enum WindowCloseConfirmation {
    #[default]
    AlwaysPrompt,
    NeverPrompt,
    // TODO: something smart where we see whether the
    // running programs are stateful
}

struct PathPossibility {
    path: PathBuf,
    is_required: bool,
}
impl PathPossibility {
    pub fn required(path: PathBuf) -> PathPossibility {
        PathPossibility {
            path,
            is_required: true,
        }
    }
    pub fn optional(path: PathBuf) -> PathPossibility {
        PathPossibility {
            path,
            is_required: false,
        }
    }
}

/// Behavior when the program spawned by wezterm terminates
#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExitBehavior {
    /// Close the associated pane
    #[default]
    Close,
    /// Close the associated pane if the process was successful
    CloseOnCleanExit,
    /// Hold the pane until it is explicitly closed
    Hold,
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExitBehaviorMessaging {
    #[default]
    Verbose,
    Brief,
    Terse,
    None,
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq)]
pub enum DroppedFileQuoting {
    /// No quoting is performed, the file name is passed through as-is
    None,
    /// Backslash escape only spaces, leaving all other characters as-is
    SpacesOnly,
    /// Use POSIX style shell word escaping
    Posix,
    /// Use Windows style shell word escaping
    Windows,
    /// Always double quote the file name
    WindowsAlwaysQuoted,
}

impl Default for DroppedFileQuoting {
    fn default() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::SpacesOnly
        }
    }
}

impl DroppedFileQuoting {
    pub fn escape(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            Self::SpacesOnly => s.replace(" ", "\\ "),
            // https://docs.rs/shlex/latest/shlex/fn.quote.html
            Self::Posix => shlex::try_quote(s)
                .unwrap_or_else(|_| "".into())
                .into_owned(),
            Self::Windows => {
                let chars_need_quoting = [' ', '\t', '\n', '\x0b', '\"'];
                if s.chars().any(|c| chars_need_quoting.contains(&c)) {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            }
            Self::WindowsAlwaysQuoted => format!("\"{}\"", s),
        }
    }
}

fn default_glyph_cache_image_cache_size() -> usize {
    256
}

fn default_shape_cache_size() -> usize {
    1024
}

fn default_line_state_cache_size() -> usize {
    1024
}

fn default_line_quad_cache_size() -> usize {
    1024
}

fn default_line_to_ele_shape_cache_size() -> usize {
    1024
}

#[derive(Debug, ToDynamic, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoldBrightening {
    /// Bold doesn't influence palette selection
    No,
    /// Bold Shifts palette from 0-7 to 8-15 and preserves bold font
    #[default]
    BrightAndBold,
    /// Bold Shifts palette from 0-7 to 8-15 and removes bold intensity
    BrightOnly,
}

impl FromDynamic for BoldBrightening {
    fn from_dynamic(
        value: &wezterm_dynamic::Value,
        options: wezterm_dynamic::FromDynamicOptions,
    ) -> Result<Self, wezterm_dynamic::Error> {
        match String::from_dynamic(value, options) {
            Ok(s) => match s.as_str() {
                "No" => Ok(Self::No),
                "BrightAndBold" => Ok(Self::BrightAndBold),
                "BrightOnly" => Ok(Self::BrightOnly),
                s => Err(wezterm_dynamic::Error::Message(format!(
                    "`{s}` is not valid, use one of `No`, `BrightAndBold` or `BrightOnly`"
                ))),
            },
            Err(err) => match bool::from_dynamic(value, options) {
                Ok(true) => Ok(Self::BrightAndBold),
                Ok(false) => Ok(Self::No),
                Err(_) => Err(err),
            },
        }
    }
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImePreeditRendering {
    /// IME preedit is rendered by WezTerm itself
    #[default]
    Builtin,
    /// IME preedit is rendered by system
    System,
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationHandling {
    #[default]
    AlwaysShow,
    NeverShow,
    SuppressFromFocusedPane,
    SuppressFromFocusedTab,
    SuppressFromFocusedWindow,
}

fn validate_row_or_col(value: &u16) -> Result<(), String> {
    if *value < 1 {
        Err("initial_cols and initial_rows must be non-zero".to_string())
    } else {
        Ok(())
    }
}

fn validate_line_height(value: &f64) -> Result<(), String> {
    if *value <= 0.0 {
        Err(format!(
            "Illegal value {value} for line_height; it must be positive and greater than zero!"
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_domain_name(name: &str) -> Result<(), String> {
    if name == "local" {
        Err(format!(
            "\"{name}\" is a built-in domain and cannot be redefined"
        ))
    } else if name == "" {
        Err("the empty string is an invalid domain name".to_string())
    } else {
        Ok(())
    }
}

/// <https://github.com/wez/wezterm/pull/2435>
/// <https://github.com/wez/wezterm/issues/2771>
/// <https://github.com/wez/wezterm/issues/2630>
fn default_macos_forward_mods() -> Modifiers {
    Modifiers::SHIFT
}

fn default_colr_rasterizer() -> FontRasterizerSelection {
    FontRasterizerSelection::Harfbuzz
}
