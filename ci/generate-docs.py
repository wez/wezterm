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
    def __init__(self, title, dirname):
        self.title = title
        self.dirname = dirname

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
            Page(
                "Lua Reference",
                "config/lua/general.md",
                children=[
                    Gen("module: wezterm", "config/lua/wezterm"),
                    Gen("object: Pane", "config/lua/pane"),
                    Gen("object: Window", "config/lua/window"),
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
        ],
    )
]

os.chdir("docs")
with open("SUMMARY.md", "w") as f:
    for page in TOC:
        page.render(f)
