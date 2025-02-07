# Privacy Policy for WezTerm

No data about your device(s) or WezTerm usage leave your device.

## Data Maintained by WezTerm

WezTerm maintains some historical data, such as recent searches or action
usage, in some of its overlays such as the debug overlay and character
selector, in order to make your usage more convenient. It is used only
by the local process, and care is taken to limit access for the associated
files on disk to only your local user identity.

WezTerm tracks the output from the commands that you have executed in
a scrollback buffer.  At the time of writing, that scrollback buffer
is an in-memory structure that is not visible to other users of the machine.
In the future, if wezterm expands to offload scrollback information to
your local disk, it will do so in such a way that other users on the
same system will not be able to inspect it.

## macOS and Data permissions

On macOS, when a GUI application that has a "bundle" launches child processes
(eg: WezTerm, running your shell, and your shell running the programs which you
direct it to run), any permissioned resource access that may be attempted by
those child processes will be reported as though WezTerm is attempting to
access those resources.

The result is that from time to time you may see a dialog about WezTerm
accessing your Contacts if run a `find` command that happens to step through
the portion of your filesystem where the contacts are stored.  Or perhaps you
are running a utility that accesses your camera; it will appear as though
WezTerm is accessing those resources, but it is not: there is no logic within
WezTerm to attempt to access your contacts, camera or any other sensitive
information.

## Update Checking

By default, once every 24 hours, wezterm makes an HTTP request to GitHub's
release API in order to determine if a newer version is available and to
notify you if that is the case.

The content of that request is private between your machine and GitHub.  The
contributors to WezTerm cannot see inside that request and therefore cannot
infer any information from it.

If you wish, you can disable update checking. See
https://wezterm.org/config/lua/config/check_for_updates.html for
more information on that.

## Third-Party Builds

The above is true of the wezterm source code and the binaries produced by
wezterm's CI and made available from https://wezterm.org/ and
https://github.com/wezterm/wezterm/.

If you obtained a pre-built wezterm binary from some other source be aware that
the person(s) building those versions may have modified them to behave
differently from the source version.
