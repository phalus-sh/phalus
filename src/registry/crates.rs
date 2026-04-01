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

/// Response from the crate listing endpoint (used for version resolution).
#[derive(Debug, Deserialize)]
struct CrateListResponse {
    versions: Vec<CrateListVersion>,
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(Debug, Deserialize)]
struct CrateListVersion {
    num: String,
    license: Option<String>,
    crate_size: Option<u64>,
    yanked: bool,
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

        // crates.io requires full semver (x.y.z). If we only have a partial
        // version like "1" or "1.0", resolve the latest matching version.
        let version_parts: Vec<&str> = clean_version.split('.').collect();
        if version_parts.len() < 3 || clean_version == "*" {
            return self
                .resolve_latest_matching(name, clean_version, version)
                .await;
        }

        let url = format!("{}/api/v1/crates/{}/{}", self.base_url, name, clean_version);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "phalus/0.5.0")
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

    /// Resolve the latest non-yanked version matching a partial version prefix.
    async fn resolve_latest_matching(
        &self,
        name: &str,
        prefix: &str,
        original_version: &str,
    ) -> Result<PackageMetadata, RegistryError> {
        let url = format!("{}/api/v1/crates/{}", self.base_url, name);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "phalus/0.5.0")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
                version: original_version.to_string(),
            });
        }

        if !response.status().is_success() {
            let err = response.error_for_status().unwrap_err();
            return Err(RegistryError::Http(err));
        }

        let listing: CrateListResponse = response
            .json()
            .await
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        // Find the latest non-yanked version matching the prefix
        let matched = listing.versions.iter().filter(|v| !v.yanked).find(|v| {
            if prefix.is_empty() || prefix == "*" {
                true
            } else {
                v.num == prefix
                    || (v.num.starts_with(prefix)
                        && v.num.as_bytes().get(prefix.len()) == Some(&b'.'))
            }
        });

        match matched {
            Some(ver) => Ok(PackageMetadata {
                name: listing.krate.name,
                version: ver.num.clone(),
                ecosystem: Ecosystem::Crates,
                description: listing.krate.description,
                license: ver.license.clone(),
                repository_url: listing.krate.repository,
                homepage_url: listing.krate.homepage,
                unpacked_size: ver.crate_size,
                registry_url: format!("{}/api/v1/crates/{}/{}", self.base_url, name, ver.num),
            }),
            None => Err(RegistryError::NotFound {
                name: name.to_string(),
                version: original_version.to_string(),
            }),
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
