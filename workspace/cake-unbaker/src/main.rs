use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;

use cake_unbaker::{attribute_matchers, flake};
use cu_ingest_to_db::database;
use cu_lib_nix::nix;
use nix::AttributePath;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long)]
    output: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Commands to inspect derivations.
    Derivation {
        #[command(subcommand)]
        command: DerivationCommands,
    },
    /// Commands to inspect package sets.
    Pkgs {
        #[command(subcommand)]
        command: PkgsCommands,
    },
    /// From different input formats, list the attributes used from a package set.
    ///
    /// The database used to list the attributes comes from the `pkgs index` command.
    ///
    /// The attributes chosen in the list may not be the exact attributes used in the input,
    /// but maps to the same derivation. The discrepancy is due to how multiple attributes may
    /// point to the same derivation. The instantiation is lossy since it only works with
    /// derivations, and does not know how the derivation was selected.
    ///
    /// The heuristics prefer shorter attribute names that are closer to the root.
    /// Since the `pkgs index` command works with `{ allowAliases = false }`, aliases should not be
    /// present.
    ///
    /// If the database houses indices for many evaluations, attribute names should still match
    /// properly, but a package that was aliased and its attribute renamed may map to the older
    /// name, depending on the timeline and luck.
    GetAttributes(GetAttributes),

    /// List the attributes used from Nixpkgs by a flake output.
    ///
    /// This command is self-contained and will index the relevant Nixpkgs automatically.
    GetFlakeAttributes {
        #[arg(long)]
        output_format: AttributesOutputFormats,

        /// Nixpkgs used by the flake output, when it can't be determined automatically.
        ///
        /// See `cake-unbaker pkgs store-path` for allowed reference types.
        #[arg(long)]
        nixpkgs: Option<nix::SourceReference>,

        /// Flake output to list the attributes of, e.g. `.#nixosConfigurations.host.config.system.build.toplevel`.
        flake_ref: String,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum AttributesOutputFormats {
    /// Unstable JSON representation of the attributes matcher data.
    ///
    /// The contents will evolve with time. Prefer another stable format for scripting and
    /// automations.
    Raw,
    /// JSON-formatted array of attribute path arrays.
    OnlyAttributePaths,
}

#[derive(Parser, Debug, Clone)]
struct GetAttributes {
    #[arg(long)]
    input_format: attribute_matchers::InputFormats,
    #[arg(long)]
    output_format: AttributesOutputFormats,
    /// Path to the packages index database.
    database: String,
    #[structopt(required = true)]
    /// Inputs from which to list the attributes of.
    inputs: Vec<String>,
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

/// Index the given package sets into the database.
fn index(database: &String, references: &[nix::SourceReference]) -> Result<(), String> {
    let db = database::open(database).expect("Unexpected error while opening the database.");

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
                path
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
            args.insert(
                "config".to_string(),
                "{ allowAliases = false; allowUnfree = true; }".to_string(),
            );
            args.insert(
                "currentSystem".to_string(),
                "builtins.currentSystem".to_string(),
            );
            // FIXME: internalize handling filter/recurse in chunked_eval
            args.insert(
                "filter".to_string(),
                "value: builtins.isAttrs value && value ? drvPath".to_string(),
            );
            args.insert(
                "recurse".to_string(),
                "value: (value.recurseForDerivations or false) != false".to_string(),
            );
            args
        };

        let expression_fragment = cu_fast_eval::nix::build_nix_cli_import_semantics_fragment(
            &expression_input,
            &expression_arguments,
        )?;

        let results = cu_fast_eval::nix::evaluate_chunked(
            cu_fast_eval::nix::DEFAULT_MAX_ALLOWABLE_PER_CHUNK,
            &expression_fragment,
            &expression_arguments,
            &instantiate_options,
        )?;

        new_drvs.append(&mut cu_ingest_to_db::ingest_evaluation(&db, results, &path));
    }

    Ok(())
}

/// Writes the attributes matched from the given paths in the requested format.
fn write_attributes<'a>(
    output: Box<dyn std::io::Write>,
    output_format: &AttributesOutputFormats,
    database: &String,
    paths: impl Iterator<Item = &'a nix::StorePath>,
) {
    let matcher = attribute_matchers::Matcher::with_database(database);
    let results = matcher.find_from_paths(paths);

    match output_format {
        AttributesOutputFormats::Raw => {
            serde_json::to_writer_pretty(output, &results)
                .expect("Unexpected error while producing raw output");
        }
        AttributesOutputFormats::OnlyAttributePaths => {
            let results: BTreeSet<AttributePath> = results.into_values().flatten().collect();
            let mut output = output;
            output
                .write_fmt(format_args!(
                    "{}",
                    cake_unbaker::to_attribute_paths_json(results.iter())
                ))
                .expect("Unexpected error while adding to store...");
        }
    };
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut output: Box<dyn std::io::Write> = match cli.output {
        Some(path) => Box::new(std::fs::File::create(&path).unwrap()),
        None => Box::new(std::io::stdout()),
    };

    match &cli.command {
        Commands::Derivation { command } => match command {
            DerivationCommands::References { path } => {
                let result = nix::derivation_references(path)
                    .expect("Unexpected error while looking for derivation references...");
                serde_json::to_writer_pretty(output, &result)
                    .expect("Unexpected error while converting to JSON...");
            }
        },
        Commands::Pkgs { command } => match command {
            PkgsCommands::Index {
                database,
                references,
            } => index(database, references).map_err(anyhow::Error::msg)?,
            PkgsCommands::StorePath { reference } => {
                let path = nix::normalized_add_to_store(reference).map_err(anyhow::Error::msg)?;
                output
                    .write_fmt(format_args!("{path}"))
                    .expect("Unexpected error while adding to store...");
            }
        },
        Commands::GetAttributes(subcommand_args) => {
            let paths = subcommand_args
                .inputs
                .iter()
                .map(|input| {
                    let file = File::open(input)
                        .unwrap_or_else(|_| panic!("Could not open input {input}"));
                    let reader = BufReader::new(file);

                    attribute_matchers::extract_derivations(&subcommand_args.input_format, reader)
                })
                .reduce(|mut a, mut b| {
                    a.append(&mut b);
                    a
                })
                .expect("No input produced from inputs?");

            write_attributes(
                output,
                &subcommand_args.output_format,
                &subcommand_args.database,
                paths.iter(),
            );
        }
        Commands::GetFlakeAttributes {
            output_format,
            nixpkgs,
            flake_ref,
        } => {
            let database = &flake::database_path()?;

            let nixpkgs = match nixpkgs {
                Some(nixpkgs) => nixpkgs.clone(),
                None => flake::nixpkgs_for(flake_ref)?,
            };
            index(database, &[nixpkgs]).map_err(anyhow::Error::msg)?;

            let drv = flake::derivation_path(flake_ref)?;
            let paths = flake::requisites(&drv)?;

            write_attributes(output, output_format, database, paths.iter());
        }
    };

    Ok(())
}
