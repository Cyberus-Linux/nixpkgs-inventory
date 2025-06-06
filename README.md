`cake-unbaker`
==============

> *TODO* Write rationale and user-facing documentation.

Components
----------

### `cu-fast-eval`

The `cu-fast-eval` tool is used to quickly evaluate a Nixpkgs-like attribute set for derivations,
outputting their derivation path, and additional metadata as JSON.

Example use:

```
 $ cu-fast-eval \
    --arg config '{ allowAliases = false; }' \
    '<nixpkgs>' \
    > evaluation.nixpkgs.json
```

> *This command follows the conventions of the standard `nix-*` CLI regarding the input expression*


Hacking
-------

Some notes about the setup.


### Organization

The Nix expressions used to build this project are found in `support/nix`.

The `default.nix` expression is the entrypoint, used for `nix-build` or importing.

The `shell.nix` relies on `default.nix` to get the proper shell expression.


### Shell

`nix-shell` can be used to get a shell for working on the project.

The development shell provides command aliases using the appropriate build invocation on the different tools.

This allows using `cu-fast-eval [...]` instead of using `cargo run --bin cu-fast-eval -- [...]`,
while still building as expected.

> **NOTE**: The aliases are provided as scripts via PATH, so that they work as expected outside of Bash.
