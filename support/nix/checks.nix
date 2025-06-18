#
# The arguments need to be provided at the base of the expression, or else
# the Nix CLI will not pass arguments as expected.
#
# Keep those input argument names in sync with `default.nix`.
#
# We're using the `null` value as a flag to imply the value should not be
# passed down to the imported `default.nix` expression.
#
# The default values are defined in `default.nix` at the root of the project.
#
{
# A Nixpkgs to build from.
  pkgs ? null
# Path used to import Nixpkgs from.
, nixpkgs_path ? null
}:

# This `let ... in` block cannot rely on Nixpkgs lib.
let
  optionalAttrs =
    condition: attrs:
    if condition then attrs else {}
  ;
  args =
    {}
    // (optionalAttrs (pkgs != null) { inherit pkgs; })
    // (optionalAttrs (nixpkgs_path != null) { inherit nixpkgs_path; })
  ;
in

let
  # Import the default set.
  self = import ../.. args;
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
