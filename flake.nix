{
  outputs = {nixpkgs,...}:
  let pkgs = nixpkgs;
  in
  {
    mkShell = import shell {inherit pkgs;};
  };
}
