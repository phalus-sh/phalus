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
pub fn type_defs_to_doc_entries(filtered: &[(String, String)], source_url: &str) -> Vec<DocEntry> {
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

/// Fetch type definitions from DefinitelyTyped when the npm package doesn't
/// include its own `.d.ts` files.
pub async fn fetch_definitely_typed(package_name: &str) -> Option<Vec<DocEntry>> {
    let client = reqwest::Client::new();
    // DefinitelyTyped types are published as @types/package-name
    let types_name = format!("@types/{}", package_name);
    let url = format!(
        "https://registry.npmjs.org/{}/latest",
        types_name.replace('/', "%2F")
    );

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    let tarball_url = body.get("dist")?.get("tarball")?.as_str()?;

    // Fetch tarball and extract .d.ts files
    let tarball_resp = client.get(tarball_url).send().await.ok()?;
    let bytes = tarball_resp.bytes().await.ok()?;

    extract_dts_from_tarball(&bytes, package_name)
}

fn extract_dts_from_tarball(bytes: &[u8], package_name: &str) -> Option<Vec<DocEntry>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let decoder = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let mut entries = Vec::new();

    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?.to_string_lossy().to_string();
        if path.ends_with(".d.ts") {
            let mut content = String::new();
            entry.read_to_string(&mut content).ok()?;
            let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
            entries.push(DocEntry {
                name: path.rsplit('/').next().unwrap_or(&path).to_string(),
                content,
                source_url: Some(format!("@types/{}", package_name)),
                content_hash,
            });
        }
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_dts_from_tarball_entries() {
        let entries = vec![
            ("package/index.js".into(), "source code".into()),
            (
                "package/index.d.ts".into(),
                "declare function foo(): void;".into(),
            ),
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
