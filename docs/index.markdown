---
title: Wez's Terminal
---

*A GPU-accelerated cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez/">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

<div>

{% for asset in site.github.latest_release.assets %}
  {% if asset.name contains 'azure' %}
    {% if asset.name contains 'windows' %}
  <a href="{{ asset.browser_download_url }}" class="btn" style="margin-right:1em">{% octicon cloud-download %} Download for Windows</a>
    {% endif %}
    {% if asset.name contains 'macos' %}
  <a href="{{ asset.browser_download_url }}" class="btn" style="margin-right:1em">{% octicon cloud-download %} Download for macOS</a>
    {% endif %}
  {% endif %}
{% endfor %}

    <a href="installation.html">Linux and other installation instructions</a>
</div>

## Features

* Runs on Linux, macOS and Windows 10
* [Multiplex terminal tabs and windows on local and remote hosts, with native mouse and scrollback](multiplexing.html)
* <a href="https://github.com/tonsky/FiraCode#fira-code-monospaced-font-with-programming-ligatures">Ligatures</a>, Color Emoji and font fallback, with true color and [dynamic color schemes](configuration.html#colors).
* <a href="https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda">Hyperlinks</a>
* <a href="features.html">a full list of features can be found here</a>

Looking for a [configuration reference?](configuration.html)

![Screenshot](screenshots/two.png)

*Screenshot of wezterm on macOS, running vim*

