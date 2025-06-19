{ }:

{ preCheck ? ""
, ...
}:

{
  preCheck = ''
    export NIX_REMOTE="local?root=$PWD"
    # Initalize the for-checking remote
    # Otherwise, tests run in parallel, and race, then fail to create the already existing store.
    "$COMMAND_NIX_INSTANTIATE" --eval --expr '{}'
    ${preCheck}
  '';
}
