use anyhow::{anyhow, Context};
use clap::builder::ValueParser;
use clap::{Parser, ValueEnum, ValueHint};
use clap_complete::{generate as generate_completion, shells, Generator as CompletionGenerator};
use config::{wezterm_version, ConfigHandle};
use mux::Mux;
use std::ffi::OsString;
use std::io::Read;
use termwiz::caps::Capabilities;
use termwiz::escape::esc::{Esc, EscCode};
use termwiz::escape::OneBased;
use termwiz::input::{InputEvent, KeyCode, KeyEvent};
use termwiz::surface::change::Change;
use termwiz::surface::Position;
use termwiz::terminal::{ScreenSize, Terminal};
use umask::UmaskSaver;
use wezterm_gui_subcommands::*;

mod asciicast;
mod cli;

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";

#[derive(Debug, Parser)]
#[command(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    version = wezterm_version()
)]
pub struct Opt {
    /// Skip loading wezterm.lua
    #[arg(long, short = 'n')]
    skip_config: bool,

    /// Specify the configuration file to use, overrides the normal
    /// configuration file resolution
    #[arg(
        long,
        value_parser,
        conflicts_with = "skip_config",
        value_hint=ValueHint::FilePath
    )]
    config_file: Option<OsString>,

    /// Override specific configuration values
    #[arg(
        long = "config",
        name = "name=value",
        value_parser=ValueParser::new(name_equals_value),
        number_of_values = 1)]
    config_override: Vec<(String, String)>,

    #[command(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, Clone, ValueEnum)]
enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Fig,
}

impl CompletionGenerator for Shell {
    fn file_name(&self, name: &str) -> String {
        match self {
            Shell::Bash => shells::Bash.file_name(name),
            Shell::Elvish => shells::Elvish.file_name(name),
            Shell::Fish => shells::Fish.file_name(name),
            Shell::PowerShell => shells::PowerShell.file_name(name),
            Shell::Zsh => shells::Zsh.file_name(name),
            Shell::Fig => clap_complete_fig::Fig.file_name(name),
        }
    }

    fn generate(&self, cmd: &clap::Command, buf: &mut dyn std::io::Write) {
        match self {
            Shell::Bash => shells::Bash.generate(cmd, buf),
            Shell::Elvish => shells::Elvish.generate(cmd, buf),
            Shell::Fish => shells::Fish.generate(cmd, buf),
            Shell::PowerShell => shells::PowerShell.generate(cmd, buf),
            Shell::Zsh => shells::Zsh.generate(cmd, buf),
            Shell::Fig => clap_complete_fig::Fig.generate(cmd, buf),
        }
    }
}

#[derive(Debug, Parser, Clone)]
enum SubCommand {
    #[command(
        name = "start",
        about = "Start the GUI, optionally running an alternative program"
    )]
    Start(StartCommand),

    #[command(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[command(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[command(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[command(name = "ls-fonts", about = "Display information about fonts")]
    LsFonts(LsFontsCommand),

    #[command(name = "show-keys", about = "Show key assignments")]
    ShowKeys(ShowKeysCommand),

    #[command(name = "cli", about = "Interact with experimental mux server")]
    Cli(cli::CliCommand),

    #[command(name = "imgcat", about = "Output an image to the terminal")]
    ImageCat(ImgCatCommand),

    #[command(
        name = "set-working-directory",
        about = "Advise the terminal of the current working directory by \
                 emitting an OSC 7 escape sequence"
    )]
    SetCwd(SetCwdCommand),

    #[command(name = "record", about = "Record a terminal session as an asciicast")]
    Record(asciicast::RecordCommand),

    #[command(name = "replay", about = "Replay an asciicast terminal session")]
    Replay(asciicast::PlayCommand),

    /// Generate shell completion information
    #[command(name = "shell-completion")]
    ShellCompletion {
        /// Which shell to generate for
        #[arg(long, value_parser)]
        shell: Shell,
    },
}

use termwiz::escape::osc::{
    ITermDimension, ITermFileData, ITermProprietary, OperatingSystemCommand,
};

#[derive(Debug, Parser, Clone)]
struct ImgCatCommand {
    /// Specify the display width; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal width.
    #[arg(long = "width")]
    width: Option<ITermDimension>,
    /// Specify the display height; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal height.
    #[arg(long = "height")]
    height: Option<ITermDimension>,
    /// Do not respect the aspect ratio.  The default is to respect the aspect
    /// ratio
    #[arg(long = "no-preserve-aspect-ratio")]
    no_preserve_aspect_ratio: bool,

    /// Set the cursor position prior to displaying the image.
    /// The default is to use the current cursor position.
    /// Coordinates are expressed in cells with 0,0 being the top left
    /// cell position.
    #[arg(long, value_parser=ValueParser::new(x_comma_y))]
    position: Option<ImagePosition>,

    /// Do not move the cursor after displaying the image.
    /// Note that when used like this from the shell, there is a very
    /// high chance that shell prompt will overwrite the image;
    /// you may wish to also use `--hold` in that case.
    #[arg(long)]
    no_move_cursor: bool,

    /// Wait for enter to be pressed after displaying the image
    #[arg(long)]
    hold: bool,

    /// How to manage passing the escape through to tmux
    #[arg(long, value_parser)]
    tmux_passthru: Option<TmuxPassthru>,

    /// The name of the image file to be displayed.
    /// If omitted, will attempt to read it from stdin.
    #[arg(value_parser, value_hint=ValueHint::FilePath)]
    file_name: Option<OsString>,
}

#[derive(Clone, Copy, Debug)]
struct ImagePosition {
    x: u32,
    y: u32,
}

fn x_comma_y(arg: &str) -> Result<ImagePosition, String> {
    if let Some(eq) = arg.find(',') {
        let (left, right) = arg.split_at(eq);
        let x = left.parse().map_err(|err| {
            format!("Expected x,y to be integer values, got {arg}. '{left}': {err:#}")
        })?;
        let y = right[1..].parse().map_err(|err| {
            format!("Expected x,y to be integer values, got {arg}. '{right}': {err:#}")
        })?;
        Ok(ImagePosition { x, y })
    } else {
        Err(format!("Expected name=value, but got {}", arg))
    }
}

#[derive(Debug)]
pub(crate) struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub _format: image::ImageFormat,
}

impl ImgCatCommand {
    fn compute_image_cell_dimensions(
        &self,
        info: ImageInfo,
        term_size: ScreenSize,
    ) -> (usize, usize) {
        let physical_cols = term_size.cols;
        let physical_rows = term_size.rows;
        let cell_pixel_width = term_size.xpixel;
        let cell_pixel_height = term_size.ypixel;
        let pixel_width = cell_pixel_width * physical_cols;
        let pixel_height = cell_pixel_height * physical_rows;

        let width = self
            .width
            .unwrap_or_default()
            .to_pixels(cell_pixel_width, physical_cols);
        let height = self
            .height
            .unwrap_or_default()
            .to_pixels(cell_pixel_height, physical_rows);

        // Compute any Automatic dimensions
        let aspect = info.width as f32 / info.height as f32;

        let (width, height) = match (width, height) {
            (None, None) => {
                // Take the image's native size
                let width = info.width as usize;
                let height = info.height as usize;
                // but ensure that it fits
                if width as usize > pixel_width || height as usize > pixel_height {
                    let width = width as f32;
                    let height = height as f32;
                    let mut candidates = vec![];

                    let x_scale = pixel_width as f32 / width;
                    if height * x_scale <= pixel_height as f32 {
                        candidates.push((pixel_width, (height * x_scale) as usize));
                    }
                    let y_scale = pixel_height as f32 / height;
                    if width * y_scale <= pixel_width as f32 {
                        candidates.push(((width * y_scale) as usize, pixel_height));
                    }

                    candidates.sort_by(|a, b| (a.0 * a.1).cmp(&(b.0 * b.1)));

                    candidates.pop().unwrap()
                } else {
                    (width, height)
                }
            }
            (Some(w), None) => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (None, Some(h)) => {
                let w = h as f32 * aspect;
                (w as usize, h)
            }
            (Some(w), Some(_)) if !self.no_preserve_aspect_ratio => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (Some(w), Some(h)) => (w, h),
        };

        // And convert to cells
        (width / cell_pixel_width, height / cell_pixel_height)
    }

    fn image_dimensions(data: &[u8]) -> anyhow::Result<ImageInfo> {
        let reader = image::io::Reader::new(std::io::Cursor::new(data)).with_guessed_format()?;
        let format = reader
            .format()
            .ok_or_else(|| anyhow::anyhow!("unknown format!?"))?;
        let (width, height) = reader.into_dimensions()?;
        Ok(ImageInfo {
            width,
            height,
            _format: format,
        })
    }

    fn run(&self) -> anyhow::Result<()> {
        let mut data = Vec::new();
        if let Some(file_name) = self.file_name.as_ref() {
            let mut f = std::fs::File::open(file_name)
                .with_context(|| anyhow!("reading image file: {:?}", file_name))?;
            f.read_to_end(&mut data)?;
        } else {
            let mut stdin = std::io::stdin();
            stdin.read_to_end(&mut data)?;
        }

        // TODO: ideally we'd provide some kind of ProbeCapabilities type
        // that can be returned from Terminal that will encode this sort
        // of thing so that we can use xtversion to know for sure.
        let is_tmux = TmuxPassthru::is_tmux();

        // TODO: ideally we'd do some kind of probing to see if conpty
        // is in the mix. For now we just assume that if we are on windows
        // then it must be in there somewhere.
        let is_conpty = cfg!(windows);

        // Not all systems understand that the cursor should move as
        // part of processing the image escapes, so we need to move it
        // explicitly after we've drawn things.
        // We can only do this reasonably sanely if we aren't setting
        // the absolute position.
        let needs_force_cursor_move =
            !self.no_move_cursor && !self.position.is_some() && (is_tmux || is_conpty);

        let caps = Capabilities::new_from_env()?;
        let mut term = termwiz::terminal::new_terminal(caps)?;
        term.set_raw_mode()?;

        let term_size = term.probe_screen_size()?;
        term.set_cooked_mode()?;

        let save_cursor = Esc::Code(EscCode::DecSaveCursorPosition);
        let restore_cursor = Esc::Code(EscCode::DecRestoreCursorPosition);

        if let Some(position) = &self.position {
            let csi = termwiz::escape::CSI::Cursor(
                termwiz::escape::csi::Cursor::CharacterAndLinePosition {
                    col: OneBased::from_zero_based(position.x),
                    line: OneBased::from_zero_based(position.y),
                },
            );
            print!("{save_cursor}{csi}");
        }

        let (begin, end) = self.tmux_passthru.unwrap_or_default().get();

        let image_dims = Self::image_dimensions(&data)
            .map(|info| self.compute_image_cell_dimensions(info, term_size));

        if let (Ok((_cursor_x, cursor_y)), true) = (&image_dims, needs_force_cursor_move) {
            // Before we emit the image, we need to emit some new lines so that
            // if the image would scroll the display, things end up in the right place
            let new_lines = "\n".repeat(*cursor_y);
            print!("{new_lines}");

            // and move back up again.
            // Note that we lose the x cursor position and land back in the first
            // column as a result of doing this.
            term.render(&[Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Relative(-1 * (*cursor_y as isize)),
            }])?;
        }

        let osc = OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
            ITermFileData {
                name: None,
                size: Some(data.len()),
                width: self.width.unwrap_or_default(),
                height: self.height.unwrap_or_default(),
                preserve_aspect_ratio: !self.no_preserve_aspect_ratio,
                inline: true,
                do_not_move_cursor: self.no_move_cursor,
                data,
            },
        )));
        println!("{begin}{osc}{end}");

        if let (Ok((_cursor_x, cursor_y)), true) = (&image_dims, needs_force_cursor_move) {
            // tell the terminal that doesn't fully understand the image sequence
            // to move the cursor to where it should end up
            term.render(&[Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Relative(*cursor_y as isize),
            }])?;
        } else if self.position.is_some() {
            print!("{restore_cursor}");
        }

        if self.hold {
            while let Ok(Some(event)) = term.poll_input(None) {
                match event {
                    InputEvent::Key(KeyEvent {
                        key: KeyCode::Enter,
                        modifiers: _,
                    }) => {
                        break;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Parser, Clone)]
struct SetCwdCommand {
    /// The directory to specify.
    /// If omitted, will use the current directory of the process itself.
    #[arg(value_parser, value_hint=ValueHint::DirPath)]
    cwd: Option<OsString>,

    /// How to manage passing the escape through to tmux
    #[arg(long, value_parser)]
    tmux_passthru: Option<TmuxPassthru>,

    /// The hostname to use in the constructed file:// URL.
    /// If omitted, the system hostname will be used.
    #[arg(value_parser, value_hint=ValueHint::Hostname)]
    host: Option<OsString>,
}

impl SetCwdCommand {
    fn run(&self) -> anyhow::Result<()> {
        let mut cwd = std::env::current_dir()?;
        if let Some(dir) = &self.cwd {
            cwd.push(dir);
        }

        let mut url = url::Url::from_directory_path(&cwd)
            .map_err(|_| anyhow::anyhow!("cwd {} is not an absolute path", cwd.display()))?;
        let host = match self.host.as_ref() {
            Some(h) => h.clone(),
            None => hostname::get()?,
        };
        let host = host.to_str().unwrap_or("localhost");
        url.set_host(Some(host))?;

        let (begin, end) = self.tmux_passthru.unwrap_or_default().get();

        let osc = OperatingSystemCommand::CurrentWorkingDirectory(url.into());
        print!("{begin}{osc}{end}");
        if !begin.is_empty() {
            // Tmux understands OSC 7 but won't automatically pass it through.
            // <https://github.com/tmux/tmux/issues/3127#issuecomment-1076300455>
            // Let's do it again explicitly now.
            print!("{osc}");
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
enum TmuxPassthru {
    Disable,
    Enable,
    #[default]
    Detect,
}

impl TmuxPassthru {
    fn is_tmux() -> bool {
        std::env::var_os("TMUX").is_some()
    }

    fn get(&self) -> (&'static str, &'static str) {
        let enabled = match self {
            Self::Enable => true,
            Self::Detect => Self::is_tmux(),
            Self::Disable => false,
        };
        if enabled {
            ("\u{1b}Ptmux;\u{1b}", "\u{1b}\\")
        } else {
            ("", "")
        }
    }
}

fn terminate_with_error_message(err: &str) -> ! {
    log::error!("{}; terminating", err);
    std::process::exit(1);
}

fn terminate_with_error(err: anyhow::Error) -> ! {
    terminate_with_error_message(&format!("{:#}", err));
}

fn main() {
    config::designate_this_as_the_main_thread();
    config::assign_error_callback(mux::connui::show_configuration_error_message);
    if let Err(e) = run() {
        terminate_with_error(e);
    }
    Mux::shutdown();
}

fn init_config(opts: &Opt) -> anyhow::Result<ConfigHandle> {
    config::common_init(
        opts.config_file.as_ref(),
        &opts.config_override,
        opts.skip_config,
    )
    .context("config::common_init")?;
    let config = config::configuration();
    config.update_ulimit()?;
    Ok(config)
}

fn run() -> anyhow::Result<()> {
    env_bootstrap::bootstrap();

    let saver = UmaskSaver::new();

    let opts = Opt::parse();

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(_)
        | SubCommand::LsFonts(_)
        | SubCommand::ShowKeys(_)
        | SubCommand::Ssh(_)
        | SubCommand::Serial(_)
        | SubCommand::Connect(_) => delegate_to_gui(saver),
        SubCommand::ImageCat(cmd) => cmd.run(),
        SubCommand::SetCwd(cmd) => cmd.run(),
        SubCommand::Cli(cli) => cli::run_cli(&opts, cli),
        SubCommand::Record(cmd) => cmd.run(init_config(&opts)?),
        SubCommand::Replay(cmd) => cmd.run(),
        SubCommand::ShellCompletion { shell } => {
            use clap::CommandFactory;
            let mut cmd = Opt::command();
            let name = cmd.get_name().to_string();
            generate_completion(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
    }
}

fn delegate_to_gui(saver: UmaskSaver) -> anyhow::Result<()> {
    use std::process::Command;

    // Restore the original umask
    drop(saver);

    let exe_name = if cfg!(windows) {
        "wezterm-gui.exe"
    } else {
        "wezterm-gui"
    };

    let exe = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("exe has no parent dir!?"))?
        .join(exe_name);

    let mut cmd = Command::new(exe);
    if cfg!(windows) {
        cmd.arg("--attach-parent-console");
    }

    cmd.args(std::env::args_os().skip(1));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Clean up random fds, except when we're running in an AppImage.
        // AppImage relies on child processes keeping alive an fd that
        // references the mount point and if we close it as part of execing
        // the gui binary, the appimage gets unmounted before we can exec.
        if std::env::var_os("APPIMAGE").is_none() {
            portable_pty::unix::close_random_fds();
        }
        let res = cmd.exec();
        return Err(anyhow::anyhow!("failed to exec {cmd:?}: {res:?}"));
    }

    #[cfg(windows)]
    {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}
