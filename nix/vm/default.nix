{
  nixpkgs,
  system,
  desktopModule,
}:

nixpkgs.lib.nixosSystem {
  inherit system;
  modules = [
    ./base.nix
    ./vm-settings.nix
    ./patch-opengl.nix # 👀
    ./shared-repo.nix
    desktopModule
  ];
}
