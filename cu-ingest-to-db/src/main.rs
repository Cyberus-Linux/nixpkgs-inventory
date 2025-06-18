use std::fs;

use cu_fast_eval::evaluation;
use cu_ingest_to_db::database;
use cu_ingest_to_db::derivation;
use cu_lib_nix::nix::derivation_references;

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

    let mut new_drvs: Vec<String> = Vec::new();

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

            // When "derivation inputs" for a derivation exists for a given derivation, skip it.
            // A derivation without any input will checked every time this runs, but these are
            // outliers, and we're more than likely building a fresh database.
            let has_inputs = conn
                .query_one(
                    &format!(
                        "SELECT COUNT(*) FROM {} WHERE derivation_path = :path",
                        database::Table::DerivationInputDerivations.get_name()
                    ),
                    database::named_params! {
                        ":path": &result.drv_path,
                    },
                    |row| row.get::<usize, usize>(0),
                )
                .unwrap()
                > 0;

            // Also make sure we don't re-add something to the list.
            // (Multiple attribute paths can refer to the same derivation)
            if !has_inputs && !new_drvs.contains(&result.drv_path) {
                // Queue new drvs to be checked.
                new_drvs.push(result.drv_path.clone());
            }
        }
    }

    eprintln!("\n\n:: Querying derivations references...\n");

    for drv_path in new_drvs {
        eprintln!(" ⇒ {}", drv_path);
        let references = derivation_references(&drv_path).unwrap();

        for store_path in references {
            let input_is_drv = store_path.ends_with(".drv");
            let reference_type = if input_is_drv { "derivation" } else { "path" };
            eprintln!("        {}: {}", reference_type, store_path);

            if input_is_drv {
                conn.insert(
                    database::InsertQuery {
                        directive: database::InsertDirective::Insert,
                        table: database::Table::DerivationInputDerivations,
                    },
                    database::named_params! {
                        ":derivation_path": &drv_path,
                        ":input_derivation": &store_path,
                        // We don't know the output (store path) referred to with `--references`.
                        // This is sufficient.
                        // If needed, parsing the `.drv` file gives us the info.
                        ":output": "(reference)",
                    },
                )
                .expect("Unable to execute insert for derivation input (derivation)");
            } else {
                conn.insert(
                    database::InsertQuery {
                        directive: database::InsertDirective::Insert,
                        table: database::Table::DerivationInputPaths,
                    },
                    database::named_params! {
                        ":derivation_path": &drv_path,
                        ":store_path": &store_path,
                        ":type": "reference",
                    },
                )
                .expect("Unable to execute insert for derivation input (path)");
            }
        }
    }
}
