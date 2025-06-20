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


Usage tips
----------

### Using without an accessible Nix store

By using `NIX_REMOTE`, it is possible to make Nix (even the standard CLI) use an alternative store path.

```bash
$ export NIX_REMOTE="local?root=$PWD/some-store"

$ cake-unbaker pkgs store-path ..../nixos-unstable/
fetching git input 'git+file:///.../nixos-unstable'
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source

$ file some-store/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source
some-store/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source: directory
```

This is especially useful when running the tool within a Nix build.

See:

 - https://nix.dev/manual/nix/2.29/store/types/index.html
 - https://nix.dev/manual/nix/latest/store/types/index.html


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

#### Working with a different Nix

By default, the shell will not provide a Nix package.
The tooling will use the ambient Nix from your host system.

It may be necessary to quickly work on the project using a specific Nix version.
This can be achieved by passing the desired *attribute name* for the Nix package as the `nix` argument to the `shell.nix` expression.

> [!NOTE]
> The following examples use `--pure` *only* to properly show the Nix package comes from the shell.
> Do not use `--pure` with the `nix-shell` you intend to work in, it will break some things.

```
 $ nix-shell --pure --run 'nix-instantiate --version' | head -n1
/tmp/nix-shell-2722726-0/rc: line 3: nix-instantiate: command not found

 $ nix-shell --pure --argstr nix nix --run 'nix-instantiate --version' | head -n1
nix-instantiate (Nix) 2.28.3

 $ nix-shell --pure --argstr nix lix --run 'nix-instantiate --version' | head -n1
nix-instantiate (Lix, like Nix) 2.91.1
```


#### Working with a different Nixpkgs

It may be necessary to quickly work on the project using a different Nixpkgs.
This can be achieved by passing the `pkgs` or `nixpkgs_path` to the `shell.nix` expression.

The following example uses `NIX_PATH` and *angled brackets path syntax* to import the current *NixOS release channel* for the *unstable* branch of upstream Nixpkgs.

```
 $ nix-shell -I nixpkgs=channel:nixos-25.05 --arg nixpkgs_path '<nixpkgs>'  --run 'rustc --version'
rustc 1.86.0 (05f9846f8 2025-03-31) (built from a source tarball)

 $ nix-shell -I nixpkgs=channel:nixos-24.11 --arg nixpkgs_path '<nixpkgs>'  --run 'rustc --version'
rustc 1.82.0 (f6e511eec 2024-10-15) (built from a source tarball)
```


### Integration tests

> [!TIP]
> The following commands only evaluates and makes the derivations using `nix-instantiate`.
> To actually build the checks, use `nix-build` instead.

The `support/nix/checks.nix` expression can be used to build the tooling against the Nix implementations packaged in Nixpkgs.

The default invocation will use the Nixpkgs pinned in the repository.

```
 $ nix-instantiate support/nix/checks.nix
...
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-cu-lib-nix-0.0.0.drv
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-nix-shell.drv
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-nix-2.28.3.drv
```

The checks can be ran on an arbitrary Nixpkgs using the expression's arguments.

The following example uses `NIX_PATH` and *angled brackets path syntax* to import the current *NixOS release channel* for the *unstable* branch of upstream Nixpkgs.

```
 $ nix-instantiate \
    -I nixpkgs=channel:nixos-unstable \
    --arg nixpkgs_path '<nixpkgs>' \
    support/nix/checks.nix
...
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-cu-lib-nix-0.0.0.drv
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-nix-shell.drv
/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-nix-2.24.14.drv
```

Running the checks against a Nixpkgs different from the one pinned in the repository may not work as expected.
It may not be a bug if it's against an older Nixpkgs version.
It may highlight an upcoming issue if it's against a newer Nixpkgs version.

As a convenience, the `support/nix/impure-checks.nix` expression can be used to handily build against a few best-effort release channels.

```
 $ nix-build --keep-going --no-out-link support/nix/impure-checks.nix
```


### Debugging tips

#### `VERBOSE` flag

By setting the `VERBOSE` environment to any value (other than an empty value), some parts of the commands may print useful information.

Information will include:

 - Pretty-printed command for calls to Nix CLI utilities.
   - NOTE: Multi-line arguments will be elided to their first line!
