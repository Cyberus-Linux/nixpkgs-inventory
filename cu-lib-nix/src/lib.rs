use std::env;
use std::path::Path;
use std::process::Command;

/// Shared Nix-related types used in cake-unbaker
pub mod nix {
    use crate::cli;
    use crate::log_command;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::process::Command;
    use std::process::Stdio;

    // Allow hardcoding a specific `nix-instantiate` at build-time, obviating the need for a wrapper.
    pub const COMMAND_NIX_INSTANTIATE: &str = match option_env!("COMMAND_NIX_INSTANTIATE") {
        Some(v) => v,
        None => "nix-instantiate",
    };

    // Allow hardcoding a specific `nix-store` at build-time, obviating the need for a wrapper.
    pub const COMMAND_NIX_STORE: &str = match option_env!("COMMAND_NIX_STORE") {
        Some(v) => v,
        None => "nix-store",
    };

    /// Used to better convey intent.
    pub type StorePath = String;

    /// An attribute path is a list representing the path to a value in (possibly nested) attribute sets.
    pub type AttributePath = Vec<String>;
    /// A list of attribute paths.
    pub type AttributePaths = Vec<AttributePath>;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct DerivationReferences {
        pub derivations: Vec<StorePath>,
        pub paths: Vec<StorePath>,
    }

    /// Returns the (deserialized from JSON) `T` evaluated result of a Nix expression.
    pub fn evaluate<T: DeserializeOwned>(
        expression: &String,
        arguments: cli::NixCliExpressionArguments,
    ) -> Result<T, String> {
        let mut cmd = Command::new(COMMAND_NIX_INSTANTIATE);

        for (name, value) in arguments {
            cmd.args(["--arg", &name, &value]);
        }

        cmd.arg("--strict")
            .arg("--json")
            .arg("--eval")
            .args(["--expr", expression]);

        log_command(&cmd);

        let result = match cmd.stderr(Stdio::inherit()).output() {
            Ok(output) => output,
            Err(err) => {
                return Err(err.to_string());
            }
        };

        // If the command failed.
        if !result.status.success() {
            let status = result.status.code().unwrap();
            return Err(format!("nix-instantiate exited with status ({})", status));
        }

        // Otherwise, deserialize into the generic type given.
        match serde_json::from_str::<T>(&String::from_utf8_lossy(&result.stdout)) {
            Ok(data) => Ok(data),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn is_derivation(store_path: &StorePath) -> bool {
        store_path.ends_with(".drv")
    }

    pub fn derivation_references(
        drv: &String,
    ) -> Result<DerivationReferences, Box<dyn std::error::Error>> {
        let mut cmd = Command::new(COMMAND_NIX_STORE);
        cmd.arg("--query").arg("--references").arg(drv);

        log_command(&cmd);

        let result = cmd
            // Stream stderr to the console
            .stderr(Stdio::inherit())
            // Capture the output
            .output()?;

        // Ignore error statuses; the derivation may not exist, and it's fine.
        if !result.status.success() {
            return Ok(DerivationReferences {
                paths: vec![],
                derivations: vec![],
            });
        }

        let (derivations, paths) = String::from_utf8_lossy(&result.stdout)
            .trim()
            .split("\n")
            .map(StorePath::from)
            .partition(is_derivation);

        Ok(DerivationReferences { paths, derivations })
    }
}

/// CLI-specific types for cake-unbaker
pub mod cli {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    /// Arguments for a Nix expression, for the CLI.
    pub type NixCliExpressionArguments = BTreeMap<String, String>;

    /// CLI argument representing the expression to evaluate.
    /// Following the CLI semantics, it can be either:
    ///
    ///  - A path to a file
    ///  - A string expression
    ///
    /// The represented value (Expression or Path) *should* differe depending on the `--expr` or
    /// `-E` flag with the standard `nix-*` commands.
    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub enum NixCliExpression {
        Expression(Option<String>),
        Path(Option<String>),
    }
}

// Prints (to stderr) a formatted useful version of the command used.
// NOTE: Multi-line arguments will be elided to their first line!
pub fn log_command(cmd: &Command) {
    let verbose = !env::var("VERBOSE").unwrap_or("".to_string()).is_empty();
    if !verbose {
        return;
    }
    let args: Vec<String> = cmd
        .get_args()
        .map(|s| {
            let mut lines = s.to_str().unwrap().lines();
            let first = lines.next().unwrap();
            match lines.next() {
                // Only one line.
                None => first.to_string(),
                // Multi-line
                Some(_) => {
                    format!("{}…", first)
                }
            }
        })
        .collect();

    let exe_name = Path::new(cmd.get_program())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    eprintln!(
        " $ {} {}",
        shlex::try_quote(exe_name).unwrap(),
        shlex::try_join(args.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).unwrap()
    )
}
