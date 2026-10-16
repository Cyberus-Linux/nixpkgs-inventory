{ inputs, ... }:
{
  imports = [ inputs.git-hooks-nix.flakeModule ];

  perSystem =
    { pkgs, self', ... }:
    {
      formatter = pkgs.nixfmt-tree;

      # Build all packages as part of `nix flake check`.
      checks = self'.packages;

      pre-commit.settings = {
        settings.rust.cargoManifestPath = "./workspace/Cargo.toml";

        hooks = {
          nixfmt.enable = true;
          rustfmt.enable = true;
          actionlint.enable = true;
          zizmor.enable = true;
        };
      };
    };
}
