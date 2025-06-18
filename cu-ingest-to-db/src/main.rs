use std::fs;

use cu_fast_eval::evaluation;
use cu_ingest_to_db::database;
use cu_ingest_to_db::derivation;

use clap::Parser;

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

    for path in args.json_paths {
        // NOTE: `serde_json::from_reader(fs::File::open(...))` is a lot slower than expected...
        let s = fs::read_to_string(&path).unwrap();
        let results: evaluation::Results<derivation::Info> = serde_json::from_str(&s).unwrap();
        let id = path;

        let exists = 0 == conn
            .insert(
                database::InsertQuery {
                    directive: database::InsertDirective::InsertOrIgnore,
                    table: database::Table::EvaluationInformations,
                },
                database::named_params! {
                    ":id": &id,
                    ":expression": &serde_json::to_string(&results.evaluation_input).unwrap(),
                    ":arguments": &serde_json::to_string(&results.evaluation_arguments).unwrap(),
                },
            )
            .expect("Unable to execute insert for evaluation_informations");

        if exists {
            eprintln!(":: Skipping data for {}, already in database.", &id);
            continue;
        }

        eprintln!(":: Adding data for {}.", &id);
        for result in results.data.iter() {
            eprintln!(">> Ingesting {}", result.attribute_to_nix());
            conn.insert(
                database::InsertQuery {
                    directive: database::InsertDirective::Insert,
                    table: database::Table::EvaluationAttributes,
                },
                database::named_params! {
                    ":evaluation_id": &id,
                    ":attribute_path": &serde_json::to_string(&result.attr_path).unwrap(),
                    ":attribute": &result.attribute_to_nix(),
                    ":derivation_path": &result.drv_path,
                    ":name": &result.name,
                    ":pname": &result.pname,
                    ":version": &result.version,
                    ":_raw": &serde_json::to_string(&result.attr_path).unwrap(),
                },
            )
            .expect("Unable to execute insert for evaluation_attributes");

            // Insert derivation data if needed...
            let exists = 0
                == conn
                    .insert(
                        database::InsertQuery {
                            directive: database::InsertDirective::InsertOrIgnore,
                            table: database::Table::Derivations,
                        },
                        database::named_params! {
                            ":path": &result.drv_path
                        },
                    )
                    .expect("Unable to execute insert for derivation");

            // ... and the derivation's data, when needed.
            // We don't need to re-add the existing `drv` data.
            // `drv` data for a given `drv` is the same, whichever Nixpkgs produced it.
            if !exists {
                println!("++ drv: {}", result.drv_path);

                // Save output paths for a given drv.
                for (output, store_path) in &result.outputs {
                    eprintln!("   {}: {}", output, store_path);
                    conn.insert(
                        database::InsertQuery {
                            directive: database::InsertDirective::Insert,
                            table: database::Table::DerivationOutputPaths,
                        },
                        database::named_params! {
                            ":derivation_path": &result.drv_path,
                            ":store_path": &store_path,
                            ":output": &output,
                        },
                    )
                    .expect("Unable to execute insert for derivation_output_paths");
                }

                // Save inputs for a given drv.
                for (input_type, list) in [("src", &result.srcs), ("patch", &result.patches)] {
                    for store_path in list {
                        eprintln!("   {}: {}", input_type, store_path);
                        conn.insert(
                            database::InsertQuery {
                                directive: database::InsertDirective::Insert,
                                table: database::Table::DerivationInputPaths,
                            },
                            database::named_params! {
                                ":derivation_path": &result.drv_path,
                                ":store_path": &store_path,
                                ":type": &input_type,
                            },
                        )
                        .expect("Unable to execute insert for derivation input");
                    }
                }
            }
        }
    }
}
