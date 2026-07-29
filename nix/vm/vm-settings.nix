{ modulesPath, ... }:

{
  imports = [
    (modulesPath + "/profiles/qemu-guest.nix")
  ];

  services.qemuGuest.enable = true;

  virtualisation.vmVariant = {
    virtualisation = {
      graphics = true;

      # Bump machine settings (defaults are misarable, e.g. 1 core)
      cores = 4;
      memorySize = 6 * 1024;
      diskSize = 8192;
    };
  };
}
