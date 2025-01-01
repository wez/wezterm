#!/usr/bin/env python3
import os
import sys
import glob
from copy import deepcopy

TRIGGER_PATHS = [
    "**/*.rs",
    "**/Cargo.lock",
    "**/Cargo.toml",
    "assets/fonts/**/*",
    "assets/icon/*",
    "ci/deploy.sh",
]

TRIGGER_PATHS_APPIMAGE = [
    "ci/appimage.sh",
    "ci/appstreamcli",
    "ci/source-archive.sh",
]

TRIGGER_PATHS_UNIX = [
    "assets/open-wezterm-here",
    "assets/shell-completion/**/*",
    "assets/shell-integration/**/*",
    "assets/wezterm-nautilus.py",
    "assets/wezterm.appdata.xml",
    "assets/wezterm.desktop",
    "get-deps",
    "ci/tag-name.sh",
    "termwiz/data/wezterm.terminfo",
]

TRIGGER_PATHS_MAC = [
    "assets/macos/**/*",
    "ci/macos-entitlement.plist",
    "get-deps",
    "ci/tag-name.sh",
]

TRIGGER_PATHS_WIN = [
    "assets/windows/**/*",
    "ci/windows-installer.iss",
]


def yv(v, depth=0):
    if v is True:
        return "true"
    if v is False:
        return "false"
    if v is None:
        return "nil"

    if isinstance(v, str):
        if "\n" in v:
            indent = "  " * depth
            result = ""
            for l in v.splitlines():
                result = result + "\n" + (f"{indent}{l}" if l else "")
            return "|" + result
        # This is hideous
        if '"' in v:
            return "'" + v + "'"
        return '"' + v + '"'

    return v


class Step(object):
    def render(self, f, depth=0):
        raise NotImplementedError(repr(self))


class RunStep(Step):
    def __init__(self, name, run, shell="bash", env=None, condition=None):
        self.name = name
        self.run = run
        self.shell = shell
        self.env = env
        self.condition = condition

    def render(self, f, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        if self.condition:
            f.write(f"{indent}  if: {self.condition}\n")
        if self.env:
            f.write(f"{indent}  env:\n")
            keys = list(self.env.keys())
            keys.sort()
            for k in keys:
                v = self.env[k]
                f.write(f"{indent}    {k}: {v}\n")
        if self.shell:
            f.write(f"{indent}  shell: {self.shell}\n")

        run = self.run

        f.write(f"{indent}  run: {yv(run, depth + 2)}\n")


class ActionStep(Step):
    def __init__(self, name, action, params=None, env=None, condition=None, id=None):
        self.name = name
        self.action = action
        self.params = params
        self.env = env
        self.condition = condition
        self.id = id

    def render(self, f, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        f.write(f"{indent}  uses: {self.action}\n")
        if self.id:
            f.write(f"{indent}  id: {self.id}\n")
        if self.condition:
            f.write(f"{indent}  if: {self.condition}\n")
        if self.params:
            f.write(f"{indent}  with:\n")
            for k, v in self.params.items():
                f.write(f"{indent}    {k}: {yv(v, depth + 3)}\n")
        if self.env:
            f.write(f"{indent}  env:\n")
            for k, v in self.env.items():
                f.write(f"{indent}    {k}: {yv(v, depth + 3)}\n")


class CacheStep(ActionStep):
    def __init__(self, name, path, key, id=None):
        super().__init__(
            name, action="actions/cache@v4", params={"path": path, "key": key}, id=id
        )


class SccacheStep(ActionStep):
    def __init__(self, name):
        super().__init__(name, action="mozilla-actions/sccache-action@v0.0.5")


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo", submodules=True, container=None):
        params = {}
        if submodules:
            params["submodules"] = "recursive"
        # Newer versions of the checkout action use a binary-incompatible node
        # binary, so we are pinned back on v3
        # https://github.com/actions/checkout/issues/1442
        version = "v3" if container is not None and "centos7" in container else "v4"
        super().__init__(name, action=f"actions/checkout@{version}", params=params)


class InstallCrateStep(ActionStep):
    def __init__(self, crate: str, key: str, version=None):
        params = {"crate": crate, "cache-key": key}
        if version is not None:
            params["version"] = version
        super().__init__(
            f"Install {crate} from Cargo",
            action="baptiste0928/cargo-install@v3",
            params=params,
        )


class Job(object):
    def __init__(self, runs_on, container=None, steps=None, env=None):
        self.runs_on = runs_on
        self.container = container
        self.steps = steps
        self.env = env

    def render(self, f, depth=0):
        f.write("\n    steps:\n")
        for s in self.steps:
            s.render(f, depth)


class Target(object):
    def __init__(
        self,
        name=None,
        os="ubuntu-latest",
        container=None,
        bootstrap_git=False,
        rust_target=None,
        continuous_only=False,
        app_image=False,
        is_tag=False,
    ):
        if not name:
            if container:
                name = container
            else:
                name = os
        self.name = name.replace(":", "")
        self.os = os
        self.container = container
        self.bootstrap_git = bootstrap_git
        self.rust_target = rust_target
        self.continuous_only = continuous_only
        self.app_image = app_image
        self.env = {}
        self.is_tag = is_tag

    def render_env(self, f, depth=0):
        self.global_env()
        if self.env:
            indent = "    "
            f.write(f"{indent}env:\n")
            for k, v in self.env.items():
                f.write(f"{indent}  {k}: {yv(v, depth + 3)}\n")

    def uses_yum(self):
        if "fedora" in self.name:
            return True
        if "centos" in self.name:
            return True
        return False

    def uses_apt(self):
        if "ubuntu" in self.name:
            return True
        if "debian" in self.name:
            return True
        return False

    def uses_apk(self):
        if "alpine" in self.name:
            return True
        return False

    def uses_zypper(self):
        if "suse" in self.name:
            return True
        return False

    def needs_sudo(self):
        if not self.container and self.uses_apt():
            return True
        return False

    def install_system_package(self, name):
        installer = None
        if self.uses_yum():
            installer = "yum"
        elif self.uses_apt():
            installer = "apt-get"
        elif self.uses_apk():
            installer = "apk"
        elif self.uses_zypper():
            installer = "zypper"
        else:
            return []
        if self.needs_sudo():
            installer = f"sudo -n {installer}"
        if self.uses_apk():
            return [RunStep(f"Install {name}", f"{installer} add {name}")]
        else:
            return [RunStep(f"Install {name}", f"{installer} install -y {name}")]

    def install_curl(self):
        if (
            self.uses_yum()
            or self.uses_apk()
            or self.uses_zypper()
            or (self.uses_apt() and self.container)
        ):
            if "centos:stream9" in self.container:
                return self.install_system_package("curl-minimal")
            else:
                return self.install_system_package("curl")
        return []

    def install_openssh_server(self):
        steps = []
        if (
            self.uses_yum()
            or self.uses_zypper()
            or (self.uses_apt() and self.container)
        ):
            steps += [
                RunStep("Ensure /run/sshd exists", "mkdir -p /run/sshd")
            ] + self.install_system_package("openssh-server")
        if self.uses_apk():
            steps += self.install_system_package("openssh")
        return steps

    def install_newer_compiler(self):
        steps = []
        if self.name == "centos7":
            steps.append(
                RunStep(
                    "Install SCL",
                    "yum install -y centos-release-scl-rh",
                )
            )
            steps.append(
                RunStep(
                    "Update compiler",
                    "yum install -y devtoolset-9-gcc devtoolset-9-gcc-c++",
                )
            )
        return steps

    def install_git(self):
        steps = []
        if self.bootstrap_git:
            GIT_VERS = "2.26.2"
            steps.append(
                CacheStep(
                    "Cache Git installation",
                    path="/usr/local/git",
                    key=f"{self.name}-git-{GIT_VERS}",
                )
            )

            pre_reqs = ""
            if self.uses_yum():
                pre_reqs = "yum install -y wget curl-devel expat-devel gettext-devel openssl-devel zlib-devel gcc perl-ExtUtils-MakeMaker make"
            elif self.uses_apt():
                pre_reqs = "apt-get install -y wget libcurl4-openssl-dev libexpat-dev gettext libssl-dev libz-dev gcc libextutils-autoinstall-perl make"
            elif self.uses_zypper():
                pre_reqs = "zypper install -y wget libcurl-devel libexpat-devel gettext-tools libopenssl-devel zlib-devel gcc perl-ExtUtils-MakeMaker make"

            steps.append(
                RunStep(
                    name="Install Git from source",
                    shell="bash",
                    run=f"""{pre_reqs}
if test ! -x /usr/local/git/bin/git ; then
    cd /tmp
    wget https://github.com/git/git/archive/v{GIT_VERS}.tar.gz
    tar xzf v{GIT_VERS}.tar.gz
    cd git-{GIT_VERS}
    make prefix=/usr/local/git install
fi
ln -s /usr/local/git/bin/git /usr/local/bin/git""",
                )
            )

        else:
            if "tumbleweed" in self.name:
                # git-core requires /usr/bin/which and that gets satisfied
                # by busybox-which by default, which blocks installing
                # rpmbuild, which depends on the which rpm directly,
                # but that is blocked by the conflicting busybox-which rpm.
                # So we explicitly install which here now
                steps += self.install_system_package("which")

            steps += self.install_system_package("git")

        return steps

    def install_rust(self, cache=True, toolchain="stable"):
        salt = "2"
        key_prefix = f"{self.name}-{self.rust_target}-{salt}-${{{{ runner.os }}}}"
        params = dict()
        if self.rust_target:
            params["target"] = self.rust_target
        steps = []
        # Manually setup rust toolchain in CentOS7 curl is too old for the action
        if "centos7" in self.name:
            steps += [
                RunStep(
                    name="Install Rustup",
                    run="""
if ! command -v rustup &>/dev/null; then
  curl --proto '=https' --tlsv1.2 --retry 10 -fsSL "https://sh.rustup.rs" | sh -s -- --default-toolchain none -y
  echo "${CARGO_HOME:-$HOME/.cargo}/bin" >> $GITHUB_PATH
fi
""",
                ),
                RunStep(
                    name="Setup Toolchain",
                    run=f"""
rustup toolchain install {toolchain} --profile minimal --no-self-update
rustup default {toolchain}
""",
                ),
            ]
        elif "macos" in self.name:
            steps += [
                RunStep(
                    name="Install Rust (ARM)",
                    run="rustup target add aarch64-apple-darwin",
                ),
                RunStep(
                    name="Install Rust (Intel)",
                    run="rustup target add x86_64-apple-darwin",
                )
            ]
        else:
            steps += [
                ActionStep(
                    name="Install Rust",
                    action=f"dtolnay/rust-toolchain@{toolchain}",
                    params=params,
                ),
            ]
        if cache:
            steps += [
                SccacheStep(name="Compile with sccache"),
                # Cache vendored dependecies
                CacheStep(
                    name="Cache Rust Dependencies",
                    path="vendor\n.cargo/config",
                    key="cargo-deps-${{ hashFiles('**/Cargo.lock') }}",
                    id="cache-cargo-vendor",
                ),
                # Vendor dependencies
                RunStep(
                    name="Vendor dependecies",
                    condition="steps.cache-cargo-vendor.outputs.cache-hit != 'true'",
                    run="cargo vendor --locked --versioned-dirs >> .cargo/config",
                ),
            ]
        return steps

    def install_system_deps(self):
        if "win" in self.name:
            return []
        sudo = "sudo -n " if self.needs_sudo() else ""
        return [
            RunStep(
                name="Install System Deps",
                run=f"{sudo}env CI=yes PATH=$PATH ./get-deps",
            )
        ]

    def fixup_windows_path(self, cmd):
        if "win" in self.name:
            return "PATH C:\\Strawberry\\perl\\bin;%PATH%\n" + cmd
        return cmd

    def build_all_release(self):
        bin_crates = [
            "wezterm",
            "wezterm-gui",
            "wezterm-mux-server",
            "strip-ansi-escapes",
        ]
        steps = []
        for bin in bin_crates:
            if "win" in self.name:
                steps += [
                    RunStep(
                        name=f"Build {bin} (Release mode)",
                        shell="cmd",
                        run=self.fixup_windows_path(f"cargo build -p {bin} --release"),
                    )
                ]
            elif "macos" in self.name:
                steps += [
                    RunStep(
                        name=f"Build {bin} (Release mode Intel)",
                        run=f"cargo build --target x86_64-apple-darwin -p {bin} --release",
                    ),
                    RunStep(
                        name=f"Build {bin} (Release mode ARM)",
                        run=f"cargo build --target aarch64-apple-darwin -p {bin} --release",
                    ),
                ]
            else:
                if self.name == "centos7":
                    enable = "source /opt/rh/devtoolset-9/enable && "
                else:
                    enable = ""
                steps += [
                    RunStep(
                        name=f"Build {bin} (Release mode)",
                        run=enable + f"cargo build -p {bin} --release",
                    )
                ]
        return steps

    def test_all(self):
        run = "cargo nextest run --all --no-fail-fast"
        if "macos" in self.name:
            run += " --target=x86_64-apple-darwin"
        if self.name == "centos7":
            run = "source /opt/rh/devtoolset-9/enable\n" + run
        return [
            # Install cargo-nextest
            InstallCrateStep("cargo-nextest", key=self.name),
            # Run tests
            RunStep(name="Test", run=self.fixup_windows_path(run), shell="cmd")
            if "win" in self.name
            else RunStep(name="Test", run=run),
        ]

    def package(self, trusted=False):
        steps = []
        deploy_env = None
        if trusted and ("mac" in self.name):
            deploy_env = {
                "MACOS_CERT": "${{ secrets.MACOS_CERT }}",
                "MACOS_CERT_PW": "${{ secrets.MACOS_CERT_PW }}",
                "MACOS_TEAM_ID": "${{ secrets.MACOS_TEAM_ID }}",
                "MACOS_APPLEID": "${{ secrets.MACOS_APPLEID }}",
                "MACOS_APP_PW": "${{ secrets.MACOS_APP_PW }}",
            }
        steps = [RunStep("Package", "bash ci/deploy.sh", env=deploy_env)]
        if self.app_image:
            # AppImage needs fuse
            steps += self.install_system_package("libfuse2")
            steps.append(RunStep("Source Tarball", "bash ci/source-archive.sh"))
            steps.append(RunStep("Build AppImage", "bash ci/appimage.sh"))
        return steps

    def upload_artifact(self):
        steps = []

        if self.uses_yum():
            steps.append(
                RunStep(
                    "Move RPM",
                    f"mv ~/rpmbuild/RPMS/*/*.rpm .",
                )
            )
        elif self.uses_apk():
            steps += [
                # Add the distro name/version into the filename
                RunStep(
                    "Rename APKs",
                    f"mv ~/packages/wezterm/x86_64/*.apk $(echo ~/packages/wezterm/x86_64/*.apk | sed -e 's/wezterm-/wezterm-{self.name}-/')",
                ),
                # Move it to the repo dir
                RunStep(
                    "Move APKs",
                    f"mv ~/packages/wezterm/x86_64/*.apk .",
                ),
                # Move and rename the keys
                RunStep(
                    "Move APK keys",
                    f"mv ~/.abuild/*.pub wezterm-{self.name}.pub",
                ),
            ]
        elif self.uses_zypper():
            steps.append(
                RunStep(
                    "Move RPM",
                    f"mv /usr/src/packages/RPMS/*/*.rpm .",
                )
            )

        patterns = self.asset_patterns()
        glob = " ".join(patterns)
        paths = "\n".join(patterns)

        return steps + [
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@v4",
                params={"name": self.name, "path": paths},
            ),
        ]

    def asset_patterns(self):
        patterns = []
        if self.uses_yum() or self.uses_zypper():
            patterns += ["wezterm-*.rpm"]
        elif "win" in self.name:
            patterns += ["WezTerm-*.zip", "WezTerm-*.exe"]
        elif "mac" in self.name:
            patterns += ["WezTerm-*.zip"]
        elif ("ubuntu" in self.name) or ("debian" in self.name):
            patterns += ["wezterm-*.deb", "wezterm-*.xz"]
        elif "alpine" in self.name:
            patterns += ["wezterm-*.apk"]
            if self.is_tag:
                patterns.append("*.pub")

        if self.app_image:
            patterns.append("*src.tar.gz")
            patterns.append("*.AppImage")
            patterns.append("*.zsync")
        return patterns

    def upload_artifact_nightly(self):
        steps = []

        if self.uses_yum():
            steps.append(
                RunStep(
                    "Move RPM",
                    f"mv ~/rpmbuild/RPMS/*/*.rpm wezterm-nightly-{self.name}.rpm",
                )
            )
        elif self.uses_apk():
            steps.append(
                RunStep(
                    "Move APKs",
                    f"mv ~/packages/wezterm/x86_64/*.apk wezterm-nightly-{self.name}.apk",
                )
            )
        elif self.uses_zypper():
            steps.append(
                RunStep(
                    "Move RPM",
                    f"mv /usr/src/packages/RPMS/*/*.rpm wezterm-nightly-{self.name}.rpm",
                )
            )

        patterns = self.asset_patterns()
        glob = " ".join(patterns)
        paths = "\n".join(patterns)

        return steps + [
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@v4",
                params={"name": self.name, "path": paths, "retention-days": 5},
            ),
        ]

    def upload_asset_nightly(self):
        steps = []

        patterns = self.asset_patterns()
        checksum = RunStep(
            "Checksum",
            f"for f in {' '.join(patterns)} ; do sha256sum $f > $f.sha256 ; done",
        )

        patterns.append("*.sha256")
        glob = " ".join(patterns)

        if self.container == "ubuntu:22.04":
            steps += [
                RunStep(
                    "Upload to gemfury",
                    f"for f in wezterm*.deb ; do curl -i -F package=@$f https://$FURY_TOKEN@push.fury.io/wez/ ; done",
                    env={"FURY_TOKEN": "${{ secrets.FURY_TOKEN }}"},
                ),
            ]

        return [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v4",
                params={"name": self.name},
            ),
            checksum,
            RunStep(
                "Upload to Nightly Release",
                f"bash ci/retry.sh gh release upload --clobber nightly {glob}",
                env={"GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}"},
            ),
        ] + steps

    def upload_asset_tag(self):
        steps = []

        patterns = self.asset_patterns()
        checksum = RunStep(
            "Checksum",
            f"for f in {' '.join(patterns)} ; do sha256sum $f > $f.sha256 ; done",
        )

        patterns.append("*.sha256")
        glob = " ".join(patterns)

        if self.container == "ubuntu:22.04":
            steps += [
                RunStep(
                    "Upload to gemfury",
                    f"for f in wezterm*.deb ; do curl -i -F package=@$f https://$FURY_TOKEN@push.fury.io/wez/ ; done",
                    env={"FURY_TOKEN": "${{ secrets.FURY_TOKEN }}"},
                ),
            ]

        return steps + [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v4",
                params={"name": self.name},
            ),
            checksum,
            RunStep(
                "Create pre-release",
                "bash ci/retry.sh bash ci/create-release.sh $(ci/tag-name.sh)",
                env={
                    "GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                },
            ),
            RunStep(
                "Upload to Tagged Release",
                f"bash ci/retry.sh gh release upload --clobber $(ci/tag-name.sh) {glob}",
                env={
                    "GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                },
            ),
        ]

    def create_flathub_pr(self):
        if not self.app_image:
            return []
        return [
            ActionStep(
                "Checkout flathub/org.wezfurlong.wezterm",
                action="actions/checkout@v4",
                params={
                    "repository": "flathub/org.wezfurlong.wezterm",
                    "path": "flathub",
                    "token": "${{ secrets.GH_PAT }}",
                },
            ),
            RunStep(
                "Create flathub commit and push",
                "bash ci/make-flathub-pr.sh",
            ),
            RunStep(
                "Submit PR",
                'cd flathub && gh pr create --fill --body "PR automatically created by release automation in the wezterm repo"',
                env={
                    "GITHUB_TOKEN": "${{ secrets.GH_PAT }}",
                },
            ),
        ]

    def create_winget_pr(self):
        steps = []
        if "windows" in self.name:
            steps += [
                ActionStep(
                    "Checkout winget-pkgs",
                    action="actions/checkout@v4",
                    params={
                        "repository": "wez/winget-pkgs",
                        "path": "winget-pkgs",
                        "token": "${{ secrets.GH_PAT }}",
                    },
                ),
                RunStep(
                    "Setup email for winget repo",
                    "cd winget-pkgs && git config user.email wez@wezfurlong.org",
                ),
                RunStep(
                    "Setup name for winget repo",
                    "cd winget-pkgs && git config user.name 'Wez Furlong'",
                ),
                RunStep(
                    "Create winget manifest and push to fork",
                    "bash ci/make-winget-pr.sh winget-pkgs WezTerm-*.exe",
                ),
                RunStep(
                    "Submit PR",
                    'cd winget-pkgs && gh pr create --fill --body "PR automatically created by release automation in the wezterm repo"',
                    env={
                        "GITHUB_TOKEN": "${{ secrets.GH_PAT }}",
                    },
                ),
            ]

        return steps

    def update_homebrew_tap(self):
        steps = []
        if "macos" in self.name:
            steps += [
                ActionStep(
                    "Checkout homebrew tap",
                    action="actions/checkout@v4",
                    params={
                        "repository": "wez/homebrew-wezterm",
                        "path": "homebrew-wezterm",
                        "token": "${{ secrets.GH_PAT }}",
                    },
                ),
                RunStep(
                    "Update homebrew tap formula",
                    "cp wezterm.rb homebrew-wezterm/Casks/wezterm.rb",
                ),
                ActionStep(
                    "Commit homebrew tap changes",
                    action="stefanzweifel/git-auto-commit-action@v5",
                    params={
                        "commit_message": "Automated update to match latest tag",
                        "repository": "homebrew-wezterm",
                    },
                ),
            ]
        elif self.app_image:
            steps += [
                ActionStep(
                    "Checkout linuxbrew tap",
                    action="actions/checkout@v4",
                    params={
                        "repository": "wez/homebrew-wezterm-linuxbrew",
                        "path": "linuxbrew-wezterm",
                        "token": "${{ secrets.GH_PAT }}",
                    },
                ),
                RunStep(
                    "Update linuxbrew tap formula",
                    "cp wezterm-linuxbrew.rb linuxbrew-wezterm/Formula/wezterm.rb",
                ),
                ActionStep(
                    "Commit linuxbrew tap changes",
                    action="stefanzweifel/git-auto-commit-action@v5",
                    params={
                        "commit_message": "Automated update to match latest tag",
                        "repository": "linuxbrew-wezterm",
                    },
                ),
            ]

        return steps

    def global_env(self):
        self.env["CARGO_INCREMENTAL"] = "0"
        self.env["SCCACHE_GHA_ENABLED"] = "true"
        self.env["RUSTC_WRAPPER"] = "sccache"
        if "macos" in self.name:
            self.env["MACOSX_DEPLOYMENT_TARGET"] = "10.9"
        if "alpine" in self.name:
            self.env["RUSTFLAGS"] = "-C target-feature=-crt-static"
        if "win" in self.name:
            self.env["RUSTUP_WINDOWS_PATH_ADD_BIN"] = "1"
        return

    def prep_environment(self, cache=True):
        steps = []
        sudo = "sudo -n " if self.needs_sudo() else ""
        if self.uses_apt():
            if self.container:
                steps += [
                    RunStep(
                        "set APT to non-interactive",
                        "echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections",
                    ),
                ]
            steps += [
                RunStep("Update APT", f"{sudo}apt update"),
            ]

        if self.uses_zypper():
            if self.container:
                steps += [
                    RunStep(
                        "Seed GITHUB_PATH to work around possible @action/core bug",
                        f'echo "$PATH:/bin:/usr/bin" >> $GITHUB_PATH',
                    ),
                    RunStep(
                        "Install util-linux",
                        "zypper install -y util-linux",
                    ),
                ]
        if self.container:
            if ("fedora" in self.container) or (
                ("centos" in self.container) and ("centos7" not in self.container)
            ):
                steps += [
                    RunStep(
                        "Install config manager",
                        "dnf install -y 'dnf-command(config-manager)'",
                    ),
                ]
            if "centos:stream8" in self.container:
                steps += [
                    RunStep(
                        "Enable PowerTools",
                        "dnf config-manager --set-enabled powertools",
                    ),
                ]
            if "centos:stream9" in self.container:
                steps += [
                    # This holds the xcb bits
                    RunStep(
                        "Enable CRB repo for X bits",
                        "dnf config-manager --set-enabled crb",
                    ),
                ]
            if "alpine" in self.container:
                steps += [
                    RunStep(
                        "Upgrade system",
                        "apk upgrade --update-cache",
                        shell="sh",
                    ),
                    RunStep(
                        "Install CI dependencies",
                        "apk add nodejs zstd wget bash coreutils tar findutils",
                        shell="sh",
                    ),
                    RunStep(
                        "Allow root login",
                        "sed 's/root:!/root:*/g' -i /etc/shadow",
                    ),
                ]
            if "opensuse" in self.container:
                steps += [
                    # This holds the xcb bits
                    RunStep(
                        "Install tar",
                        "zypper install -yl tar gzip",
                    ),
                ]

        steps += self.install_newer_compiler()
        steps += self.install_git()
        steps += self.install_curl()

        if self.uses_apt():
            if self.container:
                steps += [
                    RunStep("Update APT", f"{sudo}apt update"),
                ]

        steps += self.install_openssh_server()
        steps += self.checkout()
        # We should be able to cache mac builds now?
        steps += self.install_rust()  # cache="mac" not in self.name)
        steps += self.install_system_deps()
        return steps

    def pull_request(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package()
        steps += self.upload_artifact()

        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=self.env,
            ),
            None,
        )

    def checkout(self, submodules=True):
        steps = []
        if self.container:
            steps += [
                RunStep(
                    "Workaround git permissions issue",
                    "git config --global --add safe.directory /__w/wezterm/wezterm",
                )
            ]
        steps += [CheckoutStep(submodules=submodules, container=self.container)]
        return steps

    def continuous(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package(trusted=True)
        steps += self.upload_artifact_nightly()

        self.env["BUILD_REASON"] = "Schedule"

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=self.checkout(submodules=False) + self.upload_asset_nightly(),
        )

        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=self.env,
            ),
            uploader,
        )

    def tag(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package(trusted=True)
        steps += self.upload_artifact()
        steps += self.update_homebrew_tap()

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=self.checkout(submodules=False)
            + self.upload_asset_tag()
            + self.create_winget_pr()
            + self.create_flathub_pr(),
        )

        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=self.env,
            ),
            uploader,
        )


TARGETS = [
    Target(container="ubuntu:20.04", continuous_only=True, app_image=True),
    Target(container="ubuntu:22.04", continuous_only=True),
    Target(container="ubuntu:24.04", continuous_only=True),
    # debian 8's wayland libraries are too old for wayland-client
    # Target(container="debian:8.11", continuous_only=True, bootstrap_git=True),
    # harfbuzz's C++ is too new for debian 9's toolchain
    # Target(container="debian:9.12", continuous_only=True, bootstrap_git=True),
    Target(container="debian:10.3", continuous_only=True),
    Target(container="debian:11", continuous_only=True),
    Target(container="debian:12", continuous_only=True),
    Target(name="centos9", container="quay.io/centos/centos:stream9"),
    Target(name="macos", os="macos-latest"),
    # https://fedoraproject.org/wiki/End_of_life?rd=LifeCycle/EOL
    Target(container="fedora:38"),
    Target(container="fedora:39"),
    Target(container="fedora:40"),
    # Target(container="alpine:3.15"),
    Target(name="windows", os="windows-latest", rust_target="x86_64-pc-windows-msvc"),
]


def generate_actions(namer, jobber, trigger, is_continuous, is_tag=False):
    for t in TARGETS:
        # Clone the definition, as some Target methods called
        # in the body below have side effects that we don't
        # want to bleed across into different schedule types
        t = deepcopy(t)

        t.is_tag = is_tag
        # if t.continuous_only and not is_continuous:
        #    continue
        name = namer(t).replace(":", "")
        print(name)
        job, uploader = jobber(t)

        file_name = f".github/workflows/gen_{name}.yml"
        if job.container:
            if t.app_image:
                container = f"container:\n      image: {yv(job.container)}\n      options: --privileged"
            else:
                container = f"container: {yv(job.container)}"

        else:
            container = ""

        trigger_paths = [file_name]
        trigger_paths += TRIGGER_PATHS
        if "win" in name:
            trigger_paths += TRIGGER_PATHS_WIN
        elif "macos" in name:
            trigger_paths += TRIGGER_PATHS_MAC
        else:
            trigger_paths += TRIGGER_PATHS_UNIX
        if t.app_image:
            trigger_paths += TRIGGER_PATHS_APPIMAGE

        trigger_paths = "- " + "\n      - ".join(yv(p) for p in sorted(trigger_paths))
        trigger_with_paths = trigger.replace("@PATHS@", trigger_paths)

        with open(file_name, "w") as f:
            f.write(
                f"""name: {name}
{trigger_with_paths}
jobs:
  build:
    runs-on: {yv(job.runs_on)}
    {container}
"""
            )

            t.render_env(f)

            job.render(f, 3)

            # We upload using a native runner as github API access
            # inside a container is really unreliable and can result
            # in broken releases that can't automatically be repaired
            # <https://github.com/cli/cli/issues/4863>
            if uploader:
                f.write(
                    """
  upload:
    runs-on: ubuntu-latest
    needs: build
"""
                )
                uploader.render(f, 3)

        # Sanity check the yaml, if pyyaml is available
        try:
            import yaml

            with open(file_name) as f:
                yaml.safe_load(f)
        except ImportError:
            pass


def generate_pr_actions():
    generate_actions(
        lambda t: f"{t.name}",
        lambda t: t.pull_request(),
        trigger="""
on:
  pull_request:
    branches:
      - main
    paths:
      @PATHS@
""",
        is_continuous=False,
    )


def continuous_actions():
    generate_actions(
        lambda t: f"{t.name}_continuous",
        lambda t: t.continuous(),
        trigger="""
on:
  schedule:
    - cron: "10 3 * * *"
  push:
    branches:
      - main
    paths:
      @PATHS@
""",
        is_continuous=True,
    )


def tag_actions():
    generate_actions(
        lambda t: f"{t.name}_tag",
        lambda t: t.tag(),
        trigger="""
on:
  push:
    tags:
      - "20*"
""",
        is_continuous=True,
        is_tag=True,
    )


def remove_gen_actions():
    for name in glob.glob(".github/workflows/gen_*.yml"):
        os.remove(name)


remove_gen_actions()
generate_pr_actions()
continuous_actions()
tag_actions()
