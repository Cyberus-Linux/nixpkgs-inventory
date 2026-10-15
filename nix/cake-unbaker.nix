{
  perSystem = { pkgs, ... }: {
    packages = {
      cake-unbaker = pkgs.callPackage ./pkgs/cake-unbaker.nix { };
    };
  };
}
