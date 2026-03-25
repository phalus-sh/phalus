use reqwest::Client;
use serde::Deserialize;

use crate::{Ecosystem, PackageMetadata};
use super::RegistryError;

// ---------------------------------------------------------------------------
// Response shapes (private, only used for deserialization)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct NpmPackageResponse {
    name: String,
    version: String,
    description: Option<String>,
    license: Option<String>,
    repository: Option<NpmRepository>,
    homepage: Option<String>,
    dist: Option<NpmDist>,
}

#[derive(Debug, Deserialize)]
struct NpmRepository {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NpmDist {
    #[serde(rename = "unpackedSize")]
    unpacked_size: Option<u64>,
}

// ---------------------------------------------------------------------------
// NpmResolver
// ---------------------------------------------------------------------------

pub struct NpmResolver {
    base_url: String,
    client: Client,
}

impl NpmResolver {
    /// Create a resolver pointing at a custom base URL (useful in tests).
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Create a resolver pointing at the public npm registry.
    pub fn default_registry() -> Self {
        Self::new("https://registry.npmjs.org".to_string())
    }

    /// Resolve a specific package version and return its metadata.
    pub async fn resolve(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PackageMetadata, RegistryError> {
        let url = format!("{}/{}/{}", self.base_url, name, version);

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

        let pkg: NpmPackageResponse = response
            .json()
            .await
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        let repository_url = pkg
            .repository
            .and_then(|r| r.url)
            .map(|u| {
                u.trim_start_matches("git+")
                    .trim_end_matches(".git")
                    .to_string()
            });

        let unpacked_size = pkg.dist.and_then(|d| d.unpacked_size);

        Ok(PackageMetadata {
            name: pkg.name,
            version: pkg.version,
            ecosystem: Ecosystem::Npm,
            description: pkg.description,
            license: pkg.license,
            repository_url,
            homepage_url: pkg.homepage,
            unpacked_size,
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
    async fn test_resolve_npm_package() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/lodash/4.17.21"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "lodash",
                "version": "4.17.21",
                "description": "Lodash modular utilities.",
                "license": "MIT",
                "repository": { "type": "git", "url": "git+https://github.com/lodash/lodash.git" },
                "homepage": "https://lodash.com/",
                "dist": { "unpackedSize": 1412345 }
            })))
            .mount(&mock_server)
            .await;

        let resolver = NpmResolver::new(mock_server.uri());
        let meta = resolver.resolve("lodash", "4.17.21").await.unwrap();
        assert_eq!(meta.name, "lodash");
        assert_eq!(meta.version, "4.17.21");
        assert_eq!(meta.license, Some("MIT".to_string()));
        assert_eq!(meta.unpacked_size, Some(1412345));
        assert!(meta.repository_url.unwrap().contains("lodash"));
    }

    #[tokio::test]
    async fn test_resolve_not_found() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/nonexistent/1.0.0"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let resolver = NpmResolver::new(mock_server.uri());
        assert!(resolver.resolve("nonexistent", "1.0.0").await.is_err());
    }
}
