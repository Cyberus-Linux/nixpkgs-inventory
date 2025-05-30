use serde_json::{Result};
use std::process::Command;

const NIX_EXPR_DUMP_ATTRIBUTES: &str = include_str!("nix/dump-attributes.nix");

fn get_attributes() -> Result<Vec<Vec<String>>> {
    let output = Command::new("nix-instantiate")
        .arg("--strict")
        .arg("--json")
        .arg("--eval")
        .args(["-I", "nixpkgs=~/tmp/nixpkgs/nixos-unstable"])
        .args(["--arg", "attrs", "import <nixpkgs> {}"])
        .args(["--arg", "recurse", "value: (value.recurseForDerivations or false) != false"])
        .args(["--arg", "filter", "value: builtins.isAttrs value && value ? drvPath"])
        .args(["--expr", NIX_EXPR_DUMP_ATTRIBUTES])
        .output()
        .expect("Error while running nix-instantiate")
    ;

    let json_str: String =
        String::from_utf8(output.stdout)
            .unwrap()
    ;
    serde_json::from_str(&json_str)
}

fn main() {
    get_attributes().unwrap().iter().for_each(|item| {
        println!("JSON data: {}", item.join("."));
    });
}
