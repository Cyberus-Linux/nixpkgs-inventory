{ lib
, mkShell
, cargo
, rustc
, clippy
, rustfmt

, subprojects
, nix
}:

let
  getSubprojectAttr =
    default: attr:
    lib.mapAttrsToList (
      name: drv:
      drv.${attr} or default
    ) subprojects
  ;
in
mkShell {
  buildInputs = [
  ]
    # Pick all subprojects buildInputs.
    ++ (getSubprojectAttr [] "buildInputs")
  ;

  nativeBuildInputs = [
    nix
    cargo
    rustc
    clippy
    rustfmt
  ]
    # Pick all `dev-runner` attributes on this project's packages.
    ++ (getSubprojectAttr null "dev-runner")
    # Pick all subprojects nativeBuildInputs.
    ++ (getSubprojectAttr [] "nativeBuildInputs")
  ;
}
