{
# A Nixpkgs to build from, defaults to importing `nixpkgs_path`
  pkgs ? import nixpkgs_path {}
# When no `pkgs` is provided, refers to the default Nixpkgs from npins.
, nixpkgs_path ? (import ./npins).nixpkgs
# Name of the attribute used for the Nix package.
, nix ? "nix"
# The workspace source, for internal use mainly.
, src ? ./.
# The cargo input hash for this workspace, for internal use mainly.
, cargoHash ? "sha256-KCbWoYWQ5Dou+Ii1ZCrm7Q7n5XOZS2MigLPP5XszXkQ="
}:

# This is not using `callPackage` so `override`* isn't exposed to the CLI.
import ./support/nix/packages.nix {
  inherit (pkgs) lib;
  inherit
    pkgs
    src
    cargoHash
  ;
  nix = pkgs."${nix}";
}
