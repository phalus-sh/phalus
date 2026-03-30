use crate::license;
/// SBOM ingestion: parse CycloneDX BOM JSON and SPDX JSON into `ScannedPackage` lists.
use crate::{Ecosystem, ScannedPackage};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SbomError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported or unrecognized SBOM format")]
    Unrecognized,
}

// ---------------------------------------------------------------------------
// CycloneDX JSON (spec 1.4+)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CycloneDxBom {
    components: Option<Vec<CycloneDxComponent>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CycloneDxComponent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    component_type: Option<String>,
    name: String,
    version: Option<String>,
    purl: Option<String>,
    licenses: Option<Vec<CycloneDxLicenseEntry>>,
}

#[derive(Debug, Deserialize)]
struct CycloneDxLicenseEntry {
    license: Option<CycloneDxLicense>,
    expression: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CycloneDxLicense {
    id: Option<String>,
    name: Option<String>,
}

/// Parse a CycloneDX BOM JSON string into a list of `ScannedPackage`.
pub fn parse_cyclonedx(content: &str) -> Result<Vec<ScannedPackage>, SbomError> {
    let bom: CycloneDxBom = serde_json::from_str(content)?;
    let components = bom.components.unwrap_or_default();

    let packages = components
        .into_iter()
        .map(|c| {
            let raw_license = extract_cyclonedx_license(&c);
            let (spdx_license, classification) = match &raw_license {
                Some(raw) => {
                    let (s, cl) = license::normalize_and_classify(raw);
                    (Some(s), cl)
                }
                None => (None, crate::LicenseClass::Unknown),
            };
            let ecosystem = purl_to_ecosystem(c.purl.as_deref());
            ScannedPackage {
                name: c.name,
                version: c.version.unwrap_or_else(|| "unknown".to_string()),
                ecosystem,
                raw_license,
                spdx_license,
                classification,
                source: "sbom:cyclonedx".to_string(),
            }
        })
        .collect();

    Ok(packages)
}

fn extract_cyclonedx_license(c: &CycloneDxComponent) -> Option<String> {
    let entries = c.licenses.as_ref()?;
    // Prefer an expression at the entry level, then license.id, then license.name
    for entry in entries {
        if let Some(expr) = &entry.expression {
            return Some(expr.clone());
        }
        if let Some(lic) = &entry.license {
            if let Some(id) = &lic.id {
                return Some(id.clone());
            }
            if let Some(name) = &lic.name {
                return Some(name.clone());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// SPDX JSON (2.3+)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpdxDocument {
    packages: Option<Vec<SpdxPackage>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpdxPackage {
    name: String,
    version_info: Option<String>,
    license_concluded: Option<String>,
    license_declared: Option<String>,
    external_refs: Option<Vec<SpdxExternalRef>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpdxExternalRef {
    #[allow(dead_code)]
    reference_category: String,
    #[allow(dead_code)]
    reference_type: String,
    reference_locator: String,
}

/// Parse an SPDX JSON document into a list of `ScannedPackage`.
pub fn parse_spdx_json(content: &str) -> Result<Vec<ScannedPackage>, SbomError> {
    let doc: SpdxDocument = serde_json::from_str(content)?;
    let pkgs = doc.packages.unwrap_or_default();

    let packages = pkgs
        .into_iter()
        .map(|p| {
            // Prefer licenseConcluded, fall back to licenseDeclared.
            // "NOASSERTION" and "NONE" are SPDX sentinels we treat as absent.
            let raw_license = p
                .license_concluded
                .filter(|s| s != "NOASSERTION" && s != "NONE" && !s.is_empty())
                .or_else(|| {
                    p.license_declared
                        .filter(|s| s != "NOASSERTION" && s != "NONE" && !s.is_empty())
                });

            let (spdx_license, classification) = match &raw_license {
                Some(raw) => {
                    let (s, cl) = license::normalize_and_classify(raw);
                    (Some(s), cl)
                }
                None => (None, crate::LicenseClass::Unknown),
            };

            let ecosystem = spdx_external_refs_to_ecosystem(p.external_refs.as_deref());
            ScannedPackage {
                name: p.name,
                version: p.version_info.unwrap_or_else(|| "unknown".to_string()),
                ecosystem,
                raw_license,
                spdx_license,
                classification,
                source: "sbom:spdx".to_string(),
            }
        })
        .collect();

    Ok(packages)
}

// ---------------------------------------------------------------------------
// Auto-detect SBOM format
// ---------------------------------------------------------------------------

/// Detect whether JSON content is CycloneDX (has `bomFormat`) or SPDX (has `spdxVersion`),
/// then parse and return the package list.
pub fn parse_sbom(content: &str) -> Result<Vec<ScannedPackage>, SbomError> {
    // Quick heuristic: look for top-level keys without full parse
    let value: serde_json::Value = serde_json::from_str(content)?;
    if value.get("bomFormat").is_some() || value.get("components").is_some() {
        return parse_cyclonedx(content);
    }
    if value.get("spdxVersion").is_some() || value.get("SPDXID").is_some() {
        return parse_spdx_json(content);
    }
    Err(SbomError::Unrecognized)
}

// ---------------------------------------------------------------------------
// Helpers: PURL / external-ref → Ecosystem
// ---------------------------------------------------------------------------

fn purl_to_ecosystem(purl: Option<&str>) -> Ecosystem {
    let Some(p) = purl else {
        return Ecosystem::Npm; // default fallback
    };
    if p.starts_with("pkg:npm") {
        Ecosystem::Npm
    } else if p.starts_with("pkg:pypi") {
        Ecosystem::PyPI
    } else if p.starts_with("pkg:cargo") {
        Ecosystem::Crates
    } else if p.starts_with("pkg:golang") || p.starts_with("pkg:go") {
        Ecosystem::Go
    } else {
        Ecosystem::Npm // default for unknown package types
    }
}

fn spdx_external_refs_to_ecosystem(refs: Option<&[SpdxExternalRef]>) -> Ecosystem {
    let Some(refs) = refs else {
        return Ecosystem::Npm;
    };
    for r in refs {
        if r.reference_category == "PACKAGE-MANAGER" {
            return purl_to_ecosystem(Some(&r.reference_locator));
        }
    }
    Ecosystem::Npm
}

// ---------------------------------------------------------------------------
// Filename heuristics
// ---------------------------------------------------------------------------

/// Return true if a filename looks like a SBOM file we can parse.
pub fn is_sbom_filename(name: &str) -> bool {
    let lower = name.to_lowercase();
    // CycloneDX: bom.json, *-bom.json, cyclonedx*.json
    // SPDX: *.spdx.json, sbom.json, *-sbom.json
    lower == "bom.json"
        || lower.ends_with("-bom.json")
        || lower.ends_with(".bom.json")
        || lower.starts_with("cyclonedx")
        || lower.ends_with(".spdx.json")
        || lower == "sbom.json"
        || lower.ends_with("-sbom.json")
        || lower.ends_with(".sbom.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LicenseClass;

    #[test]
    fn parse_cyclonedx_basic() {
        let bom = r#"{
            "bomFormat": "CycloneDX",
            "specVersion": "1.4",
            "components": [
                {
                    "type": "library",
                    "name": "lodash",
                    "version": "4.17.21",
                    "purl": "pkg:npm/lodash@4.17.21",
                    "licenses": [{"license": {"id": "MIT"}}]
                },
                {
                    "type": "library",
                    "name": "left-pad",
                    "version": "1.3.0",
                    "purl": "pkg:npm/left-pad@1.3.0",
                    "licenses": [{"expression": "MIT"}]
                }
            ]
        }"#;
        let pkgs = parse_cyclonedx(bom).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "lodash");
        assert_eq!(pkgs[0].spdx_license.as_deref(), Some("MIT"));
        assert_eq!(pkgs[0].classification, LicenseClass::Permissive);
        assert_eq!(pkgs[1].raw_license.as_deref(), Some("MIT"));
    }

    #[test]
    fn parse_cyclonedx_no_license() {
        let bom = r#"{"bomFormat":"CycloneDX","components":[{"name":"foo","version":"1.0"}]}"#;
        let pkgs = parse_cyclonedx(bom).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].spdx_license, None);
        assert_eq!(pkgs[0].classification, LicenseClass::Unknown);
    }

    #[test]
    fn parse_spdx_json_basic() {
        let spdx = r#"{
            "spdxVersion": "SPDX-2.3",
            "SPDXID": "SPDXRef-DOCUMENT",
            "packages": [
                {
                    "name": "requests",
                    "versionInfo": "2.28.0",
                    "licenseConcluded": "Apache-2.0",
                    "licenseDeclared": "Apache-2.0",
                    "externalRefs": [{"referenceCategory": "PACKAGE-MANAGER","referenceType":"purl","referenceLocator":"pkg:pypi/requests@2.28.0"}]
                }
            ]
        }"#;
        let pkgs = parse_spdx_json(spdx).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "requests");
        assert_eq!(pkgs[0].spdx_license.as_deref(), Some("Apache-2.0"));
        assert_eq!(pkgs[0].classification, LicenseClass::Permissive);
        assert_eq!(pkgs[0].ecosystem, Ecosystem::PyPI);
    }

    #[test]
    fn parse_spdx_noassertion_becomes_unknown() {
        let spdx = r#"{
            "spdxVersion": "SPDX-2.3",
            "packages": [
                {
                    "name": "mystery",
                    "versionInfo": "0.1.0",
                    "licenseConcluded": "NOASSERTION",
                    "licenseDeclared": "NOASSERTION"
                }
            ]
        }"#;
        let pkgs = parse_spdx_json(spdx).unwrap();
        assert_eq!(pkgs[0].spdx_license, None);
        assert_eq!(pkgs[0].classification, LicenseClass::Unknown);
    }

    #[test]
    fn autodetect_cyclonedx() {
        let bom = r#"{"bomFormat":"CycloneDX","components":[]}"#;
        assert!(parse_sbom(bom).is_ok());
    }

    #[test]
    fn autodetect_spdx() {
        let spdx = r#"{"spdxVersion":"SPDX-2.3","packages":[]}"#;
        assert!(parse_sbom(spdx).is_ok());
    }

    #[test]
    fn autodetect_unknown_fails() {
        let unknown = r#"{"foo":"bar"}"#;
        assert!(matches!(parse_sbom(unknown), Err(SbomError::Unrecognized)));
    }

    #[test]
    fn is_sbom_filename_cases() {
        assert!(is_sbom_filename("bom.json"));
        assert!(is_sbom_filename("my-app-bom.json"));
        assert!(is_sbom_filename("sbom.json"));
        assert!(is_sbom_filename("app.spdx.json"));
        assert!(!is_sbom_filename("package.json"));
        assert!(!is_sbom_filename("Cargo.toml"));
    }
}
