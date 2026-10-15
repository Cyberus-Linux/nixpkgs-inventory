use cu_lib_nix::nix;
use nix::AttributePath;

pub mod attribute_matchers;

/// Converts a list of attribute paths into a prettier format than either `to_string` and
/// `to_string_pretty`.
///
/// This formats the list with one entry per line.
pub fn to_attribute_paths_json<'a>(results: impl IntoIterator<Item = &'a AttributePath>) -> String {
    let temp = results
        .into_iter()
        .map(|attribute_path| format!("\t{}", serde_json::to_string(attribute_path).unwrap()))
        .collect::<Vec<String>>();
    // NOTE: it is safe to join the serialized arrays with `,\n`
    // We know that wth 0 entries, nothing will be present between the begin and end of the array.
    // We know that with 1 entry one entry will be shown...
    // And that with any non-zero amount, the last entry will not receive a trailing `,\n`.
    ["[".to_string(), temp.join(",\n"), "]".to_string()].join("\n")
}
