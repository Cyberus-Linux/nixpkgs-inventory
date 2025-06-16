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
    use std::collections::HashMap;

    /// Arguments for a Nix expression, for the CLI.
    pub type NixCliExpressionArguments = HashMap<String, String>;

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
