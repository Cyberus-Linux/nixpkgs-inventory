use cu_ingest_to_db::database as db;
use cu_lib_nix::nix;
use nix::AttributePath;
use nix::AttributePaths;
use nix::StorePath;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::BufRead;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InputFormats {
    /// The output of `nix-store --requisites`.
    Requisites,
}

pub struct Matcher {
    database: db::AttributesDatabase,
}

pub fn extract_derivations(
    input_format: &InputFormats,
    input_data: impl BufRead,
) -> BTreeSet<StorePath> {
    match input_format {
        InputFormats::Requisites => input_data.lines().map(|l| l.unwrap()).collect(),
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
