# <cu-lib-nix/src/nix/normalize-to-store.nix>
# The previous line is shown when the VERBOSE flag is used.
#
# Normalize source reference as store paths
# =========================================
#
# This expression normalizes a “source reference” to a source path.
#
# Source path are *broadly* compatible with the usual expectations of the
# different parts of the Nixpkgs ecosystem.
#
#  - Store path name is `source`
#  - As much as possible, tries to get the same hash for the same “moral” input
#     - Use `builtins.fetchGit` to strip git repo.
#     - Use `builtins.fetchTarball` to fetch tarballs.
#     - Use `builtins.path {}` to pick store path name.
#
# > [!NOTE]
# > URL support is not implemented yet.
# > Supported URL types and formats not decided yet.
#

{ reference }:

# break infrec
let
  arg'reference' = reference;
in

let
  inherit (builtins.fromJSON arg'reference')
    reference
    reference_type
    ;

  reference_path =
    if (builtins.match "^/.*" reference) == [ ] then /. + reference else ./. + "/${reference}";
in

if reference_type == "url" then
  builtins.abort "TODO: URL support not implemented yet."
else if builtins.pathExists (reference_path + "/.git") then
  builtins.fetchGit {
    url = reference_path;
    shallow = true;
  }
else
  builtins.path {
    name = "source";
    path = reference_path;
  }
