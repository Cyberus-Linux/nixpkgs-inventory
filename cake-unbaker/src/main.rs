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
    };

    Ok(())
}
