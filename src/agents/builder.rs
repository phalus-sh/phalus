use crate::{CspSpec, Implementation, TargetLanguage};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("failed to parse implementation response: {0}")]
    ParseError(String),
}

const SYSTEM_PROMPT: &str = include_str!("prompts/builder_system.txt");

pub fn build_builder_prompt(csp: &CspSpec, license: &str, target_lang: &TargetLanguage) -> String {
    let mut prompt = format!(
        "Implement package: {} v{}\nLicense: {}\n",
        csp.package_name, csp.package_version, license
    );
    match target_lang {
        TargetLanguage::Same => {}
        lang => prompt.push_str(&format!(
            "\nIMPORTANT: Implement in {} using idiomatic conventions and standard library.\n",
            lang
        )),
    }
    prompt.push_str("\n--- SPECIFICATION ---\n\n");
    for doc in &csp.documents {
        prompt.push_str(&format!("=== {} ===\n{}\n\n", doc.filename, doc.content));
    }
    prompt
}

pub fn parse_implementation_response(
    response: &str,
    package_name: &str,
    target_language: &str,
) -> Result<Implementation, BuilderError> {
    let trimmed = response.trim();
    let json_str = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .strip_suffix("```")
        .unwrap_or(trimmed)
        .trim();
    let parsed: HashMap<String, String> = serde_json::from_str(json_str)
        .map_err(|e| BuilderError::ParseError(e.to_string()))?;
    Ok(Implementation {
        package_name: package_name.into(),
        files: parsed,
        target_language: target_language.into(),
    })
}

pub fn system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CspDocument, CspSpec, TargetLanguage};
    use chrono::Utc;

    fn sample_csp() -> CspSpec {
        CspSpec {
            package_name: "test-pkg".into(),
            package_version: "1.0.0".into(),
            documents: vec![CspDocument {
                filename: "01-overview.md".into(),
                content: "A simple utility".into(),
                content_hash: "abc".into(),
            }],
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn test_build_builder_prompt() {
        let csp = sample_csp();
        let prompt = build_builder_prompt(&csp, "mit", &TargetLanguage::Same);
        assert!(prompt.contains("test-pkg"));
        assert!(prompt.contains("A simple utility"));
        assert!(prompt.contains("mit"));
    }

    #[test]
    fn test_build_builder_prompt_target_lang() {
        let csp = sample_csp();
        let prompt = build_builder_prompt(&csp, "apache-2.0", &TargetLanguage::Rust);
        assert!(prompt.contains("Rust") || prompt.contains("rust"));
    }

    #[test]
    fn test_parse_implementation_response() {
        let response = r#"{"src/index.js": "module.exports = {}", "package.json": "{\"name\": \"test\"}", "LICENSE": "MIT"}"#;
        let imp = parse_implementation_response(response, "test-pkg", "javascript").unwrap();
        assert_eq!(imp.files.len(), 3);
        assert_eq!(imp.package_name, "test-pkg");
    }
}
