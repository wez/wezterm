#compdef wezterm

autoload -U is-at-least

_wezterm() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" \
'(-n --skip-config)--config-file=[Specify the configuration file to use, overrides the normal configuration file resolution]:CONFIG_FILE:_files' \
'*--config=[Override specific configuration values]:name=value: ' \
'-n[Skip loading wezterm.lua]' \
'--skip-config[Skip loading wezterm.lua]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_wezterm_commands" \
"*::: :->wezterm" \
&& ret=0
    case $state in
    (wezterm)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:wezterm-command-$line[1]:"
        case $line[1] in
            (start)
_arguments "${_arguments_options[@]}" \
'--cwd=[Specify the current working directory for the initially spawned program]:CWD:_files -/' \
'--class=[Override the default windowing system class. The default is "org.wezfurlong.wezterm". Under X11 and Windows this changes the window class. Under Wayland this changes the app_id. This changes the class for all windows spawned by this instance of wezterm, including error, update and ssh authentication dialogs]:CLASS: ' \
'--workspace=[Override the default workspace with the provided name. The default is "default"]:WORKSPACE: ' \
'--position=[Override the position for the initial window launched by this process.]:POSITION: ' \
'--domain=[Name of the multiplexer domain section from the configuration to which you'\''d like to connect. If omitted, the default domain will be used]:DOMAIN: ' \
'--no-auto-connect[If true, do not connect to domains marked as connect_automatically in your wezterm configuration file]' \
'--always-new-process[If enabled, don'\''t try to ask an existing wezterm GUI instance to start the command.  Instead, always start the GUI in this invocation of wezterm so that you can wait for the command to complete by waiting for this wezterm process to finish]' \
'(--always-new-process)--new-tab[When spawning into an existing GUI instance, spawn a new tab into the active window rather than spawn a new window]' \
'-e[Dummy argument that consumes "-e" and does nothing. This is meant as a compatibility layer for supporting the widely adopted standard of passing the command to execute to the terminal via a "-e" option. This works because we then treat the remaining cmdline as trailing options, that will automatically be parsed via the existing "prog" option. This option exists only as a fallback. It is recommended to pass the command as a normal trailing command instead if possible]' \
'--attach[When used with --domain, if the domain already has running panes, wezterm will simply attach and will NOT spawn the specified PROG. If you omit --attach when using --domain, wezterm will attach AND then spawn PROG]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm start -- bash -l` will spawn bash as if it were a login shell. \[aliases\: -e\]:_cmdambivalent' \
&& ret=0
;;
(blocking-start)
_arguments "${_arguments_options[@]}" \
'--cwd=[Specify the current working directory for the initially spawned program]:CWD:_files -/' \
'--class=[Override the default windowing system class. The default is "org.wezfurlong.wezterm". Under X11 and Windows this changes the window class. Under Wayland this changes the app_id. This changes the class for all windows spawned by this instance of wezterm, including error, update and ssh authentication dialogs]:CLASS: ' \
'--workspace=[Override the default workspace with the provided name. The default is "default"]:WORKSPACE: ' \
'--position=[Override the position for the initial window launched by this process.]:POSITION: ' \
'--domain=[Name of the multiplexer domain section from the configuration to which you'\''d like to connect. If omitted, the default domain will be used]:DOMAIN: ' \
'--no-auto-connect[If true, do not connect to domains marked as connect_automatically in your wezterm configuration file]' \
'--always-new-process[If enabled, don'\''t try to ask an existing wezterm GUI instance to start the command.  Instead, always start the GUI in this invocation of wezterm so that you can wait for the command to complete by waiting for this wezterm process to finish]' \
'(--always-new-process)--new-tab[When spawning into an existing GUI instance, spawn a new tab into the active window rather than spawn a new window]' \
'-e[Dummy argument that consumes "-e" and does nothing. This is meant as a compatibility layer for supporting the widely adopted standard of passing the command to execute to the terminal via a "-e" option. This works because we then treat the remaining cmdline as trailing options, that will automatically be parsed via the existing "prog" option. This option exists only as a fallback. It is recommended to pass the command as a normal trailing command instead if possible]' \
'--attach[When used with --domain, if the domain already has running panes, wezterm will simply attach and will NOT spawn the specified PROG. If you omit --attach when using --domain, wezterm will attach AND then spawn PROG]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm start -- bash -l` will spawn bash as if it were a login shell. \[aliases\: -e\]:_cmdambivalent' \
&& ret=0
;;
(ssh)
_arguments "${_arguments_options[@]}" \
'*-o+[Override specific SSH configuration options. \`wezterm ssh\` is able to parse some (but not all!) options from your \`~/.ssh/config\` and \`/etc/ssh/ssh_config\` files. This command line switch allows you to override or otherwise specify ssh_config style options]:name=value: ' \
'*--ssh-option=[Override specific SSH configuration options. \`wezterm ssh\` is able to parse some (but not all!) options from your \`~/.ssh/config\` and \`/etc/ssh/ssh_config\` files. This command line switch allows you to override or otherwise specify ssh_config style options]:name=value: ' \
'--class=[Override the default windowing system class. The default is "org.wezfurlong.wezterm". Under X11 and Windows this changes the window class. Under Wayland this changes the app_id. This changes the class for all windows spawned by this instance of wezterm, including error, update and ssh authentication dialogs]:CLASS: ' \
'--position=[Override the position for the initial window launched by this process.]:POSITION: ' \
'-v[Enable verbose ssh protocol tracing. The trace information is printed to the stderr stream of the process]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':user_at_host_and_port -- Specifies the remote system using the form\: `\[username@\]host\[\:port\]`. If `username@` is omitted, then your local $USER is used instead. If `\:port` is omitted, then the standard ssh port (22) is used instead:' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm ssh user@host -- bash -l` will spawn bash as if it were a login shell:_cmdambivalent' \
&& ret=0
;;
(serial)
_arguments "${_arguments_options[@]}" \
'--baud=[Set the baud rate.  The default is 9600 baud]:BAUD: ' \
'--class=[Override the default windowing system class. The default is "org.wezfurlong.wezterm". Under X11 and Windows this changes the window class. Under Wayland this changes the app_id. This changes the class for all windows spawned by this instance of wezterm, including error, update and ssh authentication dialogs]:CLASS: ' \
'--position=[Override the position for the initial window launched by this process.]:POSITION: ' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':port -- Specifies the serial device name. On Windows systems this can be a name like `COM0`. On posix systems this will be something like `/dev/ttyUSB0`:' \
&& ret=0
;;
(connect)
_arguments "${_arguments_options[@]}" \
'--class=[Override the default windowing system class. The default is "org.wezfurlong.wezterm". Under X11 and Windows this changes the window class. Under Wayland this changes the app_id. This changes the class for all windows spawned by this instance of wezterm, including error, update and ssh authentication dialogs]:CLASS: ' \
'--workspace=[Override the default workspace with the provided name. The default is "default"]:WORKSPACE: ' \
'--position=[Override the position for the initial window launched by this process.]:POSITION: ' \
'--new-tab[When spawning into an existing GUI instance, spawn a new tab into the active window rather than spawn a new window]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':domain_name -- Name of the multiplexer domain section from the configuration to which you'\''d like to connect:' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm start -- bash -l` will spawn bash as if it were a login shell:_cmdambivalent' \
&& ret=0
;;
(ls-fonts)
_arguments "${_arguments_options[@]}" \
'(--list-system --codepoints)--text=[Explain which fonts are used to render the supplied text string]:TEXT: ' \
'(--list-system)--codepoints=[Explain which fonts are used to render the specified unicode code point sequence. Code points are comma separated hex values]:CODEPOINTS: ' \
'--list-system[Whether to list all fonts available to the system]' \
'--rasterize-ascii[Show rasterized glyphs for the text in --text or --codepoints using ascii blocks]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(show-keys)
_arguments "${_arguments_options[@]}" \
'--key-table=[In lua mode, show only the named key table]:KEY_TABLE: ' \
'--lua[Show the keys as lua config statements]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(cli)
_arguments "${_arguments_options[@]}" \
'--class=[When connecting to a gui instance, if you started the gui with \`--class SOMETHING\`, you should also pass that same value here in order for the client to find the correct gui instance]:CLASS: ' \
'--no-auto-start[Don'\''t automatically start the server]' \
'--prefer-mux[Prefer connecting to a background mux server. The default is to prefer connecting to a running wezterm gui instance]' \
'-h[Print help]' \
'--help[Print help]' \
":: :_wezterm__cli_commands" \
"*::: :->cli" \
&& ret=0

    case $state in
    (cli)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:wezterm-cli-command-$line[1]:"
        case $line[1] in
            (list)
_arguments "${_arguments_options[@]}" \
'--format=[Controls the output format. "table" and "json" are possible formats]:FORMAT: ' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(list-clients)
_arguments "${_arguments_options[@]}" \
'--format=[Controls the output format. "table" and "json" are possible formats]:FORMAT: ' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(proxy)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(tlscreds)
_arguments "${_arguments_options[@]}" \
'--pem[Output a PEM file encoded copy of the credentials]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(move-pane-to-new-tab)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the pane that should be moved. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--window-id=[Specify the window into which the new tab will be created. If omitted, the window associated with the current pane is used]:WINDOW_ID: ' \
'--workspace=[If creating a new window, override the default workspace name with the provided name.  The default name is "default"]:WORKSPACE: ' \
'(--window-id)--new-window[Create tab in a new window, rather than the window currently containing the pane]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(split-pane)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the pane that should be split. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--cells=[The number of cells that the new split should have. If omitted, 50% of the available space is used]:CELLS: ' \
'(--cells)--percent=[Specify the number of cells that the new split should have, expressed as a percentage of the available space]:PERCENT: ' \
'--cwd=[Specify the current working directory for the initially spawned program]:CWD:_files -/' \
'(--cwd)--move-pane-id=[Instead of spawning a new command, move the specified pane into the newly created split]:MOVE_PANE_ID: ' \
'(--left --right --top --bottom)--horizontal[Equivalent to \`--right\`. If neither this nor any other direction is specified, the default is equivalent to \`--bottom\`]' \
'(--right --top --bottom)--left[Split horizontally, with the new pane on the left]' \
'(--left --top --bottom)--right[Split horizontally, with the new pane on the right]' \
'(--left --right --bottom)--top[Split vertically, with the new pane on the top]' \
'(--left --right --top)--bottom[Split vertically, with the new pane on the bottom]' \
'--top-level[Rather than splitting the active pane, split the entire window]' \
'-h[Print help]' \
'--help[Print help]' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm cli split-pane -- bash -l` will spawn bash as if it were a login shell:_cmdambivalent' \
&& ret=0
;;
(spawn)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE. The pane is used to determine the current domain and window]:PANE_ID: ' \
'--domain-name=[]:DOMAIN_NAME: ' \
'(--workspace --new-window)--window-id=[Specify the window into which to spawn a tab. If omitted, the window associated with the current pane is used. Cannot be used with \`--workspace\` or \`--new-window\`]:WINDOW_ID: ' \
'--cwd=[Specify the current working directory for the initially spawned program]:CWD:_files -/' \
'--workspace=[When creating a new window, override the default workspace name with the provided name.  The default name is "default". Requires \`--new-window\`]:WORKSPACE: ' \
'--new-window[Spawn into a new window, rather than a new tab]' \
'-h[Print help]' \
'--help[Print help]' \
'*::prog -- Instead of executing your shell, run PROG. For example\: `wezterm cli spawn -- bash -l` will spawn bash as if it were a login shell:_cmdambivalent' \
&& ret=0
;;
(send-text)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--no-paste[Send the text directly, rather than as a bracketed paste]' \
'-h[Print help]' \
'--help[Print help]' \
'::text -- The text to send. If omitted, will read the text from stdin:' \
&& ret=0
;;
(get-text)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--start-line=[The starting line number. 0 is the first line of terminal screen. Negative numbers proceed backwards into the scrollback. The default value is unspecified is 0, the first line of the terminal screen]:START_LINE: ' \
'--end-line=[The ending line number. 0 is the first line of terminal screen. Negative numbers proceed backwards into the scrollback. The default value if unspecified is the bottom of the the terminal screen]:END_LINE: ' \
'--escapes[Include escape sequences that color and style the text. If omitted, unattributed text will be returned]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(activate-pane-direction)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help]' \
'--help[Print help]' \
':direction -- The direction to switch to:(Up Down Left Right Next Prev)' \
&& ret=0
;;
(get-pane-direction)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':direction -- The direction to consider:(Up Down Left Right Next Prev)' \
&& ret=0
;;
(kill-pane)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(activate-pane)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(adjust-pane-size)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--amount=[Specify the number of cells to resize by, defaults to 1]:AMOUNT: ' \
'-h[Print help]' \
'--help[Print help]' \
':direction -- Specify the direction to resize in:(Up Down Left Right Next Prev)' \
&& ret=0
;;
(activate-tab)
_arguments "${_arguments_options[@]}" \
'(--tab-index --tab-relative --no-wrap --pane-id)--tab-id=[Specify the target tab by its id]:TAB_ID: ' \
'--tab-index=[Specify the target tab by its index within the window that holds the current pane. Indices are 0-based, with 0 being the left-most tab. Negative numbers can be used to reference the right-most tab, so -1 is the right-most tab, -2 is the penultimate tab and so on]:TAB_INDEX: ' \
'--tab-relative=[Specify the target tab by its relative offset. -1 selects the tab to the left. -2 two tabs to the left. 1 is one tab to the right and so on]:TAB_RELATIVE: ' \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'--no-wrap[When used with tab-relative, prevents wrapping around and will instead clamp to the left-most when moving left or right-most when moving right]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(set-tab-title)
_arguments "${_arguments_options[@]}" \
'(--pane-id)--tab-id=[Specify the target tab by its id]:TAB_ID: ' \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':title -- The new title for the tab:' \
&& ret=0
;;
(set-window-title)
_arguments "${_arguments_options[@]}" \
'(--pane-id)--window-id=[Specify the target window by its id]:WINDOW_ID: ' \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':title -- The new title for the window:' \
&& ret=0
;;
(rename-workspace)
_arguments "${_arguments_options[@]}" \
'--workspace=[Specify the workspace to rename]:WORKSPACE: ' \
'--pane-id=[Specify the current pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':new_workspace -- The new name for the workspace:' \
&& ret=0
;;
(zoom-pane)
_arguments "${_arguments_options[@]}" \
'--pane-id=[Specify the target pane. The default is to use the current pane based on the environment variable WEZTERM_PANE]:PANE_ID: ' \
'(--unzoom --toggle)--zoom[Zooms the pane if it wasn'\''t already zoomed]' \
'(--zoom --toggle)--unzoom[Unzooms the pane if it was zoomed]' \
'(--zoom --unzoom)--toggle[Toggles the zoom state of the pane]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_wezterm__cli__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:wezterm-cli-help-command-$line[1]:"
        case $line[1] in
            (list)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(list-clients)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(proxy)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(tlscreds)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(move-pane-to-new-tab)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(split-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(spawn)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(send-text)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(get-text)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-pane-direction)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(get-pane-direction)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(kill-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(adjust-pane-size)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-tab)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(set-tab-title)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(set-window-title)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(rename-workspace)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(zoom-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(imgcat)
_arguments "${_arguments_options[@]}" \
'--width=[Specify the display width; defaults to "auto" which automatically selects an appropriate size.  You may also use an integer value \`N\` to specify the number of cells, or \`Npx\` to specify the number of pixels, or \`N%\` to size relative to the terminal width]:WIDTH: ' \
'--height=[Specify the display height; defaults to "auto" which automatically selects an appropriate size.  You may also use an integer value \`N\` to specify the number of cells, or \`Npx\` to specify the number of pixels, or \`N%\` to size relative to the terminal height]:HEIGHT: ' \
'--position=[Set the cursor position prior to displaying the image. The default is to use the current cursor position. Coordinates are expressed in cells with 0,0 being the top left cell position]:POSITION: ' \
'--tmux-passthru=[How to manage passing the escape through to tmux]:TMUX_PASSTHRU:(disable enable detect)' \
'--max-pixels=[Set the maximum number of pixels per image frame. Images will be scaled down so that they do not exceed this size, unless \`--no-resample\` is also used. The default value matches the limit set by wezterm. Note that resampling the image here will reduce any animated images to a single frame]:MAX_PIXELS: ' \
'--resample-format=[Specify the image format to use to encode resampled/resized images.  The default is to match the input format, but you can choose an alternative format]:RESAMPLE_FORMAT:(png jpeg input)' \
'--resample-filter=[Specify the filtering technique used when resizing/resampling images.  The default is a reasonable middle ground of speed and quality]:RESAMPLE_FILTER:(nearest triangle catmull-rom gaussian lanczos3)' \
'--resize=[Pre-process the image to resize it to the specified dimensions, expressed as eg\: 800x600 (width x height). The resize is independent of other parameters that control the image placement and dimensions in the terminal; this is provided as a convenience preprocessing step]:WIDTHxHEIGHT: ' \
'--no-preserve-aspect-ratio[Do not respect the aspect ratio.  The default is to respect the aspect ratio]' \
'--no-move-cursor[Do not move the cursor after displaying the image. Note that when used like this from the shell, there is a very high chance that shell prompt will overwrite the image; you may wish to also use \`--hold\` in that case]' \
'--hold[Wait for enter to be pressed after displaying the image]' \
'--no-resample[Do not resample images whose frames are larger than the max-pixels value. Note that this will typically result in the image refusing to display in wezterm]' \
'--show-resample-timing[When resampling or resizing, display some diagnostics around the timing/performance of that operation]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::file_name -- The name of the image file to be displayed. If omitted, will attempt to read it from stdin:_files' \
&& ret=0
;;
(set-working-directory)
_arguments "${_arguments_options[@]}" \
'--tmux-passthru=[How to manage passing the escape through to tmux]:TMUX_PASSTHRU:(disable enable detect)' \
'-h[Print help]' \
'--help[Print help]' \
'::cwd -- The directory to specify. If omitted, will use the current directory of the process itself:_files -/' \
'::host -- The hostname to use in the constructed file\:// URL. If omitted, the system hostname will be used:_hosts' \
&& ret=0
;;
(record)
_arguments "${_arguments_options[@]}" \
'--cwd=[Start in the specified directory, instead of the default_cwd defined by your wezterm configuration]:CWD:_files' \
'-h[Print help]' \
'--help[Print help]' \
'*::prog -- Start prog instead of the default_prog defined by your wezterm configuration:' \
&& ret=0
;;
(replay)
_arguments "${_arguments_options[@]}" \
'--explain[Explain what is being sent/received]' \
'(--explain)--explain-only[Don'\''t replay, just show the explanation]' \
'(--explain)--cat[Just emit raw escape sequences all at once, with no timing information]' \
'-h[Print help]' \
'--help[Print help]' \
':cast_file:_files' \
&& ret=0
;;
(shell-completion)
_arguments "${_arguments_options[@]}" \
'--shell=[Which shell to generate for]:SHELL:(bash elvish fish power-shell zsh fig)' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_wezterm__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:wezterm-help-command-$line[1]:"
        case $line[1] in
            (start)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(blocking-start)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(ssh)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(serial)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(connect)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(ls-fonts)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(show-keys)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(cli)
_arguments "${_arguments_options[@]}" \
":: :_wezterm__help__cli_commands" \
"*::: :->cli" \
&& ret=0

    case $state in
    (cli)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:wezterm-help-cli-command-$line[1]:"
        case $line[1] in
            (list)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(list-clients)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(proxy)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(tlscreds)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(move-pane-to-new-tab)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(split-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(spawn)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(send-text)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(get-text)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-pane-direction)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(get-pane-direction)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(kill-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(adjust-pane-size)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(activate-tab)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(set-tab-title)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(set-window-title)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(rename-workspace)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(zoom-pane)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
(imgcat)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(set-working-directory)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(record)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(replay)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(shell-completion)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_wezterm_commands] )) ||
_wezterm_commands() {
    local commands; commands=(
'start:Start the GUI, optionally running an alternative program \[aliases\: -e\]' \
'blocking-start:Start the GUI in blocking mode. You shouldn'\''t see this, but you may see it in shell completions because of this open clap issue\: <https\://github.com/clap-rs/clap/issues/1335>' \
'ssh:Establish an ssh session' \
'serial:Open a serial port' \
'connect:Connect to wezterm multiplexer' \
'ls-fonts:Display information about fonts' \
'show-keys:Show key assignments' \
'cli:Interact with experimental mux server' \
'imgcat:Output an image to the terminal' \
'set-working-directory:Advise the terminal of the current working directory by emitting an OSC 7 escape sequence' \
'record:Record a terminal session as an asciicast' \
'replay:Replay an asciicast terminal session' \
'shell-completion:Generate shell completion information' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'wezterm commands' commands "$@"
}
(( $+functions[_wezterm__cli__activate-pane_commands] )) ||
_wezterm__cli__activate-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli activate-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__activate-pane_commands] )) ||
_wezterm__cli__help__activate-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help activate-pane commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__activate-pane_commands] )) ||
_wezterm__help__cli__activate-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli activate-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__activate-pane-direction_commands] )) ||
_wezterm__cli__activate-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli activate-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__activate-pane-direction_commands] )) ||
_wezterm__cli__help__activate-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help activate-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__activate-pane-direction_commands] )) ||
_wezterm__help__cli__activate-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli activate-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__cli__activate-tab_commands] )) ||
_wezterm__cli__activate-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli activate-tab commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__activate-tab_commands] )) ||
_wezterm__cli__help__activate-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help activate-tab commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__activate-tab_commands] )) ||
_wezterm__help__cli__activate-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli activate-tab commands' commands "$@"
}
(( $+functions[_wezterm__cli__adjust-pane-size_commands] )) ||
_wezterm__cli__adjust-pane-size_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli adjust-pane-size commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__adjust-pane-size_commands] )) ||
_wezterm__cli__help__adjust-pane-size_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help adjust-pane-size commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__adjust-pane-size_commands] )) ||
_wezterm__help__cli__adjust-pane-size_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli adjust-pane-size commands' commands "$@"
}
(( $+functions[_wezterm__blocking-start_commands] )) ||
_wezterm__blocking-start_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm blocking-start commands' commands "$@"
}
(( $+functions[_wezterm__help__blocking-start_commands] )) ||
_wezterm__help__blocking-start_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help blocking-start commands' commands "$@"
}
(( $+functions[_wezterm__cli_commands] )) ||
_wezterm__cli_commands() {
    local commands; commands=(
'list:list windows, tabs and panes' \
'list-clients:list clients' \
'proxy:start rpc proxy pipe' \
'tlscreds:obtain tls credentials' \
'move-pane-to-new-tab:Move a pane into a new tab' \
'split-pane:split the current pane.
Outputs the pane-id for the newly created pane on success' \
'spawn:Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success' \
'send-text:Send text to a pane as though it were pasted. If bracketed paste mode is enabled in the pane, then the text will be sent as a bracketed paste' \
'get-text:Retrieves the textual content of a pane and output it to stdout' \
'activate-pane-direction:Activate an adjacent pane in the specified direction' \
'get-pane-direction:Determine the adjacent pane in the specified direction' \
'kill-pane:Kill a pane' \
'activate-pane:Activate (focus) a pane' \
'adjust-pane-size:Adjust the size of a pane directionally' \
'activate-tab:Activate a tab' \
'set-tab-title:Change the title of a tab' \
'set-window-title:Change the title of a window' \
'rename-workspace:Rename a workspace' \
'zoom-pane:Zoom, unzoom, or toggle zoom state' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'wezterm cli commands' commands "$@"
}
(( $+functions[_wezterm__help__cli_commands] )) ||
_wezterm__help__cli_commands() {
    local commands; commands=(
'list:list windows, tabs and panes' \
'list-clients:list clients' \
'proxy:start rpc proxy pipe' \
'tlscreds:obtain tls credentials' \
'move-pane-to-new-tab:Move a pane into a new tab' \
'split-pane:split the current pane.
Outputs the pane-id for the newly created pane on success' \
'spawn:Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success' \
'send-text:Send text to a pane as though it were pasted. If bracketed paste mode is enabled in the pane, then the text will be sent as a bracketed paste' \
'get-text:Retrieves the textual content of a pane and output it to stdout' \
'activate-pane-direction:Activate an adjacent pane in the specified direction' \
'get-pane-direction:Determine the adjacent pane in the specified direction' \
'kill-pane:Kill a pane' \
'activate-pane:Activate (focus) a pane' \
'adjust-pane-size:Adjust the size of a pane directionally' \
'activate-tab:Activate a tab' \
'set-tab-title:Change the title of a tab' \
'set-window-title:Change the title of a window' \
'rename-workspace:Rename a workspace' \
'zoom-pane:Zoom, unzoom, or toggle zoom state' \
    )
    _describe -t commands 'wezterm help cli commands' commands "$@"
}
(( $+functions[_wezterm__connect_commands] )) ||
_wezterm__connect_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm connect commands' commands "$@"
}
(( $+functions[_wezterm__help__connect_commands] )) ||
_wezterm__help__connect_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help connect commands' commands "$@"
}
(( $+functions[_wezterm__cli__get-pane-direction_commands] )) ||
_wezterm__cli__get-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli get-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__get-pane-direction_commands] )) ||
_wezterm__cli__help__get-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help get-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__get-pane-direction_commands] )) ||
_wezterm__help__cli__get-pane-direction_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli get-pane-direction commands' commands "$@"
}
(( $+functions[_wezterm__cli__get-text_commands] )) ||
_wezterm__cli__get-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli get-text commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__get-text_commands] )) ||
_wezterm__cli__help__get-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help get-text commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__get-text_commands] )) ||
_wezterm__help__cli__get-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli get-text commands' commands "$@"
}
(( $+functions[_wezterm__cli__help_commands] )) ||
_wezterm__cli__help_commands() {
    local commands; commands=(
'list:list windows, tabs and panes' \
'list-clients:list clients' \
'proxy:start rpc proxy pipe' \
'tlscreds:obtain tls credentials' \
'move-pane-to-new-tab:Move a pane into a new tab' \
'split-pane:split the current pane.
Outputs the pane-id for the newly created pane on success' \
'spawn:Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success' \
'send-text:Send text to a pane as though it were pasted. If bracketed paste mode is enabled in the pane, then the text will be sent as a bracketed paste' \
'get-text:Retrieves the textual content of a pane and output it to stdout' \
'activate-pane-direction:Activate an adjacent pane in the specified direction' \
'get-pane-direction:Determine the adjacent pane in the specified direction' \
'kill-pane:Kill a pane' \
'activate-pane:Activate (focus) a pane' \
'adjust-pane-size:Adjust the size of a pane directionally' \
'activate-tab:Activate a tab' \
'set-tab-title:Change the title of a tab' \
'set-window-title:Change the title of a window' \
'rename-workspace:Rename a workspace' \
'zoom-pane:Zoom, unzoom, or toggle zoom state' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'wezterm cli help commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__help_commands] )) ||
_wezterm__cli__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help help commands' commands "$@"
}
(( $+functions[_wezterm__help_commands] )) ||
_wezterm__help_commands() {
    local commands; commands=(
'start:Start the GUI, optionally running an alternative program \[aliases\: -e\]' \
'blocking-start:Start the GUI in blocking mode. You shouldn'\''t see this, but you may see it in shell completions because of this open clap issue\: <https\://github.com/clap-rs/clap/issues/1335>' \
'ssh:Establish an ssh session' \
'serial:Open a serial port' \
'connect:Connect to wezterm multiplexer' \
'ls-fonts:Display information about fonts' \
'show-keys:Show key assignments' \
'cli:Interact with experimental mux server' \
'imgcat:Output an image to the terminal' \
'set-working-directory:Advise the terminal of the current working directory by emitting an OSC 7 escape sequence' \
'record:Record a terminal session as an asciicast' \
'replay:Replay an asciicast terminal session' \
'shell-completion:Generate shell completion information' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'wezterm help commands' commands "$@"
}
(( $+functions[_wezterm__help__help_commands] )) ||
_wezterm__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help help commands' commands "$@"
}
(( $+functions[_wezterm__help__imgcat_commands] )) ||
_wezterm__help__imgcat_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help imgcat commands' commands "$@"
}
(( $+functions[_wezterm__imgcat_commands] )) ||
_wezterm__imgcat_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm imgcat commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__kill-pane_commands] )) ||
_wezterm__cli__help__kill-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help kill-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__kill-pane_commands] )) ||
_wezterm__cli__kill-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli kill-pane commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__kill-pane_commands] )) ||
_wezterm__help__cli__kill-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli kill-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__list_commands] )) ||
_wezterm__cli__help__list_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help list commands' commands "$@"
}
(( $+functions[_wezterm__cli__list_commands] )) ||
_wezterm__cli__list_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli list commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__list_commands] )) ||
_wezterm__help__cli__list_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli list commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__list-clients_commands] )) ||
_wezterm__cli__help__list-clients_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help list-clients commands' commands "$@"
}
(( $+functions[_wezterm__cli__list-clients_commands] )) ||
_wezterm__cli__list-clients_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli list-clients commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__list-clients_commands] )) ||
_wezterm__help__cli__list-clients_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli list-clients commands' commands "$@"
}
(( $+functions[_wezterm__help__ls-fonts_commands] )) ||
_wezterm__help__ls-fonts_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help ls-fonts commands' commands "$@"
}
(( $+functions[_wezterm__ls-fonts_commands] )) ||
_wezterm__ls-fonts_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm ls-fonts commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__move-pane-to-new-tab_commands] )) ||
_wezterm__cli__help__move-pane-to-new-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help move-pane-to-new-tab commands' commands "$@"
}
(( $+functions[_wezterm__cli__move-pane-to-new-tab_commands] )) ||
_wezterm__cli__move-pane-to-new-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli move-pane-to-new-tab commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__move-pane-to-new-tab_commands] )) ||
_wezterm__help__cli__move-pane-to-new-tab_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli move-pane-to-new-tab commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__proxy_commands] )) ||
_wezterm__cli__help__proxy_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help proxy commands' commands "$@"
}
(( $+functions[_wezterm__cli__proxy_commands] )) ||
_wezterm__cli__proxy_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli proxy commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__proxy_commands] )) ||
_wezterm__help__cli__proxy_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli proxy commands' commands "$@"
}
(( $+functions[_wezterm__help__record_commands] )) ||
_wezterm__help__record_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help record commands' commands "$@"
}
(( $+functions[_wezterm__record_commands] )) ||
_wezterm__record_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm record commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__rename-workspace_commands] )) ||
_wezterm__cli__help__rename-workspace_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help rename-workspace commands' commands "$@"
}
(( $+functions[_wezterm__cli__rename-workspace_commands] )) ||
_wezterm__cli__rename-workspace_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli rename-workspace commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__rename-workspace_commands] )) ||
_wezterm__help__cli__rename-workspace_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli rename-workspace commands' commands "$@"
}
(( $+functions[_wezterm__help__replay_commands] )) ||
_wezterm__help__replay_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help replay commands' commands "$@"
}
(( $+functions[_wezterm__replay_commands] )) ||
_wezterm__replay_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm replay commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__send-text_commands] )) ||
_wezterm__cli__help__send-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help send-text commands' commands "$@"
}
(( $+functions[_wezterm__cli__send-text_commands] )) ||
_wezterm__cli__send-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli send-text commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__send-text_commands] )) ||
_wezterm__help__cli__send-text_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli send-text commands' commands "$@"
}
(( $+functions[_wezterm__help__serial_commands] )) ||
_wezterm__help__serial_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help serial commands' commands "$@"
}
(( $+functions[_wezterm__serial_commands] )) ||
_wezterm__serial_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm serial commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__set-tab-title_commands] )) ||
_wezterm__cli__help__set-tab-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help set-tab-title commands' commands "$@"
}
(( $+functions[_wezterm__cli__set-tab-title_commands] )) ||
_wezterm__cli__set-tab-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli set-tab-title commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__set-tab-title_commands] )) ||
_wezterm__help__cli__set-tab-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli set-tab-title commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__set-window-title_commands] )) ||
_wezterm__cli__help__set-window-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help set-window-title commands' commands "$@"
}
(( $+functions[_wezterm__cli__set-window-title_commands] )) ||
_wezterm__cli__set-window-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli set-window-title commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__set-window-title_commands] )) ||
_wezterm__help__cli__set-window-title_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli set-window-title commands' commands "$@"
}
(( $+functions[_wezterm__help__set-working-directory_commands] )) ||
_wezterm__help__set-working-directory_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help set-working-directory commands' commands "$@"
}
(( $+functions[_wezterm__set-working-directory_commands] )) ||
_wezterm__set-working-directory_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm set-working-directory commands' commands "$@"
}
(( $+functions[_wezterm__help__shell-completion_commands] )) ||
_wezterm__help__shell-completion_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help shell-completion commands' commands "$@"
}
(( $+functions[_wezterm__shell-completion_commands] )) ||
_wezterm__shell-completion_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm shell-completion commands' commands "$@"
}
(( $+functions[_wezterm__help__show-keys_commands] )) ||
_wezterm__help__show-keys_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help show-keys commands' commands "$@"
}
(( $+functions[_wezterm__show-keys_commands] )) ||
_wezterm__show-keys_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm show-keys commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__spawn_commands] )) ||
_wezterm__cli__help__spawn_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help spawn commands' commands "$@"
}
(( $+functions[_wezterm__cli__spawn_commands] )) ||
_wezterm__cli__spawn_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli spawn commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__spawn_commands] )) ||
_wezterm__help__cli__spawn_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli spawn commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__split-pane_commands] )) ||
_wezterm__cli__help__split-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help split-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__split-pane_commands] )) ||
_wezterm__cli__split-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli split-pane commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__split-pane_commands] )) ||
_wezterm__help__cli__split-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli split-pane commands' commands "$@"
}
(( $+functions[_wezterm__help__ssh_commands] )) ||
_wezterm__help__ssh_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help ssh commands' commands "$@"
}
(( $+functions[_wezterm__ssh_commands] )) ||
_wezterm__ssh_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm ssh commands' commands "$@"
}
(( $+functions[_wezterm__help__start_commands] )) ||
_wezterm__help__start_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help start commands' commands "$@"
}
(( $+functions[_wezterm__start_commands] )) ||
_wezterm__start_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm start commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__tlscreds_commands] )) ||
_wezterm__cli__help__tlscreds_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help tlscreds commands' commands "$@"
}
(( $+functions[_wezterm__cli__tlscreds_commands] )) ||
_wezterm__cli__tlscreds_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli tlscreds commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__tlscreds_commands] )) ||
_wezterm__help__cli__tlscreds_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli tlscreds commands' commands "$@"
}
(( $+functions[_wezterm__cli__help__zoom-pane_commands] )) ||
_wezterm__cli__help__zoom-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli help zoom-pane commands' commands "$@"
}
(( $+functions[_wezterm__cli__zoom-pane_commands] )) ||
_wezterm__cli__zoom-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm cli zoom-pane commands' commands "$@"
}
(( $+functions[_wezterm__help__cli__zoom-pane_commands] )) ||
_wezterm__help__cli__zoom-pane_commands() {
    local commands; commands=()
    _describe -t commands 'wezterm help cli zoom-pane commands' commands "$@"
}

if [ "$funcstack[1]" = "_wezterm" ]; then
    _wezterm "$@"
else
    compdef _wezterm wezterm
fi
