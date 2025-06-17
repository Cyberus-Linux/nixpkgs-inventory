let
  # Import the default set.
  self = import ../.. {};
  inherit (self)
    pkgs
  ;
  inherit (pkgs)
    recurseIntoAttrs
    nixVersions
    lix
  ;

  packagesFor =
    nix:
    recurseIntoAttrs (import ./packages.nix {
      inherit nix;
      inherit (pkgs)
        lib
        pkgs
      ;
      inherit (self.cu-fast-eval)
        cargoHash
        src
      ;
    })
  ;
in
{
  # We are checking against the major Nix implementations packaged in Nixpkgs.
  withAlternative = recurseIntoAttrs {
    stable  = packagesFor nixVersions.stable;
    latest  = packagesFor nixVersions.latest;

    # Alternative implementation of the Nix tooling.
    lix     = packagesFor lix;

    # NOTE: While the package hopefully will build, as Nixpkgs shouldn't pick
    #       an update for Nix that won't build, we could make this a non-failing check.
    git     = packagesFor nixVersions.git;

    # NOTE: This is the minimum required to minimally evaluate Nixpkgs.
    #       If it becomes a problem for the checks, we can make it a non-failing check.
    minimum = packagesFor nixVersions.minimum;
  };
}
