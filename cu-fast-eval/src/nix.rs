use std::process::Command;
use std::process::Stdio;

use crate::{log_command};

pub type AttributePath = Vec<String>;
pub type AttributePaths = Vec<AttributePath>;

// Allow hardcoding a specific `nix-instantiate` at build-time, obviating the need for a wrapper.
pub const COMMAND_NIX_INSTANTIATE: &str =
    match option_env!("COMMAND_NIX_INSTANTIATE") {
        Some(v) => v,
        None => "nix-instantiate",
    }
;

const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");

/// Returns a list of attr *paths* (lists of lists of attribute name parts).
pub fn get_attributes(common_args: &Vec<String>) -> Result<AttributePaths, Box<dyn std::error::Error>> {
    eprintln!("\n:: Evaluating attribtues\n");

    let mut cmd = Command::new(COMMAND_NIX_INSTANTIATE);
    cmd
        .arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(common_args)
        .args(["--expr", NIX_EXPR_DUMP_ATTRIBUTES])
    ;

    log_command(&cmd);

    let result =
        cmd
        // Stream stderr to the console
        .stderr(Stdio::inherit())
        // Capture the output
        .output()?
    ;

    // If the command failed *in a controlled manner*, 
    if !result.status.success() {
        std::process::exit(result.status.code().unwrap());
    }

    // Parse result
    serde_json::from_str(&String::from_utf8_lossy(&result.stdout))
        .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_get_attributes_empty() {
        let empty: AttributePaths = vec![];
        let args = vec![
            "--show-trace",
            "--arg", "attrs", "{}",
        ].into_iter().map(|s| s.to_string()).collect();
        let result = get_attributes(&args).unwrap();
        assert_eq!(result, empty);
    }
    #[test]
    fn check_get_attributes_single() {
        let single: AttributePaths = vec![vec!["a".to_string()]];
        let args = vec![
            "--show-trace",
            "--arg", "attrs", r#"{ a = 1; }"#,
        ].into_iter().map(|s| s.to_string()).collect();
        let result = get_attributes(&args).unwrap();
        assert_eq!(result, single);
    }
    #[test]
    fn check_get_attributes_recursive() {
        let recursive: AttributePaths = vec![
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string()],
            vec!["c".to_string(), "d".to_string()],
        ];
        let args = vec![
            "--show-trace",
            "--arg", "recurse", r#"_: true"#,
            "--arg", "attrs", r#"{ a.b = 1; c.d = builtins.throw "ok"; }"#,
        ].into_iter().map(|s| s.to_string()).collect();
        let result = get_attributes(&args).unwrap();
        assert_eq!(result, recursive);
    }
}
