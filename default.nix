{
# A Nixpkgs to build from, defaults to importing `nixpkgs_path`
  pkgs ? import nixpkgs_path {}
# When no `pkgs` is provided, refers to the default Nixpkgs from npins.
, nixpkgs_path ? (import ./npins).nixpkgs
# The workspace source, for internal use mainly.
, src ? ./.
# The cargo input hash for this workspace, for internal use mainly.
, cargoHash ? "sha256-2UBTo+MNDY6dQj30qlmfQQ+UMuJVi2Uwm8IHOG4684g="
}:

# This is not using `callPackage` so `override`* isn't exposed to the CLI.
import ./support/nix/packages.nix {
  inherit (pkgs) lib;
  inherit
    pkgs
    src
    cargoHash
  ;
}
