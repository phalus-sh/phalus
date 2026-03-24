use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::docs::source_guard;
use crate::DocEntry;

#[derive(Debug, Error)]
pub enum DocFetchError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("README not found")]
    NotFound,

    #[error("Source code blocked by source guard")]
    SourceCodeBlocked,

    #[error("Decode error: {0}")]
    Decode(String),
}

pub struct GitHubFetcher {
    client: reqwest::Client,
    base_url: String,
}

impl GitHubFetcher {
    pub fn new(base_url: String, token: Option<&str>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("phalus/0.1"),
        );
        if let Some(t) = token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", t)) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build reqwest client");
        Self { client, base_url }
    }

    pub fn default_github(token: Option<&str>) -> Self {
        Self::new("https://api.github.com".to_string(), token)
    }

    pub async fn fetch_readme(&self, owner: &str, repo: &str) -> Result<DocEntry, DocFetchError> {
        let url = format!("{}/repos/{}/{}/readme", self.base_url, owner, repo);
        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(DocFetchError::NotFound);
        }

        let response = response.error_for_status()?;
        let json: serde_json::Value = response.json().await?;

        let name = json["name"]
            .as_str()
            .unwrap_or("README.md")
            .to_string();

        // Check source guard on the file name
        if source_guard::is_source_code(&name) {
            return Err(DocFetchError::SourceCodeBlocked);
        }

        let encoding = json["encoding"].as_str().unwrap_or("base64");
        if encoding != "base64" {
            return Err(DocFetchError::Decode(format!(
                "unsupported encoding: {}",
                encoding
            )));
        }

        let raw = json["content"]
            .as_str()
            .ok_or_else(|| DocFetchError::Decode("missing content field".to_string()))?;

        // GitHub wraps base64 in newlines; strip them
        let raw_clean: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&raw_clean)
            .map_err(|e| DocFetchError::Decode(e.to_string()))?;
        let content = String::from_utf8(bytes)
            .map_err(|e| DocFetchError::Decode(e.to_string()))?;

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        let source_url = format!("https://github.com/{}/{}", owner, repo);

        Ok(DocEntry {
            name,
            content,
            source_url: Some(source_url),
            content_hash,
        })
    }

    /// Parse "https://github.com/owner/repo" (with optional git+/ prefix or .git suffix)
    /// into (owner, repo).
    pub fn parse_github_url(repo_url: &str) -> Option<(String, String)> {
        // Strip git+ prefix
        let url = repo_url.strip_prefix("git+").unwrap_or(repo_url);
        // Strip .git suffix
        let url = url.strip_suffix(".git").unwrap_or(url);

        // Match https://github.com/owner/repo
        let path = url
            .strip_prefix("https://github.com/")?
            .trim_end_matches('/');

        let mut parts = path.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();

        if owner.is_empty() || repo.is_empty() {
            return None;
        }

        Some((owner, repo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_fetch_readme() {
        let mock_server = MockServer::start().await;
        let readme_content = base64::engine::general_purpose::STANDARD.encode("# Hello\nThis is a readme");
        Mock::given(method("GET"))
            .and(path("/repos/lodash/lodash/readme"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": readme_content,
                "encoding": "base64",
                "name": "README.md"
            })))
            .mount(&mock_server)
            .await;

        let fetcher = GitHubFetcher::new(mock_server.uri(), None);
        let doc = fetcher.fetch_readme("lodash", "lodash").await.unwrap();
        assert!(doc.content.contains("Hello"));
        assert_eq!(doc.name, "README.md");
    }

    #[tokio::test]
    async fn test_fetch_readme_not_found() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/foo/bar/readme"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let fetcher = GitHubFetcher::new(mock_server.uri(), None);
        assert!(fetcher.fetch_readme("foo", "bar").await.is_err());
    }
}
