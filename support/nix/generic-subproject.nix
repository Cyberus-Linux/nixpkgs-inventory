{ lib
, rustPlatform

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
})
