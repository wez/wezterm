#!/usr/bin/env python3
import json
import sys

def release_to_links(rel):
    source = None
    macos = None
    fedora = None
    ubuntu = None
    windows = None
    linux_bin = None

    for asset in rel["assets"]:
        url = asset["browser_download_url"]
        name = asset["name"]
        if "-src.tar.gz" in name:
            source = (url, name)
        elif ".deb" in name:
            ubuntu = (url, name)
        elif ".tar.xz" in name:
            linux_bin = (url, name)
        elif ".rpm" in name:
            fedora = (url, name)
        elif "WezTerm-macos-" in name:
            macos = (url, name)
        elif "WezTerm-windows-" in name:
            windows = (url, name)

    return {
        "source": source,
        "ubuntu": ubuntu,
        "linux_bin": linux_bin,
        "fedora": fedora,
        "macos": macos,
        "windows": windows,
    }


def load_release_info():
    with open("/tmp/wezterm.releases.json") as f:
        release_info = json.load(f)

    latest = release_info[0]
    nightly = None
    for rel in release_info:
        if rel["tag_name"] == "nightly":
            nightly = rel
            break

    latest = release_to_links(latest)
    nightly = release_to_links(nightly)

    subst = {}
    for (kind, (url, name)) in latest.items():
        subst["{{ %s_stable }}" % kind] = url
        subst["{{ %s_stable_asset }}" % kind] = name
    for (kind, (url, name)) in nightly.items():
        subst["{{ %s_pre }}" % kind] = url
        subst["{{ %s_pre_asset }}" % kind] = name

    with open("docs/installation.markdown", "r") as input:
        with open("docs/installation.md", "w") as output:
            for line in input:
                for (search, replace) in subst.items():
                    line = line.replace(search, replace)
                output.write(line)

def main():
    load_release_info()

main()
