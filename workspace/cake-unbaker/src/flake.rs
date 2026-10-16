//! Helpers to work with flake references.

use std::collections::BTreeMap;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail, ensure};
use cu_lib_nix::log_command;
use cu_lib_nix::nix::{self, ReferenceType, SourceReference, StorePath};
use serde::Deserialize;

/// Runs a Nix CLI command and returns its standard output.
fn run_with_output(mut cmd: Command) -> Result<String> {
    log_command(&cmd);

    let result = cmd.stderr(Stdio::inherit()).output()?;

    ensure!(
        result.status.success(),
        "{:?} exited with status ({})",
        cmd.get_program(),
        result.status
    );

    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

/// Create a `Command` to run Nix with Flakes enabled.
fn nix_command() -> Command {
    let mut cmd = Command::new(nix::COMMAND_NIX);
    cmd.args(["--extra-experimental-features", "nix-command flakes"]);
    cmd
}

/// Returns the path to the shared Nixpkgs index database, creating its directory if needed.
///
/// This is usually `~/.cache/cake-unbaker/nixpkgs.sqlite`.
pub fn database_path() -> Result<String> {
    let path = xdg::BaseDirectories::with_prefix("cake-unbaker")
        .place_cache_file("nixpkgs.sqlite")
        .context("Could not create the cache directory")?;

    path.into_os_string()
        .into_string()
        .map_err(|path| anyhow!("Cache path is not valid UTF-8: {path:?}"))
}

/// Given a flake reference to a derivation, returns its derivation path.
pub fn derivation_path(flake_ref: &str) -> Result<StorePath> {
    let mut cmd = nix_command();
    cmd.args(["path-info", "--derivation", flake_ref]);
    run_with_output(cmd)
}

/// Returns the requisites (closure) of the given derivation.
pub fn requisites(drv: &StorePath) -> Result<Vec<StorePath>> {
    let mut cmd = Command::new(nix::COMMAND_NIX_STORE);
    cmd.args(["--query", "--requisites", drv]);
    Ok(run_with_output(cmd)?.lines().map(StorePath::from).collect())
}

/// A flake and its inputs, as fetched to the store by `nix flake archive`.
#[derive(Deserialize, Debug)]
struct ArchivedFlake {
    path: StorePath,
    #[serde(default)]
    inputs: BTreeMap<String, ArchivedFlake>,
}

/// Fetches the given flake and its inputs, and returns their store paths.
// TODO Should we parse the `flake.lock` file instead?
fn archive(flake: &str) -> Result<ArchivedFlake> {
    let mut cmd = nix_command();
    cmd.args(["flake", "archive", "--json", "--dry-run", flake]);
    Ok(serde_json::from_str(&run_with_output(cmd)?)?)
}

/// Given a flake reference, finds the Nixpkgs it was built from.
///
/// For NixOS configurations, this is the Nixpkgs the configuration was evaluated with.
/// Otherwise, this is the flake's `nixpkgs` input.
pub fn nixpkgs_for(flake_ref: &str) -> Result<SourceReference> {
    let (flake, attribute) = flake_ref.split_once('#').unwrap_or((flake_ref, ""));

    let path = match attribute.split('.').collect::<Vec<_>>().as_slice() {
        ["nixosConfigurations", name, ..] => {
            let mut cmd = nix_command();
            cmd.args([
                "eval",
                "--raw",
                // Reduces execution time on my system from 4.5s to 1s.
                "--apply",
                "builtins.toString",
                &format!("{flake}#nixosConfigurations.{name}.pkgs.path"),
            ]);
            run_with_output(cmd)?
        }
        _ => if let Some(nixpkgs) = archive(flake)?.inputs.get("nixpkgs") {
            nixpkgs.path.clone()
        } else {
            bail!(
                "{flake} has no `nixpkgs` input, use `--nixpkgs` to provide the Nixpkgs used by {flake_ref}."
            )
        }
    };

    Ok(SourceReference {
        reference: path,
        reference_type: ReferenceType::Path,
    })
}
