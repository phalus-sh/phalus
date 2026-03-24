use sha2::{Digest, Sha256};

use crate::docs::source_guard;
use crate::DocEntry;

/// Filter a list of tarball entries to only those that are `.d.ts` files and
/// pass the source guard (i.e. not in test/spec directories).
pub fn filter_type_definitions(entries: &[(String, String)]) -> Vec<(String, String)> {
    entries
        .iter()
        .filter(|(name, _)| name.ends_with(".d.ts") && !source_guard::is_source_code(name))
        .cloned()
        .collect()
}

/// Convert filtered `.d.ts` entries into `DocEntry` values, computing SHA-256
/// hashes and attaching the provided `source_url`.
pub fn type_defs_to_doc_entries(
    filtered: &[(String, String)],
    source_url: &str,
) -> Vec<DocEntry> {
    filtered
        .iter()
        .map(|(name, content)| {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let content_hash = format!("{:x}", hasher.finalize());
            DocEntry {
                name: name.clone(),
                content: content.clone(),
                source_url: Some(source_url.to_string()),
                content_hash,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_dts_from_tarball_entries() {
        let entries = vec![
            ("package/index.js".into(), "source code".into()),
            ("package/index.d.ts".into(), "declare function foo(): void;".into()),
            ("package/lib/types.d.ts".into(), "interface Bar {}".into()),
            ("package/test/helper.js".into(), "test code".into()),
        ];
        let filtered = filter_type_definitions(&entries);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|(name, _)| name.contains("index.d.ts")));
        assert!(filtered.iter().any(|(name, _)| name.contains("types.d.ts")));
    }

    #[test]
    fn test_empty_entries() {
        let entries: Vec<(String, String)> = vec![];
        let filtered = filter_type_definitions(&entries);
        assert!(filtered.is_empty());
    }
}
