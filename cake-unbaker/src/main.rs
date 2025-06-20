use cu_ingest_to_db::database;
use cu_lib_nix::nix;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Commands to inspect derivations
    Derivation {
        #[command(subcommand)]
        command: DerivationCommands,
    },
    /// Commands to inspect package sets
    Pkgs {
        #[command(subcommand)]
        command: PkgsCommands,
    },
}

#[derive(Subcommand, Debug)]
enum DerivationCommands {
    /// Outputs references as a JSON object.
    ///
    /// JSON object contains two arrays:
    ///   - derivations
    ///   - paths
    ///
    /// NOTE: Format is not guaranteed to be stable across versions.
    #[command(verbatim_doc_comment)]
    References {
        /// Path to get references from
        path: nix::StorePath,
    },
}

#[derive(Subcommand, Debug)]
enum PkgsCommands {
    /// Index a package set attributes and derivations
    #[command(verbatim_doc_comment)]
    Index {
        database: String,
        /// Package sets, from references, to add to the database.
        ///
        /// The package sets will have their paths normalized by adding them to the Nix store.
        /// See `cake-unbaker pkgs store-path` for allowed reference types.
        references: Vec<nix::SourceReference>,
    },
    /// Import package set, from reference, to the store.
    ///
    /// This provides a relatively stable identifier (the store path) for a given package set
    /// reference.
    ///
    /// Allowed reference types:
    ///   - Path
    ///
    /// When given a path, if it is detected to be a Git repository, use the reference as
    /// `fetchGit` input.
    ///
    /// The “same” inputs should produce the same store paths, whichever source they are coming
    /// from, as much as possible.
    ///
    /// The name of the store path is printed on the standard output.
    #[command(verbatim_doc_comment)]
    StorePath { reference: nix::SourceReference },
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Derivation { command } => match command {
            DerivationCommands::References { path } => {
                let result = nix::derivation_references(path)
                    .expect("Unexpected error while looking for derivation references...");
                let io = std::io::stdout();
                serde_json::to_writer_pretty(io, &result)
                    .expect("Unexpected error while converting to JSON...");
            }
        },
        Commands::Pkgs { command } => match command {
            PkgsCommands::Index {
                database,
                references,
            } => {
                let db = database::open(&database)
                    .expect("Unexpected error while opening the database.");

                eprintln!(":: Normalizing references...");
                let mut new_drvs = Vec::new();
                let paths: Vec<_> = references
                    .iter()
                    .map(|reference| {
                        eprintln!("   {reference}");
                        let path = nix::normalized_add_to_store(reference).unwrap();
                        eprintln!("   Normalized to: {path}");
                        (reference, path)
                    })
                    .collect();

                eprintln!("\n:: Evaluating references...");
                for (_reference, path) in paths {
                    // First check if the database already knows about the (normalized) store path.
                    if db.has_evaluation(&path) {
                        eprintln!(
                            "    ← Skipping evaluation for {}. Already in database.",
                            &path
                        );
                        continue;
                    }
                    eprintln!("    → Evaluating {path}...");

                    let instantiate_options = nix::cli::NixInstantiateOptions {
                        read_write_mode: Some(true),
                        ..nix::cli::NixInstantiateOptions::default()
                    };

                    let expression_input = nix::cli::NixCliExpression::Path(Some(path.clone()));
                    let expression_arguments = {
                        let mut args = nix::cli::NixCliExpressionArguments::new();
                        args.insert("config".to_string(), "{ allowAliases = false; }".to_string());
                        args.insert("currentSystem".to_string(), "builtins.currentSystem".to_string());
                        // FIXME: internalize handling filter/recurse in chunked_eval
                        args.insert("filter".to_string(), "value: builtins.isAttrs value && value ? drvPath".to_string());
                        args.insert("recurse".to_string(), "value: (value.recurseForDerivations or false) != false".to_string());
                        args
                    };

                    let expression_fragment =
                        cu_fast_eval::nix::build_nix_cli_import_semantics_fragment(
                            &expression_input,
                            &expression_arguments,
                        )?;

                    let results = cu_fast_eval::nix::evaluate_chunked(
                        cu_fast_eval::nix::DEFAULT_MAX_ALLOWABLE_PER_CHUNK,
                        &expression_fragment,
                        &expression_arguments,
                        &instantiate_options,
                    )?;

                    new_drvs.append(&mut cu_ingest_to_db::ingest_evaluation(
                        &db, results, &path,
                    ));
                }
            }
            PkgsCommands::StorePath { reference } => {
                println!("{}", nix::normalized_add_to_store(reference)?)
            }
        },
    };

    Ok(())
}
