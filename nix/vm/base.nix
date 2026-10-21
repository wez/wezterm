{ pkgs, ... }:

{
  users.mutableUsers = false;
  users.groups.devs = { };
  users.users.dev = {
    isNormalUser = true;
    group = "devs";
    extraGroups = [ "wheel" ];
    initialPassword = "test";
  };
  services.displayManager.autoLogin = {
    enable = true;
    user = "dev";
  };

  environment.systemPackages = [
    pkgs.neovim
    pkgs.ripgrep
    pkgs.git
  ];

  fonts.enableDefaultPackages = true;
  fonts.packages = [pkgs.jetbrains-mono];

  nix.settings = {
    trusted-users = [
      "root"
      "dev"
    ];
    experimental-features = [
      "nix-command"
      "flakes"
    ];
  };

  documentation.nixos.enable = false;
  system.stateVersion = pkgs.lib.trivial.release;
}
