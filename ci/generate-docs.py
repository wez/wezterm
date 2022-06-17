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
            else:
                try:
                    with open(f"{self.dirname}/index.markdown", "r") as f:
                        idx.write(f.read())
                        idx.write("\n\n")
                except FileNotFoundError:
                    pass
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
                    Page("Keyboard Concepts", "config/keyboard-concepts.md"),
                    Page("Key Binding", "config/keys.md"),
                    Page("Key Tables", "config/key-tables.md"),
                    Page("Default Key Assignments", "config/default-keys.md"),
                    Page("Mouse Binding", "config/mouse.md"),
                    Page("Colors & Appearance", "config/appearance.md"),
                    GenColorScheme("Color Schemes", "colorschemes"),
                ],
            ),
            Page("Scrollback", "scrollback.md"),
            Page("Quick Select Mode", "quickselect.md"),
            Page("Copy Mode", "copymode.md"),
            Page("Hyperlinks", "hyperlinks.md"),
            Page("Shell Integration", "shell-integration.md"),
            Page("iTerm Image Protocol", "imgcat.md"),
            Page("SSH", "ssh.md"),
            Page("Serial Ports & Arduino", "serial.md"),
            Page("Multiplexing", "multiplexing.md"),
            Page("Escape Sequences", "escape-sequences.md"),
            Page("F.A.Q.", "faq.md"),
            Page("Getting Help", "help.md"),
            Page("Contributing", "contributing.md"),
            Page("CLI Reference", "cli/general.md", children=[Gen("cli", "cli/cli")]),
            Page(
                "Lua Reference",
                "config/lua/general.md",
                children=[
                    Gen(
                        "module: wezterm",
                        "config/lua/wezterm",
                    ),
                    Gen(
                        "module: wezterm.mux",
                        "config/lua/wezterm.mux",
                    ),
                    Gen(
                        "struct: Config",
                        "config/lua/config",
                    ),
                    Gen(
                        "enum: KeyAssignment",
                        "config/lua/keyassignment",
                    ),
                    Page("object: LocalProcessInfo", "config/lua/LocalProcessInfo.md"),
                    Page("object: MuxWindow", "config/lua/MuxWindow.md"),
                    Page("object: MuxTab", "config/lua/MuxTab.md"),
                    Page("object: MuxPane", "config/lua/MuxPane.md"),
                    Page("object: PaneInformation", "config/lua/PaneInformation.md"),
                    Page("object: TabInformation", "config/lua/TabInformation.md"),
                    Page("object: SshDomain", "config/lua/SshDomain.md"),
                    Page("object: SpawnCommand", "config/lua/SpawnCommand.md"),
                    Page("object: TlsDomainClient", "config/lua/TlsDomainClient.md"),
                    Page("object: TlsDomainServer", "config/lua/TlsDomainServer.md"),
                    Gen(
                        "object: Pane",
                        "config/lua/pane",
                    ),
                    Gen(
                        "object: Window",
                        "config/lua/window",
                    ),
                    Page("object: WslDomain", "config/lua/WslDomain.md"),
                    Gen(
                        "events: Gui",
                        "config/lua/gui-events",
                    ),
                    Gen(
                        "events: Multiplexer",
                        "config/lua/mux-events",
                    ),
                    Gen(
                        "events: Window",
                        "config/lua/window-events",
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
