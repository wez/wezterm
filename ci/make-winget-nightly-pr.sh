#!/bin/bash
# Updates the wez.wezterm.nightly winget manifest with the latest nightly build hash.
# This script is called from the nightly CI workflow after uploading the new installer.
#
# Usage: bash ci/make-winget-nightly-pr.sh <winget_repo_path> <setup_exe>
set -xe

winget_repo=$1
setup_exe=$2
TAG_NAME="nightly"
PACKAGE_ID="wez.wezterm.nightly"
PACKAGE_VERSION=$(ci/tag-name.sh)

cd "$winget_repo" || exit 1

# Sync with upstream
git remote add upstream https://github.com/microsoft/winget-pkgs.git || true
git fetch upstream master --quiet
git checkout -b "update-wezterm-nightly-${PACKAGE_VERSION}" upstream/master

exehash=$(sha256sum -b "../$setup_exe" | cut -f1 -d' ' | tr a-f A-F)
release_date=$(date +%Y-%m-%d)

manifest_dir="manifests/w/wez/wezterm.nightly/${PACKAGE_VERSION}"
mkdir -p "$manifest_dir"

cat > "$manifest_dir/${PACKAGE_ID}.installer.yaml" <<-EOT
PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${PACKAGE_VERSION}
MinimumOSVersion: 10.0.17763.0
InstallerType: inno
UpgradeBehavior: install
ReleaseDate: ${release_date}
Installers:
- Architecture: x64
  InstallerUrl: https://github.com/wezterm/wezterm/releases/download/${TAG_NAME}/WezTerm-nightly-setup.exe
  InstallerSha256: ${exehash}
  ProductCode: '{BCF6F0DA-5B9A-408D-8562-F680AE6E1EAF}_is1'
ManifestType: installer
ManifestVersion: 1.1.0
EOT

cat > "$manifest_dir/${PACKAGE_ID}.locale.en-US.yaml" <<-EOT
PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${PACKAGE_VERSION}
PackageLocale: en-US
Publisher: Wez Furlong
PublisherUrl: https://wezfurlong.org/
PublisherSupportUrl: https://github.com/wezterm/wezterm/issues
Author: Wez Furlong
PackageName: WezTerm (Nightly)
PackageUrl: http://wezterm.org
License: MIT
LicenseUrl: https://github.com/wezterm/wezterm/blob/main/LICENSE.md
ShortDescription: A GPU-accelerated cross-platform terminal emulator and multiplexer implemented in Rust (nightly build)
ReleaseNotesUrl: https://wezterm.org/changelog.html
ManifestType: defaultLocale
ManifestVersion: 1.1.0
EOT

cat > "$manifest_dir/${PACKAGE_ID}.yaml" <<-EOT
PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${PACKAGE_VERSION}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.1.0
EOT

git add --all
git diff --cached
git commit -m "New version: ${PACKAGE_ID} version ${PACKAGE_VERSION}"
git push --set-upstream origin "update-wezterm-nightly-${PACKAGE_VERSION}" --quiet
