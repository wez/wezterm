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

        if "\n" in run:
            spacer = " " * 8
            f.write(f"      run: |\n")
            f.write(spacer + run.replace("\n", "\n" + spacer) + "\n")
        else:
            f.write(f"      run: {yv(run)}\n")


class ActionStep(Step):
    def __init__(self, name, action, params=None):
        self.name = name
        self.action = action
        self.params = params

    def render(self, f, env):
        f.write(f"    - name: {yv(self.name)}\n")
        f.write(f"      uses: {self.action}\n")
        if self.params:
            f.write("      with:\n")
            for k, v in self.params.items():
                f.write(f"         {k}: {yv(v)}\n")


class CacheStep(ActionStep):
    def __init__(self, name, path, key):
        super().__init__(
            name, action="actions/cache@v1", params={"path": path, "key": key}
        )


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo"):
        # We use v1 rather than v2 or later because v1 handles
        # submodules in a convenient way.  Subsequent versions
        # require multiple lines of boilerplate that seem fragile
        # if things change in the future.
        super().__init__(name, action="actions/checkout@v1")


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

    def uses_yum(self):
        if "fedora" in self.name:
            return True
        if "centos" in self.name:
            return True
        return False

    def install_sudo(self):
        if self.uses_yum():
            return [RunStep("Install Sudo", "yum install -y sudo")]
        return []

    def install_git(self):
        if self.bootstrap_git:
            return [
                CacheStep(
                    "Cache Git installation",
                    path="/usr/local/git",
                    key=f"{self.name}-git",
                ),
                RunStep(
                    name="Install Git from source",
                    shell="bash",
                    run="""
VERS=2.25.0

if test ! -x /usr/local/git/bin/git ; then
    yum install -y wget curl-devel expat-devel gettext-devel openssl-devel zlib-devel gcc perl-ExtUtils-MakeMaker make
    cd /tmp
    wget https://mirrors.edge.kernel.org/pub/software/scm/git/git-$VERS.tar.gz
    tar xzf git-$VERS.tar.gz
    cd git-$VERS
    make prefix=/usr/local/git install
fi

ln -s /usr/local/git/bin/git /usr/local/bin/git
        """,
                ),
            ]
        if self.uses_yum():
            return [RunStep(name="Install System Git", run="sudo yum install -y git")]
        return []

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
        return [RunStep(name="Install System Deps", run="sudo ./get-deps")]

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
        return [RunStep("Package", "bash ci/deploy.sh")]

    def upload_artifact(self):
        run = "mkdir pkg_\n"
        if self.uses_yum():
            run += "mv ~/rpmbuild/RPMS/*/*.rpm pkg_"
        if ("win" in self.name) or ("mac" in self.name):
            run += "mv *.zip pkg_"
        if "ubuntu" in self.name:
            run += "mv *.deb *.xz *.AppImage pkg_"

        return [
            RunStep("Move Package for artifact upload", run),
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@master",
                params={"name": self.name, "path": "pkg_"},
            ),
        ]

    def global_env(self):
        env = {}
        if "macos" in self.name:
            env["MACOSX_DEPLOYMENT_TARGET"] = "10.9"
        return env

    def prep_environment(self):
        steps = []
        steps += self.install_sudo()
        steps += self.install_git()
        steps += [CheckoutStep()]
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


TARGETS = [
    Target(name="ubuntu:16", os="ubuntu-16.04"),
    Target(name="macos", os="macos-latest"),
    Target(container="fedora:31"),
    Target(container="centos:7", bootstrap_git=True),
    Target(name="windows", os="vs2017-win2016", rust_target="x86_64-pc-windows-msvc"),
]


def generate_pr_actions():
    for t in TARGETS:
        name = f"{t.name}_pr".replace(":", "")
        print(name)
        job = t.pull_request()
        file_name = f".github/workflows/gen_{name}.yml"
        if job.container:
            container = f"container: {yv(job.container)}"
        else:
            container = ""

        with open(file_name, "w") as f:
            f.write(
                f"""
name: {name}
on:
  push:
    branches:
    - master
  pull_request:
    branches:
    - master

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


generate_pr_actions()
