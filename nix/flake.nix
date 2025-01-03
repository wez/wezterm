{
  description = "A GPU-accelerated cross-platform terminal emulator and multiplexer written by @wez and implemented in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # NOTE: @2024-05 Nix flakes does not support getting git submodules of 'self'.
    # refs:
    # - https://discourse.nixos.org/t/get-nix-flake-to-include-git-submodule/30324
    # - https://github.com/NixOS/nix/pull/7862
    #
    # ... In the meantime we kinda duplicate the dependencies here then replace the submodules with
    # links to each repo in package sources.
    #
    # Try to use tags when possible to increase readability
    # (note: `git submodule status` in wezterm repo will show the `git describe` result for each
    # submodule, can help finding a tag if any)
    freetype2 = {
      url = "github:freetype/freetype/VER-2-13-3";
      flake = false;
    };
    harfbuzz = {
      url = "github:harfbuzz/harfbuzz/9.0.0";
      flake = false;
    };
    libpng = {
      url = "github:pnggroup/libpng/v1.6.44";
      flake = false;
    };
    zlib = {
      url = "github:madler/zlib/v1.3.1";
      flake = false;
    };
  };

  outputs =
    inputs@{ self, ... }:
    inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import inputs.rust-overlay) ];
        pkgs = import (inputs.nixpkgs) { inherit system overlays; };

        inherit (inputs.nixpkgs) lib;
        inherit (pkgs) stdenv;

        nativeBuildInputs =
          with pkgs;
          [
            installShellFiles
            ncurses # tic for terminfo
            pkg-config
            python3
          ]
          ++ lib.optional stdenv.isDarwin perl;

        buildInputs =
          with pkgs;
          [
            fontconfig
            zlib
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
          ++ lib.optionals stdenv.isDarwin ([
            libiconv
          ]);

        libPath = lib.makeLibraryPath (
          with pkgs;
          [
            xorg.xcbutilimage
            libGL
            vulkan-loader
          ]
        );

        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.rust-bin.stable.latest.minimal;
          rustc = pkgs.rust-bin.stable.latest.minimal;
        };
      in
      {
        packages.default = rustPlatform.buildRustPackage rec {
          inherit buildInputs nativeBuildInputs;

          name = "wezterm";
          src = ./..;
          version = self.shortRev or "dev";

          cargoLock = {
            lockFile = ../Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          prePatch = ''
            rm -rf deps/freetype/{freetype2,libpng,zlib} deps/harfbuzz/harfbuzz

            ln -s ${inputs.freetype2} deps/freetype/freetype2
            ln -s ${inputs.libpng} deps/freetype/libpng
            ln -s ${inputs.zlib} deps/freetype/zlib
            ln -s ${inputs.harfbuzz} deps/harfbuzz/harfbuzz
          '';

          postPatch = ''
            echo ${version} > .tag

            # tests are failing with: Unable to exchange encryption keys
            rm -r wezterm-ssh/tests

            # hash does not work well with NixOS
            substituteInPlace assets/shell-integration/wezterm.sh \
              --replace-fail 'hash wezterm 2>/dev/null' 'command type -P wezterm &>/dev/null' \
              --replace-fail 'hash base64 2>/dev/null' 'command type -P base64 &>/dev/null' \
              --replace-fail 'hash hostname 2>/dev/null' 'command type -P hostname &>/dev/null' \
              --replace-fail 'hash hostnamectl 2>/dev/null' 'command type -P hostnamectl &>/dev/null'
          '';

          preFixup =
            lib.optionalString stdenv.isLinux ''
              patchelf \
                --add-needed "${pkgs.libGL}/lib/libEGL.so.1" \
                --add-needed "${pkgs.vulkan-loader}/lib/libvulkan.so.1" \
                $out/bin/wezterm-gui
            ''
            + lib.optionalString stdenv.isDarwin ''
              mkdir -p "$out/Applications"
              OUT_APP="$out/Applications/WezTerm.app"
              cp -r assets/macos/WezTerm.app "$OUT_APP"
              rm $OUT_APP/*.dylib
              cp -r assets/shell-integration/* "$OUT_APP"
              ln -s $out/bin/{wezterm,wezterm-mux-server,wezterm-gui,strip-ansi-escapes} "$OUT_APP"
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
            # the headless variant is useful when deploying wezterm's mux server on remote severs
            headless = rustPlatform.buildRustPackage {
              pname = "wezterm-headless";
              inherit
                version
                src
                postPatch
                cargoLock
                meta
                ;

              nativeBuildInputs = [ pkgs.pkg-config ];

              buildInputs = [ pkgs.openssl ];

              cargoBuildFlags = [
                "--package"
                "wezterm"
                "--package"
                "wezterm-mux-server"
              ];

              doCheck = false;

              postInstall = ''
                install -Dm644 assets/shell-integration/wezterm.sh -t $out/etc/profile.d
                install -Dm644 ${passthru.terminfo}/share/terminfo/w/wezterm -t $out/share/terminfo/w
              '';
            };

            terminfo =
              pkgs.runCommand "wezterm-terminfo"
                {
                  nativeBuildInputs = [ pkgs.ncurses ];
                }
                ''
                  mkdir -p $out/share/terminfo $out/nix-support
                  tic -x -o $out/share/terminfo ${src}/termwiz/data/wezterm.terminfo
                '';
          };

          meta.mainProgram = "wezterm";
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

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
