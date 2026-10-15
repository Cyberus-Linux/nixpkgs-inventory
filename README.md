`cake-unbaker`
==============

The `cake-unbaker` tool provides a few subcommands and plumbing commands to work with Nix-built projects.

The main things `cake-unbaker` currently does:

 - Index a package set's derivations. (`cake-unbaker pkgs index`)
 - Map derivation paths back to package set attributes, from index. (`cake-unbaker get-attributes`)

Additional accessory features exposed:

 - *Correctly* importing source references to the store with a normalized CLI. (`cake-unbaker pkgs store-path`)


Contents
--------

[[_TOC_]]


Quick Start
-----------

You will need to know which Nixpkgs revisions were used with the inputs you want to “unbake”.

> [!TIP]
> A NixOS toplevel output path may include the necessary information!
>
> ```
> /nix/store/[...]eee-nixos-system-$USER-$HOST-kernel_$KERNEL_VERSION-25.11pre999999.30a61f056ac4
>                                                                                    ^^^^^^^^^^^^
> ```

### Example

This example:

 - Gets a specific Nixpkgs checkout
 - Indexes the packages
 - Creates a sample “project”
 - Lists attribute sets from the requisites

<details>
<summary>Example console transcript</summary>

```
 # This first step left as an exercise to the reader
 $ cd "${cake_unbaker_checkout}"

 # The shell is the development shell, but also allows using the tools directly.
 $ nix-shell

 # Get a Nixpkgs checkout, `--depth=1` used to reduce the download size
 $ git clone --depth=1 \
    --revision=30a61f056ac492e3b7cdcb69c1e6abdcf00e39cf \
    https://github.com/NixOS/nixpkgs \
    arbitrary-nixpkgs

 # Index the package set
 $ cake-unbaker pkgs index nixpkgs-30a61f056ac4.sqlite arbitrary-nixpkgs
:: Normalizing references...
   SourceReference(path, arbitrary-nixpkgs)
fetching git input 'git+file:///.../cake-unbaker/arbitrary-nixpkgs'
   Normalized to: /nix/store/1l4nm55xcq55cdp1xz3x5mfgr4c24058-source

:: Evaluating references...
    → Evaluating /nix/store/1l4nm55xcq55cdp1xz3x5mfgr4c24058-source...
[...]
>> Ingesting "zziplib"
++ drv: /nix/store/801grh56fjdk3c9fjn0nsfc71y5r5ghb-zziplib-0.13.79.drv
   out: /nix/store/prv453cs1ihkg1kyh1wc0bnifaa00i00-zziplib-0.13.79
   src: /nix/store/4m3j3lp4riivwqca4wy9jm1307wvgkqc-source
>> Ingesting "zzuf"
++ drv: /nix/store/63qxpzcfxwcbyqss7c2jkhzg0y4k7w0x-zzuf-0.15.drv
   out: /nix/store/vff9abqk235bg2d8s049ddvcv41zfm1l-zzuf-0.15
   src: /nix/store/aq4bnwryp7mqvffdhy7wf0pwfssgcpdb-source
   patch: /nix/store/jwmjycxaj2qdzg790v8qz8ybla05jins-zzuf-zzat-c99.patch
   patch: /nix/store/b3wnc4cm6p7j8kh81vr7wslc6rw43d59-zzuf-glibc.patch

 # Make a silly sample project
 $ cat > weird-hello.nix <<'EOF'
{ pkgs }:

pkgs.writeShellScript "weird-hello" ''
  set -x
  ${pkgs.hello}/bin/hello
  ${pkgs.hello-cpp}/bin/hello-cpp
  ${pkgs.hello-go}/bin/hello-go
''
EOF

 # Build this weird hello project
 $ nix-build --arg pkgs 'import ./arbitrary-nixpkgs {}' ./weird-hello.nix 
this derivation will be built:
  /nix/store/kal22dvdm1zn45gqk2ix25mnsljzrpa7-weird-hello.drv
these 3 paths will be fetched (0.57 MiB download, 1.75 MiB unpacked):
  /nix/store/1apy255z15pclskgkzsbzldgdn87zbyr-hello-2.12.2
  /nix/store/fxn18i7202bv9bh1x1yqrygahbc826xy-hello-cpp
  /nix/store/bcgch2rnw38mkma8lg1wgmadad4pyl7y-hello-go
copying path '/nix/store/bcgch2rnw38mkma8lg1wgmadad4pyl7y-hello-go' from 'https://cache.nixos.org'...
copying path '/nix/store/1apy255z15pclskgkzsbzldgdn87zbyr-hello-2.12.2' from 'https://cache.nixos.org'...
copying path '/nix/store/fxn18i7202bv9bh1x1yqrygahbc826xy-hello-cpp' from 'https://cache.nixos.org'...
building '/nix/store/nd4bzg9dp4kc7yqdkk1k4wghjhss2v1l-weird-hello.drv'...
/nix/store/rk46z5ix6h25vddx46k6i7xdzaraigxy-weird-hello

 # Get the derivation's requisites for the project
 $ nix-store --query \
    --requisites /nix/store/kal22dvdm1zn45gqk2ix25mnsljzrpa7-weird-hello.drv > weird-hello.requisites.txt

 # Extract back *all* the attribute paths involved
 $ cake-unbaker get-attributes \
    --input-format requisites \
    --output-format only-attribute-paths \
    nixpkgs-30a61f056ac4.sqlite \
    weird-hello.requisites.txt \
    > attributes.json

 # It's a long list...
 $ jq '. | length' < attributes.json
222
```
</details>


Components
----------

### `cake-unbaker`

This is the main command of this tool.

<!-- [COMMAND_HELP:cake-unbaker] -->

```
Usage: cake-unbaker [OPTIONS] <COMMAND>

Commands:
  derivation      Commands to inspect derivations
  pkgs            Commands to inspect package sets
  get-attributes  From different input formats, list the attributes used from a package set
  help            Print this message or the help of the given subcommand(s)

Options:
      --output <OUTPUT>
  -h, --help             Print help
  -V, --version          Print version
```

<!-- [/COMMAND_HELP:cake-unbaker] -->


### `cake-unbaker pkgs index`

<!-- [COMMAND_HELP:cake-unbaker pkgs index] -->

```
Index a package set attributes and derivations

Usage: cake-unbaker pkgs index <DATABASE> [REFERENCES]...

Arguments:
  <DATABASE>


  [REFERENCES]...
          Package sets, from references, to add to the database.

          The package sets will have their paths normalized by adding them to the Nix store. See
          `cake-unbaker pkgs store-path` for allowed reference types.

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

<!-- [/COMMAND_HELP:cake-unbaker pkgs index] -->


### `cake-unbaker get-attributes`

<!-- [COMMAND_HELP:cake-unbaker get-attributes] -->

```
From different input formats, list the attributes used from a package set.

The database used to list the attributes comes from the `pkgs index` command.

The attributes chosen in the list may not be the exact attributes used in the input, but maps to the
same derivation. The discrepancy is due to how multiple attributes may point to the same derivation.
The instantiation is lossy since it only works with derivations, and does not know how the
derivation was selected.

The heuristics prefer shorter attribute names that are closer to the root. Since the `pkgs index`
command works with `{ allowAliases = false }`, aliases should not be present.

If the database houses indices for many evaluations, attribute names should still match properly,
but a package that was aliased and its attribute renamed may map to the older name, depending on the
timeline and luck.

Usage: cake-unbaker get-attributes --input-format <INPUT_FORMAT> --output-format <OUTPUT_FORMAT> <DATABASE> <INPUTS>...

Arguments:
  <DATABASE>
          Path to the packages index database

  <INPUTS>...
          Inputs from which to list the attributes of

Options:
      --input-format <INPUT_FORMAT>
          Possible values:
          - sbomnix:    The `cdx.json` output from sbomnix
          - requisites: The output of `nix-store --requisites`

      --output-format <OUTPUT_FORMAT>
          Possible values:
          - raw:                  Unstable JSON representation of the attributes matcher data
          - only-attribute-paths: JSON-formatted array of attribute path arrays

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

<!-- [/COMMAND_HELP:cake-unbaker get-attributes] -->


### Additional plumbing

Some of the plumbing used in `cake-unbaker` can be accessed using their commands.

Generally, these commands would be first used within a quick proof-of-concept to evaluate the feasibility of an idea, or to benefit from the `cake-unbaker` inside external non-Rust tooling.

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

<details>
<summary>Help text</summary>

<!-- [COMMAND_HELP:cu-fast-eval] -->

```
Usage: cu-fast-eval [OPTIONS] [FILE]

Arguments:
  [FILE]
          Expression to evaluate, follows the standard Nix CLI semantics.

           - When no file argument is given, defaults to `./default.nix`.
           - When an angled-bracket argument is given, follows `NIX_PATH` resolution.
           - When a relative path is given, relative to CWD.
           - When an absolute path is given, used as-is.

          The `--expr` argument changes the interpretation of `[FILE]`.

           - The given string is used as a Nix expression to evaluated.

Options:
      --output <OUTPUT>
          Filename to save the results. [default: <stdout>]

      --cores <CORES>
          Number of threads to use. [default: logical cpu count]

      --max-chunk-size <MAX_CHUNK_SIZE>
          Maximum size for a chunk

          [default: 16000]

      --recurse-with <RECURSE_WITH>
          Nix expression to determine whether to recurse in an attrset or not.

          The default follows the Nixpkgs conventions.

          [default: "value: (value.recurseForDerivations or false) != false"]

      --filter-with <FILTER_WITH>
          Nix expression to determine whether to keep the value or not.

          The default keeps only derivations.

          [default: "value: builtins.isAttrs value && value ? drvPath"]

  -E, --expr
          Interpret [FILE] as an expression to evaluate

      --arg <NAME> <VALUE>
          Arguments passed directly to `nix-instantiate` as `--arg name value`

      --show-trace
          Whether to print a stack trace in case of Nix expression evaluation errors

      --read-write-mode
          Whether to perform evaluation in read/write mode

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

<!-- [/COMMAND_HELP:cu-fast-eval] -->
</details>


### `cu-ingest-to-db`

The `cu-ingest-to-db` tool is used to manually import the JSON output of evaluations from `cu-fast-eval` to a database.

<details>
<summary>Help text</summary>
<!-- [COMMAND_HELP:cu-ingest-to-db] -->

```
Usage: cu-ingest-to-db <DATABASE> [JSON_PATHS]...

Arguments:
  <DATABASE>
  [JSON_PATHS]...

Options:
  -h, --help     Print help
  -V, --version  Print version
```

<!-- [/COMMAND_HELP:cu-ingest-to-db] -->
</details>


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

The Rust code is a Cargo workspace found in `workspace`.

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
