{ buildPackages
}:

{ nativeBuildInputs ? []
, preCheck ? ""
, ...
}:

let
  inherit (buildPackages) nix;
in

{
  COMMAND_NIX_INSTANTIATE = "${nix}/bin/nix-instantiate";
  preCheck = ''
    export NIX_REMOTE="local?root=$PWD"
    # Don't expose Nix on PATH until we check things.
    export PATH="${nix}/bin/:$PATH"
    # Initalize the for-checking remote
    # Otherwise, tests run in parallel, and race, then fail to create the already existing store.
    nix-instantiate --eval --expr '{}'
    ${preCheck}
  '';
}
