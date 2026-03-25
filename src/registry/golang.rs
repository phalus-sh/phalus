use reqwest::Client;
use serde::Deserialize;

use super::RegistryError;
use crate::{Ecosystem, PackageMetadata};

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GoProxyInfo {
    #[serde(rename = "Version")]
    version: String,
}

// ---------------------------------------------------------------------------
// GoResolver
// ---------------------------------------------------------------------------

pub struct GoResolver {
    base_url: String,
    client: Client,
}

impl GoResolver {
    /// Create a resolver pointing at a custom base URL (useful in tests).
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Create a resolver pointing at the public Go module proxy.
    pub fn default_registry() -> Self {
        Self::new("https://proxy.golang.org".to_string())
    }

    /// Resolve a specific module version and return its metadata.
    ///
    /// `name` is the full module path (e.g. `github.com/gin-gonic/gin`).
    /// `version` should include the leading `v` (e.g. `v1.9.1`).
    pub async fn resolve(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PackageMetadata, RegistryError> {
        let url = format!("{}/{}/@v/{}.info", self.base_url, name, version);

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

        let info: GoProxyInfo = response
            .json()
            .await
            .map_err(|e| RegistryError::Parse(e.to_string()))?;

        Ok(PackageMetadata {
            name: name.to_string(),
            version: info.version,
            ecosystem: Ecosystem::Go,
            description: None,
            license: None,
            repository_url: None,
            homepage_url: None,
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
    async fn test_resolve_go_module() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/github.com/gin-gonic/gin/@v/v1.9.1.info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "v1.9.1",
                "Time": "2023-05-15T10:00:00Z"
            })))
            .mount(&mock_server)
            .await;

        let resolver = GoResolver::new(mock_server.uri());
        let meta = resolver
            .resolve("github.com/gin-gonic/gin", "v1.9.1")
            .await
            .unwrap();
        assert_eq!(meta.version, "v1.9.1");
        assert_eq!(meta.ecosystem, Ecosystem::Go);
    }
}
