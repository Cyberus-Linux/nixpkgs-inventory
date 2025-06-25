//! CLI-specific types for cake-unbaker

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
