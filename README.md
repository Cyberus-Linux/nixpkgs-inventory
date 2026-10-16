# nixpkgs-inventory

`nixpkgs-inventory` lists the Nixpkgs attributes that a NixOS configuration or
a package built with Nix depends on.

## Usage

Pass a flake reference to the output you want to inspect:

```console
$ nix run github:cyberus-linux/nixpkgs-inventory -- \
    .#nixosConfigurations.myhost.config.system.build.toplevel > attributes.txt
```

For a package, use its flake output instead, e.g. `.#packages.x86_64-linux.default`.

The attributes are printed to standard output, one per line.

By default, only runtime dependencies are listed. Add `--include-build-deps`
(`-b`) to also list everything needed to build the output.

Getting the runtime dependencies requires building the output. If its
architecture differs from the machine running the tool, the output must
either be fully built and available from a binary cache, or you need a remote
builder for its architecture.

### Which Nixpkgs is used

For NixOS configurations, the Nixpkgs the configuration was evaluated with is
detected automatically. For other outputs, the flake's `nixpkgs` input is used.
If that is not correct, pass it explicitly with `--nixpkgs /path/to/nixpkgs`.

The first run for a given Nixpkgs indexes it, which takes a few minutes. The
index is cached in `~/.cache`, so later runs are fast.

### Limitations

Only store paths that come unchanged from a Nixpkgs attribute are listed. Not
listed are:

- Packages that are overridden.
- Packages built from a different Nixpkgs than the detected one.
- Packages for systems other than `x86_64-linux` and `aarch64-linux`, such as
  macOS (`aarch64-darwin`). Nixpkgs is only indexed for these two systems.
- Cross-compiled packages, e.g. an `aarch64-linux` configuration built on
  `x86_64-linux` via `pkgsCross`.

## More information

For more information on using [Cyberus Linux](https://cyberus-linux.com/) check out [`docs.cyberus-linux.com`](https://docs.cyberus-linux.com/).

For the low-level commands and development documentation, see
[workspace/README.md](workspace/README.md).
