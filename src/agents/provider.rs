use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    #[error("empty response")]
    EmptyResponse,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

pub struct LlmProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl LlmProvider {
    pub fn new(api_key: &str, model: &str, base_url: Option<&str>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.unwrap_or("https://api.anthropic.com").to_string(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".into(),
                content: user_prompt.to_string(),
            }],
        };

        let resp = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
            });
        }

        let body: AnthropicResponse = resp.json().await?;
        body.content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or(ProviderError::EmptyResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_construction() {
        let provider = LlmProvider::new("test-key", "claude-sonnet-4-6", None);
        assert_eq!(provider.model(), "claude-sonnet-4-6");
        assert_eq!(provider.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn test_provider_custom_base_url() {
        let provider = LlmProvider::new("key", "model", Some("http://localhost:8080"));
        assert_eq!(provider.base_url, "http://localhost:8080");
    }
}
