use clap::Parser;
use cu_ingest_to_db::database;
use std::fs;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    database: String,
    json_paths: Vec<String>,
}

fn main() {
    let args = Args::parse();

    let conn =
        database::open(&args.database).expect("Unexpected error while opening the database.");

    let mut new_drvs = Vec::new();

    for path in args.json_paths {
        // NOTE: `serde_json::from_reader(fs::File::open(...))` is a lot slower than expected...
        let s = fs::read_to_string(&path).unwrap();
        let results = serde_json::from_str(&s).unwrap();
        new_drvs.append(&mut cu_ingest_to_db::ingest_evaluation(
            &conn, results, &path,
        ));
    }

    eprintln!("\n\n:: Resolving derivation references...\n");

    cu_ingest_to_db::resolve_derivation_references(&conn, new_drvs);
}
