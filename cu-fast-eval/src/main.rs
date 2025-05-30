use shlex::{self};
use AsRef::{self};
use serde::{Deserialize, Serialize};
use serde_json::{Result};
use std::process::Command;
use std::process::Stdio;

const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");
const NIX_EXPR_PROCESS_CHUNK: &str = include_str!("nix/process-chunk.nix");

// Prints (to stderr) a formatted useful version of the command used.
// NOTE: Multi-line arguments will be elided to their first line!
fn log_command(cmd: &Command) {
    let args: Vec<String> =
        cmd
            .get_args()
            .collect::<Vec<_>>()
            .iter().map( |s| {
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

// Returns a list of attr *paths* (lists of lists of attribute name parts).
fn get_attributes() -> Result<Vec<Vec<String>>> {
    eprintln!("\n:: Evaluating attribtues\n");

    let mut cmd = Command::new("nix-instantiate");
    cmd
        .arg("--strict")
        .arg("--json")
        .arg("--eval")
        // FIXME: following must come from arguments parsing.
        /* XXX */.args(["-I", "nixpkgs=~/tmp/nixpkgs/nixos-unstable"])
        /* XXX */.args(["--arg", "attrs", "import <nixpkgs> {}"])
        /* XXX */.args(["--arg", "recurse", "value: (value.recurseForDerivations or false) != false"])
        /* XXX */.args(["--arg", "filter", "value: builtins.isAttrs value && value ? drvPath"])
        .args(["--expr", NIX_EXPR_DUMP_ATTRIBUTES])
    ;
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

fn main() {
    let attributes = get_attributes().unwrap();
    eprintln!(":: Number of attributes: {}", attributes.len());
    //attributes.iter().for_each(|item| {
    //    println!("JSON data: {}", item.join("."));
    //});
}
