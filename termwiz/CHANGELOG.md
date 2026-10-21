# Changelog

All notable changes to this project will be documented in this file.

## [termwiz-0.23.3] - 2025-03-20

### 💼 Other

- Update changelog
- Set workspace pest dep to 2.7

## [termwiz-0.23.2] - 2025-03-19

### 🐛 Bug Fixes

- Tmux -CC error on %config-error (#6773)
- Deal with Windows Terminal mouse move with no buttons SGR report

### 💼 Other

- Add update-changelog.sh
- Fix panic when appending long runs to clusterline
- Update for Unicode 16
- Update for unicode 16
- Add is_white_space_(char|grapheme) helper functions
- Bump version

## [termwiz-0.23.0] - 2025-02-10

### 🚀 Features

- Add a way to spawn populated LineEditor

### 🐛 Bug Fixes

- Better handling of hyperlinks with parentheses

### 💼 Other

- Support NO_COLOR environment variable (#5020)
- Update criterion
- Update reqwest to 0.12
- Speculative compile fix for termwiz
- Dedup some nix versions
- Update/dedup num, num-derive
- Nix requires fs + mman for shm_open
- Update terminfo
- Remove semver dep
- Fixup per code review
- Fixup regex range
- Parse ConEmu progress OSC sequence
- Prep for making a new release
- Update dynamic dep for publish

## [termwiz-0.22.0] - 2024-01-27

### 🐛 Bug Fixes

- Update bitflags to 2.0 and fix compilation errors
- Update signal-hook to 0.3 and fix compilation errors
- Formatting
- Use serde with bitflags
- Derive Eq on Selection
- Issue 3935 handle F13-F24 (#3937)

### 💼 Other

- :File -> EncodedLease
- Update sha2
- Plumb more modifier+state information through
- Improve support for numpad buttons
- :Modifiers is now wezterm_input_types::Modifiers
- Ignore caps/num lock when encoding keys in xterm encoding
- Move led status to separate enum
- Input: avoid panic for certain numpad keys
- Use double height text for host key failure case
- Windows: speculative build fix
- Work better with tmux and conpty
- Refactor terminal probing
- Fix windows build
- Adjust screensize probing
- TeenyString: use heap string when col width > 2
- Ordered-float -> 4.1.0
- Add terminator to Sync capability
- Update env-logger
- Surface: Fix cell diffing in presence of wide cells
- Surface: Fix cursor movement in DiffState
- Bump version of crates.io publish
- Prep for crates.io publish

### 🚜 Refactor

- Dedup ctrl_mapping function
- Move encode_modifiers to Modifiers::encode_xterm

### 📚 Documentation

- Shift from return {} style to config.something style

### ⚙️ Miscellaneous Tasks

- Update phf to 0.11
- Update nix to 0.26
- Update criterion to 0.4

## [termwiz-0.20.0] - 2023-02-12

### 🐛 Bug Fixes

- Correctly set WHEEL_POSITIVE
- *(input)* Support alt-left-bracket

### 💼 Other

- Add statusline entries
- Update base64
- Horizontal scroll support
- Update Symbols Nerd Font Mono
- Bump version ready for publish

### ⚡ Performance

- Adjust clustering when bidi is disabled

## [termwiz-0.19.0] - 2022-11-02

### 🐛 Bug Fixes

- Temp fix for Android build error

### 💼 Other

- Request xterm modifyOtherKeys
- Update widechar_width for unicode 15
- Fixup for 32-bit systems
- Upgrade finl_unicode to 1.2
- Release 0.19

## [termwiz-0.18.0] - 2022-09-22

### 💼 Other

- Associate appdata with a Line
- Remove reverse video attribute from Line
- Use interior mutability for Line::set_appdata
- Remove assertions
- Slim down size of clustered line storage
- Add test to track size_of Action
- Save 8 bytes per line
- Track size of sgr enum
- Update finl_unicode
- Update nerdfont symbol data
- Fix recognizing \u{1b}[>4;m as modifyOtherKeys
- Ignore various unsupported private mode codes
- Recognize some dec private SGR codes
- Add flag to force use of standard ANSI SGR codes
- Prepare for a 0.18.0 release

### ⚡ Performance

- Cache quads by line

## [termwiz-0.17.1] - 2022-08-03

### 💼 Other

- Fix min version dep on winapi. publish 0.17.1

## [termwiz-0.17.0] - 2022-08-02

### 🐛 Bug Fixes

- Properly restore cooked mode in windows

### 💼 Other

- Recognize utf8 encoded c1 codes in more cases
- Update to use newer mouse protocol by default
- Remove anyhow from public interface
- Ordered-float 2.1 -> 2.5
- Avoid file io when terminfo db is pre-filled
- Erase_cell could panic if erasing beyond EOL
- Respect Dec Private Mode 8452
- Hide internal red, green, blue fields
- Store 10bpc
- Move fg/bg color accesses to accessors
- Split color into thin/fat components
- Cargo update, and a couple of dependabot suggestions
- Improve CSI parsing fidelity
- Simplify csi dispatch
- Recognize the XTGETTCAP DCS sequence
- Fixup regression in sgr parsing for empty parameter case
- Parse APC sequences
- Parse and encode kitty image protocol
- Add KittyImageData::load_data
- Add ImageData::hash
- Change image frame caching scheme to use frame hash
- Better fidelity Emoji_Presentation logic
- Break clusters when presentation varies
- Fixup window transparency
- Replace LineBits::DIRTY with a sequence number
- Update to include more tmux extensions
- Fix test build when image feature is disabled
- Improve performance of emoji presentation lookup
- Fix off-by-one in computing line length
- Restore line length pre-allocation
- Update unicode-segmentation to 1.8
- Fix lost sync with the cell position in the renderer
- Update bitflags -> 1.3
- Move key encoding to termwiz
- Fix DCH removing cells instead of setting to current bg color
- Ordered-float -> 2.8
- Fix encoding F1-F4
- Fix shaping metrics
- Fancier tab bar
- Allow degenerate empty parameter in iTerm2 protocol
- Import widechar_width wcwidth implementation
- Fix 10bpc color bug. Allow hsl color specs
- Allow arbitrary whitespace between hsl color components
- S/Fuschia/Fuchsia/g
- Windows: force CP_UTF8, fix alt-screen and examples
- Publish 0.15
- Introduce right&bottom padding to ImageCell
- Fix overinvalidation of selection on Windows
- Make seqno a required param for Line
- Clamp grapheme column width to 2
- Sync with upstream wichdechar_width
- Introduce KeyboardEncoding enum, existence of win32-input-mode
- Generate win32-input-mode for a subset of keys
- Fix modifier only win32-input-mode reporting
- Add Backspace to win32-input-mode list
- Revise win32-input-mode flow
- Ordered-float -> 2.10
- Capture initial screen contents
- Fix mouse reporting on windows
- Recognize some bidi-related escape sequences
- Allow harfbuzz to guess the script
- Add experimental_bidi config option and very basic bidi
- Feed reordered runs through to harfbuzz
- Tag Line with bidi mode
- Re-order numerics so that 10 comes after 1 etc.
- Image -> 0.24
- Update widechar_width
- Allow slightly poorly formed sixel data
- Improve sixel parsing performance.
- Remove leftover debug
- Improve handling of image attachments
- Add test for partial bracketed paste
- Fix partial bracketed paste panic
- Add comment about bracketed paste offset
- Prep for crates.io
- Fix code formatting
- Remove pretty_env_loggger
- Treat graphemes in Unassigned range as width=1 rather than 0
- Fixup test build for ambiguous width change
- Use perfect hashing for emoji variation sequences
- Ordered-float
- Replace pretty_assertions dep with k9
- Nix -> 0.24
- Avoid panic with a malformed escape
- Cut over to wezterm-dynamic
- Allow specifying the animation speed for images
- Implement OSC 1337 ReportCellSize
- Add kitty keyboard CSI control escapes
- Support kitty keyboard protocol
- Fix key up events in neovim + kitty keyboard protocol
- Increase MAX_OSC to load dynamic color scripts
- Fix encoding for modified F1-F4
- Make emoji presentation very slightly more efficient
- Add criterion benches for wcwidth
- Micro-optimize grapheme_column_width
- Add static WcLookupTable to codegen
- Add bench for Cell creation/drop
- Refactor Line::visible_cells()
- Introduce possibility of alternate cell backing
- Add clustered line storage for line
- :as_str() -> Cow<str>
- Default to cluster storage
- Avoid cluster -> vec conversions in a few more cases
- Don't claim that visible_cells is double-ended
- Micro-optimize ClusteredLine::new(), set_last_wrapped
- Microoptimize ClusteredLine::set_last_cell_was_wrapped
- Microoptimize set cell
- Refactor getting logical lines
- :wrap now prefers cluster storage
- Refactor: split line into sub-modules
- ColorSpec now allows for alpha to be tracked
- Allow setting alpha for SGR fg, bg attributes
- Use `6` for the rgba colorspace
- Add test cases for parse_first_as_vec w/ OSC+ST
- Ensure ST sequence is grouped with OSC
- Bump version to 0.17

### 🚜 Refactor

- Move cursor sprite generation to be dynamic
- Move color parsing into wezerm-color-types crate
- Move sixel parser/builder to own file

### 📚 Documentation

- Embed nerd fonts symbols on the nerdfonts page

## [termwiz-0.13.0] - 2021-04-14

### 💼 Other

- Add example program that strips escape sequences
- Fix a widget layout and optimization issue
- Fix panic in RgbColor::from_rgb_str w/ empty string
- Fixup DECKPAM/DECANM/DECCKM interaction
- Enable scrollback in the error window
- Recognize wezterm as supporting iterm2 images
- Implement window/pixel size responses
- Fix DCS parsing
- Fix dec private mode parsing
- Shift Boxing of DCS around
- Fix swapped width/height in size reporting sequences
- Add types for sixel parsing
- Sixel rendering basically working
- Add dec private mode 1070 for sixel color map control
- Add CursorVisibility
- Separate CursorVisibility from CursorShape
- Trim `num` dep to `num_traits`
- Make serde an optional dep
- Upgrade unicode-segmentation
- Also allow `#FFF` form of color spec
- Improve esctest BS test conformance
- Improve rgb color parsing conformance with XParseColor
- Improve CSI parsing conformance
- Fix hyperlink matching issue with double wide chars
- Improved emulation conformance in a number of areas
- Recognize CSI > PP ; Pv m sequence
- Improve Debug impl for EnterDeviceControlMode
- Add Display impl for device control mode
- Add basic support for DECRQSS
- Add Overline support
- Allow OperatingSystemCommandCode to have non-numeric mappings
- Handle OSC L and OSC l
- More freebsd compat
- Implement leader key binding support
- Fix bounds checking in Line::compute_double_click_range
- Add a test to assert size of cell structs
- Reduce Cell memory consumption by 24 bytes
- Save 8 bytes per Cell in common case
- Fixup windows tests after b3f51e8ee20fe8431d3a75e374dae1c149840a84
- Add a way to render previews
- Avoid panic for some malformed escapes
- Add OSC 133 Semantic Marker sequences
- Add Cell::semantic_type
- Add ScrollToPrompt key assignment
- Fixup sixel width calculation
- Normalize the lazy-static version
- Deallocate "fat" attrs if all are none
- Improve shaping of emoji
- Improve fallback font scaling
- Allow optional width and height specifier
- Ordered-float -> 2.0
- Misc updates
- Allow for CSI parameters to be : separated
- Remove anyhow::Result from public API
- Fixup win32 build
- Really fixup windows build
- Update to ordered-float 2.1
- Add window::effective_config() method
- Add wezterm.format function
- Improve restoring cursor visibility under tmux
- Alternate plan for restoring cursor visibility in tmux
- Revert tmux workarounds
- Use cnorm instead of cvvis for CursorVisibility::Visible
- Shrink-to-fit Line::cells when clearing the line
- Remove redundant semicolons
- Fix some clippy stuff
- Do not use terminfo for unsupported 256-colors
- Prep for release

### 📚 Documentation

- Start documenting supporting escape sequences

## [termwiz-0.9.0] - 2020-05-17

### 💼 Other

- Allow using terminfo on Windows
- Ensure virtual terminal processing is enabled on windows
- Remove Position::NoChange, fixup multiline line editing and moar!
- Improve performance of windows console renderer
- Line editor: fixup cursor positioning for multiline
- Windows: fix bounds check for cursor positioning
- Windows: tidy up flushing a bit
- Windows: fixup viewport handling
- Windows: auto-detect virtual terminal support
- Windows: allow ESC to be recognized again
- Recognize MS terminal mode 25 for cursor visibility
- Line editor: allow for multi-line prompts
- Windows: toggle autonewline when toggling cooked/raw
- Line editor: exit completion state for a single completion result
- Line editor: allow application cursor keys
- History::get() -> Option<Cow<str>>
- Line editor: allow embedded app to override key map
- Line editor: allow custom editor actions
- Line editor: add incremental history search
- Line editor: add ChangeSequence helper
- Line editor: fix a couple of bugs
- Process OSC 110+: ResetDynamicColors
- Use long form ST when rendering OSC
- Fix categorization of F keys
- Cut 0.9.0 release

## [termwiz-0.7.1] - 2020-04-04

### 💼 Other

- Fix poll interval.  show update indicator
- Windows: fix default text foreground color

## [termwiz-0.7.0] - 2020-02-22

### 💼 Other

- Lineedit: replace a println with render/flush
- Make the E's show up on the cursor positioning test
- Remove dep on palette
- Do not depend on derive_builder
- Bump regex to 1.0+
- Bump version for publish

## [termwiz-0.6.0] - 2020-01-18

### 💼 Other

- Fix a bug where the cursor style wasn't restore on exit
- Fixup input parser to match backspace/delete
- Fixup test case for 18bbd2ac6f69973ab9961e82f54a1e318dfbf6d5
- Windows: normalize SHIFT+ASCII

## [termwiz-0.5.0] - 2019-12-22

### 💼 Other

- Extract configuration to a trait

## [termwiz-0.4.0] - 2019-06-30

### 💼 Other

- Update version

## [termwiz-0.3.1] - 2019-06-03

### 💼 Other

- Avoid emitting a resize event on a spurious sigwinch read
- Avoid emitting a wake event on a spurious pipe wakeup
- Ensure that the tty is in blocking mode
- Bump version for crates.io/streampager

## [termwiz-0.3.0] - 2019-06-02

### 💼 Other

- Use filedescriptor crate instead of RawFd bits
- Use filedescriptor crate instead of RawHandle bits

## [termwiz-0.1.0] - 2019-05-28

### 💼 Other

- :contains is now stable; use it.
- Fixup key_tester example on macOS
- Add very basic line editor
- Improve line editor
- Add ctrl-L repaint/refresh binding
- Split out action and movement concepts
- Implement ctrl-k and fixup deletion of emoji
- Add word movement commands
- Add ctrl-w to delete word up to cursor
- Ctrl-c cancels the current line
- Introduce host concept, prompt and coloring
- Add history
- Add ctrl-d -> EOF
- Add tab completion support
- Prep for publishing on crates.io

<!-- generated by git-cliff -->
