#!/usr/bin/env python3
import sys
import os
import glob
import re
import subprocess


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
                idx.write(f"  - [{page.title}]({page.title}.md)\n")


def image_dimensions(filename):
    try:
        out = subprocess.check_output(["identify", filename])
        fields = out.split()
        while fields[0] != b"PNG":
            fields = fields[1:]
        return [int(x) for x in fields[1].split(b"x")]
    except FileNotFoundError:
        return [100, 100]


class GenColorScheme(object):
    def __init__(self, title, dirname, index=None):
        self.title = title
        self.dirname = dirname
        self.index = index

    def render(self, output, depth=0):
        names = sorted(glob.glob(f"{self.dirname}/*"))
        children = []
        for scheme_prefix in names:
            title = os.path.basename(scheme_prefix).rsplit(".", 1)[0]
            if title == "index":
                continue

            scheme_filename = f"{scheme_prefix}/index.md"
            children.append(Page(title, scheme_filename))

            with open(scheme_filename, "w") as idx:
                images = sorted(glob.glob(f"{scheme_prefix}/*.png"))
                for img in images:
                    width, height = image_dimensions(img)
                    img = os.path.basename(img)
                    title = os.path.basename(img).rsplit(".", 1)[0]
                    idx.write(f"# {title}\n")
                    idx.write(
                        f'<img width="{width}" height="{height}" src="{img}" alt="{title}">\n\n'
                    )
                    idx.write("To use this scheme, add this to your config:\n")
                    idx.write(
                        f"""
```lua
return {{
  color_scheme = "{title}",
}}
```

"""
                    )

        index_filename = f"{self.dirname}/index.md"
        index_page = Page(self.title, index_filename, children=children)
        index_page.render(output, depth)

        with open(f"{self.dirname}/index.md", "w") as idx:
            idx.write("Color schemes listed by first letter\n\n")
            for page in children:
                upper = page.title.upper()
                idx.write(f"  - [{upper}]({page.title}/index.md)\n")


TOC = [
    Page(
        "wezterm",
        "index.md",
        children=[
            Page(
                "Install",
                "installation.md",
                children=[
                    Page("Windows", "install/windows.md"),
                    Page("macOS", "install/macos.md"),
                    Page("Linux", "install/linux.md"),
                    Page("FreeBSD", "install/freebsd.md"),
                    Page("Build from source", "install/source.md"),
                ],
            ),
            Page("Features", "features.md"),
            Page("Change Log", "changelog.md"),
            Page(
                "Configuration",
                "config/files.md",
                children=[
                    Page("Launching Programs", "config/launch.md"),
                    Page("Fonts", "config/fonts.md"),
                    Page("Font Shaping", "config/font-shaping.md"),
                    Page("Key Binding", "config/keys.md"),
                    Page("Mouse Binding", "config/mouse.md"),
                    Page("Colors & Appearance", "config/appearance.md"),
                ],
            ),
            Page("Scrollback", "scrollback.md"),
            Page("Copy Mode", "copymode.md"),
            Page("Hyperlinks", "hyperlinks.md"),
            Page("Shell Integration", "shell-integration.md"),
            Page("iTerm Image Protocol", "imgcat.md"),
            Page("SSH", "ssh.md"),
            Page("Serial Ports & Arduino", "serial.md"),
            Page("Mulitplexing", "multiplexing.md"),
            Page("Escape Sequences", "escape-sequences.md"),
            Page("F.A.Q.", "faq.md"),
            Page("Getting Help", "help.md"),
            Page("Contributing", "contributing.md"),
            GenColorScheme("Color Schemes", "colorschemes"),
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
                        "struct: Config",
                        "config/lua/config",
                        index="""
# `Config` struct

The `return` statement at the end of your `wezterm.lua` file returns
a table that is interpreted as the internal `Config` struct type.

This section documents the various available fields in the config
struct.

At the time of writing, it is not a complete list!

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
                    Page("object: SshDomain", "config/lua/SshDomain.md"),
                    Page("object: SpawnCommand", "config/lua/SpawnCommand.md"),
                    Page("object: TlsDomainClient", "config/lua/TlsDomainClient.md"),
                    Page("object: TlsDomainServer", "config/lua/TlsDomainServer.md"),
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
