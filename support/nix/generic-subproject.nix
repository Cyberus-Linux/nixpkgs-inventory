{ lib
, rustPlatform
, writeShellScriptBin

, name
, src
, cargoHash
}:

let
  # We're using the source input for all the actual information.
  CargoTOML = builtins.fromTOML (builtins.readFile (src + "/${name}/Cargo.toml"));
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = CargoTOML.package.name;
  version = CargoTOML.package.version;

  # Workspace subproject
  buildAndTestSubdir = finalAttrs.pname;

  inherit src;
  inherit cargoHash;

  passthru = {
    # Provide a default development shell helper to "just" run the command.
    dev-runner = writeShellScriptBin name ''
      PS4=" $ "; set -x
      exec cargo run --bin "${name}" -- "$@"
    '';
  };
})
