use config::keyassignment::*;
use config::{ConfigHandle, DeferredKeyCode};
use ordered_float::NotNan;
use std::borrow::Cow;
use std::convert::TryFrom;
use window::{KeyCode, Modifiers};
use KeyAssignment::*;

type ExpandFn = fn(&mut Expander);

/// Describes an argument/parameter/context that is required
/// in order for the command to have meaning.
/// The intent is for this to be used when filtering the items
/// that should be shown in eg: a context menu.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ArgType {
    /// Operates on the active pane
    ActivePane,
    /// Operates on the active tab
    ActiveTab,
    /// Operates on the active window
    ActiveWindow,
}

/// A helper function used to synthesize key binding permutations.
/// If the input is a character on a US ANSI keyboard layout, returns
/// the the typical character that is produced when holding down
/// the shift key and pressing the original key.
/// This doesn't produce an exhaustive list because there are only
/// a handful of default assignments in the command DEFS below.
fn us_layout_shift(s: &str) -> String {
    match s {
        "1" => "!".to_string(),
        "2" => "@".to_string(),
        "3" => "#".to_string(),
        "4" => "$".to_string(),
        "5" => "%".to_string(),
        "6" => "^".to_string(),
        "7" => "&".to_string(),
        "8" => "*".to_string(),
        "9" => "(".to_string(),
        "0" => ")".to_string(),
        "[" => "{".to_string(),
        "]" => "}".to_string(),
        "=" => "+".to_string(),
        "-" => "_".to_string(),
        "'" => "\"".to_string(),
        s if s.len() == 1 => s.to_ascii_uppercase(),
        s => s.to_string(),
    }
}

/// `CommandDef` defines a command in the UI.
pub struct CommandDef {
    /// Brief description
    pub brief: &'static str,
    /// A longer, more detailed, description
    pub doc: &'static str,
    /// A function that can produce 0 or more ExpandedCommand's.
    /// The intent is that we can use this to dynamically populate
    /// a list of commands for a given context.
    pub exp: ExpandFn,
    /// The key assignments associated with this command.
    pub keys: &'static [(Modifiers, &'static str)],
    /// The argument types/context in which this command is valid.
    pub args: &'static [ArgType],
}

impl std::fmt::Debug for CommandDef {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("CommandDef")
            .field("brief", &self.brief)
            .field("doc", &self.doc)
            .field("keys", &self.keys)
            .field("args", &self.args)
            .finish()
    }
}

impl CommandDef {
    /// Blech. Depending on the OS, a shifted key combination
    /// such as CTRL-SHIFT-L may present as either:
    /// CTRL+SHIFT + mapped lowercase l
    /// CTRL+SHIFT + mapped uppercase l
    /// CTRL       + mapped uppercase l
    ///
    /// This logic synthesizes the different combinations so
    /// that it isn't such a headache to maintain the mapping
    /// and prevents missing cases.
    ///
    /// Note that the mapped form of these things assumes
    /// US layout for some of the special shifted/punctuation cases.
    /// It's not perfect.
    ///
    /// The synthesis here requires that the defaults in
    /// the keymap below use the lowercase form of single characters!
    fn permute_keys(&self, config: &ConfigHandle) -> Vec<(Modifiers, KeyCode)> {
        let mut keys = vec![];

        for &(mods, label) in self.keys {
            let key = DeferredKeyCode::try_from(label)
                .unwrap()
                .resolve(config.key_map_preference)
                .clone();

            let ukey = DeferredKeyCode::try_from(us_layout_shift(label))
                .unwrap()
                .resolve(config.key_map_preference)
                .clone();

            keys.push((mods, key.clone()));

            if mods == Modifiers::SUPER {
                // We want each SUPER/CMD version of the keys to also have
                // CTRL+SHIFT version(s) for environments where SUPER/CMD
                // is reserved for the window manager.
                // This bit synthesizes those.
                keys.push((Modifiers::CTRL | Modifiers::SHIFT, key.clone()));
                if ukey != key {
                    keys.push((Modifiers::CTRL | Modifiers::SHIFT, ukey.clone()));
                    keys.push((Modifiers::CTRL, ukey.clone()));
                }
            } else if mods.contains(Modifiers::SHIFT) && ukey != key {
                keys.push((mods, ukey.clone()));
                keys.push((mods - Modifiers::SHIFT, ukey.clone()));
            }
        }

        keys
    }

    /// Produces the list of default key assignments and actions.
    /// Used by the InputMap.
    pub fn default_key_assignments(
        config: &ConfigHandle,
    ) -> Vec<(Modifiers, KeyCode, KeyAssignment)> {
        let mut result = vec![];
        for cmd in Self::expanded_commands(config) {
            for (mods, code) in cmd.keys {
                result.push((mods, code.clone(), cmd.action.clone()));
            }
        }
        result
    }

    /// Produces the complete set of expanded commands.
    pub fn expanded_commands(config: &ConfigHandle) -> Vec<ExpandedCommand> {
        let mut result = vec![];
        for def in DEFS {
            let expander = Expander::new(def, config);
            result.append(&mut expander.expand());
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct ExpandedCommand {
    pub brief: Cow<'static, str>,
    pub doc: Cow<'static, str>,
    pub action: KeyAssignment,
    pub keys: Vec<(Modifiers, KeyCode)>,
}

#[derive(Debug)]
pub struct Expander {
    template: &'static CommandDef,
    commands: Vec<ExpandedCommand>,
    config: ConfigHandle,
}

impl Expander {
    pub fn push(&mut self, action: KeyAssignment) {
        let expanded = ExpandedCommand {
            brief: self.template.brief.into(),
            doc: self.template.doc.into(),
            keys: self.template.permute_keys(&self.config),
            action,
        };
        self.commands.push(expanded);
    }

    pub fn new(template: &'static CommandDef, config: &ConfigHandle) -> Self {
        Self {
            template,
            commands: vec![],
            config: config.clone(),
        }
    }

    pub fn expand(mut self) -> Vec<ExpandedCommand> {
        (self.template.exp)(&mut self);
        self.commands
    }
}

static DEFS: &[CommandDef] = &[
    CommandDef {
        brief: "Paste primary selection",
        doc: "Pastes text from the primary selection",
        exp: |exp| exp.push(PasteFrom(ClipboardPasteSource::PrimarySelection)),
        keys: &[(Modifiers::SHIFT, "Insert")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Copy to primary selection",
        doc: "Copies text from the primary selection",
        exp: |exp| {
            exp.push(CopyTo(ClipboardCopyDestination::PrimarySelection));
        },
        keys: &[(Modifiers::CTRL, "Insert")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Copy to clipboard",
        doc: "Copies text to the clipboard",
        exp: |exp| exp.push(CopyTo(ClipboardCopyDestination::Clipboard)),
        keys: &[(Modifiers::SUPER, "c"), (Modifiers::NONE, "Copy")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Paste from clipboard",
        doc: "Pastes text from the clipboard",
        exp: |exp| exp.push(PasteFrom(ClipboardPasteSource::Clipboard)),
        keys: &[(Modifiers::SUPER, "v"), (Modifiers::NONE, "Paste")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Toggle full screen mode",
        doc: "Switch between normal and full screen mode",
        exp: |exp| {
            exp.push(ToggleFullScreen);
        },
        keys: &[(Modifiers::ALT, "Return")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Hide/Minimize Window",
        doc: "Hides/Mimimizes the current window",
        exp: |exp| {
            exp.push(Hide);
        },
        keys: &[(Modifiers::SUPER, "m")],
        args: &[ArgType::ActiveWindow],
    },
    #[cfg(target_os = "macos")]
    CommandDef {
        brief: "Hide Application (macOS only)",
        doc: "Hides all of the windows of the application. \
              This is macOS specific.",
        exp: |exp| {
            exp.push(HideApplication);
        },
        keys: &[(Modifiers::SUPER, "h")],
        args: &[],
    },
    #[cfg(target_os = "macos")]
    CommandDef {
        brief: "Quit WezTerm (macOS only)",
        doc: "Quits WezTerm",
        exp: |exp| {
            exp.push(QuitApplication);
        },
        keys: &[(Modifiers::SUPER, "q")],
        args: &[],
    },
    CommandDef {
        brief: "New Window",
        doc: "Launches the default program into a new window",
        exp: |exp| {
            exp.push(SpawnWindow);
        },
        keys: &[(Modifiers::SUPER, "n")],
        args: &[],
    },
    CommandDef {
        brief: "Clear scrollback",
        doc: "Clears any text that has scrolled out of the \
              viewport of the current pane",
        exp: |exp| {
            exp.push(ClearScrollback(ScrollbackEraseMode::ScrollbackOnly));
        },
        keys: &[(Modifiers::SUPER, "k")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Search pane output",
        doc: "Enters the search mode UI for the current pane",
        exp: |exp| {
            exp.push(Search(Pattern::CurrentSelectionOrEmptyString));
        },
        keys: &[(Modifiers::SUPER, "f")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Show debug overlay",
        doc: "Activates the debug overlay and Lua REPL",
        exp: |exp| {
            exp.push(ShowDebugOverlay);
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "l")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Enter QuickSelect mode",
        doc: "Activates the quick selection UI for the current pane",
        exp: |exp| {
            exp.push(QuickSelect);
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "Space")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Enter Emoji / Character selection mode",
        doc: "Activates the character selection UI for the current pane",
        exp: |exp| {
            exp.push(CharSelect(CharSelectArguments::default()));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "u")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Enter Pane selection mode",
        doc: "Activates the pane selection UI",
        exp: |exp| {
            exp.push(PaneSelect(PaneSelectArguments::default()));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "p")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Decrease font size",
        doc: "Scales the font size smaller by 10%",
        exp: |exp| {
            exp.push(DecreaseFontSize);
        },
        keys: &[(Modifiers::SUPER, "-"), (Modifiers::CTRL, "-")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Reset font size",
        doc: "Restores the font size to match your configuration file",
        exp: |exp| {
            exp.push(ResetFontSize);
        },
        keys: &[(Modifiers::SUPER, "0"), (Modifiers::CTRL, "0")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Increase font size",
        doc: "Scales the font size larger by 10%",
        exp: |exp| {
            exp.push(IncreaseFontSize);
        },
        keys: &[(Modifiers::SUPER, "="), (Modifiers::CTRL, "=")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "New Tab",
        doc: "Create a new tab in the same domain as the current pane",
        exp: |exp| {
            exp.push(SpawnTab(SpawnTabDomain::CurrentPaneDomain));
        },
        keys: &[(Modifiers::SUPER, "t")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 1st Tab",
        doc: "Activates the left-most tab",

        exp: |exp| {
            exp.push(ActivateTab(0));
        },
        keys: &[(Modifiers::SUPER, "1")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 2nd Tab",
        doc: "Activates the 2nd tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(1));
        },
        keys: &[(Modifiers::SUPER, "2")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 3rd Tab",
        doc: "Activates the 3rd tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(2));
        },
        keys: &[(Modifiers::SUPER, "3")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 4th Tab",
        doc: "Activates the 4th tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(3));
        },
        keys: &[(Modifiers::SUPER, "4")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 5th Tab",
        doc: "Activates the 5th tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(4));
        },
        keys: &[(Modifiers::SUPER, "5")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 6th Tab",
        doc: "Activates the 6th tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(5));
        },
        keys: &[(Modifiers::SUPER, "6")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 7th Tab",
        doc: "Activates the 7th tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(6));
        },
        keys: &[(Modifiers::SUPER, "7")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate 8th Tab",
        doc: "Activates the 8th tab from the left",
        exp: |exp| {
            exp.push(ActivateTab(7));
        },
        keys: &[(Modifiers::SUPER, "8")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate right-most tab",
        doc: "Activates the tab on the far right",
        exp: |exp| {
            exp.push(ActivateTab(-1));
        },
        keys: &[(Modifiers::SUPER, "9")],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Close current tab",
        doc: "Closes the current tab, terminating all the \
            processes that are running in its panes.",
        exp: |exp| {
            exp.push(CloseCurrentTab { confirm: true });
        },
        keys: &[(Modifiers::SUPER, "w")],
        args: &[ArgType::ActiveTab],
    },
    CommandDef {
        brief: "Activate the tab to the left",
        doc: "Activates the tab to the left. If this is the left-most \
            tab then cycles around and activates the right-most tab",
        exp: |exp| {
            exp.push(ActivateTabRelative(-1));
        },
        keys: &[
            (Modifiers::SUPER.union(Modifiers::SHIFT), "["),
            (Modifiers::CTRL.union(Modifiers::SHIFT), "Tab"),
            (Modifiers::CTRL, "PageUp"),
        ],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Activate the tab to the right",
        doc: "Activates the tab to the right. If this is the right-most \
            tab then cycles around and activates the left-most tab",
        exp: |exp| {
            exp.push(ActivateTabRelative(1));
        },
        keys: &[
            (Modifiers::SUPER.union(Modifiers::SHIFT), "]"),
            (Modifiers::CTRL, "Tab"),
            (Modifiers::CTRL, "PageDown"),
        ],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Reload configuration",
        doc: "Reloads the configuration file",
        exp: |exp| {
            exp.push(ReloadConfiguration);
        },
        keys: &[(Modifiers::SUPER, "r")],
        args: &[],
    },
    CommandDef {
        brief: "Move tab one place to the left",
        doc: "Rearranges the tabs so that the current tab moves \
            one place to the left",
        exp: |exp| {
            exp.push(MoveTabRelative(-1));
        },
        keys: &[(Modifiers::SUPER.union(Modifiers::SHIFT), "PageUp")],
        args: &[ArgType::ActiveTab],
    },
    CommandDef {
        brief: "Move tab one place to the right",
        doc: "Rearranges the tabs so that the current tab moves \
            one place to the right",
        exp: |exp| {
            exp.push(MoveTabRelative(1));
        },
        keys: &[(Modifiers::SUPER.union(Modifiers::SHIFT), "PageDown")],
        args: &[ArgType::ActiveTab],
    },
    CommandDef {
        brief: "Scroll Up One Page",
        doc: "Scrolls the viewport up by 1 page",
        exp: |exp| exp.push(ScrollByPage(NotNan::new(-1.0).unwrap())),
        keys: &[(Modifiers::SHIFT, "PageUp")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Scroll Down One Page",
        doc: "Scrolls the viewport down by 1 page",

        exp: |exp| exp.push(ScrollByPage(NotNan::new(1.0).unwrap())),
        keys: &[(Modifiers::SHIFT, "PageDown")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate Copy Mode",
        doc: "Enter mouse-less copy mode to select text using only \
            the keyboard",
        exp: |exp| {
            exp.push(ActivateCopyMode);
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "x")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Split Vertically (Top/Bottom)",
        doc: "Split the current pane vertically into two panes, by spawning \
            the default program into the bottom half",
        exp: |exp| {
            exp.push(SplitVertical(SpawnCommand {
                domain: SpawnTabDomain::CurrentPaneDomain,
                ..Default::default()
            }));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "'",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Split Horizontally (Left/Right)",
        doc: "Split the current pane horizontally into two panes, by spawning \
            the default program into the right hand side",
        exp: |exp| {
            exp.push(SplitHorizontal(SpawnCommand {
                domain: SpawnTabDomain::CurrentPaneDomain,
                ..Default::default()
            }));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "5",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Adjust Pane Size to the Left",
        doc: "Adjusts the closest split divider to the left",
        exp: |exp| {
            exp.push(AdjustPaneSize(PaneDirection::Left, 1));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "LeftArrow",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Adjust Pane Size to the Right",
        doc: "Adjusts the closest split divider to the right",
        exp: |exp| {
            exp.push(AdjustPaneSize(PaneDirection::Right, 1));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "RightArrow",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Adjust Pane Size Upwards",
        doc: "Adjusts the closest split divider towards the top",
        exp: |exp| {
            exp.push(AdjustPaneSize(PaneDirection::Up, 1));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "UpArrow",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Adjust Pane Size Downwards",
        doc: "Adjusts the closest split divider towards the bottom",
        exp: |exp| {
            exp.push(AdjustPaneSize(PaneDirection::Down, 1));
        },
        keys: &[(
            Modifiers::CTRL
                .union(Modifiers::ALT)
                .union(Modifiers::SHIFT),
            "DownArrow",
        )],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate Pane Left",
        doc: "Activates the pane to the left of the current pane",
        exp: |exp| {
            exp.push(ActivatePaneDirection(PaneDirection::Left));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "LeftArrow")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate Pane Right",
        doc: "Activates the pane to the right of the current pane",
        exp: |exp| {
            exp.push(ActivatePaneDirection(PaneDirection::Right));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "RightArrow")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate Pane Up",
        doc: "Activates the pane to the top of the current pane",
        exp: |exp| {
            exp.push(ActivatePaneDirection(PaneDirection::Up));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "UpArrow")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate Pane Down",
        doc: "Activates the pane to the bottom of the current pane",
        exp: |exp| {
            exp.push(ActivatePaneDirection(PaneDirection::Down));
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "DownArrow")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Toggle Pane Zoom",
        doc: "Toggles the zoom state for the current pane",
        exp: |exp| {
            exp.push(TogglePaneZoomState);
        },
        keys: &[(Modifiers::CTRL.union(Modifiers::SHIFT), "z")],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Activate the last active tab",
        doc: "If there was no prior active tab, has no effect.",
        exp: |exp| exp.push(ActivateLastTab),
        keys: &[],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Clear the key table stack",
        doc: "Removes all entries from the stack",
        exp: |exp| exp.push(ClearKeyTableStack),
        keys: &[],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Clear the scrollback and viewport",
        doc: "Removes all content from the screen and scrollback",
        exp: |exp| exp.push(ClearScrollback(ScrollbackEraseMode::ScrollbackOnly)),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Close the active pane",
        doc: "Terminates the process in the pane and closes it",
        exp: |exp| exp.push(CloseCurrentPane { confirm: true }),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Open link at mouse cursor",
        doc: "If there is no link under the mouse cursor, has no effect.",
        exp: |exp| exp.push(OpenLinkAtMouseCursor),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Reset the window and font size",
        doc: "Restores the original window and font size",
        exp: |exp| exp.push(ResetFontAndWindowSize),
        keys: &[],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Scroll to the bottom",
        doc: "Scrolls to the bottom of the viewport",
        exp: |exp| exp.push(ScrollToBottom),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Scroll to the top",
        doc: "Scrolls to the top of the viewport",
        exp: |exp| exp.push(ScrollToTop),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
    CommandDef {
        brief: "Show the launcher",
        doc: "Shows the launcher menu",
        exp: |exp| exp.push(ShowLauncher),
        keys: &[],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Navigate tabs",
        doc: "Shows the tab navigator",
        exp: |exp| exp.push(ShowTabNavigator),
        keys: &[],
        args: &[ArgType::ActiveWindow],
    },
    CommandDef {
        brief: "Detach the domain of the active pane",
        doc: "Detaches (disconnects from) the domain of the active pane",
        exp: |exp| exp.push(DetachDomain(SpawnTabDomain::CurrentPaneDomain)),
        keys: &[],
        args: &[ArgType::ActivePane],
    },
];
