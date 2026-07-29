---
hide:
    - toc
---

=== "Flatpak"

    ## Installing on Linux via Flathub

    WezTerm is available in flatpak format and published on
    [Flathub](https://flathub.org/apps/details/org.wezfurlong.wezterm), which is
    aggregated into the GNOME Software application and other similar
    storefront/software catalog applications.

    !!! warning
        flatpaks run in an isolated sandbox which can cause some issues
        especially for power users. It is recommended that you graduate
        to a native package if/when you decide to fully embrace wezterm.

    <a href='https://flathub.org/apps/details/org.wezfurlong.wezterm'><img width='240' alt='Get it on Flathub' src='https://flathub.org/api/badge?locale=en'/></a>

    To install using the command line:

    First, [setup flatpak on your system](https://flatpak.org/setup/), then:

    ```console
    $ flatpak install flathub org.wezfurlong.wezterm
    ```

    and then run:

    ```console
    $ flatpak run org.wezfurlong.wezterm
    ```

    You may wish to define an alias for convenience:

    ```console
    $ alias wezterm='flatpak run org.wezfurlong.wezterm'
    ```

    !!! note
        flatpaks run in an isolated sandbox so some functionality may behave a little
        differently when compared to installing the native package format for your
        system.

        * starting wezterm using `wezterm cli` subcommands will block on the first
        run since you logged in if you haven't already launched the gui.
        * Process inspection functions such as determining the current directory
        for a pane will not work

        The flatpak is provided primarily for ease of trying out wezterm with
        low commitment, and you are encouraged to use native packages for your
        system once you're ready to get the most out of wezterm.

    Only stable releases are allowed to be published to Flathub, so if
    you want/need to try a nightly download you will need to use one of
    the other installation options.

=== "AppImage"

    ## Installing on Linux using AppImage

    WezTerm is available in [AppImage](https://appimage.org/) format; a
    self-contained single file that doesn't require installation or
    any special privileges to run, and that is compatible with a wide
    range of Linux distributions.

    Download and make the file executable and you're ready to run!

    [AppImage :material-tray-arrow-down:]({{ ubuntu20_AppImage_stable }}){ .md-button }
    [Nightly AppImage :material-tray-arrow-down:]({{ ubuntu24_AppImage_nightly }}){ .md-button }

    ```console
    $ curl -LO {{ ubuntu24_AppImage_stable }}
    $ chmod +x {{ ubuntu24_AppImage_stable_asset }}
    ```

    You may then execute the appimage directly to launch wezterm, with no
    specific installation steps required:

    ```console
    $ ./{{ ubuntu24_AppImage_stable_asset }}
    ```

    That said, you may wish to make it a bit more convenient:

    ```console
    $ mkdir ~/bin
    $ mv ./{{ ubuntu24_AppImage_stable_asset }} ~/bin/wezterm
    $ ~/bin/wezterm
    ```

    * Configuration instructions can be [found here](../config/files.md)

=== "Ubuntu/Debian"
    ## Using the APT repo

    You can configure your system to use that APT repo by following these
    steps:

    ```console
    $ curl -fsSL https://apt.fury.io/wez/gpg.key | sudo gpg --yes --dearmor -o /usr/share/keyrings/wezterm-fury.gpg
    $ echo 'deb [signed-by=/usr/share/keyrings/wezterm-fury.gpg] https://apt.fury.io/wez/ * *' | sudo tee /etc/apt/sources.list.d/wezterm.list
    $ sudo chmod 644 /usr/share/keyrings/wezterm-fury.gpg
    ```

    Update your dependencies:

    ```console
    $ sudo apt update
    ```

    Now you can install wezterm:

    ```console
    $ sudo apt install wezterm
    ```

    or to install a nightly build:

    ```console
    $ sudo apt install wezterm-nightly
    ```

    !!! note
        The nightly build conflicts with the regular build, so you may install
        one or the other, but not both at the same time.

    ## Pre-built `.deb` packages

    The CI system builds `.deb` files for a variety of Ubuntu and Debian
    distributions.  These are often compatible with other Debian style systems;
    if you don't find one that exactly matches your system you can try
    installing one from an older version of your distribution, or use one
    of the Debian packages linked below.  Failing that, you can try the
    AppImage download which should work on most Linux systems.

    |Distro      | Stable   |        | Nightly|            |
    |------------|----------|--------|--------|------------|
    |Ubuntu20    |[amd64]({{ ubuntu20_deb_stable }}) ||No longer supported| |
    |Ubuntu22    |[amd64]({{ ubuntu22_deb_stable }}) |[arm64]({{ ubuntu22_arm64_deb_stable}})|[amd64]({{ ubuntu22_deb_nightly }})|[arm64]({{ ubuntu22_arm64_deb_nightly}})|
    |Ubuntu24    |Nightly Only                       |Nightly Only                           |[amd64]({{ ubuntu24_deb_nightly }})|[arm64]({{ ubuntu24_arm64_deb_nightly}})|
    |Debian11    |[amd64]({{ debian11_deb_stable }}) ||No longer supported| |
    |Debian12    |[amd64]({{ debian12_deb_stable }}) |[arm64]({{ debian12_arm64_deb_stable }})|[amd64]({{ debian12_deb_nightly }})|[arm64]({{ debian12_arm64_deb_nightly }}) |

    To download and install from the CLI, you can use something like this, which
    shows how to install the Ubuntu 22 package:

    ```console
    $ curl -LO {{ ubuntu22_deb_stable }}
    $ sudo apt install -y ./{{ ubuntu22_deb_stable_asset }}
    ```

    * The package installs `/usr/bin/wezterm` and `/usr/share/applications/org.wezfurlong.wezterm.desktop`
    * Configuration instructions can be [found here](../config/files.md)

=== "Copr"
    ## Installing on Fedora and rpm-based Systems via Copr

    Nightly builds of wezterm are now available via the
    [Copr](https://copr.fedorainfracloud.org/) build service.

    You can see the current list of available distributions and architectures
    [on the wezterm-nightly project
    page](https://copr.fedorainfracloud.org/coprs/wezfurlong/wezterm-nightly/).
    At the time that this page was written, the following distributions are
    available in Copr for `x86_64` and `aarch64`:

    * Centos Stream 8 and 9
    * Fedora 38, 39, 40, rawhide
    * openSUSE Leap 15.5
    * openSUSE Tumbleweed
    * RHEL 8, 9


    To perform initial installation:

    ```console
    $ sudo dnf copr enable wezfurlong/wezterm-nightly
    $ sudo dnf install wezterm
    ```
    
    ## Fedora Atomic Desktops (Silverblue, Kinoite) specific

    To perform initial installation:

    ```console
    $ sudo wget https://copr.fedorainfracloud.org/coprs/wezfurlong/wezterm-nightly/repo/fedora-$(rpm -E %fedora)/wezfurlong-wezterm-nightly-fedora-$(rpm -E %fedora).repo -O /etc/yum.repos.d/_copr:copr.fedorainfracloud.org:wezfurlong:wezterm-nightly.repo
    $ sudo rpm-ostree install wezterm
    ```

    ## openSUSE specific

    To perform initial installation:

    ```console
    $ sudo zypper in dnf
    $ sudo dnf copr enable wezfurlong/wezterm-nightly <repository>
    ```
    where `<repository>` is one of the following, depending on the flavor and architecture:
    `opensuse-tumbleweed-x86_64`, `opensuse-tumbleweed-aarch64`, `opensuse-leap-15.5-x86_64`, `opensuse-leap-15.5-aarch64`.

    ```console
    $ sudo dnf install wezterm
    ```

    ## Update

    ```console
    $ sudo dnf update wezterm
    ```

=== "Fedora/RPM"
    ## Installing on Fedora and rpm-based Systems

    !!! note
        It is recommended that you install via Copr so that it is easiest
        to stay up to date as future versions of wezterm are released.

    The CI system builds `.rpm` files on CentOS and Fedora systems.
    These are likely compatible with other rpm-based distributions.
    Alternatively, you can try the AppImage download with should work
    on most Linux systems.

    |Distro      | Stable           | Nightly             |
    |------------|------------------|---------------------|
    |CentOS8     |[{{ centos8_rpm_stable_asset }}]({{ centos8_rpm_stable }}) |No longer supported|
    |CentOS9     |[{{ centos9_rpm_stable_asset }}]({{ centos9_rpm_stable }})|[{{ centos9_rpm_nightly_asset }}]({{ centos9_rpm_nightly }})|
    |Fedora37    |[{{ fedora37_rpm_stable_asset }}]({{ fedora37_rpm_stable }})|No longer supported|
    |Fedora38    |[{{ fedora38_rpm_stable_asset }}]({{ fedora38_rpm_stable }})|No longer supported|
    |Fedora39    |[{{ fedora39_rpm_stable_asset }}]({{ fedora39_rpm_stable }})|No longer supported|
    |Fedora40    |Nightly only|[{{ fedora40_rpm_nightly_asset }}]({{ fedora40_rpm_nightly }})|
    |Fedora41    |Nightly only|[{{ fedora41_rpm_nightly_asset }}]({{ fedora41_rpm_nightly }})|

    To download and install from the CLI you can use something like this, which
    shows how to install the Fedora 39 package:

    ```console
    $ sudo dnf install -y {{ fedora39_rpm_stable }}
    ```

=== "openSUSE"
    ## openSUSE

    !!! note
        It is recommended that you install via Copr so that it is easiest
        to stay up to date as future versions of wezterm are released.

    ## openSUSE Tumbleweed/Slowroll

    The stable version of WezTerm is available in the official repositories.

    ```console
    $ zypper install wezterm
    ```

    ## openSUSE Leap

    Use Copr or build if from source.

=== "Arch"
    ## Arch Linux

    WezTerm is available in the [Extra
    repository](https://archlinux.org/packages/extra/x86_64/wezterm/).

    Be sure to also install the `ttf-nerd-fonts-symbols-mono` package!

    The version available in the extra repository may lag behind the latest
    wezterm release, so you may wish to use one of these AUR options:

    |What                 |Where|
    |---------------------|-|
    |Build from source    |<https://aur.archlinux.org/packages/wezterm-git/>|

=== "Linuxbrew"
    ## Linuxbrew Tap

    If you are a [Linuxbrew](https://docs.brew.sh/Homebrew-on-Linux) user, you
    can install wezterm from our tap:

    ```console
    $ brew tap wezterm/wezterm-linuxbrew
    $ brew install wezterm/wezterm-linuxbrew/wezterm
    ```

    !!! note
        The fully-qualified `wezterm/wezterm-linuxbrew/wezterm` name is
        required on Linux because `wezterm` in `homebrew-core` is a
        macOS-only cask and `brew install wezterm` would resolve to
        that and fail with "macOS is required for this software".

    If you'd like to use a nightly build you can perform a head install:

    ```console
    $ brew install --HEAD wezterm/wezterm-linuxbrew/wezterm
    ```

    to upgrade to a newer nightly, it is simplest to remove then
    install:

    ```console
    $ brew rm wezterm
    $ brew install --HEAD wezterm/wezterm-linuxbrew/wezterm
    ```
=== "Nix/NixOS"

    ## Nix
    
    WezTerm is available in nixpkgs as `wezterm`.

    ```nix
    {
        # configuration.nix

        environment.systemPackages = [
            pkgs.wezterm
        ]
    }
    ```

    !!! note "Git must be available in $PATH before attempting install"

        The Wezterm package uses Nix's `builtins.fetchGit` which depends on the `git`
        binary being available in `$PATH` during the _evaluation_ phase (before building packages).

        Git must be installed before attempting to install wezterm.

        Note: `builtins.fetchGit` is used because of `cargoLock.allowBuiltinFetchGit` in `buildRustPackage` call.

        (This is a known Nix issue, tracked in [nix#3533](https://github.com/NixOS/nix/issues/3533)
        & [nix#9807](https://github.com/NixOS/nix/issues/9807))


    ### Flake
    
    If you need a newer version use the flake. Use the cachix if you want to avoid building WezTerm from source.

    The flake is in the `nix` directory, so the url will be something like `github:wezterm/wezterm?dir=nix`

    Here's an example for NixOS configurations:
    
    ```nix
    {
        inputs.wezterm.url = "github:wezterm/wezterm?dir=nix";
        # ...

        outputs = inputs @ {nixpkgs, ...}:{
            nixosConfigurations.HOSTNAME = nixpkgs.lib.nixosSystem {
                specialArgs = { inherit inputs; }; # Make sure you pass inputs through to your nixosConfiguration like this
                modules = [
                    # ...
                ];
            };
        };
    }
    ```
    And for home-manager you can do the following:

    ```nix
    # flake.nix
    
    {
        inputs.wezterm.url = "github:wezterm/wezterm?dir=nix";
        # ...

        outputs = inputs @ {nixpkgs, home-manager, ...}:{
            homeConfigurations."user@HOSTNAME" = home-manager.lib.homeManagerConfiguration {
                pkgs = nixpkgs.legacyPackages.x86_64-linux;
                extraSpecialArgs = { inherit inputs; }; # Pass inputs to homeManagerConfiguration
                modules = [
                    ./home.nix
                ];
            };        
        };
    }
    ```
    ```nix
    # home.nix
    
    {inputs, pkgs, ...}:{
        programs.wezterm = {
            enable = true;
            package = inputs.wezterm.packages.${pkgs.system}.default;
        };
    }
    ```


    ### Cachix

    Successful builds of the nightly nix action are pushed to this binary cache.

    ```nix
    # nixosConfiguration module
    {
        nix.settings = {
            substituters = ["https://wezterm.cachix.org"];
            trusted-public-keys = ["wezterm.cachix.org-1:kAbhjYUC9qvblTE+s7S+kl5XM1zVa4skO+E/1IDWdH0="];
        };
    }
    ```

=== "Void"
    ## Void Linux

    ```console
    $ sudo xbps-install -S wezterm
    ```

    Be sure to also install the `nerd-fonts-ttf` package!

=== "Gentoo"
    ## Gentoo Linux

    WezTerm stable/release is available in main Gentoo tree:

    ```console
    $ sudo emerge -a x11-terms/wezterm
    ```

    WezTerm nightly build is available in [Gentoo GURU](https://wiki.gentoo.org/wiki/Project:GURU/Information_for_End_Users) overlay/repository

    First install `app-eselect/eselect-repository` if it is not already installed, and next enable the GURU repository:

    ```console
    $ sudo emerge -a app-eselect/eselect-repository
    $ sudo eselect repository enable guru
    ```

    Install/Update `x11-terms/wezterm-nightly-bin`:

    ```console
    $ sudo emerge -a x11-terms/wezterm-nightly-bin
    ```

    !!! note
        `wezterm-nightly-bin` is a live ebuild, it will pull the latest
        nightly binary everytime it's merged. Also, it's a binary
        package and therefore not built from source.

=== "Raw"
    ## Raw Linux Binary

    Another option for linux is a raw binary archive.  These are the same
    binaries that are built for Ubuntu but provided in a tarball.

    [Raw Linux Binary :material-tray-arrow-down:]({{ linux_raw_bin_stable }}){ .md-button }
    [Nightly Raw Linux Binary :material-tray-arrow-down:]({{ linux_raw_bin_nightly }}){ .md-button }


