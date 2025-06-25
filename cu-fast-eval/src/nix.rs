use std::path::PathBuf;

use cu_lib_nix::nix;
use nix::cli::{NixCliExpression, NixCliExpressionArguments};
use nix::AttributePaths;

const NIX_EXPR_CLI_IMPORT: &str = include_str!("nix/cli-import.nix");
const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");
const NIX_EXPR_PROCESS_CHUNK: &str = include_str!("nix/process-chunk.nix");

pub fn resolve_path(input_path: Option<String>) -> String {
    let mut path = PathBuf::from(match input_path {
        Some(path) => path,
        None => "./default.nix".to_string(),
    });

    path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .expect("Unable to get current_dir()")
            .join(path)
    };

    // We weant an absolute path, equivalent to what Nix does.
    // The `canonicalize` method would resolve symlinks, which is undesired here.
    path_clean::clean(path).to_string_lossy().to_string()
}

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
    expression_arguments: &nix::cli::NixCliExpressionArguments,
    options: &nix::cli::NixInstantiateOptions,
) -> Result<AttributePaths, String> {
    nix::evaluate(
        &NIX_EXPR_DUMP_ATTRIBUTES.to_string(),
        expression_arguments,
        options,
    )
}

/// Returns the serde_json Array of serde_json Objects for the evaluated list of Nix attribute sets.
pub fn evaluate_chunk(
    path: &str,
    expression_arguments: &nix::cli::NixCliExpressionArguments,
    options: &nix::cli::NixInstantiateOptions,
) -> Result<serde_json::Value, String> {
    let mut arguments = expression_arguments.clone();
    arguments.insert("chunkPath".to_string(), path.to_string());

    nix::evaluate(&NIX_EXPR_PROCESS_CHUNK.to_string(), &arguments, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::cli::NixCliExpressionArguments;
    use nix::cli::NixInstantiateOptions;
    use std::fs;

    const ALWAYS: &str = "v: true";

    #[test]
    fn check_get_attributes_empty() {
        let empty: AttributePaths = vec![];
        let mut expression_arguments = NixCliExpressionArguments::new();
        expression_arguments.insert("attrs".to_string(), "{}".to_string());

        let options = NixInstantiateOptions {
            show_trace: Some(true),
            ..NixInstantiateOptions::default()
        };

        let result = get_attributes(&expression_arguments, &options).unwrap();
        assert_eq!(result, empty);
    }

    #[test]
    fn check_get_attributes_single() {
        let single: AttributePaths = vec![vec!["a".to_string()]];

        let mut expression_arguments = NixCliExpressionArguments::new();
        expression_arguments.insert("attrs".to_string(), r#"{ a = 1; }"#.to_string());

        let options = NixInstantiateOptions {
            show_trace: Some(true),
            ..NixInstantiateOptions::default()
        };

        let result = get_attributes(&expression_arguments, &options).unwrap();
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

        const EXPR: &str = r#"{ a.b = 1; c.d = builtins.throw "ok"; }"#;

        let mut expression_arguments = NixCliExpressionArguments::new();
        expression_arguments.insert("attrs".to_string(), EXPR.to_string());
        expression_arguments.insert("filter".to_string(), ALWAYS.to_string());
        expression_arguments.insert("recurse".to_string(), ALWAYS.to_string());

        let options = NixInstantiateOptions {
            show_trace: Some(true),
            ..NixInstantiateOptions::default()
        };

        let result = get_attributes(&expression_arguments, &options).unwrap();
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

        const EXPR: &str = r#"
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
          "#;

        let options = NixInstantiateOptions {
            show_trace: Some(true),
            ..NixInstantiateOptions::default()
        };

        let mut expression_arguments = NixCliExpressionArguments::new();
        expression_arguments.insert("filter".to_string(), ALWAYS.to_string());
        expression_arguments.insert("attrs".to_string(), EXPR.to_string());

        let result = evaluate_chunk(path, &expression_arguments, &options).unwrap();

        // FIXME: This is unlikely to be fun to work with.
        // TODO: evaluate_chunk should take an argument to transform values.
        // We will then return simpler values than full-blown derivations.
        // We can then move the "hard" Nix logic regarding evaluating the derivations outside of evaluate_chunk.
        assert_eq!(
            result.to_string(),
            "[{\"attrPath\":[\"a\"],\"drvPath\":\"/nix/store/5nvz1xv9gfmw01gf6hjzylffa2d7291m-1.drv\",\"meta\":null,\"name\":\"1\",\"outputs\":{\"out\":\"/nix/store/fpdknchlh3hdwrgd3xfm3ficb25f8081-1\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null},{\"attrPath\":[\"b\"],\"drvPath\":\"/nix/store/5f6qc7qz5i4gcvifhkl0fiwy7jzi5f5v-2.drv\",\"meta\":null,\"name\":\"2\",\"outputs\":{\"out\":\"/nix/store/dwh9k5gaxxg2fwa1i0190nzykfmkwycq-2\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null},{\"attrPath\":[\"c\",\"d\"],\"drvPath\":\"/nix/store/0g2qnbsfz15lhdcrw3acj4c77sd3qhk3-4.drv\",\"meta\":null,\"name\":\"4\",\"outputs\":{\"out\":\"/nix/store/gw834zp0bifxd7n4v9m7pn0kd3wjyypd-4\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null}]"
        );
    }
}
