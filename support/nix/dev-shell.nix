{ lib
, mkShell
, cargo
, rustc
, clippy
, rustfmt

, subprojects
}:
mkShell {
  nativeBuildInputs = [
    cargo
    rustc
    clippy
    rustfmt
  ]
  ++ (
    # Pick all `dev-runner` attributes on this project's packages.
    lib.mapAttrsToList (
      name: drv:
      drv.dev-runner or null
    ) subprojects
  )
  ;
}
