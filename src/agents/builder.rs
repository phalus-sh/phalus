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

    // Try delimiter-based format first (===FILE: path===...===END_FILE===)
    let files = parse_delimiter_format(trimmed);
    if !files.is_empty() {
        return Ok(Implementation {
            package_name: package_name.into(),
            files,
            target_language: target_language.into(),
        });
    }

    // Fall back to JSON extraction
    let obj = super::analyzer::extract_json_object(trimmed).ok_or_else(|| {
        BuilderError::ParseError(
            "could not find valid JSON object or ===FILE=== delimiters in response".into(),
        )
    })?;

    let parsed: HashMap<String, String> = obj
        .into_iter()
        .map(|(k, v)| {
            let val = match v {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string_pretty(&other).unwrap_or_default(),
            };
            (k, val)
        })
        .collect();
    Ok(Implementation {
        package_name: package_name.into(),
        files: parsed,
        target_language: target_language.into(),
    })
}

/// Parse ===FILE: path===...===END_FILE=== delimiter format.
fn parse_delimiter_format(text: &str) -> HashMap<String, String> {
    let mut files = HashMap::new();
    let mut current_path: Option<String> = None;
    let mut current_content = String::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("===FILE:") {
            // Save previous file if any
            if let Some(path) = current_path.take() {
                files.insert(path, current_content.trim_end().to_string());
                current_content.clear();
            }
            // Extract path: "===FILE: src/index.js===" -> "src/index.js"
            let path = rest.trim_end_matches('=').trim().to_string();
            if !path.is_empty() {
                current_path = Some(path);
            }
        } else if line.trim() == "===END_FILE===" {
            if let Some(path) = current_path.take() {
                files.insert(path, current_content.trim_end().to_string());
                current_content.clear();
            }
        } else if current_path.is_some() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Handle unclosed last file
    if let Some(path) = current_path {
        files.insert(path, current_content.trim_end().to_string());
    }

    files
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
    fn test_parse_implementation_response_json() {
        let response = r#"{"src/index.js": "module.exports = {}", "package.json": "{\"name\": \"test\"}", "LICENSE": "MIT"}"#;
        let imp = parse_implementation_response(response, "test-pkg", "javascript").unwrap();
        assert_eq!(imp.files.len(), 3);
        assert_eq!(imp.package_name, "test-pkg");
    }

    #[test]
    fn test_parse_implementation_response_delimiter() {
        let response = r#"Here is the implementation:

===FILE: src/index.js===
function add(a, b) {
    return a + b;
}
module.exports = { add };
===END_FILE===

===FILE: package.json===
{
  "name": "test",
  "version": "1.0.0"
}
===END_FILE===

===FILE: LICENSE===
MIT License
===END_FILE===
"#;
        let imp = parse_implementation_response(response, "test-pkg", "javascript").unwrap();
        assert_eq!(imp.files.len(), 3);
        assert!(imp.files["src/index.js"].contains("function add"));
        assert!(imp.files["package.json"].contains("\"name\""));
    }
}
