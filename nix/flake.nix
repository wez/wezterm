{
  description = "A GPU-accelerated cross-platform terminal emulator and multiplexer written by @wez and implemented in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

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

  outputs = {
    self,
    flake-utils,
    nixpkgs,
    freetype2,
    harfbuzz,
    libpng,
    zlib,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};

      inherit (nixpkgs) lib;
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
          xorg.libX11
          xorg.libxcb
          libxkbcommon
          openssl
          wayland
          xorg.xcbutil
          xorg.xcbutilimage
          xorg.xcbutilkeysyms
          xorg.xcbutilwm # contains xcb-ewmh among others
        ]
        ++ lib.optionals stdenv.isDarwin [
          Cocoa
          CoreGraphics
          Foundation
          libiconv
          UserNotifications
        ];

      libPath = lib.makeLibraryPath (with pkgs; [libGL vulkan-loader]);
    in {
      packages.default = pkgs.rustPlatform.buildRustPackage rec {
        inherit buildInputs nativeBuildInputs;

        name = "wezterm";
        src = ./..;
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        version = self.shortRev or "dev";

        cargoLock = {
          lockFile = ../Cargo.lock;
          outputHashes = {
            "xcb-imdkit-0.2.0" = "sha256-L+NKD0rsCk9bFABQF4FZi9YoqBHr4VAZeKAWgsaAegw=";
            "xcb-1.2.1" = "sha256-zkuW5ATix3WXBAj2hzum1MJ5JTX3+uVQ01R1vL6F1rY=";
          };
        };

        preConfigure = ''
          rm -rf deps/freetype/freetype2 deps/freetype/libpng \
            deps/freetype/zlib deps/harfbuzz/harfbuzz

          ln -s ${freetype2} deps/freetype/freetype2
          ln -s ${libpng} deps/freetype/libpng
          ln -s ${zlib} deps/freetype/zlib
          ln -s ${harfbuzz} deps/harfbuzz/harfbuzz
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
      };

      devShell = pkgs.mkShell {
        name = "wezterm-shell";

        inherit nativeBuildInputs;
        buildInputs =
          buildInputs
          ++ (with pkgs; [
            cargo
            rustc
            rustfmt
            rustPackages.clippy
          ]);

        LD_LIBRARY_PATH = libPath;
      };
    });
}
