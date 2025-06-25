use std::cmp;
use std::fs;

use cu_fast_eval::evaluation;
use cu_fast_eval::get_env_int;
use cu_fast_eval::nix;
use cu_fast_eval::nix::{build_nix_cli_import_semantics_fragment, evaluate_chunk, get_attributes};
use cu_lib_nix::nix::cli::{NixCliExpression, NixCliExpressionArguments, NixInstantiateOptions};
use cu_lib_nix::nix::AttributePaths;

use clap::Parser;
use rayon::prelude::*;

// This is arbitrary, but looked like a good boundary for memory consumption.
// It's also sufficiently big that on a single-system Nixpkgs eval, it's bigger
// than `attributes.len() / 16`.
const DEFAULT_MAX_ALLOWABLE_PER_CHUNK: usize = 16_000;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    // Arguments affecting this tool's non-nix semantics.
    #[arg(long)]
    /// Filename to save the results. [default: <stdout>]
    output: Option<String>,
    #[arg(long)]
    /// Number of threads to use. [default: logical cpu count]
    cores: Option<usize>,
    #[arg(long, default_value_t = DEFAULT_MAX_ALLOWABLE_PER_CHUNK)]
    /// Maximum size for a chunk.
    max_chunk_size: usize,

    // Arguments affecting this tool's evaluation
    #[arg(
        long,
        default_value = "value: (value.recurseForDerivations or false) != false"
    )]
    /// Nix expression to determine whether to recurse in an attrset or not.
    ///
    /// The default follows the Nixpkgs conventions.
    recurse_with: String,

    /// Nix expression to determine whether to keep the value or not.
    ///
    /// The default keeps only derivations.
    #[arg(
        long,
        default_value = "value: builtins.isAttrs value && value ? drvPath"
    )]
    filter_with: String,

    // Arguments affecting the user's input
    #[arg(short = 'E', long = "expr")]
    /// Interpret [FILE] as an expression to evaluate.
    is_expression: bool,

    #[arg(long = "arg", num_args = 2, action = clap::ArgAction::Append, value_names = ["NAME", "VALUE"])]
    /// Arguments passed directly to `nix-instantiate` as `--arg name value`
    nix_args: Option<Vec<String>>,

    /// Expression to evaluate, follows the standard Nix CLI semantics.
    ///
    ///  - When no file argument is given, defaults to `./default.nix`.
    ///  - When an angled-bracket argument is given, follows `NIX_PATH` resolution.
    ///  - When a relative path is given, relative to CWD.
    ///  - When an absolute path is given, used as-is.
    ///
    /// The `--expr` argument changes the interpretation of `[FILE]`.
    ///
    ///  - The given string is used as a Nix expression to evaluated.
    #[arg(verbatim_doc_comment)]
    file: Option<String>,

    // Arguments affecting Nix operation
    /// Whether to print a stack trace in case of Nix expression evaluation errors.
    #[arg(long)]
    show_trace: bool,

    #[arg(long)]
    /// Whether to perform evaluation in read/write mode.
    read_write_mode: bool,
}

impl Args {
    /// Parses the standard Nix CLI arguments, to NixInstantiateOptions.
    pub fn to_instantiate_options(&self) -> NixInstantiateOptions {
        let args = Args::parse();
        let mut options = NixInstantiateOptions::default();

        if args.show_trace {
            options.show_trace = Some(true);
        }
        if args.read_write_mode {
            options.read_write_mode = Some(true);
        }

        options
    }
}

fn main() -> Result<(), String> {
    // Path used to serialize the data, so it can be dropped from memory.
    let workdir = tempfile::tempdir().unwrap();

    // Parse CLI
    let mut args = Args::parse();

    // Build the shared CLI arguments for `nix-instantiate`.
    let instantiate_options = args.to_instantiate_options();

    // Figure out what the expression to evaluate is.
    let expression_input = if args.is_expression {
        NixCliExpression::Expression(args.file)
    } else {
        NixCliExpression::Path(args.file)
    };

    // Prepare expression arguments from CLI.
    let mut expression_arguments = {
        let mut ret = NixCliExpressionArguments::new();
        match args.nix_args {
            Some(args) => {
                // Transform `NAME VALUE` pairs into NixCliExpressionArguments values
                for pair in args.chunks(2) {
                    let [name, value] = pair else { unreachable!() };
                    ret.insert(name.to_string(), value.to_string());
                }
            }
            None => { /* no-op */ }
        }
        ret
    };

    // The function used to determine if recursing is needed.
    expression_arguments.insert("recurse".to_string(), args.recurse_with);

    // The function used to determine if an attribute is kept.
    expression_arguments.insert("filter".to_string(), args.filter_with);

    // This is the attribute set being evaluated.
    // This represents the "user input".
    // The fragment produced here is equivalent to the nix-* CLI semantics.
    let attribute_set = build_nix_cli_import_semantics_fragment(
        expression_input.clone(),
        expression_arguments.clone(),
    )
    .unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        std::process::exit(1);
    });
    expression_arguments.insert("attrs".to_string(), attribute_set.to_string());

    // No value? Use PROCESSOR_COUNT if available. This is part of the public API.
    if args.cores.is_none() {
        args.cores = get_env_int("PROCESSOR_COUNT")?
    }
    // Implicitly fall back to NIX_BUILD_CORES as a default. This is not part of the public API.
    if args.cores.is_none() {
        args.cores = get_env_int("NIX_BUILD_CORES")?
    }

    // Got a specific configuration?
    if args.cores.is_some() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.cores.unwrap())
            .build_global()
            .unwrap();
    }

    // Let rayon tell us what ended-up being chosen.
    let cores = rayon::current_num_threads();

    eprintln!("\n:: Evaluating attributes\n");

    let attributes = get_attributes(&expression_arguments, &instantiate_options).unwrap();
    // The smallest between:
    //  - The maximum allowable attributes per chunks
    //  - The natural distributed chunk size
    let optimal_chunk_size = cmp::min(
        DEFAULT_MAX_ALLOWABLE_PER_CHUNK,
        // NOTE: We're `ceil`ing here so we have *at most* `cores` chunks.
        // Otherwise we'd get `cores + 1` chunks with remainders in the additional one.
        (attributes.len() as f64 / cores as f64).ceil() as usize,
    );
    if args.max_chunk_size > optimal_chunk_size {
        args.max_chunk_size = optimal_chunk_size;
    }

    // Evaluate attribute names, and turn into chunks of JSON, as files.
    let chunk_paths: Vec<_> = attributes
        .chunks(args.max_chunk_size)
        .enumerate()
        .map(|(i, chunk)| {
            let path = workdir.path().join(format!("eval-chunk.{:08}.json", i));
            fs::write(&path, serde_json::to_string_pretty(&chunk).unwrap())
                .expect("Error while writing attributes to file...");
            path
        })
        .collect();

    eprintln!("\n");
    eprintln!(":: Cores: {}", cores);
    eprintln!(":: Attributes count: {}", attributes.len());
    eprintln!(":: Optimal chunk size: {}", optimal_chunk_size);
    eprintln!(":: Current chunk size: {}", args.max_chunk_size);
    eprintln!(":: Chunk count: {}", chunk_paths.len());

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
            let data = evaluate_chunk::<serde_json::Value>(
                chunk_path,
                &expression_arguments,
                &instantiate_options,
            )
            .unwrap_or_else(|_| panic!("Unexpected error while handling chunk {}", chunk_path));

            let file = fs::File::open(chunk_path)
                .expect("Unexpected error while opening JSON chunk information...");

            let chunk_attributes: AttributePaths = serde_json::from_reader(file)
                .expect("Unexpected error while parsing JSON chunk information...");

            let good_attributes = data
                .iter()
                .map(|entry| entry.attr_path.clone())
                .collect::<AttributePaths>()
                .to_vec();

            let attributes_with_errors = chunk_attributes
                .into_iter()
                .filter(|el| !good_attributes.contains(el))
                .collect::<AttributePaths>()
                .to_vec();

            evaluation::Results {
                evaluation_arguments: None,
                evaluation_input: None,
                attributes_with_errors,
                data,
            }
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

    eprintln!("\n:: Writing output...\n");

    let io: Box<dyn std::io::Write> = match args.output {
        Some(path) => Box::new(std::fs::File::create(&path).unwrap()),
        None => Box::new(std::io::stdout()),
    };

    result.evaluation_arguments = Some(expression_arguments);

    result.evaluation_input = match expression_input {
        NixCliExpression::Path(v) => Some(NixCliExpression::Path(Some(nix::resolve_path(v)))),
        v => Some(v),
    };

    serde_json::to_writer_pretty(io, &result).expect("Error while writing results...");

    Ok(())
}
