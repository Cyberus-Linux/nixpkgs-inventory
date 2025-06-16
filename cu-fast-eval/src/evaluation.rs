use serde::{Deserialize, Serialize};

use cu_lib_nix::cli::{NixCliExpression, NixCliExpressionArguments};
use cu_lib_nix::nix;

#[derive(Serialize, Deserialize, Debug)]
/// Results of an evaluation from cu-fast-eval.
/// Results include information about the evaluation producing it, and the `data`.
/// The `data` is always a `Vec`, as it must be a Nix list to be chunked.
pub struct Results<Data> {
    pub evaluation_arguments: Option<NixCliExpressionArguments>,
    pub evaluation_input: Option<NixCliExpression>,
    pub attributes_with_errors: nix::AttributePaths,
    pub data: Vec<Data>,
}

impl Default for Results<serde_json::Value> {
    fn default() -> Self {
        Self::new()
    }
}

impl Results<serde_json::Value> {
    pub fn new() -> Self {
        Self {
            evaluation_arguments: Some(NixCliExpressionArguments::new()),
            evaluation_input: Some(NixCliExpression::Expression(None)),
            attributes_with_errors: Vec::new(),
            data: Vec::new(),
        }
    }
}
