#!/usr/bin/env python3
import json
import sys
import re

CATEGORIZE = {
    r".centos(\d+)(:?\S+)?.rpm$": "centos\\1_rpm",
    r".fedora(\d+)(:?\S+)?.rpm$": "fedora\\1_rpm",
    r".el(\d+).x86_64.rpm$": "centos\\1_rpm",
    r".fc(\d+).x86_64.rpm$": "fedora\\1_rpm",
    r".opensuse_leap(.*).rpm$": "opensuse_leap_rpm",
    r".opensuse_tumbleweed(.*).rpm$": "opensuse_tumbleweed_rpm",
    r"Debian(\d+)(\.\d+)?\.deb$": "debian\\1_deb",
    r"Ubuntu(\d+)(\.\d+)?.AppImage$": "ubuntu\\1_AppImage",
    r"Ubuntu(\d+)(\.\d+)?.deb$": "ubuntu\\1_deb",
    r"Ubuntu18.04.tar.xz$": "linux_raw_bin",
    r"^wezterm-\d+-\d+-[a-f0-9]+.tar.xz$": "linux_raw_bin",
    r"src.tar.gz$": "src",
    r"^WezTerm-macos-.*.zip$": "macos_zip",
    r"^WezTerm-windows-.*.zip$": "windows_zip",
    r"^WezTerm-.*.setup.exe$": "windows_exe",
    r"alpine(\d+)\.(\d+)(:?-\S+)?.apk": "alpine\\1_\\2_apk",
}


def categorize(rel):
    downloads = {}

    tag_name = "wezterm-%s" % rel["tag_name"]
    for asset in rel["assets"]:
        url = asset["browser_download_url"]
        name = asset["name"]

        for k, v in CATEGORIZE.items():
            matches = re.search(k, name)
            if matches:
                v = matches.expand(v)
                downloads[v] = (url, name, tag_name)

    return downloads


def pretty(o):
    return json.dumps(o, indent=4, sort_keys=True, separators=(",", ":"))


def build_subst(subst, stable, categorized):
    for (kind, info) in categorized.items():
        if info is None:
            continue
        url, name, dir = info
        kind = f"{kind}_{stable}"
        subst["{{ %s }}" % kind] = url
        subst["{{ %s_asset }}" % kind] = name
        subst["{{ %s_dir }}" % kind] = dir


def load_release_info():
    with open("/tmp/wezterm.releases.json") as f:
        release_info = json.load(f)

    with open("/tmp/wezterm.nightly.json") as f:
        nightly = json.load(f)

    latest = None
    for rel in release_info:
        if rel["prerelease"]:
            continue
        latest = rel
        break

    latest = categorize(latest)
    nightly = categorize(nightly)

    print("latest: ", pretty(latest))
    print("nightly: ", pretty(nightly))

    subst = {}
    build_subst(subst, "stable", latest)
    build_subst(subst, "nightly", nightly)
    print(pretty(subst))

    for name in [
        "install/windows",
        "install/macos",
        "install/linux",
        "install/source",
        "install/freebsd",
    ]:
        with open(f"docs/{name}.markdown", "r") as input:
            with open(f"docs/{name}.md", "w") as output:
                for line in input:
                    for (search, replace) in subst.items():
                        line = line.replace(search, replace)
                    output.write(line)


def main():
    load_release_info()


main()
