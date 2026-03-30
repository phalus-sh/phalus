use crate::config::RetryConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::warn;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    #[error("empty response")]
    EmptyResponse,
    #[error("request timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },
    #[error("retries exhausted after {attempts} attempts: {last_error}")]
    RetriesExhausted { attempts: u32, last_error: String },
}

/// Which wire protocol to use when talking to the LLM backend.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderKind {
    /// Anthropic Messages API (`/v1/messages`).
    Anthropic,
    /// OpenAI-compatible Chat Completions API (`/v1/chat/completions`).
    /// Works with OpenAI, OpenRouter, Ollama, vLLM, LiteLLM, etc.
    OpenAi,
}

impl ProviderKind {
    /// Parse from the config string. Anything not explicitly "anthropic" is
    /// treated as OpenAI-compatible.
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            _ => Self::OpenAi,
        }
    }

    fn default_base_url(&self) -> &str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAi => "https://api.openai.com",
        }
    }
}

// ---------------------------------------------------------------------------
// Anthropic wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ChatMessage>,
}

// ---------------------------------------------------------------------------
// OpenAI wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
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

// ---------------------------------------------------------------------------
// LlmProvider
// ---------------------------------------------------------------------------

pub struct LlmProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    kind: ProviderKind,
    retry: RetryConfig,
}

impl LlmProvider {
    pub fn new(
        api_key: &str,
        model: &str,
        base_url: Option<&str>,
        retry: RetryConfig,
        kind: ProviderKind,
    ) -> Self {
        let base_url = base_url.unwrap_or_else(|| kind.default_base_url());
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.to_string(),
            kind,
            retry,
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
        let max_attempts = self.retry.max_retries + 1;
        let mut last_error = String::new();

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let backoff_ms = self.retry.initial_backoff_ms * (1u64 << (attempt - 1));
                warn!(
                    attempt,
                    backoff_ms, "LLM request failed, retrying after backoff"
                );
                sleep(Duration::from_millis(backoff_ms)).await;
            }

            let result = match self.kind {
                ProviderKind::Anthropic => {
                    self.attempt_anthropic(system_prompt, user_prompt, max_tokens)
                        .await
                }
                ProviderKind::OpenAi => {
                    self.attempt_openai(system_prompt, user_prompt, max_tokens)
                        .await
                }
            };

            match result {
                Ok(text) => return Ok(text),
                Err(e) => {
                    last_error = e.to_string();
                    if !is_retryable(&e) {
                        return Err(e);
                    }
                }
            }
        }

        Err(ProviderError::RetriesExhausted {
            attempts: max_attempts,
            last_error,
        })
    }

    // -----------------------------------------------------------------------
    // Anthropic Messages API
    // -----------------------------------------------------------------------

    async fn attempt_anthropic(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            system: system_prompt.to_string(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_prompt.to_string(),
            }],
        };

        let timeout = Duration::from_secs(self.retry.timeout_secs);

        let fut = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send();

        let resp = tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| ProviderError::Timeout {
                timeout_secs: self.retry.timeout_secs,
            })?
            .map_err(ProviderError::Http)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
            });
        }

        let body: AnthropicResponse = resp.json().await.map_err(ProviderError::Http)?;
        body.content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or(ProviderError::EmptyResponse)
    }

    // -----------------------------------------------------------------------
    // OpenAI-compatible Chat Completions API
    // -----------------------------------------------------------------------

    async fn attempt_openai(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, ProviderError> {
        let request = OpenAiRequest {
            model: self.model.clone(),
            max_tokens,
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_prompt.to_string(),
                },
            ],
        };

        let timeout = Duration::from_secs(self.retry.timeout_secs);

        let fut = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&request)
            .send();

        let resp = tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| ProviderError::Timeout {
                timeout_secs: self.retry.timeout_secs,
            })?
            .map_err(ProviderError::Http)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
            });
        }

        let body: OpenAiResponse = resp.json().await.map_err(ProviderError::Http)?;
        body.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or(ProviderError::EmptyResponse)
    }
}

/// Returns true for errors that are safe to retry.
fn is_retryable(err: &ProviderError) -> bool {
    match err {
        ProviderError::Timeout { .. } => true,
        ProviderError::Api { status, .. } => *status == 429 || *status >= 500,
        ProviderError::Http(e) => e.is_timeout() || e.is_connect(),
        ProviderError::EmptyResponse | ProviderError::RetriesExhausted { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn default_retry() -> RetryConfig {
        RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 1,
            timeout_secs: 5,
        }
    }

    fn make_anthropic_provider(base_url: &str) -> LlmProvider {
        LlmProvider::new(
            "test-key",
            "claude-sonnet-4-6",
            Some(base_url),
            default_retry(),
            ProviderKind::Anthropic,
        )
    }

    fn make_openai_provider(base_url: &str) -> LlmProvider {
        LlmProvider::new(
            "test-key",
            "gpt-4o",
            Some(base_url),
            default_retry(),
            ProviderKind::OpenAi,
        )
    }

    fn anthropic_success_body() -> serde_json::Value {
        serde_json::json!({
            "content": [{"type": "text", "text": "hello"}]
        })
    }

    fn openai_success_body() -> serde_json::Value {
        serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "hello"}}]
        })
    }

    #[test]
    fn test_provider_kind_from_str() {
        assert_eq!(ProviderKind::parse("anthropic"), ProviderKind::Anthropic);
        assert_eq!(ProviderKind::parse("Anthropic"), ProviderKind::Anthropic);
        assert_eq!(ProviderKind::parse("openai"), ProviderKind::OpenAi);
        assert_eq!(ProviderKind::parse("openrouter"), ProviderKind::OpenAi);
        assert_eq!(ProviderKind::parse("ollama"), ProviderKind::OpenAi);
        assert_eq!(ProviderKind::parse("vllm"), ProviderKind::OpenAi);
    }

    #[test]
    fn test_provider_construction_anthropic() {
        let provider = LlmProvider::new(
            "test-key",
            "claude-sonnet-4-6",
            None,
            RetryConfig::default(),
            ProviderKind::Anthropic,
        );
        assert_eq!(provider.model(), "claude-sonnet-4-6");
        assert_eq!(provider.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn test_provider_construction_openai() {
        let provider = LlmProvider::new(
            "test-key",
            "gpt-4o",
            None,
            RetryConfig::default(),
            ProviderKind::OpenAi,
        );
        assert_eq!(provider.model(), "gpt-4o");
        assert_eq!(provider.base_url, "https://api.openai.com");
    }

    #[test]
    fn test_provider_custom_base_url() {
        let provider = LlmProvider::new(
            "key",
            "model",
            Some("http://localhost:11434"),
            RetryConfig::default(),
            ProviderKind::OpenAi,
        );
        assert_eq!(provider.base_url, "http://localhost:11434");
    }

    #[tokio::test]
    async fn test_anthropic_successful_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_body()))
            .mount(&server)
            .await;

        let provider = make_anthropic_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_openai_successful_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(openai_success_body()))
            .mount(&server)
            .await;

        let provider = make_openai_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_openai_retries_on_500() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .up_to_n_times(2)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(openai_success_body()))
            .mount(&server)
            .await;

        let provider = make_openai_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_retries_on_500() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .up_to_n_times(2)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_body()))
            .mount(&server)
            .await;

        let provider = make_anthropic_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_retries_on_429() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_body()))
            .mount(&server)
            .await;

        let provider = make_anthropic_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_exhausted_retries_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("always fails"))
            .mount(&server)
            .await;

        let provider = LlmProvider::new(
            "test-key",
            "model",
            Some(&server.uri()),
            RetryConfig {
                max_retries: 2,
                initial_backoff_ms: 1,
                timeout_secs: 5,
            },
            ProviderKind::Anthropic,
        );
        let result = provider.complete("sys", "user", 100).await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, ProviderError::RetriesExhausted { attempts: 3, .. }),
            "expected RetriesExhausted with 3 attempts, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_non_retryable_4xx_fails_immediately() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .mount(&server)
            .await;

        let provider = make_anthropic_provider(&server.uri());
        let result = provider.complete("sys", "user", 100).await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, ProviderError::Api { status: 400, .. }),
            "expected Api 400 error, got: {err}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_timeout_is_retried() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(anthropic_success_body())
                    .set_delay(Duration::from_secs(10)),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_body()))
            .mount(&server)
            .await;

        let provider = LlmProvider::new(
            "test-key",
            "model",
            Some(&server.uri()),
            RetryConfig {
                max_retries: 2,
                initial_backoff_ms: 1,
                timeout_secs: 1,
            },
            ProviderKind::Anthropic,
        );
        let result = provider.complete("sys", "user", 100).await;
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(&ProviderError::Timeout { timeout_secs: 30 }));
        assert!(is_retryable(&ProviderError::Api {
            status: 429,
            message: "rate limited".into()
        }));
        assert!(is_retryable(&ProviderError::Api {
            status: 500,
            message: "server error".into()
        }));
        assert!(!is_retryable(&ProviderError::Api {
            status: 400,
            message: "bad request".into()
        }));
        assert!(!is_retryable(&ProviderError::EmptyResponse));
    }
}
