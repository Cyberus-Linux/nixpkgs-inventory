{
# A Nixpkgs to build from, defaults to importing `nixpkgs_path`
  pkgs ? import nixpkgs_path {}
# When no `pkgs` is provided, refers to the default Nixpkgs from npins.
, nixpkgs_path ? (import ./npins).nixpkgs
# Name of the attribute used for the Nix package.
, nix ? "nix"
# The workspace source, for internal use mainly.
, src ? ./.
}:

# This is not using `callPackage` so `override`* isn't exposed to the CLI.
import ./support/nix/packages.nix {
  inherit (pkgs) lib;
  inherit
    pkgs
    src
  ;
  nix =
    if nix == null
    then null
    else pkgs."${nix}"
  ;
}
