#!/usr/bin/env python3
import base64
import glob
import json
import os
import re
import subprocess
import sys


class Page(object):
    def __init__(self, title, filename, children=None):
        self.title = title
        self.filename = filename
        self.children = children or []

    def render(self, output, depth=0, mode="mdbook"):
        indent = "  " * depth
        bullet = "- " if depth > 0 else ""
        if mode == "mdbook":
            if self.filename:
                output.write(f"{indent}{bullet}[{self.title}]({self.filename})\n")
        elif mode == "mkdocs":
            if depth > 0:
                if len(self.children) == 0:
                    output.write(f'{indent}{bullet}"{self.title}": {self.filename}\n')
                else:
                    output.write(f'{indent}{bullet}"{self.title}":\n')
                    if self.filename:
                        output.write(
                            f'{indent}  {bullet}"{self.title}": {self.filename}\n'
                        )
        for kid in self.children:
            kid.render(output, depth + 1, mode)


# autogenerate an index page from the contents of a directory
class Gen(object):
    def __init__(self, title, dirname, index=None, extract_title=False):
        self.title = title
        self.dirname = dirname
        self.index = index
        self.extract_title = extract_title

    def render(self, output, depth=0, mode="mdbook"):
        names = sorted(glob.glob(f"{self.dirname}/*.md"))
        children = []
        for filename in names:
            title = os.path.basename(filename).rsplit(".", 1)[0]
            if title == "index":
                continue

            if self.extract_title:
                with open(filename, "r") as f:
                    title = f.readline().strip("#").strip()

            children.append(Page(title, filename))

        index_filename = f"{self.dirname}/index.md"
        index_page = Page(self.title, index_filename, children=children)
        index_page.render(output, depth, mode)
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
                idx.write(f"  - [{page.title}]({os.path.basename(page.filename)})\n")


def load_scheme(scheme):
    ident = re.sub(
        "[^a-z0-9_]", "_", scheme["metadata"]["name"].lower().replace("+", "plus")
    )

    if "ansi" not in scheme["colors"]:
        raise Exception(f"scheme {scheme} is missing ansi colors!!?")
    colors = scheme["colors"]["ansi"] + scheme["colors"]["brights"]

    data = {
        "name": scheme["metadata"]["name"],
        "prefix": scheme["metadata"]["prefix"],
        "ident": ident,
        "fg": scheme["colors"]["foreground"],
        "bg": scheme["colors"]["background"],
        "metadata": scheme["metadata"],
    }

    # <https://github.com/asciinema/asciinema-player/wiki/Custom-terminal-themes>
    css = f"""
.asciinema-theme-{ident} .asciinema-terminal {{
    color: {data["fg"]};
    background-color: {data["bg"]};
    border-color: {data["bg"]};
}}

.asciinema-theme-{ident} .fg-bg {{
    color: {data["bg"]};
}}

.asciinema-theme-{ident} .bg-fg {{
    background-color: {data["fg"]};
}}
"""

    if "cursor_border" in scheme["colors"]:
        data["cursor"] = scheme["colors"]["cursor_border"]

        css += f"""
.asciinema-theme-{ident} .cursor-b {{
    background-color: {data["cursor"]} !important;
}}
"""

    if "selection_fg" in scheme["colors"] and "selection_bg" in scheme["colors"]:
        selection_bg = scheme["colors"]["selection_bg"]
        selection_fg = scheme["colors"]["selection_fg"]

        css += f"""
.asciinema-theme-{ident} .asciinema-terminal ::selection {{
    color: {selection_fg};
    background-color: {selection_bg};
}}
"""

    for idx, color in enumerate(colors):
        css += f"""
.asciinema-theme-{ident} .fg-{idx} {{
    color: {color};
}}
.asciinema-theme-{ident} .bg-{idx} {{
    background-color: {color};
}}
"""

    data["css"] = css
    return data


def screen_shot_table(scheme):
    T = "gYw"
    lines = [
        scheme["name"],
        "",
        "         def     40m     41m     42m     43m     44m     45m     46m     47m",
    ]
    for fg_space in [
        "    m",
        "   1m",
        "  30m",
        "1;30m",
        "  31m",
        "1;31m",
        "  32m",
        "1;32m",
        "  33m",
        "1;33m",
        "  34m",
        "1;34m",
        "  35m",
        "1;35m",
        "  36m",
        "1;36m",
        "  37m",
        "1;37m",
    ]:
        fg = fg_space.strip()
        line = f" {fg_space} \033[{fg}  {T}  "

        for bg in ["40m", "41m", "42m", "43m", "44m", "45m", "46m", "47m"]:
            line += f" \033[{fg}\033[{bg}  {T}  \033[0m"
        lines.append(line)

    lines.append("")
    lines.append("")

    screen = "\r\n".join(lines)

    header = {
        "version": 2,
        "width": 80,
        "height": 24,
        "title": scheme["name"],
    }
    header = json.dumps(header, sort_keys=True)
    data = json.dumps([0.0, "o", screen])

    return base64.b64encode(f"{header}\n{data}\n".encode("UTF-8")).decode("UTF-8")


class GenColorScheme(object):
    def __init__(self, title, dirname, index=None):
        self.title = title
        self.dirname = dirname
        self.index = index

    def render(self, output, depth=0, mode="mdbook"):
        with open("colorschemes/data.json") as f:
            scheme_data = json.load(f)
        by_prefix = {}
        by_name = {}
        for scheme in scheme_data:
            scheme = load_scheme(scheme)
            prefix = scheme["prefix"]
            if prefix not in by_prefix:
                by_prefix[prefix] = []
            by_prefix[prefix].append(scheme)
            by_name[scheme["name"]] = scheme

        style_filename = f"{self.dirname}/scheme.css"
        with open(style_filename, "w") as style_file:
            for scheme in by_name.values():
                style_file.write(scheme["css"])
                style_file.write("\n")
        js_filename = f"{self.dirname}/scheme.js"
        with open(js_filename, "w") as js_file:
            data_by_scheme = {}
            for scheme in by_name.values():
                ident = scheme["ident"]
                data = screen_shot_table(scheme)
                data_by_scheme[ident] = data

            js_file.write(f"SCHEME_DATA = {json.dumps(data_by_scheme)};\n")
            js_file.write(
                f"""
function load_scheme_player(ident) {{
  var data = SCHEME_DATA[ident];
  AsciinemaPlayer.create(
    'data:text/plain;base64,' + data,
    document.getElementById(ident + '-player'), {{
    theme: ident,
    autoPlay: true,
  }});
}}
"""
            )

        children = []
        for scheme_prefix in sorted(by_prefix.keys()):
            scheme_filename = f"{self.dirname}/{scheme_prefix}/index.md"
            os.makedirs(os.path.dirname(scheme_filename), exist_ok=True)
            children.append(Page(scheme_prefix, scheme_filename))

            with open(scheme_filename, "w") as idx:
                idents_to_load = []

                idx.write(
                    f"""---
title: Color Schemes with first letter "{scheme_prefix}"
---

"""
                )

                for scheme in by_prefix[scheme_prefix]:
                    title = scheme["name"]
                    idx.write(f"## {title}\n")

                    data = screen_shot_table(scheme)
                    ident = scheme["ident"]
                    idents_to_load.append(ident)

                    idx.write(
                        f"""
<div id="{ident}-player"></div>
"""
                    )

                    author = scheme["metadata"].get("author", None)
                    if author:
                        idx.write(f"Author: `{author}`<br/>\n")
                    origin_url = scheme["metadata"].get("origin_url", None)
                    if origin_url:
                        idx.write(f"Source: <{origin_url}><br/>\n")
                    version = scheme["metadata"].get("wezterm_version", None)
                    if version and version != "Always":
                        idx.write(f"{{{{since('{version}')}}}}<br/>\n")

                    aliases = scheme["metadata"]["aliases"]
                    if len(aliases) > 0:
                        alias_list = []
                        for a in aliases:
                            alias_list.append(f"`{a}`")
                        aliases = ", ".join(alias_list)
                        idx.write(f"This scheme is also known as {aliases}.<br/>\n")

                    idx.write("\nTo use this scheme, add this to your config:\n")
                    idx.write(
                        f"""
```lua
config.color_scheme = '{title}'
```

"""
                    )

                idents_to_load = json.dumps(idents_to_load)
                idx.write(
                    f"""
<script>
document.addEventListener("DOMContentLoaded", function() {{
  {idents_to_load}.forEach(ident => load_scheme_player(ident));
}});
</script>
"""
                )

        index_filename = f"{self.dirname}/index.md"
        index_page = Page(self.title, index_filename, children=children)
        index_page.render(output, depth, mode)

        with open(f"{self.dirname}/index.md", "w") as idx:
            idx.write(f"{len(scheme_data)} Color schemes listed by first letter\n\n")
            for page in children:
                upper = page.title.upper()
                idx.write(f"  - [{upper}]({page.title}/index.md)\n")


TOC = [
    Page(
        "WezTerm",
        "index.md",
        children=[
            Page("Features", "features.md"),
            Page("Scrollback", "scrollback.md"),
            Page("Quick Select Mode", "quickselect.md"),
            Page("Copy Mode", "copymode.md"),
            Page("Hyperlinks", "hyperlinks.md"),
            Page("Shell Integration", "shell-integration.md"),
            Page("iTerm Image Protocol", "imgcat.md"),
            Page("SSH", "ssh.md"),
            Page("Serial Ports & Arduino", "serial.md"),
            Page("Multiplexing", "multiplexing.md"),
        ],
    ),
    Page(
        "Download",
        "installation.md",
        children=[
            Page("Windows", "install/windows.md"),
            Page("macOS", "install/macos.md"),
            Page("Linux", "install/linux.md"),
            Page("FreeBSD", "install/freebsd.md"),
            Page("Build from source", "install/source.md"),
        ],
    ),
    Page(
        "Configuration",
        "config/files.md",
        children=[
            Page("Colors & Appearance", "config/appearance.md"),
            Page("Launching Programs", "config/launch.md"),
            Page("Fonts", "config/fonts.md"),
            Page("Font Shaping", "config/font-shaping.md"),
            Page("Keyboard Concepts", "config/keyboard-concepts.md"),
            Page("Key Binding", "config/keys.md"),
            Page("Key Tables", "config/key-tables.md"),
            Page("Default Key Assignments", "config/default-keys.md"),
            Page("Keyboard Encoding", "config/key-encoding.md"),
            Page("Mouse Binding", "config/mouse.md"),
            GenColorScheme("Color Schemes", "colorschemes"),
            Gen("Recipes", "recipes", extract_title=True),
        ],
    ),
    Page(
        "Full Config & Lua Reference",
        "config/lua/general.md",
        children=[
            Gen(
                "Config Options",
                "config/lua/config",
            ),
            Gen(
                "module: wezterm",
                "config/lua/wezterm",
            ),
            Gen(
                "module: wezterm.color",
                "config/lua/wezterm.color",
            ),
            Gen(
                "module: wezterm.gui",
                "config/lua/wezterm.gui",
            ),
            Gen(
                "module: wezterm.mux",
                "config/lua/wezterm.mux",
            ),
            Gen(
                "module: wezterm.procinfo",
                "config/lua/wezterm.procinfo",
            ),
            Gen(
                "module: wezterm.serde",
                "config/lua/wezterm.serde",
            ),
            Gen(
                "module: wezterm.time",
                "config/lua/wezterm.time",
            ),
            Gen(
                "module: wezterm.url",
                "config/lua/wezterm.url",
            ),
            Gen(
                "enum: KeyAssignment",
                "config/lua/keyassignment",
            ),
            Gen(
                "enum: CopyModeAssignment",
                "config/lua/keyassignment/CopyMode",
            ),
            Gen("object: Color", "config/lua/color"),
            Page("object: ExecDomain", "config/lua/ExecDomain.md"),
            Page("object: LocalProcessInfo", "config/lua/LocalProcessInfo.md"),
            Gen("object: MuxDomain", "config/lua/MuxDomain"),
            Gen("object: MuxWindow", "config/lua/mux-window"),
            Gen("object: MuxTab", "config/lua/MuxTab"),
            Page("object: PaneInformation", "config/lua/PaneInformation.md"),
            Page("object: TabInformation", "config/lua/TabInformation.md"),
            Page("object: SshDomain", "config/lua/SshDomain.md"),
            Page("object: SpawnCommand", "config/lua/SpawnCommand.md"),
            Gen("object: Time", "config/lua/wezterm.time/Time"),
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
    Page(
        "CLI Reference",
        "cli/general.md",
        children=[
            Gen("wezterm cli", "cli/cli"),
            Page("wezterm connect", "cli/connect.md"),
            Page("wezterm imgcat", "cli/imgcat.md"),
            Page("wezterm ls-fonts", "cli/ls-fonts.md"),
            Page("wezterm record", "cli/record.md"),
            Page("wezterm replay", "cli/replay.md"),
            Page("wezterm serial", "cli/serial.md"),
            Page("wezterm set-working-directory", "cli/set-working-directory.md"),
            Page("wezterm show-keys", "cli/show-keys.md"),
            Page("wezterm ssh", "cli/ssh.md"),
            Page("wezterm start", "cli/start.md"),
        ],
    ),
    Page(
        "Reference",
        None,
        children=[
            Page("Escape Sequences", "escape-sequences.md"),
            Page("What is a Terminal?", "what-is-a-terminal.md"),
        ],
    ),
    Page(
        "Get Help",
        None,
        children=[
            Page("Troubleshooting", "troubleshooting.md"),
            Page("F.A.Q.", "faq.md"),
            Page("Getting Help", "help.md"),
            Page("Contributing", "contributing.md"),
        ],
    ),
    Page("Change Log", "changelog.md"),
    Page("Sponsor", "sponsor.md"),
]

os.chdir("docs")

with open("../mkdocs.yml", "w") as f:
    f.write("# this is auto-generated by docs/generate-toc.py, do not edit\n")
    f.write("INHERIT: docs/mkdocs-base.yml\n")
    f.write("nav:\n")
    for page in TOC:
        page.render(f, depth=1, mode="mkdocs")


with open("SUMMARY.md", "w") as f:
    f.write("[root](index.md)\n")
    for page in TOC:
        page.render(f, depth=1, mode="mdbook")
