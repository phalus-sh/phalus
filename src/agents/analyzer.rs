use crate::{CspDocument, CspSpec, Documentation};
use chrono::Utc;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyzerError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("failed to parse CSP response: {0}")]
    ParseError(String),
}

const SYSTEM_PROMPT: &str = include_str!("prompts/analyzer_system.txt");

const CSP_KEYS: &[&str] = &[
    "01-overview",
    "02-api-surface",
    "03-behavior-spec",
    "04-edge-cases",
    "05-configuration",
    "06-type-definitions",
    "07-error-catalog",
    "08-compatibility-notes",
    "09-test-scenarios",
    "10-metadata",
];

pub fn build_analyzer_prompt(docs: &Documentation) -> String {
    let description = docs.package.description.as_deref().unwrap_or("");
    let mut prompt = format!(
        "Analyze the following documentation for package: {} v{}\nEcosystem: {}\nDescription: {}\n\n",
        docs.package.name,
        docs.package.version,
        docs.package.ecosystem,
        description,
    );
    for doc in &docs.documents {
        prompt.push_str(&format!("--- {} ---\n{}\n\n", doc.name, doc.content));
    }
    prompt
}

pub fn parse_csp_response(
    response: &str,
    package_name: &str,
    package_version: &str,
) -> Result<CspSpec, AnalyzerError> {
    let trimmed = response.trim();

    // Try to extract a valid JSON object from the response.
    // The LLM may wrap it in markdown fences or prose text.
    let obj = extract_json_object(trimmed)
        .ok_or_else(|| AnalyzerError::ParseError("could not find valid JSON object in response".into()))?;

    let mut documents = Vec::new();
    for key in CSP_KEYS {
        // Handle both string values and nested objects/arrays
        let content = match obj.get(*key) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
            None => String::new(),
        };
        let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let filename = if *key == "02-api-surface" || *key == "10-metadata" {
            format!("{}.json", key)
        } else if *key == "06-type-definitions" {
            format!("{}.d.ts", key)
        } else {
            format!("{}.md", key)
        };
        documents.push(CspDocument {
            filename,
            content,
            content_hash,
        });
    }

    Ok(CspSpec {
        package_name: package_name.into(),
        package_version: package_version.into(),
        documents,
        generated_at: Utc::now(),
    })
}

/// Try to find and parse a valid JSON object from a string that may contain
/// surrounding text, markdown fences, etc. Tries parsing from each `{` position.
pub fn extract_json_object(text: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut start = 0;
    while let Some(pos) = text[start..].find('{') {
        let abs_pos = start + pos;
        // Try parsing the largest possible substring from this `{`
        // by scanning backwards from the end for `}`
        let remainder = &text[abs_pos..];
        if let Some(end) = remainder.rfind('}') {
            let candidate = &remainder[..=end];
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(candidate) {
                return Some(map);
            }
        }
        start = abs_pos + 1;
    }
    None
}

pub fn system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocEntry, Documentation, Ecosystem, PackageMetadata};

    #[test]
    fn test_build_analyzer_prompt() {
        let docs = Documentation {
            package: PackageMetadata {
                name: "test-pkg".into(),
                version: "1.0.0".into(),
                ecosystem: Ecosystem::Npm,
                description: Some("A test package".into()),
                license: Some("MIT".into()),
                repository_url: None,
                homepage_url: None,
                unpacked_size: None,
                registry_url: "https://registry.npmjs.org/test-pkg/1.0.0".into(),
            },
            documents: vec![DocEntry {
                name: "README.md".into(),
                content: "# Test\nA test library".into(),
                source_url: Some("https://example.com".into()),
                content_hash: "abc".into(),
            }],
            content_hash: "def".into(),
        };
        let prompt = build_analyzer_prompt(&docs);
        assert!(prompt.contains("test-pkg"));
        assert!(prompt.contains("A test library"));
        assert!(prompt.contains("1.0.0"));
    }

    #[test]
    fn test_parse_csp_response() {
        let response = r#"{"01-overview": "A utility lib", "02-api-surface": "{}", "03-behavior-spec": "spec", "04-edge-cases": "none", "05-configuration": "none", "06-type-definitions": "declare module", "07-error-catalog": "none", "08-compatibility-notes": "node 14+", "09-test-scenarios": "test1", "10-metadata": "{}"}"#;
        let csp = parse_csp_response(response, "test-pkg", "1.0.0").unwrap();
        assert_eq!(csp.documents.len(), 10);
        assert_eq!(csp.package_name, "test-pkg");
    }

    #[test]
    fn test_parse_csp_response_invalid() {
        assert!(parse_csp_response("not json", "pkg", "1.0").is_err());
    }
}
