# Wez's Terminal

## This fork: Windows IME nightly

[Download the Windows x64 IME nightly](https://github.com/CoderTCY/wezterm/releases/tag/ime-nightly):
`WezTerm-nightly-setup.exe` (installer) or `WezTerm-windows-nightly.zip`
(portable), with SHA-256 checksums and `build-info.txt`. These are unofficial,
unsigned builds; Windows SmartScreen may warn.

The [IME nightly workflow](https://github.com/CoderTCY/wezterm/actions/workflows/wezterm-ime-nightly.yml)
lives on `main`. It runs on pushes to `main`, on manual dispatch, and daily
(`17 3 * * *`; GitHub may delay). It checks upstream `main` and
`fix/8229-windows-ime-caret` and **never pushes to either branch**. The Windows
runner checks out the patch branch (or upstream when the PR is merged), merges
upstream in the runner, and builds that tree. The patch branch is read-only:
update or rebase it yourself; [upstream PR #8231](https://github.com/wezterm/wezterm/pull/8231)
is no longer updated by the bot. A push to the patch branch cannot trigger this
workflow (the workflow file is intentionally absent from that branch, so the PR
does not expose it); the next daily run picks the change up, or run the
workflow manually for an immediate build. Do not force-reset/sync this fork to
upstream: that would discard fork-only commits, including this workflow.

A runner-local `merge=union` rule preserves both sides of changelog additions;
any source-code conflict fails the build instead of silently choosing one side.
The patch is never merged into fork `main`, and fork `main` is not auto-synced;
sync it manually if you want. The workflow also fails if the patch branch tree
contains `.github/workflows/wezterm-ime-nightly.yml`, so the PR cannot expose it.

Successful builds update the rolling prerelease `ime-nightly`. Its tag points
at the patch branch tip (or fork `main` after the PR is merged), not at the
runner-only merge commit, so the release source archive does not include the
merge; `build-info.txt` records the upstream, patch, and runner-merge SHAs.
Unchanged upstream+patch sources skip the build after a successful publication;
manual dispatch always rebuilds, and failed builds are retried on the next
trigger.

When PR #8231 is merged, the workflow stops applying the patch and builds
upstream `main` instead. If the PR is closed without merging, the run fails
rather than silently publishing an unpatched build; update or remove the
workflow at that point.

The workflow uses only the automatic `GITHUB_TOKEN`; no personal access token
or `UPSTREAM_SYNC_TOKEN` is needed. Build and release jobs have read-only
contents except the release job, which needs `contents: write` to update the
rolling release and tag.

<img height="128" alt="WezTerm Icon" src="https://raw.githubusercontent.com/wezterm/wezterm/main/assets/icon/wezterm-icon.svg" align="left"> *A GPU-accelerated cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

User facing docs and guide at: https://wezterm.org/

![Screenshot](docs/screenshots/two.png)

*Screenshot of wezterm on macOS, running vim*

## Installation

https://wezterm.org/installation

## Getting help

This is a spare time project, so please bear with me.  There are a couple of channels for support:

* You can use the [GitHub issue tracker](https://github.com/wezterm/wezterm/issues) to see if someone else has a similar issue, or to file a new one.
* Start or join a thread in our [GitHub Discussions](https://github.com/wezterm/wezterm/discussions); if you have general
  questions or want to chat with other wezterm users, you're welcome here!
* There is a [Matrix room via Element.io](https://matrix.to/#/#wezterm:matrix.org)
  for (potentially!) real time discussions.

The GitHub Discussions and Element/Gitter rooms are better suited for questions
than bug reports, but don't be afraid to use whichever you are most comfortable
using and we'll work it out.

## Supporting the Project

If you use and like WezTerm, please consider sponsoring it: your support helps
to cover the fees required to maintain the project and to validate the time
spent working on it!

[Read more about sponsoring](https://wezterm.org/sponsor.html).

* [![Sponsor WezTerm](https://img.shields.io/github/sponsors/wez?label=Sponsor%20WezTerm&logo=github&style=for-the-badge)](https://github.com/sponsors/wez)
* [Patreon](https://patreon.com/WezFurlong)
* [Ko-Fi](https://ko-fi.com/wezfurlong)
* [Liberapay](https://liberapay.com/wez)
