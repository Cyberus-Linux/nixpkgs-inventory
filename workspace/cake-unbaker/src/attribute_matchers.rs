use cu_ingest_to_db::database as db;
use cu_lib_nix::nix;
use nix::AttributePath;
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
            if let Ok(value) = value {
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

            // NOTE: Some entries don't have these.
            pub purl: Option<String>,
            pub cpe: Option<String>,
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
    // Same depth?
    if a.len() == b.len() {
        // Compare name length
        let a = a.join(".");
        let b = b.join(".");

        // Same name length?
        if a.len() == b.len() {
            // Lexicographically compare
            a.cmp(&b)
        } else {
            // Prefer shortest name length
            a.len().cmp(&b.len())
        }
    } else {
        // Prefer shallowest depth
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
    ///
    /// The store path can either be a derivation or an output path of a derivation.
    pub fn find_from_path(&self, path: &StorePath) -> Option<AttributePath> {
        let mut results: Vec<(AttributePath, Option<String>)> = self
            .database
            .query_map(
                r#"
                SELECT attribute_path, pname
                    FROM evaluation_attributes
                    WHERE derivation_path = (:path)
                       OR derivation_path IN (
                           SELECT derivation_path
                               FROM derivation_output_paths
                               WHERE store_path = (:path)
                       )
                ;
            "#,
                db::named_params! {
                    ":path": &path,
                },
                |row| {
                    let json: String = row.get(0)?;
                    Ok((serde_json::from_str(&json).unwrap(), row.get(1)?))
                },
            )
            .expect("Unexpectedly failed while getting attribute paths");

        if results.is_empty() {
            return None;
        }

        // Apply heuristics...
        results.sort_by(|(a, a_pname), (b, b_pname)| {
            let a_parent_path = a.clone()[0..a.len() - 1].join(".");
            let b_parent_path = b.clone()[0..b.len() - 1].join(".");

            // When at the same depth, and the parent path is equal in length.
            if a.len() == b.len() && a_parent_path.len() == b_parent_path.len() {
                // Same pname on either?
                let same_pname = a_pname.is_some() && b_pname.is_some() && a_pname == b_pname;
                // They both match the attribute name?
                let both_match = a_pname.as_ref() == a.last() && b_pname.as_ref() == b.last();

                if !(same_pname && both_match) {
                    // Prefer `a` if its pname matches the attribute name.
                    if a_pname.is_some() && a_pname.as_ref() == a.last() {
                        return Ordering::Less;
                    }
                    // Prefer `b` if its pname matches the attribute name.
                    if b_pname.is_some() && b_pname.as_ref() == b.last() {
                        return Ordering::Greater;
                    }
                }
                // Fall through if they both match the attribute name.
                // (e.g. `xyz.abc` and `qwe.abc` for `pname == "abc"`)
            }

            likeliest_attribute_path(a, b)
        });
        let (attribute_path, _pname) = results[0].clone();

        // Then return the first element.
        Some(attribute_path)
    }

    /// Returns mapping of store paths to attribute paths.
    pub fn find_from_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a StorePath>,
    ) -> BTreeMap<StorePath, Option<AttributePath>> {
        paths
            .into_iter()
            .map(|path| (path.clone(), self.find_from_path(path)))
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
