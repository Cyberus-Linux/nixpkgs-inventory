use cu_ingest_to_db::database as db;
use cu_lib_nix::nix;
use nix::AttributePath;
use nix::AttributePaths;
use nix::StorePath;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::BufRead;

/// Store some helpers and structs for the sbomnix input format.
pub mod sbomnix {
    pub mod cdx {
        use cu_lib_nix::nix;
        use nix::StorePath;
        use serde::de;
        use serde::{Deserialize, Serialize};

        /// Validates the json document is CDX.
        fn assert_bom_format<'de, D: de::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<String, D::Error> {
            let value = <String>::deserialize(deserializer);
            if value.is_ok() {
                let value = value.unwrap();
                if value == "CycloneDX" {
                    Ok(value)
                } else {
                    Err(de::Error::custom(format_args!(
                        "Format '{value}' invalid for an sbomnix CDX document."
                    )))
                }
            } else {
                value.map_err(de::Error::custom)
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct Document {
            #[serde(rename = "bomFormat")]
            #[serde(deserialize_with = "assert_bom_format")]
            pub bom_format: String,
            #[serde(rename = "specVersion")]
            pub spec_version: String,
            pub version: u32,
            #[serde(rename = "serialNumber")]
            pub serial_number: String,
            pub metadata: Metadata,
            pub components: Components,
            pub dependencies: Dependencies,
        }

        impl Document {
            pub fn get_component(&self, bom_ref: &BomRef) -> Option<&Component> {
                self.components
                    .iter()
                    .find(|component| &component.bom_ref == bom_ref)
            }
        }

        /// For an sbomnix, the BOM refs always are a Nix store path.
        pub type BomRef = StorePath;

        #[derive(Serialize, Deserialize, Clone, Debug)]
        // NOTE: currently incomplete, and only goes as far as needed.
        pub struct Metadata {
            // TODO: validate `metadata.tools` contains `sbomnix`.
            pub tools: Vec<serde_json::Value>,
            pub component: Component,
        }

        pub type Components = Vec<Component>;

        #[derive(Serialize, Deserialize, Clone, Debug)]
        // NOTE: currently incomplete, and only goes as far as needed.
        pub struct Component {
            #[serde(rename = "type")]
            // NOTE: sbomnix only outputs `library` types.
            pub component_type: String,
            #[serde(rename = "bom-ref")]
            pub bom_ref: BomRef,
            pub properties: ComponentProperties,
            pub purl: String,
            pub cpe: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ComponentProperties(Vec<ComponentProperty>);

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ComponentProperty {
            name: String,
            value: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct Dependencies(Vec<Dependency>);

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct Dependency {
            #[serde(rename = "ref")]
            pub bom_ref: BomRef,
            #[serde(rename = "dependsOn")]
            pub depends_on: Option<Vec<String>>,
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InputFormats {
    /// The `cdx.json` output from sbomnix.
    Sbomnix,
    /// The output of `nix-store --requisites`.
    Requisites,
}

pub struct Matcher {
    database: db::AttributesDatabase,
}

/// Extracts format-specific derivation information from the input file. This may not include the
/// full requisites. It may also be missing the build-time inputs.
pub fn extract_derivations(
    input_format: &InputFormats,
    input_data: impl BufRead,
) -> BTreeSet<StorePath> {
    match input_format {
        InputFormats::Requisites => input_data.lines().map(|l| l.unwrap()).collect(),
        InputFormats::Sbomnix => {
            let mut ret = BTreeSet::new();

            let sbom: sbomnix::cdx::Document =
                serde_json::from_reader(input_data).expect("Parsing sbomnix cdx document failed");

            // We can crawl just the `components[][bom-ref]` to get the input drvs.
            // This can be compared against this barbaric `grep`.
            //   $ grep '\.drv' sbom.cdx.json | sed -e 's/.*://' -e 's/^ *"//' -e 's/",\?$//' | sort -u
            // Which should output the same list.
            for component in sbom.components {
                ret.insert(component.bom_ref);
            }

            ret
        }
    }
}

/// Picks the attribute path that is shallower (i.e. a over a.a), then the shorter joined
/// string, and then lexicographically ordered first.
pub fn likeliest_attribute_path(a: &AttributePath, b: &AttributePath) -> Ordering {
    if a.len() == b.len() {
        let a = a.join(".");
        let b = b.join(".");
        if a.len() == b.len() {
            a.cmp(&b)
        } else {
            a.len().cmp(&b.len())
        }
    } else {
        a.len().cmp(&b.len())
    }
}

impl Matcher {
    pub fn with_database(path: &String) -> Self {
        let database = cu_ingest_to_db::database::open(path)
            .expect("Unexpected error while opening the database.");

        Matcher { database }
    }

    /// Given a single store path, finds the likeliest attribute path to reach it.
    pub fn find_from_path(&self, path: &StorePath) -> Option<AttributePath> {
        let mut results: AttributePaths = self
            .database
            .query_map(
                r#"
                SELECT attribute_path
                    FROM evaluation_attributes
                    WHERE derivation_path = (:derivation_path)
                ;
            "#,
                db::named_params! {
                    ":derivation_path": &path,
                },
                |row| {
                    let json: String = row.get(0).unwrap();
                    Ok(serde_json::from_str(&json).unwrap())
                },
            )
            .expect("Unexpectedly failed while getting attribute paths");

        if results.is_empty() {
            return None;
        }

        // Apply heuristics...
        results.sort_by(likeliest_attribute_path);

        // Then return the first element.
        Some(results[0].clone())
    }

    /// Returns mapping of store paths to attribute paths.
    pub fn find_from_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a StorePath>,
    ) -> BTreeMap<StorePath, Option<AttributePath>> {
        paths
            .into_iter()
            .map(|path| {
                let attribute_path = self.find_from_path(path);
                if attribute_path.is_some() {
                    (path.clone(), attribute_path)
                } else {
                    (path.clone(), None)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute_matchers;
    #[test]
    fn likeliest_attribute_path() {
        let mut attribute_paths = [
            vec!["c_1_2".to_string()],
            // Should be least likely, so last.
            vec!["a".to_string(), "b".to_string()],
            // Should not match, even though lexicographically first
            vec!["bar".to_string()],
            // Should be likeliest (as it's shortest), so first
            vec!["c".to_string()],
            vec!["car".to_string()],
            vec!["dar".to_string()],
        ];
        attribute_paths.sort_by(attribute_matchers::likeliest_attribute_path);

        assert_eq!(attribute_paths[0], vec!["c".to_string()]);
        assert_eq!(attribute_paths[5], vec!["a".to_string(), "b".to_string()]);
    }
}
