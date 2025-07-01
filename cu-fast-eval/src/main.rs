use std::env;

use clap::Parser;

use cu_fast_eval::get_env_int;
use cu_fast_eval::nix;
use cu_fast_eval::nix::{build_nix_cli_import_semantics_fragment, evaluate_chunked};
use cu_lib_nix::nix::cli::{NixCliExpression, NixCliExpressionArguments, NixInstantiateOptions};

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
    #[arg(long, default_value_t = nix::DEFAULT_MAX_ALLOWABLE_PER_CHUNK)]
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
    let verbose = !env::var("VERBOSE").unwrap_or("".to_string()).is_empty();

    // Parse CLI
    let mut args = Args::parse();

    // Build the shared CLI arguments for `nix-instantiate`.
    let instantiate_options = args.to_instantiate_options();

    // Figure out what the expression to evaluate is.
    let expression_input = if args.is_expression {
        NixCliExpression::Expression(args.file)
    } else {
        NixCliExpression::Path(Some(nix::resolve_path(args.file)))
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
    let expression_fragment =
        build_nix_cli_import_semantics_fragment(&expression_input, &expression_arguments)
            .unwrap_or_else(|msg| {
                eprintln!("error: {msg}");
                std::process::exit(1);
            });

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

    if verbose {
        eprintln!("\n:: Starting evaluation...\n");
    }

    let mut result = evaluate_chunked::<serde_json::Value>(
        args.max_chunk_size,
        &expression_fragment,
        &expression_arguments,
        &instantiate_options,
    )
    .expect("Chunked evaluation unexpectedly failed");

    if verbose {
        eprintln!("\n:: Writing output...\n");
    }

    let io: Box<dyn std::io::Write> = match args.output {
        Some(path) => Box::new(std::fs::File::create(&path).unwrap()),
        None => Box::new(std::io::stdout()),
    };

    result.evaluation_arguments = Some(expression_arguments);
    result.evaluation_input = Some(expression_input);
    serde_json::to_writer_pretty(io, &result).expect("Error while writing results...");

    Ok(())
}
