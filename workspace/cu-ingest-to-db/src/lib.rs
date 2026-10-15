use cu_fast_eval::evaluation;
use cu_lib_nix::nix;
use cu_lib_nix::nix::derivation_references;
use rayon::prelude::*;

pub mod database;

/// Types describing the results of evaluating a Nixpkgs-like attrset.
pub mod derivation {
    use cu_lib_nix::nix;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::collections::HashSet;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(untagged)]
    pub enum StringOrList {
        String(String),
        List(Vec<String>),
    }

    // TODO: implement license fields parsing.
    pub type MetaLicense = serde_json::Value;

    #[derive(Serialize, Deserialize, Debug)]
    /// Nixpkgs-flavoured metadata attrset.
    pub struct Meta {
        pub available: Option<bool>,
        pub broken: Option<bool>,
        pub insecure: Option<bool>,
        pub unfree: Option<bool>,
        pub unsupported: Option<bool>,

        #[serde(rename = "outputsToInstall")]
        pub outputs_to_install: Option<Vec<String>>,

        pub homepage: Option<StringOrList>,

        pub description: Option<String>,
        #[serde(rename = "longDescription")]
        pub long_description: Option<String>,

        #[serde(rename = "mainProgram")]
        pub main_program: Option<String>,

        pub position: Option<String>,

        pub license: Option<MetaLicense>,
        pub licenses: Option<Vec<MetaLicense>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    /// Data and metadata about a derivation.
    pub struct Info {
        #[serde(rename = "attrPath")]
        pub attr_path: nix::AttributePath,
        #[serde(rename = "drvPath")]
        pub drv_path: String,
        pub meta: Option<Meta>,
        pub name: String,
        pub outputs: BTreeMap<String, String>,
        pub pname: Option<String>,
        pub version: Option<String>,
        pub srcs: HashSet<String>,
        pub patches: Vec<String>,
    }

    impl Info {
        /// Formats the attribute path as the joined "deep" access form for usage in Nix.
        /// i.e. `["a" "b" "c"]` → `"a"."b"."c"`
        pub fn attribute_to_nix(&self) -> String {
            self.attr_path
                .iter()
                .map(|component| serde_json::to_string(component).unwrap())
                .collect::<Vec<String>>()
                .join(".")
        }
    }
}

/// Ingests the results of a `cu-fast-eval` into `path`, returning derivations without associated
/// references data.
///
/// Run `resolve_derivation_references` on the returned value after ingesting.
pub fn ingest_evaluation(
    conn: &database::AttributesDatabase,
    results: evaluation::Results<derivation::Info>,
    id: &String,
) -> Vec<nix::StorePath> {
    let mut drvs_without_references_data = Vec::new();

    let exists = 0
        == conn
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
        return drvs_without_references_data;
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
                ":_raw": &serde_json::to_string(&result).unwrap(),
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
                eprintln!("   {output}: {store_path}");
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

            // Some derivations may refer to the same source archive more than once.
            let srcs: Vec<_> = result.srcs.clone().into_iter().collect();

            // Save inputs for a given drv.
            for (input_type, list) in [("src", &srcs), ("patch", &result.patches)] {
                for store_path in list {
                    eprintln!("   {input_type}: {store_path}");
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
        if !has_inputs && !drvs_without_references_data.contains(&result.drv_path) {
            // Queue new drvs to be checked.
            drvs_without_references_data.push(result.drv_path.clone());
        }
    }

    drvs_without_references_data
}

/// Resolve referencs for the given derivations, and saves them to the given database.
/// The database connection is *moved*, as it needs to be made tread-safe.
pub fn resolve_derivation_references(
    conn: &database::AttributesDatabase,
    drvs: Vec<nix::StorePath>,
) {
    drvs.into_par_iter().for_each(|drv_path| {
        eprintln!(" ⇒ {drv_path}");
        let references = derivation_references(&drv_path).unwrap();

        // Get a lock on the actual db connection once the long-running command is done.

        for store_path in references.derivations {
            eprintln!("        derivation: {store_path}");
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
        }
        for store_path in references.paths {
            eprintln!("        path: {store_path}");
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
    })
}

#[cfg(test)]
mod tests {
    mod lib {
        use crate::derivation;

        const HELLO_DERIVATION_DATA: &str = r#"
            {
              "attrPath": [
                "hello"
              ],
              "drvPath": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1.drv",
              "meta": {
                "available": true,
                "broken": false,
                "changelog": "https://git.savannah.gnu.org/cgit/hello.git/plain/NEWS?h=v2.12.1",
                "description": "Program that produces a familiar, friendly greeting",
                "homepage": "https://www.gnu.org/software/hello/manual/",
                "insecure": false,
                "license": {
                  "deprecated": false,
                  "free": true,
                  "fullName": "GNU General Public License v3.0 or later",
                  "redistributable": true,
                  "shortName": "gpl3Plus",
                  "spdxId": "GPL-3.0-or-later",
                  "url": "https://spdx.org/licenses/GPL-3.0-or-later.html"
                },
                "longDescription": "GNU Hello is a program that prints \"Hello, world!\" when you run it.\nIt is fully customizable.\n",
                "mainProgram": "hello",
                "maintainers": [
                  {
                    "email": "post@steffenvogel.de",
                    "github": "stv0g",
                    "githubId": 285829,
                    "keys": [
                      {
                        "fingerprint": "09BE 3BAE 8D55 D4CD 8579  285A 9675 EAC3 4897 E6E2"
                      }
                    ],
                    "matrix": "@stv0ge:matrix.org",
                    "name": "Steffen Vogel"
                  }
                ],
                "maintainersPosition": {
                  "column": 5,
                  "file": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source/pkgs/by-name/he/hello/package.nix",
                  "line": 55
                },
                "name": "hello-2.12.1",
                "outputsToInstall": [
                  "out"
                ],
                "platforms": [
                  "i686-cygwin",
                  "x86_64-cygwin",
                  "x86_64-darwin",
                  "aarch64-darwin",
                  "i686-freebsd",
                  "x86_64-freebsd",
                  "aarch64-freebsd",
                  "aarch64-genode",
                  "i686-genode",
                  "x86_64-genode",
                  "x86_64-solaris",
                  "javascript-ghcjs",
                  "aarch64-linux",
                  "armv5tel-linux",
                  "armv6l-linux",
                  "armv7a-linux",
                  "armv7l-linux",
                  "i686-linux",
                  "loongarch64-linux",
                  "m68k-linux",
                  "microblaze-linux",
                  "microblazeel-linux",
                  "mips-linux",
                  "mips64-linux",
                  "mips64el-linux",
                  "mipsel-linux",
                  "powerpc64-linux",
                  "powerpc64le-linux",
                  "riscv32-linux",
                  "riscv64-linux",
                  "s390-linux",
                  "s390x-linux",
                  "x86_64-linux",
                  "mmix-mmixware",
                  "aarch64-netbsd",
                  "armv6l-netbsd",
                  "armv7a-netbsd",
                  "armv7l-netbsd",
                  "i686-netbsd",
                  "m68k-netbsd",
                  "mipsel-netbsd",
                  "powerpc-netbsd",
                  "riscv32-netbsd",
                  "riscv64-netbsd",
                  "x86_64-netbsd",
                  "aarch64_be-none",
                  "aarch64-none",
                  "arm-none",
                  "armv6l-none",
                  "avr-none",
                  "i686-none",
                  "microblaze-none",
                  "microblazeel-none",
                  "mips-none",
                  "mips64-none",
                  "msp430-none",
                  "or1k-none",
                  "m68k-none",
                  "powerpc-none",
                  "powerpcle-none",
                  "riscv32-none",
                  "riscv64-none",
                  "rx-none",
                  "s390-none",
                  "s390x-none",
                  "vc4-none",
                  "x86_64-none",
                  "i686-openbsd",
                  "x86_64-openbsd",
                  "x86_64-redox",
                  "wasm64-wasi",
                  "wasm32-wasi",
                  "aarch64-windows",
                  "x86_64-windows",
                  "i686-windows"
                ],
                "position": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source/pkgs/by-name/he/hello/package.nix:47",
                "unfree": false,
                "unsupported": false
              },
              "name": "hello-2.12.1",
              "outputs": {
                "out": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1"
              },
              "patches": [],
              "pname": "hello",
              "srcs": [
                "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1.tar.gz",
                "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1.tar.gz"
              ],
              "version": "2.12.1"
            }
        "#;

        #[test]
        fn test_derivation_data() {
            let data: derivation::Info = serde_json::from_str(HELLO_DERIVATION_DATA).unwrap();
            assert_eq!(data.name, "hello-2.12.1".to_string());
            assert_eq!(data.pname, Some("hello".to_string()));
            assert_eq!(data.version, Some("2.12.1".to_string()));
            assert_eq!(data.attr_path, vec!("hello"));
            assert_eq!(
                data.meta.unwrap().description,
                Some("Program that produces a familiar, friendly greeting".to_string())
            );
            let srcs: Vec<_> = data.srcs.clone().into_iter().collect();
            assert_eq!(
                srcs,
                vec!["/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1.tar.gz"],
            );
        }
    }
}
