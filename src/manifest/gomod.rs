use super::ManifestError;
use crate::{Ecosystem, PackageRef, ParsedManifest};
use std::path::Path;

pub struct GoModParser;

impl GoModParser {
    pub fn detect(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "go.mod")
            .unwrap_or(false)
    }

    pub fn parse(content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut packages = Vec::new();
        let mut in_require_block = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Start of a multi-line require block.
            if trimmed == "require (" {
                in_require_block = true;
                continue;
            }

            // End of a require block.
            if in_require_block && trimmed == ")" {
                in_require_block = false;
                continue;
            }

            // Single-line require: `require module/path v1.2.3`
            if let Some(rest) = trimmed.strip_prefix("require ") {
                if let Some((name, version)) = parse_module_line(rest.trim()) {
                    packages.push(PackageRef {
                        name,
                        version_constraint: version,
                        ecosystem: Ecosystem::Go,
                    });
                }
                continue;
            }

            // Inside a multi-line require block.
            if in_require_block && !trimmed.is_empty() && !trimmed.starts_with("//") {
                if let Some((name, version)) = parse_module_line(trimmed) {
                    packages.push(PackageRef {
                        name,
                        version_constraint: version,
                        ecosystem: Ecosystem::Go,
                    });
                }
            }
        }

        Ok(ParsedManifest {
            manifest_type: "go.mod".to_string(),
            packages,
        })
    }
}

/// Parse a single `module/path v1.2.3` entry, ignoring trailing `// indirect` comments.
fn parse_module_line(line: &str) -> Option<(String, String)> {
    // Strip inline comments.
    let without_comment = line.split("//").next().unwrap_or(line).trim();
    let mut parts = without_comment.split_whitespace();
    let name = parts.next()?.to_string();
    let version = parts.next()?.to_string();
    Some((name, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_mod() {
        let input = r#"
module example.com/myapp

go 1.21

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/lib/pq v1.10.9
)
"#;
        let manifest = GoModParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert_eq!(manifest.packages[0].name, "github.com/gin-gonic/gin");
        assert_eq!(manifest.packages[0].version_constraint, "v1.9.1");
        assert_eq!(manifest.packages[0].ecosystem, Ecosystem::Go);
    }

    #[test]
    fn test_detect() {
        assert!(GoModParser::detect(Path::new("go.mod")));
    }

    #[test]
    fn test_single_require() {
        let input = "module foo\n\ngo 1.21\n\nrequire github.com/bar/baz v1.0.0\n";
        let manifest = GoModParser::parse(input).unwrap();
        assert_eq!(manifest.packages.len(), 1);
    }
}
