# Arguments are documented in `default.nix`.
{ pkgs ? import nixpkgs_path {}
, nixpkgs_path ? (import ./npins).nixpkgs
, nix ? "nix"
}:

(import ./. {
  inherit
    pkgs
    nix
  ;
}).dev-shell
