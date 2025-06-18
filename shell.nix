# Arguments are documented in `default.nix`.
{ pkgs ? import nixpkgs_path {}
, nixpkgs_path ? (import ./npins).nixpkgs
, nix ? null
}:

(import ./. {
  inherit
    pkgs
    nix
  ;
}).dev-shell
