# flake.nix
{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    naersk.url = "github:nmattia/naersk";

  };

  outputs = { self, fenix, flake-utils, nixpkgs, naersk,}:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system} // { inherit (fenix.packages.${system}.latest) cargo rustc rust-src; };
        inherit (pkgs) lib stdenv;

      package = pkgs.wezterm;

      in
      rec {
        devShell = pkgs.mkShell {
          inherit (defaultPackage) nativeBuildInputs;
          buildInputs = with pkgs; [ rust-analyzer rustfmt ] ++ defaultPackage.buildInputs;
          RUST_SRC_PATH = "${pkgs.rust-src}/lib/rustlib/src/rust/library";

        };
        defaultPackage = package;
      });
}

