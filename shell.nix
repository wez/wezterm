{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  buildInputs = with pkgs; pkgs.wezterm.buildInputs ++ [cargo rustc rust-analyzer];
}
