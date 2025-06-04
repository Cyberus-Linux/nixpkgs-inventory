use std::collections::HashMap;
use std::process::Command;
use std::process::Stdio;

use crate::log_command;

pub type AttributePath = Vec<String>;
pub type AttributePaths = Vec<AttributePath>;
pub type NixCliExpressionArguments = HashMap<String, String>;

pub enum NixCliExpression {
    Expression(Option<String>),
    Path(Option<String>),
}

// Allow hardcoding a specific `nix-instantiate` at build-time, obviating the need for a wrapper.
pub const COMMAND_NIX_INSTANTIATE: &str = match option_env!("COMMAND_NIX_INSTANTIATE") {
    Some(v) => v,
    None => "nix-instantiate",
};

const NIX_EXPR_CLI_IMPORT: &str = include_str!("nix/cli-import.nix");
const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");
const NIX_EXPR_PROCESS_CHUNK: &str = include_str!("nix/process-chunk.nix");

/// Returns a nix expression fragment that evaluates an expression with the same semantics as the
/// standard nix-* CLI tools would.
pub fn build_nix_cli_import_semantics_fragment(
    expression: NixCliExpression,
    arguments: NixCliExpressionArguments,
) -> Result<String, String> {
    let expression_arguments = {
        let attrset_body = arguments
            .iter()
            .map(|(name, value)| format!("{0} = ({1});", serde_json::json!(&name), value))
            .collect::<Vec<String>>()
            .join("\n");
        format!("{{\n{}\n}}", attrset_body)
    };

    // The expression the user wants to see evaluated.
    // We need to wrap arguments handling in here.
    let expression_input = match expression {
        // The expression flag without a value is an error.
        NixCliExpression::Expression(None) => {
            return Err("An expression must be given with `--expr`.".to_string());
        }
        // For an expression, we drop the value in place.
        NixCliExpression::Expression(Some(expr)) => expr,
        NixCliExpression::Path(None) => "./.".to_string(),
        // We are handling paths, or maybe the default case.
        NixCliExpression::Path(Some(v)) => {
            // Convert argument to a Nix string (by using the fact JSON strings are pretty much
            // drop-in compatible with Nix strings).
            let as_string: String = serde_json::json!(&v).to_string();
            // NOTE: By using a stringified string for the path (via JSON formatting), we can can ignore
            // marshalling the value to an "actual" path value, as long as we check for absolute or relative paths.

            // Directly use absolute paths as a string
            if v.starts_with("/") {
                as_string
            }
            // Directly use angled bracket paths
            else if v.starts_with("<") {
                v
            }
            // Paste given path with `${./.}/`, making an absolute path out of a relative one.
            // (Yes, the formatting string looks hard to parse, because we need to escape `${}`.)
            else {
                format!(r#"./. + "/${{{0}}}""#, as_string)
            }
        }
    };

    Ok(format!(
        "({0}) ({1}) ({2})",
        NIX_EXPR_CLI_IMPORT, expression_input, expression_arguments
    ))
}

/// Returns a list of attr *paths* (lists of lists of attribute name parts).
pub fn get_attributes(
    common_args: &Vec<String>,
) -> Result<AttributePaths, Box<dyn std::error::Error>> {
    eprintln!("\n:: Evaluating attribtues\n");

    let mut cmd = Command::new(COMMAND_NIX_INSTANTIATE);
    cmd.arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(common_args)
        .args(["--expr", NIX_EXPR_DUMP_ATTRIBUTES]);

    log_command(&cmd);

    let result = cmd
        // Stream stderr to the console
        .stderr(Stdio::inherit())
        // Capture the output
        .output()?;

    // If the command failed *in a controlled manner*,
    if !result.status.success() {
        std::process::exit(result.status.code().unwrap());
    }

    // Parse result
    serde_json::from_str(&String::from_utf8_lossy(&result.stdout))
        .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
}

/// Returns the serde_json Array of serde_json Objects for the evaluated list of Nix attribute sets.
pub fn evaluate_chunk(
    common_args: &Vec<String>,
    path: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(COMMAND_NIX_INSTANTIATE);
    cmd.arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(common_args)
        .args(["--expr", NIX_EXPR_PROCESS_CHUNK]);

    // Ensure `chunkPath` is the one we are passing to `nix-instantiate`.
    cmd.args(["--arg", "chunkPath", path]);

    log_command(&cmd);

    let result = cmd
        // Stream stderr to the console
        .stderr(Stdio::inherit())
        // Capture the output
        .output()?;

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
    use std::fs;

    #[test]
    fn check_get_attributes_empty() {
        let empty: AttributePaths = vec![];
        let args = vec!["--show-trace", "--arg", "attrs", "{}"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let result = get_attributes(&args).unwrap();
        assert_eq!(result, empty);
    }
    #[test]
    fn check_get_attributes_single() {
        let single: AttributePaths = vec![vec!["a".to_string()]];
        let args = vec!["--show-trace", "--arg", "attrs", r#"{ a = 1; }"#]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
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
            "--arg",
            "recurse",
            r#"_: true"#,
            "--arg",
            "attrs",
            r#"{ a.b = 1; c.d = builtins.throw "ok"; }"#,
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let result = get_attributes(&args).unwrap();
        assert_eq!(result, recursive);
    }

    #[test]
    fn check_evaluate_chunk() {
        let workdir = tempfile::tempdir().unwrap();
        let path_binding = workdir.path().join(format!("eval-chunk.{:08}.json", 1234));
        let path = path_binding.to_str().unwrap();
        let chunk = vec![vec!["a"], vec!["b"], vec!["c", "d"]];
        fs::write(path, serde_json::to_string_pretty(&chunk).unwrap())
            .expect("Error while writing attributes to file...");
        let args = vec![
            "--show-trace",
            "--arg",
            "filter",
            "v: true",
            "--arg",
            "attrs",
            r#"
                let
                    mkDrv =
                        name:
                        derivation {
                            inherit name;
                            builder = "bogus";
                            system = "bogus";
                        }
                    ;
                in
                { a = mkDrv "1"; b = mkDrv "2"; c = { d = mkDrv "4"; }; }
            "#,
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let result = evaluate_chunk(&args, path).unwrap();

        // FIXME: This is unlikely to be fun to work with.
        // TODO: evaluate_chunk should take an argument to transform values.
        // We will then return simpler values than full-blown derivations.
        // We can then move the "hard" Nix logic regarding evaluating the derivations outside of evaluate_chunk.
        assert_eq!(
            result.to_string(),
            "[{\"attrPath\":[\"a\"],\"drvPath\":\"/nix/store/5nvz1xv9gfmw01gf6hjzylffa2d7291m-1.drv\",\"meta\":null,\"name\":\"1\",\"outputs\":{\"out\":\"/nix/store/fpdknchlh3hdwrgd3xfm3ficb25f8081-1\"},\"pname\":null,\"version\":null},{\"attrPath\":[\"b\"],\"drvPath\":\"/nix/store/5f6qc7qz5i4gcvifhkl0fiwy7jzi5f5v-2.drv\",\"meta\":null,\"name\":\"2\",\"outputs\":{\"out\":\"/nix/store/dwh9k5gaxxg2fwa1i0190nzykfmkwycq-2\"},\"pname\":null,\"version\":null},{\"attrPath\":[\"c\",\"d\"],\"drvPath\":\"/nix/store/0g2qnbsfz15lhdcrw3acj4c77sd3qhk3-4.drv\",\"meta\":null,\"name\":\"4\",\"outputs\":{\"out\":\"/nix/store/gw834zp0bifxd7n4v9m7pn0kd3wjyypd-4\"},\"pname\":null,\"version\":null}]"
        );
    }
}
