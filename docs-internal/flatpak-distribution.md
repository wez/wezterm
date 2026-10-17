# Flatpak distribution

## Release build

For a release build, the flatpak definition is
`assets/flatpak/org.phiterm.phiterm.RELEASE-TEMPLATE.json` with metadata in
`assets/flatpak/org.phiterm.phiterm.appdata.RELEASE-TEMPLATE.xml`.

A GHA workflow runs the `ci/flathub-prepare-pr.sh` script from repo root to prepare a few metadata
files, followed by opening a PR on the [flathub repo][flathub-repo] for the new version.

Once merged, flathub builders will auto-schedule the actual build for later inclusion in the flathub
app catalog. This can be followed on <https://builds.flathub.org/>.

[flathub-repo]: https://github.com/flathub/org.phiterm.phiterm

## Local build

For a local build, the flatpak definition is in
`assets/flatpak/org.phiterm.phiterm.LOCAL-TESTING.json`. It is basically the same as the
release one but uses the local repo instead of a specific release from github.

The script needs the following dependencies:

- flatpak-cargo-generator (auto-downloaded if not in `$PATH`, needs python3 with pip)
- flatpak-builder
- appstreamcli

> [!NOTE]
> If using Nix, `nix develop` gives you these automatically.

Run `ci/flatpak-build.sh` from the repo root to prepare, build & install the flatpak for the current
user. Then run the built flatpak with `flatpak run org.phiterm.phiterm`.

> [!NOTE]
> The script stores tmp data in `/var/tmp/phiterm-flatpak-*` directories.
