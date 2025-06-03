{ pkgs ? import nixpkgs_path {}
, nixpkgs_path ? (import ./npins).nixpkgs
}:

(import ./. { inherit pkgs; }).dev-shell
