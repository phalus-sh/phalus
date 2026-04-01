use reqwest::Client;
use serde::Deserialize;

use super::RegistryError;
use crate::{Ecosystem, PackageMetadata};

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PypiResponse {
    info: PypiInfo,
}

#[derive(Debug, Deserialize)]
struct PypiInfo {
    name: String,
    version: String,
    summary: Option<String>,
    license: Option<String>,
    home_page: Option<String>,
    #[serde(default)]
    classifiers: Vec<String>,
}

// ---------------------------------------------------------------------------
// PypiResolver
// ---------------------------------------------------------------------------

pub struct PypiResolver {
    base_url: String,
    client: Client,
}

impl PypiResolver {
    /// Create a resolver pointing at a custom base URL (useful in tests).
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Create a resolver pointing at the public PyPI registry.
    pub fn default_registry() -> Self {
        Self::new("https://pypi.org".to_string())
    }

    /// Resolve a package version (exact or constraint like ==2.31.0) and return metadata.
    pub async fn resolve(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PackageMetadata, RegistryError> {
        // Strip version constraint operators
        let clean_version = version
            .trim_start_matches("===")
            .trim_start_matches("~=")
            .trim_start_matches("!=")
            .trim_start_matches("==")
            .trim_start_matches(">=")
            .trim_start_matches("<=")
            .trim_start_matches('>')
            .trim_start_matches('<')
            .trim();

        // If version is "*" or empty, fetch latest
        let url = if clean_version.is_empty() || clean_version == "*" {
            format!("{}/pypi/{}/json", self.base_url, name)
        } else {
            format!("{}/pypi/{}/{}/json", self.base_url, name, clean_version)
        };

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
                version: version.to_string(),
            });
        }

        if !response.status().is_success() {
            let err = response.error_for_status().unwrap_err();
            return Err(RegistryError::Http(err));
        }

        // Get raw JSON for project_urls extraction
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        let pkg: PypiResponse = serde_json::from_value(body.clone())
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        let repository_url = body
            .get("info")
            .and_then(|i| i.get("project_urls"))
            .and_then(|urls| urls.as_object())
            .and_then(|urls| {
                urls.get("Source")
                    .or(urls.get("Repository"))
                    .or(urls.get("GitHub"))
            })
            .and_then(|v| v.as_str())
            .map(String::from);

        // Resolve license: prefer clean SPDX-like values from the `license` field,
        // fall back to classifiers if it's empty or contains full license text.
        let license = resolve_pypi_license(&pkg.info);

        Ok(PackageMetadata {
            name: pkg.info.name,
            version: pkg.info.version,
            ecosystem: Ecosystem::PyPI,
            description: pkg.info.summary,
            license,
            repository_url,
            homepage_url: pkg.info.home_page,
            unpacked_size: None,
            registry_url: url,
        })
    }
}

/// Determine the best license string from PyPI info.
///
/// The PyPI `license` field can be:
/// - A clean SPDX identifier ("MIT", "Apache-2.0")
/// - The full license text (numpy)
/// - An informal string ("Apache-2.0 license")
/// - Empty (mdurl)
///
/// When the field is empty or looks like full text (>50 chars or contains
/// "copyright"/"redistribution"), extract from `classifiers` instead.
fn resolve_pypi_license(info: &PypiInfo) -> Option<String> {
    let raw = info.license.as_deref().unwrap_or("").trim();

    // If license field looks like a clean identifier, use it
    if !raw.is_empty() && raw.len() < 50 && !is_license_text(raw) {
        // Strip trailing noise like " license"
        let cleaned = raw
            .trim_end_matches(" license")
            .trim_end_matches(" License")
            .trim();
        return Some(cleaned.to_string());
    }

    // Fall back to classifiers
    extract_license_from_classifiers(&info.classifiers)
}

fn is_license_text(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("copyright")
        || lower.contains("redistribution")
        || lower.contains("permission is hereby granted")
        || lower.contains("the software is provided")
        || s.contains('\n')
}

/// Extract SPDX-like license from PyPI trove classifiers.
/// e.g. "License :: OSI Approved :: MIT License" → "MIT"
fn extract_license_from_classifiers(classifiers: &[String]) -> Option<String> {
    for classifier in classifiers {
        if !classifier.starts_with("License :: ") {
            continue;
        }
        // "License :: OSI Approved :: MIT License" → "MIT License"
        let parts: Vec<&str> = classifier.split(" :: ").collect();
        if let Some(last) = parts.last() {
            let mapped = map_classifier_to_spdx(last.trim());
            if !mapped.is_empty() {
                return Some(mapped.to_string());
            }
        }
    }
    None
}

fn map_classifier_to_spdx(classifier: &str) -> &str {
    match classifier {
        "MIT License" => "MIT",
        "BSD License" => "BSD-3-Clause",
        "Apache Software License" => "Apache-2.0",
        "GNU General Public License v2 (GPLv2)" => "GPL-2.0-only",
        "GNU General Public License v2 or later (GPLv2+)" => "GPL-2.0-or-later",
        "GNU General Public License v3 (GPLv3)" => "GPL-3.0-only",
        "GNU General Public License v3 or later (GPLv3+)" => "GPL-3.0-or-later",
        "GNU Lesser General Public License v2 (LGPLv2)" => "LGPL-2.0-only",
        "GNU Lesser General Public License v2 or later (LGPLv2+)" => "LGPL-2.0-or-later",
        "GNU Lesser General Public License v3 (LGPLv3)" => "LGPL-3.0-only",
        "GNU Lesser General Public License v3 or later (LGPLv3+)" => "LGPL-3.0-or-later",
        "ISC License (ISCL)" => "ISC",
        "Mozilla Public License 2.0 (MPL 2.0)" => "MPL-2.0",
        "European Union Public Licence 1.2 (EUPL 1.2)" => "EUPL-1.2",
        "The Unlicense (Unlicense)" => "Unlicense",
        "CC0 1.0 Universal (CC0 1.0) Public Domain Dedication" => "CC0-1.0",
        "Public Domain" => "Unlicense",
        _ => {
            // Try stripping " License" suffix
            classifier.strip_suffix(" License").unwrap_or(classifier)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_resolve_pypi_package() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pypi/requests/2.31.0/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "info": {
                    "name": "requests",
                    "version": "2.31.0",
                    "summary": "HTTP for Humans",
                    "license": "Apache-2.0",
                    "project_urls": { "Source": "https://github.com/psf/requests" },
                    "home_page": "https://requests.readthedocs.io"
                }
            })))
            .mount(&mock_server)
            .await;

        let resolver = PypiResolver::new(mock_server.uri());
        let meta = resolver.resolve("requests", "2.31.0").await.unwrap();
        assert_eq!(meta.name, "requests");
        assert_eq!(meta.ecosystem, Ecosystem::PyPI);
    }
}
