{ inputs, ... }:
{
  imports = [ inputs.git-hooks-nix.flakeModule ];

  perSystem =
    { pkgs, ... }:
    {
      formatter = pkgs.nixfmt-tree;

      pre-commit.settings = {
        settings.rust.cargoManifestPath = "./workspace/Cargo.toml";

        hooks = {
          nixfmt.enable = true;
          rustfmt.enable = true;
        };
      };
    };
}
