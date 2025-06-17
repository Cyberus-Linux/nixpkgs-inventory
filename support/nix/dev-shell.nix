{ lib
, mkShell
, cargo
, rustc
, clippy
, rustfmt

, subprojects
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
  nativeBuildInputs = [
    cargo
    rustc
    clippy
    rustfmt
  ]
    # Pick all `dev-runner` attributes on this project's packages.
    ++ (getSubprojectAttr null "dev-runner")
  ;
}
