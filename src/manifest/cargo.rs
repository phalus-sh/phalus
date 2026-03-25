use super::ManifestError;
use crate::{Ecosystem, PackageRef, ParsedManifest};
use std::path::Path;

pub struct CargoParser;

impl CargoParser {
    pub fn detect(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "Cargo.toml")
            .unwrap_or(false)
    }

    pub fn parse(content: &str) -> Result<ParsedManifest, ManifestError> {
        let value: toml::Value = content
            .parse()
            .map_err(|e: toml::de::Error| ManifestError::Parse(e.to_string()))?;

        let mut packages = Vec::new();

        if let Some(deps) = value.get("dependencies").and_then(|d| d.as_table()) {
            for (name, spec) in deps {
                let version_constraint = match spec {
                    // Simple string form: serde = "1.0"
                    toml::Value::String(v) => v.clone(),
                    // Inline table form: tokio = { version = "1", ... }
                    toml::Value::Table(t) => t
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                        .to_string(),
                    _ => "*".to_string(),
                };

                packages.push(PackageRef {
                    name: name.clone(),
                    version_constraint,
                    ecosystem: Ecosystem::Crates,
                });
            }
        }

        Ok(ParsedManifest {
            manifest_type: "Cargo.toml".to_string(),
            packages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_toml() {
        let input = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
"#;
        let manifest = CargoParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert_eq!(manifest.packages[0].ecosystem, Ecosystem::Crates);
    }

    #[test]
    fn test_detect() {
        assert!(CargoParser::detect(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_no_dependencies() {
        let input = r#"
[package]
name = "empty"
version = "0.1.0"
"#;
        let manifest = CargoParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 0);
    }
}
