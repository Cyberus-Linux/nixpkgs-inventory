use std::env;
use std::path::Path;
use std::process::Command;

/// Shared Nix-related types used in cake-unbaker
pub mod nix {
    use crate::cli;
    use crate::log_command;
    use regex::Regex;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::fmt;
    use std::process::Command;
    use std::process::Stdio;
    use std::str::FromStr;

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

    /// Reference to a source input.
    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct SourceReference {
        pub reference: String,
        pub reference_type: ReferenceType,
    }

    impl fmt::Display for SourceReference {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "SourceReference({}, {})",
                self.reference_type, self.reference
            )
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub enum ReferenceType {
        #[serde(rename = "path")]
        Path,
        #[serde(rename = "url")]
        Url,
    }

    impl fmt::Display for ReferenceType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let ret = match self {
                ReferenceType::Path => "path",
                ReferenceType::Url => "url",
            };
            write!(f, "{}", ret)
        }
    }

    impl FromStr for SourceReference {
        type Err = String;
        /// Given a string input, sniffs out its structure.
        fn from_str(reference: &str) -> Result<Self, Self::Err> {
            let mut reference = reference.to_string();
            let re = Regex::new(r"^[a-zA-Z0-9]+://").unwrap();

            // Special-case `file://` URLs. Only handle "absolute paths".
            // (Nix rejects `file://<authority>/...`, so in practice only handles absolute paths)
            if reference.starts_with("file:///") {
                reference = reference[7..].to_string();
            }

            if re.is_match(&reference) {
                Ok(SourceReference {
                    reference,
                    reference_type: ReferenceType::Url,
                })
            } else {
                Ok(SourceReference {
                    reference,
                    reference_type: ReferenceType::Path,
                })
            }
        }
    }

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
        options: cli::NixInstantiateOptions,
    ) -> Result<T, String> {
        let mut cmd = Command::new(COMMAND_NIX_INSTANTIATE);

        for (name, value) in arguments {
            cmd.args(["--arg", &name, &value]);
        }

        // NOTE: we are overriding the provided options, for the arguments we need strictly need.
        let options = options.merge(cli::NixInstantiateOptions {
            // Value needs to be evaluated fully
            strict: Some(true),
            // Serialization format
            json: Some(true),
            // This is an evaluation, not an instantiation
            eval: Some(true),
            // We're passing an expression
            expression: Some(true),
            ..Default::default()
        });

        cmd.args(options.to_cli_arguments()).arg(expression);

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

    /// Given a serializable value, encodes it as JSON, then again so it can be given to `--arg`.
    pub fn double_encode<T: Serialize>(value: T) -> String {
        let first = serde_json::to_string(&value).unwrap();
        serde_json::to_string(&first).unwrap()
    }

    /// Given a source reference, imports it to the store and returns the now normalized store path.
    pub fn normalized_add_to_store(reference: &SourceReference) -> Result<String, String> {
        const EXPR: &str = include_str!("nix/normalize-to-store.nix");
        let mut arguments = cli::NixCliExpressionArguments::new();
        arguments.insert("reference".to_string(), double_encode(reference));
        evaluate(
            &EXPR.to_string(),
            arguments,
            cli::NixInstantiateOptions::default(),
        )
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

    /// CLI options for `nix-instantiate`, as a type.
    ///
    /// It is recommended to only set values you care about, and rely on `..Default::default()` for
    /// leftover default values.
    ///
    /// See `merge` for composing different NixInstantiateOptions.
    ///
    /// The Default values are intended to merge properly and not overwrite unset (`None`) values.
    #[derive(Debug, Default)]
    pub struct NixInstantiateOptions {
        /// `--expr` or `-E`
        pub expression: Option<bool>,
        /// `--strict`
        pub strict: Option<bool>,
        /// `--json`
        pub json: Option<bool>,
        /// `--eval`
        pub eval: Option<bool>,
        /// `--read-write-mode`
        pub read_write_mode: Option<bool>,
        /// `--readonly-mode`
        pub readonly_mode: Option<bool>,
    }

    impl NixInstantiateOptions {
        /// Merge `other`'s values into `self`.
        pub fn merge(self, other: Self) -> Self {
            Self {
                expression: other.expression.or(self.expression),
                strict: other.strict.or(self.strict),
                json: other.json.or(self.json),
                eval: other.eval.or(self.eval),
                read_write_mode: other.read_write_mode.or(self.read_write_mode),
                readonly_mode: other.readonly_mode.or(self.readonly_mode),
            }
        }
    }

    impl NixInstantiateOptions {
        pub fn to_cli_arguments(&self) -> Vec<String> {
            let mut arguments: Vec<String> = Vec::new();

            if self.expression.unwrap_or(false) {
                arguments.push("--expr".to_string());
            }
            if self.strict.unwrap_or(false) {
                arguments.push("--strict".to_string());
            }
            if self.json.unwrap_or(false) {
                arguments.push("--json".to_string());
            }
            if self.eval.unwrap_or(false) {
                arguments.push("--eval".to_string());
            }
            if self.read_write_mode.unwrap_or(false) {
                arguments.push("--read-write-mode".to_string());
            }
            if self.readonly_mode.unwrap_or(false) {
                arguments.push("--readonly-mode".to_string());
            }

            arguments
        }
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
