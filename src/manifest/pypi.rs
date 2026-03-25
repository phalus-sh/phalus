use crate::{Ecosystem, PackageRef, ParsedManifest};
use super::ManifestError;
use std::path::Path;

pub struct PypiParser;

impl PypiParser {
    pub fn detect(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "requirements.txt")
            .unwrap_or(false)
    }

    pub fn parse(content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut packages = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Strip inline comments.
            let dep = trimmed
                .split('#')
                .next()
                .unwrap_or(trimmed)
                .trim();

            if dep.is_empty() {
                continue;
            }

            // Split on the first version operator found.
            let operators = ["===", "~=", "!=", "==", ">=", "<=", ">", "<"];
            let mut found = None;
            for op in &operators {
                if let Some(pos) = dep.find(op) {
                    found = Some((pos, *op));
                    break;
                }
            }

            let (name, version_constraint) = if let Some((pos, op)) = found {
                let pkg_name = dep[..pos].trim().to_string();
                let constraint = format!("{}{}", op, dep[pos + op.len()..].trim());
                (pkg_name, constraint)
            } else {
                (dep.to_string(), "*".to_string())
            };

            packages.push(PackageRef {
                name,
                version_constraint,
                ecosystem: Ecosystem::PyPI,
            });
        }

        Ok(ParsedManifest {
            manifest_type: "requirements.txt".to_string(),
            packages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_requirements_txt() {
        let input = "requests==2.31.0\nflask>=2.0.0\nnumpy";
        let manifest = PypiParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 3);
        assert_eq!(manifest.packages[0].name, "requests");
        assert_eq!(manifest.packages[0].version_constraint, "==2.31.0");
        assert_eq!(manifest.packages[0].ecosystem, Ecosystem::PyPI);
        assert_eq!(manifest.packages[2].version_constraint, "*");
    }

    #[test]
    fn test_skip_comments_and_empty_lines() {
        let input = "# comment\nrequests==2.31.0\n\n  # another\nflask>=2.0";
        let manifest = PypiParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 2);
    }

    #[test]
    fn test_detect() {
        assert!(PypiParser::detect(Path::new("requirements.txt")));
        assert!(!PypiParser::detect(Path::new("package.json")));
    }
}
