{ lib
, rustPlatform
, writeShellScriptBin

, name
, src
}:

let
  subproject = src + "/${name}";
  # We're using the source input for all the actual information.
  CargoTOML = builtins.fromTOML (builtins.readFile (subproject + "/Cargo.toml"));
in
rustPlatform.buildRustPackage (rec {
  pname = CargoTOML.package.name;
  version = CargoTOML.package.version;

  # Workspace subproject
  buildAndTestSubdir = pname;

  inherit src;

  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  passthru = {
    # Provide a default development shell helper to "just" run the command.
    dev-runner = writeShellScriptBin name ''
      PS4=" $ "; set -x
      exec cargo run --bin "${name}" -- "$@"
    '';
  };
})
