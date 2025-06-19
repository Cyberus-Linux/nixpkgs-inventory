{ lib
, rustPlatform
, writeShellScriptBin
, nix

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

  # Setting `COMMAND_NIX_*` in the environment at build-time makes
  # the tools keep an hermetic reference to the Nix CLI tooling used at build
  # time. See: e.g. `pub const COMMAND_NIX_INSTANTIATE` in the program source.
  COMMAND_NIX_INSTANTIATE = "${nix}/bin/nix-instantiate";
  COMMAND_NIX_STORE = "${nix}/bin/nix-store";

  passthru = {
    # Provide a default development shell helper to "just" run the command.
    dev-runner = writeShellScriptBin name ''
      PS4=" $ "; set -x
      exec cargo run --bin "${name}" -- "$@"
    '';
  };
})
