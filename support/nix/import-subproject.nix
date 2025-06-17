{ lib
, callPackage
, rustPlatform
, writeShellScriptBin

, name
, src
, cargoHash
}:

let
  subproject = src + "/${name}";
  # Also allow overriding the derivation details.
  overridesPath = subproject + "/overrides.nix";
  # Or completely self-describe.
  defaultPath = subproject + "/default.nix";

  args = { inherit name src cargoHash; };
in

if builtins.pathExists defaultPath
then callPackage defaultPath args
else 

let
  self = callPackage ./generic-subproject.nix args;
  overrides =
    if builtins.pathExists overridesPath
    then ((callPackage overridesPath {}) self)
    else {}
  ;
in
  self.overrideAttrs overrides
