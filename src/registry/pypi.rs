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

        Ok(PackageMetadata {
            name: pkg.info.name,
            version: pkg.info.version,
            ecosystem: Ecosystem::PyPI,
            description: pkg.info.summary,
            license: pkg.info.license,
            repository_url,
            homepage_url: pkg.info.home_page,
            unpacked_size: None,
            registry_url: url,
        })
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
