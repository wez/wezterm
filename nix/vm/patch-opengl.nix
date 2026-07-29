{ pkgs, ... }:

{
  hardware.graphics = {
    enable = true;
    extraPackages = [
      pkgs.mesa
      pkgs.libglvnd # provides libEGL.so.1
    ];
  };
  # NOTE: I managed to start the wezterm binary with manual LD patching:
  # ```
  # find -name 'libEGL.so.1' 2>/dev/null
  # LD_LIBRARY_PATH=/nix/store/path/to/libglvnd/lib ./target/debug/wezterm-gui
  # ```
  # (although I got many error/warning from graphics rendering on start)
  # So we patch that in the env directly 👀
  environment.sessionVariables = {
    LD_LIBRARY_PATH = [ "${pkgs.libglvnd}/lib" ];
  };
}
