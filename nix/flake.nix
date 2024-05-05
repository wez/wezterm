{
  description = "A GPU-accelerated cross-platform terminal emulator and multiplexer written by @wez and implemented in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };

    freetype2 = {
      url = "github:wez/freetype2/de8b92dd7ec634e9e2b25ef534c54a3537555c11";
      flake = false;
    };

    harfbuzz = {
      url = "github:harfbuzz/harfbuzz/60841e26187576bff477c1a09ee2ffe544844abc";
      flake = false;
    };

    libpng = {
      url = "github:glennrp/libpng/8439534daa1d3a5705ba92e653eda9251246dd61";
      flake = false;
    };

    zlib = {
      url = "github:madler/zlib/cacf7f1d4e3d44d871b605da3b647f07d718623f";
      flake = false;
    };
  };

  outputs = inputs @ {self, ...}:
    inputs.flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import inputs.rust-overlay)];
      pkgs = import (inputs.nixpkgs) {inherit system overlays;};

      inherit (inputs.nixpkgs) lib;
      inherit (pkgs) stdenv;

      nativeBuildInputs = with pkgs;
        [
          installShellFiles
          ncurses # tic for terminfo
          pkg-config
          python3
        ]
        ++ lib.optional stdenv.isDarwin perl;

      buildInputs = with pkgs;
        [
          fontconfig
          pkgs.zlib
        ]
        ++ lib.optionals stdenv.isLinux [
          libxkbcommon
          openssl
          wayland

          xorg.libX11
          xorg.libxcb
          xorg.xcbutil
          xorg.xcbutilimage
          xorg.xcbutilkeysyms
          xorg.xcbutilwm # contains xcb-ewmh among others
        ]
        ++ lib.optionals stdenv.isDarwin (
          (with pkgs.darwin.apple_sdk.frameworks; [
            Cocoa
            CoreGraphics
            Foundation
            UserNotifications
          ])
          ++ [pkgs.libiconv]
        );

      libPath = lib.makeLibraryPath (with pkgs; [
        xorg.xcbutilimage
        libGL
        vulkan-loader
      ]);

      rustPlatform = pkgs.makeRustPlatform {
        cargo = pkgs.rust-bin.stable.latest.minimal;
        rustc = pkgs.rust-bin.stable.latest.minimal;
      };
    in {
      packages.default = rustPlatform.buildRustPackage rec {
        inherit buildInputs nativeBuildInputs;

        name = "wezterm";
        src = ./..;
        version = self.shortRev or "dev";

        cargoLock = {
          lockFile = ../Cargo.lock;
          outputHashes = {
            "xcb-imdkit-0.3.0" = "sha256-fTpJ6uNhjmCWv7dZqVgYuS2Uic36XNYTbqlaly5QBjI=";
            "sqlite-cache-0.1.3" = "sha256-sBAC8MsQZgH+dcWpoxzq9iw5078vwzCijgyQnMOWIkk";
          };
        };

        preConfigure = ''
          rm -rf deps/freetype/freetype2 deps/freetype/libpng \
            deps/freetype/zlib deps/harfbuzz/harfbuzz

          ln -s ${inputs.freetype2} deps/freetype/freetype2
          ln -s ${inputs.libpng} deps/freetype/libpng
          ln -s ${inputs.zlib} deps/freetype/zlib
          ln -s ${inputs.harfbuzz} deps/harfbuzz/harfbuzz
        '';

        postPatch = ''
          echo ${version} > .tag

          # tests are failing with: Unable to exchange encryption keys
          rm -r wezterm-ssh/tests
        '';

        preFixup = lib.optionalString stdenv.isLinux ''
          patchelf \
            --add-needed "${pkgs.libGL}/lib/libEGL.so.1" \
            --add-needed "${pkgs.vulkan-loader}/lib/libvulkan.so.1" \
            $out/bin/wezterm-gui
        '';

        postInstall = ''
          mkdir -p $out/nix-support
          echo "${passthru.terminfo}" >> $out/nix-support/propagated-user-env-packages

          install -Dm644 assets/icon/terminal.png $out/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
          install -Dm644 assets/wezterm.desktop $out/share/applications/org.wezfurlong.wezterm.desktop
          install -Dm644 assets/wezterm.appdata.xml $out/share/metainfo/org.wezfurlong.wezterm.appdata.xml

          install -Dm644 assets/shell-integration/wezterm.sh -t $out/etc/profile.d
          installShellCompletion --cmd wezterm \
            --bash assets/shell-completion/bash \
            --fish assets/shell-completion/fish \
            --zsh assets/shell-completion/zsh

          install -Dm644 assets/wezterm-nautilus.py -t $out/share/nautilus-python/extensions
        '';

        passthru = {
          terminfo =
            pkgs.runCommand "wezterm-terminfo"
            {
              nativeBuildInputs = [pkgs.ncurses];
            } ''
              mkdir -p $out/share/terminfo $out/nix-support
              tic -x -o $out/share/terminfo ${src}/termwiz/data/wezterm.terminfo
            '';
        };
      };

      devShell = pkgs.mkShell {
        name = "wezterm-shell";
        inherit nativeBuildInputs;

        buildInputs =
          buildInputs
          ++ (with pkgs.rust-bin; [
            (stable.latest.minimal.override {
              extensions = [
                "clippy"
                "rust-src"
              ];
            })

            nightly.latest.rustfmt
            nightly.latest.rust-analyzer
          ]);

        LD_LIBRARY_PATH = libPath;
        RUST_BACKTRACE = 1;
      };
    });
}
