{
  perSystem = { pkgs, self', ... }: {
    packages = {
      default = pkgs.callPackage ./pkgs/default-package.nix {
        inherit (self'.packages) cake-unbaker;
      };
    };
  };
}
