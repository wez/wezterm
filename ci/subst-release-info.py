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
    appimage = None

    tag_name = "wezterm-%s" % rel["tag_name"]

    for asset in rel["assets"]:
        url = asset["browser_download_url"]
        name = asset["name"]
        if "-src.tar.gz" in name:
            source = (url, name, tag_name)
        elif ".deb" in name:
            ubuntu = (url, name, tag_name)
        elif ".tar.xz" in name:
            linux_bin = (url, name, tag_name)
        elif ".rpm" in name:
            fedora = (url, name, tag_name)
        elif "WezTerm-macos-" in name:
            macos = (url, name, tag_name)
        elif "WezTerm-windows-" in name:
            windows = (url, name, tag_name)
        elif ".AppImage" in name:
            appimage = (url, name, tag_name)

    return {
        "source": source,
        "ubuntu": ubuntu,
        "linux_bin": linux_bin,
        "fedora": fedora,
        "macos": macos,
        "windows": windows,
        "appimage": appimage,
    }

def pretty(o):
    return json.dumps(o, indent=4, sort_keys=True, separators=(',', ':'))

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

    print('latest: ', pretty(latest))
    print('nightly: ', pretty(nightly))

    subst = {}
    for (kind, info) in latest.items():
        if info is None:
            continue
        url, name, dir = info
        subst["{{ %s_stable }}" % kind] = url
        subst["{{ %s_stable_asset }}" % kind] = name
        subst["{{ %s_stable_dir }}" % kind] = dir

    with open("docs/installation.markdown", "r") as input:
        with open("docs/installation.md", "w") as output:
            for line in input:
                for (search, replace) in subst.items():
                    line = line.replace(search, replace)
                output.write(line)

def main():
    load_release_info()

main()
