use std::path::Path;
use std::process::Command;

/// Shared Nix-related types used in cake-unbaker
pub mod nix {
    /// An attribute path is a list representing the path to a value in (possibly nested) attribute sets.
    pub type AttributePath = Vec<String>;
    /// A list of attribute paths.
    pub type AttributePaths = Vec<AttributePath>;
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
