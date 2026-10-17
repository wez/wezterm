{ ... }:

let
  repoTarget = "/home/dev/repo";
in
{
  virtualisation.vmVariant = {
    virtualisation.sharedDirectories."phiterm-repo" = {
      # NOTE: env var needs to be set on VM start!
      source = ''"$REPO"'';
      target = repoTarget;
    };
  };

  fileSystems.${repoTarget} = {
    device = "phiterm-repo";
    fsType = "9p";
    options = [
      # automount to keep boot fast
      "x-systemd.automount"
      # Read/Write access
      "rw"
      # noauto/nofail so the guest still boots even if the host path disappears
      "noauto"
      "nofail"
    ];
  };
}
