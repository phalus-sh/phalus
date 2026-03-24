use crate::{Ecosystem, PackageRef, ParsedManifest};
use super::ManifestError;
use std::path::Path;

pub struct NpmParser;

impl NpmParser {
    pub fn detect(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "package.json")
            .unwrap_or(false)
    }

    pub fn parse(content: &str) -> Result<ParsedManifest, ManifestError> {
        let value: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| ManifestError::Parse(e.to_string()))?;

        let mut packages = Vec::new();

        if let Some(deps) = value.get("dependencies").and_then(|d| d.as_object()) {
            for (name, version) in deps {
                let version_constraint = version
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                packages.push(PackageRef {
                    name: name.clone(),
                    version_constraint,
                    ecosystem: Ecosystem::Npm,
                });
            }
        }

        Ok(ParsedManifest {
            manifest_type: "package.json".to_string(),
            packages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_package_json() {
        let input = r#"{
            "name": "my-app",
            "version": "1.0.0",
            "dependencies": {
                "lodash": "^4.17.21",
                "express": "~4.18.2"
            }
        }"#;
        let manifest = NpmParser::parse(input).unwrap();
        assert_eq!(manifest.manifest_type, "package.json");
        assert_eq!(manifest.packages.len(), 2);

        let lodash = manifest.packages.iter().find(|p| p.name == "lodash").unwrap();
        assert_eq!(lodash.version_constraint, "^4.17.21");
        assert_eq!(lodash.ecosystem, Ecosystem::Npm);
    }

    #[test]
    fn test_parse_empty_dependencies() {
        let input = r#"{ "name": "empty", "version": "1.0.0" }"#;
        let manifest = NpmParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 0);
    }

    #[test]
    fn test_detect_package_json() {
        assert!(NpmParser::detect(Path::new("package.json")));
        assert!(NpmParser::detect(Path::new("/some/path/package.json")));
        assert!(!NpmParser::detect(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_parse_invalid_json() {
        assert!(NpmParser::parse("not json").is_err());
    }
}
