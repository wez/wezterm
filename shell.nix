{pkgs ? import <nixpkgs> {}}:
with pkgs;
mkShell ({
  nativeBuildInputs = wezterm.nativeBuildInputs ++  [rust-analyzer rustfmt clippy];
  buildInputs = wezterm.buildInputs;
})
