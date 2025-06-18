{ lib
, pkgs
, src ? ../..
, nix ? pkgs.nix
}:

(lib.makeScope pkgs.newScope (
  self:
  let
    inherit (self) callPackage;
    # Workspace information
    inherit
      (builtins.fromTOML (builtins.readFile (src + "/Cargo.toml")))
      workspace
    ;

    sharedInputs = {
      inherit src;
    };

    subprojects =
      # Using the workspace members, build an attrset of them all.
      lib.genAttrs
      workspace.members
      (
        name: 
        callPackage ./import-subproject.nix (sharedInputs // {
          inherit name;
        })
      )
    ;
  in
  subprojects // {
    # Make using the CLI a bit neater.
    # This keeps `callPackage` around, as it allows using this scope semantics directly.
    self = builtins.removeAttrs self [
      "newScope"
      "overrideScope"
      "packages"
      "self"
    ];
    dev-shell = callPackage ./dev-shell.nix { inherit subprojects; };

    # Ref on the Nix used for this package set.
    inherit nix;

    # Ref on the package set.
    inherit pkgs;
  }
)).self
