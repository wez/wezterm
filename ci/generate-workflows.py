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
                if self.shell is "bash":
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
            name, action="actions/cache@v1", params={"path": path, "key": key}
        )


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo"):
        super().__init__(name,
            action="actions/checkout@v2",
            params={"submodules":"recursive"})


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
            GIT_VERS = "2.25.0"
            steps.append(
                CacheStep(
                    "Cache Git installation",
                    path="/usr/local/git",
                    key=f"{self.name}-git-{GIT_VERS}",
                ))

            pre_reqs = ""
            if self.uses_yum():
                pre_reqs = "yum install -y wget curl-devel expat-devel gettext-devel openssl-devel zlib-devel gcc perl-ExtUtils-MakeMaker make"
            elif self.uses_apt():
                pre_reqs = "apt-get install -y wget libcurl4-openssl-dev libexpat-dev gettext libssl-dev libz-dev gcc libextutils-autoinstall-perl make"

            steps.append(RunStep(
                name="Install Git from source",
                shell="bash",
                run=f"""
{pre_reqs}

if test ! -x /usr/local/git/bin/git ; then
    cd /tmp
    wget https://mirrors.edge.kernel.org/pub/software/scm/git/git-{GIT_VERS}.tar.gz
    tar xzf git-{GIT_VERS}.tar.gz
    cd git-{GIT_VERS}
    make prefix=/usr/local/git install
fi

ln -s /usr/local/git/bin/git /usr/local/bin/git
        """))

        else:
            steps += self.install_system_package("git")

        return steps

    def install_rust(self):
        key_prefix = (
            f"{self.name}-{self.rust_target}-${{{{ hashFiles('Cargo.lock') }}}}"
        )
        params = {
            "profile": "minimal",
            "toolchain": "stable",
            "override": True,
            "components": "rustfmt",
        }
        if self.rust_target:
            params["target"] = self.rust_target
        return [
            ActionStep(
                name="Install Rust", action="actions-rs/toolchain@v1", params=params,
            ),
            CacheStep(
                name="Cache cargo registry",
                path="~/.cargo/registry",
                key=f"{key_prefix}-cargo-registry",
            ),
            CacheStep(
                name="Cache cargo index",
                path="~/.cargo/git",
                key=f"{key_prefix}-cargo-index",
            ),
            CacheStep(
                name="Cache cargo build",
                path="target",
                key=f"{key_prefix}-cargo-build-target",
            ),
        ]

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
        return [RunStep(name="Build (Release mode)", run="cargo build --all --release")]

    def test_all_release(self):
        return [RunStep(name="Test (Release mode)", run="cargo test --all --release")]

    def package(self):
        steps = [RunStep("Package", "bash ci/deploy.sh")]
        if self.app_image:
            steps.append(RunStep("Build AppImage", "bash ci/appimage.sh"))
        return steps

    def upload_artifact(self):
        run = "mkdir pkg_\n"
        if self.uses_yum():
            run += "mv ~/rpmbuild/RPMS/*/*.rpm pkg_\n"
        if ("win" in self.name) or ("mac" in self.name):
            run += "mv *.zip pkg_\n"
        if ("ubuntu" in self.name) or ("debian" in self.name):
            run += "mv *.deb *.xz pkg_\n"
        if self.app_image:
            run += "mv *.AppImage pkg_\n"

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
        elif ("win" in self.name) or ("mac" in self.name):
            patterns += ["WezTerm-*.zip"]
        elif ("ubuntu" in self.name) or ("debian" in self.name):
            patterns += ["wezterm-*.deb", "wezterm-*.xz", "wezterm-*.tar.gz"]

        if self.app_image:
            patterns.append("*.AppImage")
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
                params={"files": "\n".join(patterns)},
                env={"GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",},
            )
        ]

    def global_env(self):
        env = {}
        if "macos" in self.name:
            env["MACOSX_DEPLOYMENT_TARGET"] = "10.9"
        return env

    def prep_environment(self):
        steps = []
        if self.container:
            if self.uses_apt():
                steps += [
                    RunStep(
                        "set APT to non-interactive",
                        "echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections"
                    ),
                    RunStep("Update APT", "apt update"),
                ]
            if self.container == "centos:8":
                steps += [
                    RunStep(
                        "Install config manager",
                        "dnf install -y 'dnf-command(config-manager)'"
                    ),
                    RunStep(
                        "Enable PowerTools",
                        "dnf config-manager --set-enabled PowerTools"
                    )
                ]
        steps += self.install_git()
        steps += self.install_curl()
        steps += [
            CheckoutStep(),
            # We need tags in order to use git describe for build/packaging
            RunStep(
                "Fetch tags",
                "git fetch --depth=1 origin +refs/tags/*:refs/tags/*"
            ),
            RunStep(
                "Fetch tag/branch history",
                "git fetch --prune --unshallow"
            ),
        ]
        steps += self.install_rust()
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

        return Job(runs_on=self.os, container=self.container, steps=steps, env=env,)

    def tag(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all_release()
        steps += self.package()
        steps += self.upload_asset_tag()

        env = self.global_env()
        return Job(runs_on=self.os, container=self.container, steps=steps, env=env,)


TARGETS = [
    Target(name="ubuntu:16", os="ubuntu-16.04", app_image=True),
    Target(name="ubuntu:18", os="ubuntu-18.04", continuous_only=True),
    Target(container="ubuntu:19.10", continuous_only=True),

    # The container gets stuck while running get-deps, so disable for now
    Target(container="ubuntu:20.04", continuous_only=True),

    # debian 8's wayland libraries are too old for wayland-client
    # Target(container="debian:8.11", continuous_only=True, bootstrap_git=True),

    Target(container="debian:9.12", continuous_only=True, bootstrap_git=True),
    Target(container="debian:10.3", continuous_only=True),
    Target(name="macos", os="macos-latest"),
    Target(container="fedora:31"),
    Target(container="fedora:32"),
    Target(container="centos:7", bootstrap_git=True),
    Target(container="centos:8"),
    Target(name="windows", os="vs2017-win2016", rust_target="x86_64-pc-windows-msvc"),
]


def generate_actions(namer, jobber, trigger, is_continuous):
    for t in TARGETS:
        #if t.continuous_only and not is_continuous:
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
  push:
    branches:
    - master
  pull_request:
    branches:
    - master
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
    - cron: "10 * * * *"
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
