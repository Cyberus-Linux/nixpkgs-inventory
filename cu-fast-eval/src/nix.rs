use std::cmp;
use std::env;
use std::fs;
use std::path::PathBuf;

use rayon::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::evaluation;
use cu_lib_nix::nix;
use nix::cli::{NixCliExpression, NixCliExpressionArguments};
use nix::AttributePath;
use nix::AttributePaths;

// This is arbitrary, but looked like a good boundary for memory consumption.
// It's also sufficiently big that on a single-system Nixpkgs eval, it's bigger
// than `attributes.len() / 16`.
pub const DEFAULT_MAX_ALLOWABLE_PER_CHUNK: usize = 16_000;

const NIX_EXPR_CLI_IMPORT: &str = include_str!("nix/cli-import.nix");
const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");
const NIX_EXPR_PROCESS_CHUNK: &str = include_str!("nix/process-chunk.nix");

pub fn resolve_path(input_path: Option<String>) -> String {
    let mut path = PathBuf::from(input_path.unwrap_or_else(|| "./default.nix".to_string()));

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
    expression: &NixCliExpression,
    arguments: &NixCliExpressionArguments,
) -> Result<String, String> {
    let expression_arguments = {
        let attrset_body = arguments
            .iter()
            .map(|(name, value)| format!("{0} = ({1});", serde_json::json!(&name), value))
            .collect::<Vec<String>>()
            .join("\n");
        format!("{{\n{attrset_body}\n}}")
    };

    // The expression the user wants to see evaluated.
    // We need to wrap arguments handling in here.
    let expression_input = match expression.clone() {
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
            let as_string: String = serde_json::json!(v).to_string();
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
                format!(r#"./. + "/${{{as_string}}}""#)
            }
        }
    };

    Ok(format!(
        "({NIX_EXPR_CLI_IMPORT}) ({expression_input}) ({expression_arguments})"
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

#[derive(Serialize, Deserialize, Debug)]
struct AttributeResult<T> {
    #[serde(rename = "attrPath")]
    pub attr_path: AttributePath,
    pub success: bool,
    pub value: MaybeAttributeResult<T>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
/// Used to allow deserializing `T` when there was a failure.
enum MaybeAttributeResult<T> {
    Success(T),
    Failure(bool),
}

/// Returns the `Vec<AttributeResult<T>>` of values for the Nix attribute set chunk.
pub fn evaluate_chunk<T: DeserializeOwned>(
    path: &str,
    expression_arguments: &nix::cli::NixCliExpressionArguments,
    options: &nix::cli::NixInstantiateOptions,
) -> Result<evaluation::Results<T>, String> {
    let mut arguments = expression_arguments.clone();
    arguments.insert("chunkPath".to_string(), path.to_string());

    let results = nix::evaluate::<Vec<AttributeResult<T>>>(
        &NIX_EXPR_PROCESS_CHUNK.to_string(),
        &arguments,
        options,
    )?;

    let attributes_with_errors: AttributePaths = results
        .iter()
        .filter_map(|result| {
            if !result.success {
                Some(result.attr_path.clone())
            } else {
                None
            }
        })
        .collect();

    let data: Vec<T> = results
        .into_iter()
        .filter_map(|result| {
            if result.success {
                match result.value {
                    MaybeAttributeResult::Success(value) => Some(value),
                    _ => {
                        panic!(
                            "Unexpectedly found something else than `T` for {}",
                            serde_json::to_string(&result.attr_path).unwrap()
                        );
                    }
                }
            } else {
                None
            }
        })
        .collect();

    Ok(evaluation::Results {
        // Returning None as `expression` may not be the intended user-facing expression.
        // It may be a wrapper specific to the specifc usage of `evaluate_chunked`.
        evaluation_input: None,
        evaluation_arguments: Some(expression_arguments.clone()),
        attributes_with_errors,
        data,
    })
}

/// Evaluates the given expression and arguments in chunks.
///
/// The Nix `expression` must resolve to an attribute set.
pub fn evaluate_chunked<T: DeserializeOwned + std::marker::Send>(
    mut max_chunk_size: usize,
    expression: &str,
    expression_arguments: &nix::cli::NixCliExpressionArguments,
    options: &nix::cli::NixInstantiateOptions,
) -> Result<evaluation::Results<T>, String> {
    let verbose = !env::var("VERBOSE").unwrap_or("".to_string()).is_empty();

    // Path used to serialize the data, so it can be dropped from memory.
    let workdir = tempfile::tempdir().unwrap();

    // The user-provided expression is given as `attrs` to `process-chunk.nix`.
    let mut evaluation_arguments = expression_arguments.clone();
    evaluation_arguments.insert("attrs".to_string(), expression.to_string());

    let attributes = get_attributes(&evaluation_arguments, options).unwrap();

    // Let rayon tell us what ended-up being chosen.
    let cores = rayon::current_num_threads();

    // The smallest between:
    //  - The maximum allowable attributes per chunks
    //  - The natural distributed chunk size
    let optimal_chunk_size = cmp::min(
        DEFAULT_MAX_ALLOWABLE_PER_CHUNK,
        // NOTE: We're `ceil`ing here so we have *at most* `cores` chunks.
        // Otherwise we'd get `cores + 1` chunks with remainders in the additional one.
        (attributes.len() as f64 / cores as f64).ceil() as usize,
    );
    if max_chunk_size > optimal_chunk_size {
        max_chunk_size = optimal_chunk_size;
    }

    // Evaluate attribute names, and turn into chunks of JSON, as files.
    let chunk_paths: Vec<_> = attributes
        .chunks(max_chunk_size)
        .enumerate()
        .map(|(i, chunk)| {
            let path = workdir.path().join(format!("eval-chunk.{i:08}.json"));
            fs::write(&path, serde_json::to_string_pretty(&chunk).unwrap())
                .expect("Error while writing attributes to file...");
            path
        })
        .collect();

    if verbose {
        eprintln!("\n");
        eprintln!(":: Cores: {cores}");
        eprintln!(":: Attributes count: {}", attributes.len());
        eprintln!(":: Optimal chunk size: {optimal_chunk_size}");
        eprintln!(":: Current chunk size: {max_chunk_size}");
        eprintln!(":: Chunk count: {}", chunk_paths.len());
    }

    drop(attributes);

    let mut result = chunk_paths
        .par_iter()
        .map(|chunk_path| {
            let chunk_path = chunk_path.to_str().unwrap();

            //
            // TODO: Implement recovering from *fatal* evaluation errors.
            // This can be achieved by
            //   (1) making the `process::exit()` into an error type return
            //   (2) making the `chunk_paths` handling a queue that can be filled during execution
            //   (3) if more than one argument in chunk
            //         split the chunk in two parts, re-queue
            //       else
            //         mark as a fatal evaluation failure
            //
            // NOTE: A normal Nixpkgs evaluation from a successful channel should never hit a fatal
            //       evaluation failure, or else something went wrong with CI.
            //
            // This will slow down evaluation in the exceptional case where there is a fatal
            // evaluation failure, but since we are already chunking it will still be quick enough,
            // as long as we're not hitting the pathological case of every attribute failing.
            // In that case, this could take longer than evaluating the attribute set in a serial
            // manner. Though this shouldn't happen.
            //
            evaluate_chunk::<T>(chunk_path, &evaluation_arguments, options).unwrap_or_else(|err| {
                panic!("Unexpected error while handling chunk {chunk_path} ({err})")
            })
        })
        .reduce(
            || evaluation::Results {
                evaluation_arguments: None,
                evaluation_input: None,
                attributes_with_errors: vec![],
                data: vec![],
            },
            |mut acc, mut curr| {
                acc.attributes_with_errors
                    .append(&mut curr.attributes_with_errors);
                acc.data.append(&mut curr.data);
                acc
            },
        );

    result.evaluation_arguments = Some(expression_arguments.clone());

    Ok(result)
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

        let result: Vec<serde_json::Value> = evaluate_chunk(path, &expression_arguments, &options)
            .unwrap()
            .data;

        // FIXME: This is unlikely to be fun to work with.
        // TODO: evaluate_chunk should take an argument to transform values.
        // We will then return simpler values than full-blown derivations.
        // We can then move the "hard" Nix logic regarding evaluating the derivations outside of evaluate_chunk.
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            "[{\"attrPath\":[\"a\"],\"drvPath\":\"/nix/store/5nvz1xv9gfmw01gf6hjzylffa2d7291m-1.drv\",\"meta\":null,\"name\":\"1\",\"outputs\":{\"out\":\"/nix/store/fpdknchlh3hdwrgd3xfm3ficb25f8081-1\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null},{\"attrPath\":[\"b\"],\"drvPath\":\"/nix/store/5f6qc7qz5i4gcvifhkl0fiwy7jzi5f5v-2.drv\",\"meta\":null,\"name\":\"2\",\"outputs\":{\"out\":\"/nix/store/dwh9k5gaxxg2fwa1i0190nzykfmkwycq-2\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null},{\"attrPath\":[\"c\",\"d\"],\"drvPath\":\"/nix/store/0g2qnbsfz15lhdcrw3acj4c77sd3qhk3-4.drv\",\"meta\":null,\"name\":\"4\",\"outputs\":{\"out\":\"/nix/store/gw834zp0bifxd7n4v9m7pn0kd3wjyypd-4\"},\"patches\":[],\"pname\":null,\"srcs\":[],\"version\":null}]"
        );
    }
}
