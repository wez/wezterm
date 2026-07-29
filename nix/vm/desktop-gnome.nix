{ pkgs, ... }:

{
  services.displayManager.gdm.enable = true;
  services.desktopManager.gnome = {
    enable = true;
    # The following overrides are necessary to have fractional scaling settings available
    # (not just 100%/200%)
    # ref: https://discourse.nixos.org/t/how-to-set-fractional-scaling-via-nix-configuration-for-gnome-wayland/56774/4
    extraGSettingsOverridePackages = [ pkgs.mutter ];
    extraGSettingsOverrides = ''
      [org.gnome.mutter]
      experimental-features=['scale-monitor-framebuffer', 'xwayland-native-scaling']
    '';
  };

  # Skip adding many Gnome packages (lighter VM!)
  # Copied from https://github.com/ghostty-org/ghostty/blob/main/nix/vm/common-gnome.nix
  environment.gnome.excludePackages = with pkgs; [
    atomix
    baobab
    cheese
    epiphany
    evince
    file-roller
    geary
    gnome-backgrounds
    gnome-calculator
    gnome-calendar
    gnome-clocks
    gnome-connections
    gnome-contacts
    gnome-disk-utility
    gnome-extension-manager
    gnome-logs
    gnome-maps
    gnome-music
    gnome-photos
    gnome-software
    gnome-system-monitor
    gnome-text-editor
    gnome-themes-extra
    gnome-tour
    gnome-user-docs
    gnome-weather
    hitori
    iagno
    loupe
    orca
    seahorse
    simple-scan
    snapshot
    sushi
    tali
    totem
    yelp
  ];
}
