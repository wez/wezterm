#!/usr/bin/env python3
import os
import sys


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
    def render(self, f, env, depth=0):
        raise NotImplementedError(repr(self))


class RunStep(Step):
    def __init__(self, name, run, shell="bash", env=None):
        self.name = name
        self.run = run
        self.shell = shell
        self.env = env

    def render(self, f, env, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        if self.env:
            f.write(f"{indent}  env:\n")
            for k, v in self.env.items():
                f.write(f"{indent}    {k}: {v}\n")
        if self.shell:
            f.write(f"{indent}  shell: {self.shell}\n")

        run = self.run

        if env:
            for k, v in env.items():
                if self.shell == "bash":
                    run = f"export {k}={v}\n{run}\n"

        f.write(f"{indent}  run: {yv(run, depth + 2)}\n")


class ActionStep(Step):
    def __init__(self, name, action, params=None, env=None, condition=None):
        self.name = name
        self.action = action
        self.params = params
        self.env = env
        self.condition = condition

    def render(self, f, env, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        f.write(f"{indent}  uses: {self.action}\n")
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
    def __init__(self, name, path, key):
        super().__init__(
            name, action="actions/cache@v2.1.7", params={"path": path, "key": key}
        )


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo", submodules=True):
        params = {}
        if submodules:
            params["submodules"] = "recursive"
        super().__init__(name, action="actions/checkout@v2", params=params)


class Job(object):
    def __init__(self, runs_on, container=None, steps=None, env=None):
        self.runs_on = runs_on
        self.container = container
        self.steps = steps
        self.env = env

    def render(self, f, depth=0):
        for s in self.steps:
            s.render(f, self.env, depth)


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
        else:
            return []
        if self.needs_sudo():
            installer = f"sudo -n {installer}"
        return [RunStep(f"Install {name}", f"{installer} install -y {name}")]

    def install_curl(self):
        if self.uses_yum() or (self.uses_apt() and self.container):
            return self.install_system_package("curl")
        return []

    def install_openssh_server(self):
        if self.uses_yum() or (self.uses_apt() and self.container):
            return [
                RunStep("Ensure /run/sshd exists", "mkdir -p /run/sshd")
            ] + self.install_system_package("openssh-server")
        return []

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
            steps += self.install_system_package("git")

        return steps

    def install_rust(self, cache=True):
        salt = "2"
        key_prefix = f"{self.name}-{self.rust_target}-{salt}-${{{{ runner.os }}}}-${{{{ hashFiles('**/Cargo.lock') }}}}"
        params = {
            "profile": "minimal",
            "toolchain": "stable",
            "override": True,
            "components": "rustfmt",
        }
        if self.rust_target:
            params["target"] = self.rust_target
        steps = [
            ActionStep(
                name="Install Rust",
                action="actions-rs/toolchain@v1",
                params=params,
                env={"ACTIONS_ALLOW_UNSECURE_COMMANDS": "true"},
            ),
        ]
        if "macos" in self.name:
            steps += [
                RunStep(
                    name="Install Rust (ARM)",
                    run="rustup target add aarch64-apple-darwin",
                )
            ]
        if cache:
            cache_paths = ["~/.cargo/registry", "~/.cargo/git", "target"]
            steps += [
                CacheStep(
                    name="Cache cargo",
                    path="\n".join(cache_paths),
                    key=f"{key_prefix}-cargo",
                ),
            ]
        return steps

    def install_system_deps(self):
        if "win" in self.name:
            return []
        sudo = "sudo -n " if self.needs_sudo() else ""
        return [
            RunStep(name="Install System Deps", run=f"{sudo}env PATH=$PATH ./get-deps")
        ]

    def build_all_release(self):
        if "win" in self.name:
            return [
                RunStep(
                    name="Build (Release mode)",
                    shell="cmd",
                    run="""
PATH C:\\Strawberry\\perl\\bin;%PATH%
cargo build --all --release""",
                )
            ]
        if "macos" in self.name:
            return [
                RunStep(
                    name="Build (Release mode Intel)",
                    run="cargo build --target x86_64-apple-darwin --all --release",
                ),
                RunStep(
                    name="Build (Release mode ARM)",
                    run="cargo build --target aarch64-apple-darwin --all --release",
                ),
            ]
        if self.name == "centos7":
            enable = "source /opt/rh/devtoolset-9/enable && "
        else:
            enable = ""
        return [
            RunStep(
                name="Build (Release mode)", run=enable + "cargo build --all --release"
            )
        ]

    def test_all_release(self):
        if "macos" in self.name:
            return [
                RunStep(
                    name="Test (Release mode)",
                    run="cargo test --target x86_64-apple-darwin --all --release",
                )
            ]
        if self.name == "centos7":
            enable = "source /opt/rh/devtoolset-9/enable && "
        else:
            enable = ""
        return [
            RunStep(
                name="Test (Release mode)", run=enable + "cargo test --all --release"
            )
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

        patterns = self.asset_patterns()
        glob = " ".join(patterns)
        paths = "\n".join(patterns)

        return steps + [
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@v2",
                params={"name": self.name, "path": paths},
            ),
        ]

    def asset_patterns(self):
        patterns = []
        if self.uses_yum():
            patterns += ["wezterm-*.rpm"]
        elif "win" in self.name:
            patterns += ["WezTerm-*.zip", "WezTerm-*.exe"]
        elif "mac" in self.name:
            patterns += ["WezTerm-*.zip"]
        elif ("ubuntu" in self.name) or ("debian" in self.name):
            patterns += ["wezterm-*.deb", "wezterm-*.xz"]

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

        patterns = self.asset_patterns()
        glob = " ".join(patterns)
        paths = "\n".join(patterns)

        return steps + [
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@v2",
                params={"name": self.name, "path": paths, "retention-days": 5},
            ),
        ]

    def upload_asset_nightly(self):
        steps = []

        patterns = self.asset_patterns()
        glob = " ".join(patterns)

        return steps + [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v2",
                params={"name": self.name},
            ),
            RunStep(
                "Upload to Nightly Release",
                f"bash ci/retry.sh gh release upload --clobber nightly {glob}",
                env={"GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}"},
            ),
        ]

    def upload_asset_tag(self):
        steps = []

        patterns = self.asset_patterns()
        glob = " ".join(patterns)

        return steps + [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v2",
                params={"name": self.name},
            ),
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

    def update_homebrew_tap(self):
        steps = []
        if "macos" in self.name:
            steps += [
                ActionStep(
                    "Checkout homebrew tap",
                    action="actions/checkout@v2",
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
                    action="stefanzweifel/git-auto-commit-action@v4",
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
                    action="actions/checkout@v2",
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
                    action="stefanzweifel/git-auto-commit-action@v4",
                    params={
                        "commit_message": "Automated update to match latest tag",
                        "repository": "linuxbrew-wezterm",
                    },
                ),
            ]

        return steps

    def global_env(self):
        env = {}
        if "macos" in self.name:
            env["MACOSX_DEPLOYMENT_TARGET"] = "10.9"
        return env

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

        if self.container:
            if ("fedora" in self.container) or ("centos" in self.container):
                steps += [
                    RunStep(
                        "Install config manager",
                        "dnf install -y 'dnf-command(config-manager)'",
                    ),
                ]
            if "centos" in self.container:
                steps += [
                    RunStep(
                        "Enable PowerTools",
                        "dnf config-manager --set-enabled powertools",
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
        steps += [
            CheckoutStep(),
        ]
        steps += self.install_rust(cache="mac" not in self.name)
        steps += self.install_system_deps()
        return steps

    def pull_request(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package()
        steps += self.upload_artifact()
        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=self.global_env(),
            ),
            None,
        )

    def continuous(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package(trusted=True)
        steps += self.upload_artifact_nightly()

        env = self.global_env()
        env["BUILD_REASON"] = "Schedule"

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=[CheckoutStep(submodules=False)] + self.upload_asset_nightly(),
        )

        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=env,
            ),
            uploader,
        )

    def tag(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package(trusted=True)
        steps += self.upload_artifact()
        steps += self.update_homebrew_tap()

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=[CheckoutStep(submodules=False)] + self.upload_asset_tag(),
        )

        env = self.global_env()
        return (
            Job(
                runs_on=self.os,
                container=self.container,
                steps=steps,
                env=env,
            ),
            uploader,
        )


TARGETS = [
    Target(name="ubuntu:18", os="ubuntu-18.04", app_image=True),
    Target(container="ubuntu:20.04", continuous_only=True),
    # debian 8's wayland libraries are too old for wayland-client
    # Target(container="debian:8.11", continuous_only=True, bootstrap_git=True),
    Target(container="debian:9.12", continuous_only=True, bootstrap_git=True),
    Target(container="debian:10.3", continuous_only=True),
    Target(container="debian:11", continuous_only=True),
    Target(name="macos", os="macos-11"),
    # https://fedoraproject.org/wiki/End_of_life?rd=LifeCycle/EOL
    Target(container="fedora:33"),
    Target(container="fedora:34"),
    Target(container="fedora:35"),
    Target(container="centos:8"),
    Target(name="windows", os="windows-latest", rust_target="x86_64-pc-windows-msvc"),
]


def generate_actions(namer, jobber, trigger, is_continuous):
    for t in TARGETS:
        # if t.continuous_only and not is_continuous:
        #    continue
        name = namer(t).replace(":", "")
        print(name)
        job, uploader = jobber(t)

        file_name = f".github/workflows/gen_{name}.yml"
        if job.container:
            container = f"container: {yv(job.container)}"
        else:
            container = ""

        with open(file_name, "w") as f:
            f.write(
                f"""name: {name}
{trigger}
jobs:
  build:
    runs-on: {yv(job.runs_on)}
    {container}
    steps:
"""
            )

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
    steps:
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
    paths-ignore:
      - ".cirrus.yml"
      - "docs/*"
      - "ci/build-docs.sh"
      - "ci/generate-docs.py"
      - "ci/subst-release-info.py"
      - ".github/workflows/pages.yml"
      - ".github/workflows/verify-pages.yml"
      - ".github/ISSUE_TEMPLATE/*"
      - "**/*.md"
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
    paths-ignore:
      - ".cirrus.yml"
      - "docs/**"
      - "ci/build-docs.sh"
      - "ci/generate-docs.py"
      - "ci/subst-release-info.py"
      - ".github/workflows/pages.yml"
      - ".github/workflows/verify-pages.yml"
      - ".github/ISSUE_TEMPLATE/*"
      - "**/*.md"
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
    )


generate_pr_actions()
continuous_actions()
tag_actions()
