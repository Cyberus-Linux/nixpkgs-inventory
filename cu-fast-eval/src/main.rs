use AsRef::{self};
use std::cmp;
use std::env;
use std::fs;
use std::process::Command;
use std::process::Stdio;
use serde::{Deserialize, Serialize};

use clap::Parser;
use rayon::prelude::*;
use shlex::{self};

const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");
const NIX_EXPR_PROCESS_CHUNK: &str = include_str!("nix/process-chunk.nix");

// This is arbitrary, but looked like a good boundary for memory consumption.
// It's also sufficiently big that on a single-system Nixpkgs eval, it's bigger
// than `attributes.len() / 16`.
const DEFAULT_MAX_ALLOWABLE_PER_CHUNK: usize = 16_000;

fn get_env_int(name: &str) -> Result<Option<usize>, String> {
    match env::var(name) {
        Ok(value) => {
            if !value.is_empty() {
                match value.parse::<usize>() {
                    Ok(value) => { Ok(Some(value)) }
                    Err(err) => { Err(format!("{} needs to be an integer. ({})", name, err)) }
                }
            }
            else {
                Ok(None)
            }
        }
        Err(err) => {
            match err {
                env::VarError::NotPresent => { Ok(None) }
                _ => { Err(format!("Unexpected error while handling {} environment variable. {}", name, err)) }
            }
        }
    }
}

// Prints (to stderr) a formatted useful version of the command used.
// NOTE: Multi-line arguments will be elided to their first line!
fn log_command(cmd: &Command) {
    let args: Vec<String> =
        cmd
            .get_args().map( |s| {
                let mut lines =
                    s.to_str().unwrap()
                    .lines()
                ;
                let first = lines.next().unwrap();
                match lines.next() {
                    // Only one line.
                    None => { first.to_string() }
                    // Multi-line
                    Some(_) => { format!("{}…", first) }
                }
            })
            .collect()
    ;
    eprintln!(" $ {}", shlex::try_join(args.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).unwrap())
}

/// Returns a list of attr *paths* (lists of lists of attribute name parts).
fn get_attributes(nix_args: &Option<Vec<String>>) -> serde_json::Result<AttributePaths> {
    eprintln!("\n:: Evaluating attribtues\n");

    let mut cmd = Command::new("nix-instantiate");
    cmd
        .arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(["--expr", NIX_EXPR_DUMP_ATTRIBUTES])
    ;

    if let Some(args) = nix_args { cmd.args(args); };

    log_command(&cmd);

    let result =
        cmd
        // Stream stderr to the console
        .stderr(Stdio::inherit())
        // Capture the output
        .output()
        .expect("Unexpected error while running nix-instantiate")
    ;

    // Parse result
    serde_json::from_str(&String::from_utf8_lossy(&result.stdout))
}

/// Returns the serde_json Array of serde_json Objects for the evaluated list of Nix attribute sets.
fn evaluate_chunk(nix_args: &Option<Vec<String>>, path: &str) -> serde_json::Result<serde_json::Value> {
    let mut cmd = Command::new("nix-instantiate");
    cmd
        .arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(["--expr", NIX_EXPR_PROCESS_CHUNK])
    ;

    // Add the user's arguments
    if let Some(args) = nix_args { cmd.args(args); };

    // Ensure `chunkPath` is the one we are passing to `nix-instantiate`.
    cmd.args(["--arg", "chunkPath", path]);

    log_command(&cmd);

    let result =
        cmd
        // Stream stderr to the console
        .stderr(Stdio::inherit())
        // Capture the output
        .output()
        .expect("Error while running nix-instantiate")
    ;

    // Parse result
    serde_json::from_str(&String::from_utf8_lossy(&result.stdout))
}

type AttributePath = Vec<String>;
type AttributePaths = Vec<AttributePath>;

type EvaluationData = Vec<serde_json::Value>;

#[derive(Serialize, Deserialize)]
struct EvaluationResults {
    good_attributes: AttributePaths,
    bad_attributes: AttributePaths,
    data: EvaluationData,
}

impl EvaluationResults {
    pub fn new() -> Self {
        Self {
            good_attributes: Vec::new(),
            bad_attributes: Vec::new(),
            data: EvaluationData::new(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(long)]
    /// Filename to save the results. [default: <stdout>]
    output: Option<String>,
    #[arg(long)]
    /// Number of threads to use. [default: logical cpu count]
    cores: Option<usize>,
    #[arg(long, default_value_t = DEFAULT_MAX_ALLOWABLE_PER_CHUNK)]
    /// Maximum size for a chunk.
    max_chunk_size: usize,
    #[arg(long = "arg", num_args = 2, action = clap::ArgAction::Append, value_names = ["NAME", "VALUE"])]
    /// Arguments passed directly to `nix-instantiate` as `--arg name value`
    nix_args: Option<Vec<String>>,
}

fn nix_args_pair_to_cli(pair: Vec<String>) -> Vec<String> {
    let mut args = Vec::new();
    args.push("--arg".to_string());
    for arg in pair {
        args.push(arg.to_string())
    }
    args
}

fn main() -> Result<(), String> {
    // Path used to serialize the data, so it can be dropped from memory.
    let workdir = tempfile::tempdir().unwrap();

    let mut args = Args::parse();

    // Transform `NAME VALUE` back into `--arg NAME VALUE`.
    if args.nix_args.is_some() {
        args.nix_args = Some(
            args.nix_args.unwrap()
                .chunks(2)
                .flat_map(|pair| nix_args_pair_to_cli(pair.to_vec()))
                .collect::<Vec<String>>()
                .to_vec()
        );
    }

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
        rayon::ThreadPoolBuilder::new().num_threads(args.cores.unwrap()).build_global().unwrap();
    }

    // Let rayon tell us what ended-up being chosen.
    let cores = rayon::current_num_threads();

    let attributes = get_attributes(&args.nix_args).unwrap();
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
    let chunk_paths: Vec<_> =
        attributes
        .chunks(args.max_chunk_size)
        .enumerate()
        .map(|(i, chunk)| {
            let path = workdir.path().join(format!("eval-chunk.{:08}.json", i));
            fs::write(&path, serde_json::to_string_pretty(&chunk).unwrap())
                .expect("Error while writing attributes to file...")
            ;
            path
        })
        .collect()
    ;

    eprintln!("\n");
    eprintln!(":: Cores: {}", cores);
    eprintln!(":: Attributes count: {}", attributes.len());
    eprintln!(":: Optimal chunk size: {}", optimal_chunk_size);
    eprintln!(":: Current chunk size: {}", args.max_chunk_size);
    eprintln!(":: Chunk count: {}", chunk_paths.len());

    drop(attributes);

    let result = chunk_paths
        .par_iter()
        .map(|chunk_path| {
            let chunk_path = chunk_path.to_str().unwrap();

            let data: EvaluationData =
                evaluate_chunk(&args.nix_args, chunk_path)
                    .unwrap_or_else(|_| panic!("Unexpected error while handling chunk {}", chunk_path))
                    .as_array()
                    .unwrap_or_else(|| panic!("Unexpected error while handling chunk {}", chunk_path))
                    .to_vec()
            ;

            let file = fs::File::open(chunk_path)
                .expect("Unexpected error while opening JSON chunk information...")
            ;
            let chunk_attributes: AttributePaths = serde_json::from_reader(file)
                .expect("Unexpected error while parsing JSON chunk information...")
            ;

            let good_attributes = data.iter()
                .map(|entry| {
                    serde_json::from_value(
                        entry.get("attrPath")
                            .expect("Expecting key `attrPath` on evaluation results attribute set.")
                            .clone()
                    )
                    .unwrap()
                })
                .collect::<AttributePaths>()
                .to_vec()
            ;

            let bad_attributes = chunk_attributes.into_iter()
                .filter(|el| {
                    !good_attributes.contains(el)
                })
                .collect::<AttributePaths>()
                .to_vec()
            ;

            EvaluationResults {
                good_attributes,
                bad_attributes,
                data,
            }
        })
        .fold(||{ EvaluationResults::new() }, |mut acc, mut curr| {
            acc.good_attributes.append(&mut curr.good_attributes);
            acc.bad_attributes.append(&mut curr.bad_attributes);
            acc.data.append(&mut curr.data);
            acc
        })
        .collect_vec_list()
    ;

    let io: Box<dyn std::io::Write> = match args.output {
        Some(path) => {
            Box::new(std::fs::File::create(&path).unwrap())
        }
        None => {
            Box::new(std::io::stdout())
        }
    };

    serde_json::to_writer_pretty(io, &result)
        .expect("Error while writing results...")
    ;

    Ok(())
}
