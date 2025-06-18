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
  # Setting `COMMAND_NIX_INSTANTIATE` in the environment at build-time makes
  # the tool keep an hermetic reference to the Nix CLI tooling used at build
  # time. See: `pub const COMMAND_NIX_INSTANTIATE` in `src/nix.rs`.
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
