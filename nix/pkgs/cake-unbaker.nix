{
  lib,
  rustPlatform,
  pkg-config,
  sqlite,
  nix,
}:

let
  workspaceRoot = ../../workspace;
  CargoTOML = builtins.fromTOML (builtins.readFile (workspaceRoot + "/cake-unbaker/Cargo.toml"));
in
rustPlatform.buildRustPackage {
  pname = "cake-unbaker";
  inherit (CargoTOML.package) version;

  src = lib.fileset.toSource {
    root = workspaceRoot;
    fileset = lib.fileset.difference workspaceRoot (
      lib.fileset.maybeMissing (workspaceRoot + "/target")
    );
  };

  cargoLock.lockFile = workspaceRoot + "/Cargo.lock";

  buildInputs = [ sqlite ];
  nativeBuildInputs = [ pkg-config ];

  # Setting `COMMAND_NIX_*` in the environment at build-time makes
  # the tools keep an hermetic reference to the Nix CLI tooling used at build
  # time. See: e.g. `pub const COMMAND_NIX_INSTANTIATE` in the program source.
  COMMAND_NIX = "${nix}/bin/nix";
  COMMAND_NIX_INSTANTIATE = "${nix}/bin/nix-instantiate";
  COMMAND_NIX_STORE = "${nix}/bin/nix-store";

  # Tests evaluate Nix expressions. They need a writable store.
  preCheck = ''
    export NIX_REMOTE="local?root=$PWD"
    "$COMMAND_NIX_INSTANTIATE" --eval --expr '{}'
  '';
}
