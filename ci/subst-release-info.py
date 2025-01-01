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
    r"Ubuntu(\d+)(\.\d+)?\.arm64\.deb$": "ubuntu\\1_arm64_deb",
    r"Debian(\d+)(\.\d+)?\.arm64\.deb$": "debian\\1_arm64_deb",
    r"Ubuntu20.04.tar.xz$": "linux_raw_bin",
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
    for kind, info in categorized.items():
        if info is None:
            continue
        url, name, dir = info
        kind = f"{kind}_{stable}"
        subst[kind] = url
        subst[f"{kind}_asset"] = name
        subst[f"{kind}_dir"] = dir


def load_release_info():
    with open("/tmp/wezterm.releases.json") as f:
        release_info = json.load(f)

    with open("/tmp/wezterm.nightly.json") as f:
        nightly = json.load(f)

    latest = None
    for rel in release_info:
        if type(rel) is str:
            print("Error", pretty(release_info))
            raise Exception("Error obtaining release info")

        # print(pretty(rel))
        if rel["prerelease"]:
            continue
        latest = rel
        break

    latest = categorize(latest)
    nightly = categorize(nightly)

    # print("latest: ", pretty(latest))
    # print("nightly: ", pretty(nightly))

    subst = {}
    build_subst(subst, "stable", latest)
    build_subst(subst, "nightly", nightly)

    with open(f"docs/releases.json", "w") as output:
        json.dump(subst, output)


def main():
    load_release_info()


main()
