#
# This expression is ***impure*** and will check against the current stage of the different release channels.
#
# This is only a convenience check.
#
let
  recurseIntoAttrs = attrs: attrs // { recurseForDerivations = true; };
  escapedAttrName = builtins.replaceStrings ["."] ["_"];
  checkChannel =
    identifier:
    let
      pkgs = import (fetchTarball "channel:${identifier}") {};
    in
    {
      "${escapedAttrName identifier}" = recurseIntoAttrs (
        import ./checks.nix { inherit pkgs; }
      );
    }
  ;

  checks =
    {}
    # Requires edition 2024 features to be understood by the compiler for dependencies.
    #// (checkChannel "nixos-24.05")
    // (checkChannel "nixos-24.11")
    // (checkChannel "nixos-25.05")
    // (checkChannel "nixos-unstable")
  ;
in
checks // {
  inherit checks;
}
