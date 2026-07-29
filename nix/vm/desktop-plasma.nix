{ pkgs, ... }:

{
  services.desktopManager.plasma6.enable = true;
  services.displayManager.sddm = {
    enable = true;
    wayland = {
      enable = true;
    };
  };

  # Skip some optional Plasma packages
  # https://wiki.nixos.org/wiki/KDE#Excluding_applications_from_the_default_install
  environment.plasma6.excludePackages = with pkgs.kdePackages; [
    aurorae
    plasma-browser-integration
    plasma-workspace-wallpapers
    kwin-x11
    ark
    elisa
    gwenview
    okular
    kate
    khelpcenter
    baloo-widgets # baloo information in Dolphin
    dolphin-plugins
    ffmpegthumbs
    krdp
  ];
}
