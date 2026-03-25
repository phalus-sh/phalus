use reqwest::Client;
use serde::Deserialize;

use super::RegistryError;
use crate::{Ecosystem, PackageMetadata};

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CratesResponse {
    version: CratesVersion,
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(Debug, Deserialize)]
struct CratesVersion {
    num: String,
    license: Option<String>,
    crate_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
}

// ---------------------------------------------------------------------------
// CratesResolver
// ---------------------------------------------------------------------------

pub struct CratesResolver {
    base_url: String,
    client: Client,
}

impl CratesResolver {
    /// Create a resolver pointing at a custom base URL (useful in tests).
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Create a resolver pointing at the public crates.io registry.
    pub fn default_registry() -> Self {
        Self::new("https://crates.io".to_string())
    }

    /// Resolve a crate version (exact or constraint like "1.0") and return metadata.
    pub async fn resolve(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PackageMetadata, RegistryError> {
        // Strip semver range operators
        let clean_version = version
            .trim_start_matches('^')
            .trim_start_matches('~')
            .trim_start_matches(">=")
            .trim_start_matches("<=")
            .trim_start_matches('>')
            .trim_start_matches('<')
            .trim_start_matches('=')
            .trim();

        let url = format!("{}/api/v1/crates/{}/{}", self.base_url, name, clean_version);

        let response = self
            .client
            .get(&url)
            // crates.io requires a User-Agent header.
            .header("User-Agent", "phalus/0.1.0")
            .send()
            .await?;

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

        let pkg: CratesResponse = response
            .json()
            .await
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        Ok(PackageMetadata {
            name: pkg.krate.name,
            version: pkg.version.num,
            ecosystem: Ecosystem::Crates,
            description: pkg.krate.description,
            license: pkg.version.license,
            repository_url: pkg.krate.repository,
            homepage_url: pkg.krate.homepage,
            unpacked_size: pkg.version.crate_size,
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
    async fn test_resolve_crate() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/crates/serde/1.0.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": {
                    "num": "1.0.0",
                    "license": "MIT OR Apache-2.0",
                    "dl_path": "/api/v1/crates/serde/1.0.0/download",
                    "crate_size": 50000
                },
                "crate": {
                    "name": "serde",
                    "description": "Serialization framework",
                    "repository": "https://github.com/serde-rs/serde",
                    "homepage": null
                }
            })))
            .mount(&mock_server)
            .await;

        let resolver = CratesResolver::new(mock_server.uri());
        let meta = resolver.resolve("serde", "1.0.0").await.unwrap();
        assert_eq!(meta.name, "serde");
        assert_eq!(meta.ecosystem, Ecosystem::Crates);
    }
}
