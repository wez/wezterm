#!/usr/bin/env python3
import sys
import os
import glob
import re


class Page(object):
    def __init__(self, title, filename, children=None):
        self.title = title
        self.filename = filename
        self.children = children or []

    def render(self, output, depth=0):
        indent = "  " * depth
        bullet = "- " if depth > 0 else ""
        output.write(f"{indent}{bullet}[{self.title}]({self.filename})\n")
        for kid in self.children:
            kid.render(output, depth + 1)


# autogenerate an index page from the contents of a directory
class Gen(object):
    def __init__(self, title, dirname, index=None):
        self.title = title
        self.dirname = dirname
        self.index = index

    def render(self, output, depth=0):
        print(self.dirname)
        names = sorted(glob.glob(f"{self.dirname}/*.md"))
        children = []
        for filename in names:
            title = os.path.basename(filename).rsplit(".", 1)[0]
            if title == "index":
                continue
            children.append(Page(title, filename))

        index_filename = f"{self.dirname}/index.md"
        index_page = Page(self.title, index_filename, children=children)
        index_page.render(output, depth)
        with open(f"{self.dirname}/index.md", "w") as idx:
            if self.index:
                idx.write(self.index)
                idx.write("\n\n")
            for page in children:
                page.render(idx, 1)


TOC = [
    Page(
        "wezterm",
        "index.markdown",
        children=[
            Page(
                "Install",
                "installation.md",
                children=[
                    Page("Windows", "install/windows.md"),
                    Page("macOS", "install/macos.md"),
                    Page("Linux", "install/linux.md"),
                    Page("Build from source", "install/source.md"),
                ],
            ),
            Page("Features", "features.markdown"),
            Page("Change Log", "changelog.markdown"),
            Page(
                "Configuration",
                "config/files.markdown",
                children=[
                    Page("Launching Programs", "config/launch.markdown"),
                    Page("Fonts", "config/fonts.markdown"),
                    Page("Font Shaping", "config/font-shaping.markdown"),
                    Page("Misc", "config/misc.markdown"),
                    Page("Key & Mouse Binding", "config/keys.markdown"),
                    Page("Colors & Appearance", "config/appearance.markdown"),
                ],
            ),
            Page("Scrollback", "scrollback.markdown"),
            Page("Copy Mode", "copymode.markdown"),
            Page("Hyperlinks", "hyperlinks.markdown"),
            Page("Shell Integration", "shell-integration.markdown"),
            Page("iTerm Image Protocol", "imgcat.markdown"),
            Page("SSH", "ssh.markdown"),
            Page("Serial Ports & Arduino", "serial.markdown"),
            Page("Mulitplexing", "multiplexing.markdown"),
            Page("F.A.Q.", "faq.markdown"),
            Page("Getting Help", "help.markdown"),
            Page("Contributing", "contributing.markdown"),
            Page(
                "Lua Reference",
                "config/lua/general.md",
                children=[
                    Gen(
                        "module: wezterm",
                        "config/lua/wezterm",
                        index="""
# `require wezterm`

The wezterm module is the primary module that exposes wezterm configuration
and control to your config file.

You will typically place:

```lua
local wezterm = require 'wezterm';
```

at the top of your configuration file to enable it.

## Available functions, constants
""",
                    ),
                    Gen(
                        "enum: KeyAssignment",
                        "config/lua/keyassignment",
                        index="""
# `KeyAssignment` enumeration

A `KeyAssignment` represents a pre-defined function that can be applied
to control the Window, Tab, Pane state typically when a key or mouse event
is triggered.

Internally, in the underlying Rust code, `KeyAssignment` is an enum
type with a variant for each possible action known to wezterm.  In Lua,
enums get represented as a table with a single key corresponding to
the variant name.

In most cases the [`wezterm.action`](../wezterm/action.md) function is
used to create an instance of `KeyAssignment` and make it a bit more
clear and convenient.

## Available Key Assignments

""",
                    ),
                    Gen(
                        "object: Pane",
                        "config/lua/pane",
                        index="""
# `Pane` object

A Pane object cannot be created in lua code; it is typically passed to your
code via an event callback.  A Pane object is a handle to a live instance of a
Pane that is known to the wezterm process.  A Pane object tracks the psuedo
terminal (or real serial terminal) and associated process(es) and the parsed
screen and scrollback.

A Pane object can be used to send input to the associated processes and
introspect the state of the terminal emulation for that pane.

## Available methods

""",
                    ),
                    Gen(
                        "object: Window",
                        "config/lua/window",
                        index="""
# `Window` object

A Window object cannot be created in lua code; it is typically passed to
your code via an event callback.  A Window object is a handle to a GUI
TermWindow running in the wezterm process.

## Available methods

""",
                    ),
                ],
            ),
        ],
    )
]

os.chdir("docs")
with open("SUMMARY.md", "w") as f:
    for page in TOC:
        page.render(f)
