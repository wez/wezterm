#!/usr/bin/env python3
import os
import sys


def yv(v):
    if v is True:
        return "true"
    if v is False:
        return "false"
    if v is None:
        return "nil"

    if isinstance(v, str):
        if "\n" in v:
            spacer = " " * 12
            return "|\n" + spacer + v.replace("\n", "\n" + spacer) + "\n"
        return '"' + v + '"'

    return v


class Step(object):
    def render(self, f, env):
        raise NotImplementedError(repr(self))


class RunStep(Step):
    def __init__(self, name, run, shell="bash"):
        self.name = name
        self.run = run
        self.shell = shell

    def render(self, f, env):
        f.write(f"    - name: {yv(self.name)}\n")
        if self.shell:
            f.write(f"      shell: {self.shell}\n")

        run = self.run

        if env:
            for k, v in env.items():
                if self.shell == "bash":
                    run = f"export {k}={v}\n{run}\n"

        f.write(f"      run: {yv(run)}\n")


class ActionStep(Step):
    def __init__(self, name, action, params=None, env=None):
        self.name = name
        self.action = action
        self.params = params
        self.env = env

    def render(self, f, env):
        f.write(f"    - name: {yv(self.name)}\n")
        f.write(f"      uses: {self.action}\n")
        if self.params:
            f.write("      with:\n")
            for k, v in self.params.items():
                f.write(f"         {k}: {yv(v)}\n")
        if self.env:
            f.write("      env:\n")
            for k, v in self.env.items():
                f.write(f"         {k}: {yv(v)}\n")


class CacheStep(ActionStep):
    def __init__(self, name, path, key):
        super().__init__(
            name, action="actions/cache@v2", params={"path": path, "key": key}
        )


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo"):
        super().__init__(
            name, action="actions/checkout@v2", params={"submodules": "recursive"}
        )


class Job(object):
    def __init__(self, runs_on, container=None, steps=None, env=None):
        self.runs_on = runs_on
        self.container = container
        self.steps = steps
        self.env = env

    def render(self, f):
        for s in self.steps:
            s.render(f, self.env)


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
                    run=f"""
{pre_reqs}

if test ! -x /usr/local/git/bin/git ; then
    cd /tmp
    wget https://github.com/git/git/archive/v{GIT_VERS}.tar.gz
    tar xzf v{GIT_VERS}.tar.gz
    cd git-{GIT_VERS}
    make prefix=/usr/local/git install
fi

ln -s /usr/local/git/bin/git /usr/local/bin/git
        """,
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
                RunStep(name="Install Rust (ARM)", run="rustup target add aarch64-apple-darwin")
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
        return [RunStep(name="Install System Deps", run=f"{sudo}./get-deps")]

    def check_formatting(self):
        return [RunStep(name="Check formatting", run="cargo fmt --all -- --check")]

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
                    run="cargo build --target x86_64-apple-darwin --all --release"),
                RunStep(
                    name="Build (Release mode ARM)",
                    run="cargo build --target aarch64-apple-darwin --all --release"),
            ]
        return [RunStep(name="Build (Release mode)", run="cargo build --all --release")]

    def test_all_release(self):
        if "macos" in self.name:
            return [RunStep(name="Test (Release mode)", run="cargo test --target x86_64-apple-darwin --all --release")]
        return [RunStep(name="Test (Release mode)", run="cargo test --all --release")]

    def package(self):
        steps = [RunStep("Package", "bash ci/deploy.sh")]
        if self.app_image:
            steps.append(RunStep("Source Tarball", "bash ci/source-archive.sh"))
            steps.append(RunStep("Build AppImage", "bash ci/appimage.sh"))
        return steps

    def upload_artifact(self):
        run = "mkdir pkg_\n"
        if self.uses_yum():
            run += "mv ~/rpmbuild/RPMS/*/*.rpm pkg_\n"
        if "win" in self.name:
            run += "mv *.zip *.exe pkg_\n"
        if "mac" in self.name:
            run += "mv *.zip pkg_\n"
        if ("ubuntu" in self.name) or ("debian" in self.name):
            run += "mv *.deb *.xz pkg_\n"
        if self.app_image:
            run += "mv *.AppImage *.zsync pkg_\n"

        return [
            RunStep("Move Package for artifact upload", run),
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@master",
                params={"name": self.name, "path": "pkg_"},
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
            patterns += ["wezterm-*.deb", "wezterm-*.xz", "wezterm-*.tar.gz"]

        if self.app_image:
            patterns.append("*.AppImage")
            patterns.append("*.zsync")
        return patterns

    def upload_asset_nightly(self):
        steps = []

        if self.uses_yum():
            steps.append(
                RunStep(
                    "Move RPM",
                    f"mv ~/rpmbuild/RPMS/*/*.rpm wezterm-nightly-{self.name}.rpm",
                )
            )

        patterns = self.asset_patterns()

        return steps + [
            ActionStep(
                "Upload to Nightly Release",
                action="wez/upload-release-assets@releases/v1",
                params={
                    "files": ";".join(patterns),
                    "release-tag": "nightly",
                    "repo-token": "${{ secrets.GITHUB_TOKEN }}",
                },
            )
        ]

    def upload_asset_tag(self):
        steps = []

        if self.uses_yum():
            steps.append(RunStep("Move RPM", "mv ~/rpmbuild/RPMS/*/*.rpm ."))

        patterns = self.asset_patterns()

        return steps + [
            ActionStep(
                "Upload to Tagged Release",
                action="softprops/action-gh-release@v1",
                params={"files": "\n".join(patterns), "prerelease": True},
                env={
                    "GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                },
            )
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
                    "cp wezterm.rb homebrew-wezterm/Formula/wezterm.rb",
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

    def update_tagged_aur(self):
        steps = []

        if self.app_image:
            # The AppImage build step also expands the PKGBUILD template
            steps += [
                ActionStep(
                    "Update AUR",
                    action="KSXGitHub/github-actions-deploy-aur@master",
                    params={
                        "pkgname": "wezterm-bin",
                        "pkgbuild": "PKGBUILD",
                        "commit_username": "wez",
                        "commit_email": "wez@wezfurlong.org",
                        "ssh_private_key": "${{ secrets.AUR_SSH_PRIVATE_KEY }}",
                        "commit_message": "Automated update to match latest tag",
                    },
                )
            ]

        return steps

    def global_env(self):
        env = {}
        if "macos" in self.name:
            env["MACOSX_DEPLOYMENT_TARGET"] = "10.9"
        return env

    def prep_environment(self, cache=True):
        steps = []
        if self.uses_apt():
            if self.container:
                steps += [
                    RunStep(
                        "set APT to non-interactive",
                        "echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections",
                    ),
                ]
            sudo = "sudo -n " if self.needs_sudo() else ""
            steps += [
                RunStep("Update APT", f"{sudo}apt update"),
            ]
        if self.container:
            if self.container == "centos:8":
                steps += [
                    RunStep(
                        "Install config manager",
                        "dnf install -y 'dnf-command(config-manager)'",
                    ),
                    RunStep(
                        "Enable PowerTools",
                        "dnf config-manager --set-enabled powertools",
                    ),
                ]
        steps += self.install_git()
        steps += self.install_curl()
        steps += [
            CheckoutStep(),
            # We need tags in order to use git describe for build/packaging
            RunStep(
                "Fetch tags", "git fetch --depth=1 origin +refs/tags/*:refs/tags/*"
            ),
            RunStep("Fetch tag/branch history", "git fetch --prune --unshallow"),
        ]
        steps += self.install_rust(cache="mac" not in self.name)
        steps += self.install_system_deps()
        return steps

    def pull_request(self):
        steps = self.prep_environment()
        steps += self.check_formatting()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package()
        steps += self.upload_artifact()
        return Job(
            runs_on=self.os,
            container=self.container,
            steps=steps,
            env=self.global_env(),
        )

    def continuous(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package()
        steps += self.upload_asset_nightly()

        env = self.global_env()
        env["BUILD_REASON"] = "Schedule"

        return Job(
            runs_on=self.os,
            container=self.container,
            steps=steps,
            env=env,
        )

    def tag(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package()
        steps += self.upload_asset_tag()
        steps += self.update_tagged_aur()
        steps += self.update_homebrew_tap()

        env = self.global_env()
        return Job(
            runs_on=self.os,
            container=self.container,
            steps=steps,
            env=env,
        )


TARGETS = [
    Target(name="ubuntu:16", os="ubuntu-16.04", app_image=True),
    Target(name="ubuntu:18", os="ubuntu-18.04", continuous_only=True),
    Target(container="ubuntu:20.04", continuous_only=True),
    # debian 8's wayland libraries are too old for wayland-client
    # Target(container="debian:8.11", continuous_only=True, bootstrap_git=True),
    Target(container="debian:9.12", continuous_only=True, bootstrap_git=True),
    Target(container="debian:10.3", continuous_only=True),
    Target(name="macos", os="macos-11.0"),
    Target(container="fedora:31"),
    Target(container="fedora:32"),
    Target(container="fedora:33"),
    Target(container="centos:7", bootstrap_git=True),
    Target(container="centos:8"),
    Target(name="windows", os="vs2017-win2016", rust_target="x86_64-pc-windows-msvc"),
]


def generate_actions(namer, jobber, trigger, is_continuous):
    for t in TARGETS:
        # if t.continuous_only and not is_continuous:
        #    continue
        name = namer(t).replace(":", "")
        print(name)
        job = jobber(t)

        file_name = f".github/workflows/gen_{name}.yml"
        if job.container:
            container = f"container: {yv(job.container)}"
        else:
            container = ""

        with open(file_name, "w") as f:
            f.write(
                f"""
name: {name}
{trigger}

jobs:
  build:
    strategy:
      fail-fast: false
    runs-on: {yv(job.runs_on)}
    {container}
    steps:
"""
            )

            job.render(f)

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
    - 'docs/*'
    - 'ci/build-docs.sh'
    - 'ci/generate-docs.py'
    - 'ci/subst-release-info.py'
    - '.github/workflows/pages.yml'
    - '**/*.md'
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
    - 'docs/**'
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
