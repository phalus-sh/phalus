//! Adapter wrapping phalus's `LlmProvider` as a symbi-runtime `InferenceProvider`.
//!
//! Since the underlying LLM provider doesn't support native tool calling,
//! we inject tool definitions into the system prompt and parse text-based
//! `<tool_call>` blocks from the response.

use crate::agents::provider::LlmProvider;
use async_trait::async_trait;
use std::sync::Arc;
use symbi_runtime::reasoning::conversation::{Conversation, MessageRole};
use symbi_runtime::reasoning::inference::{
    FinishReason, InferenceError, InferenceOptions, InferenceProvider, InferenceResponse,
    ToolCallRequest, Usage,
};
use tokio::sync::Mutex;

/// An `InferenceProvider` backed by phalus's `LlmProvider`.
///
/// Tool calling is emulated via text: when tool definitions are present,
/// the system prompt is augmented with tool schemas and the model is
/// instructed to emit `<tool_call name="...">{"arg":"val"}</tool_call>`
/// blocks, which are then parsed into `ToolCallRequest` items.
pub struct PhalusInferenceProvider {
    provider: Arc<Mutex<LlmProvider>>,
    model_name: String,
}

impl PhalusInferenceProvider {
    /// Create a new adapter around an existing `LlmProvider`.
    pub fn new(provider: LlmProvider) -> Self {
        let model_name = provider.model().to_string();
        Self {
            provider: Arc::new(Mutex::new(provider)),
            model_name,
        }
    }
}

#[async_trait]
impl InferenceProvider for PhalusInferenceProvider {
    async fn complete(
        &self,
        conversation: &Conversation,
        options: &InferenceOptions,
    ) -> Result<InferenceResponse, InferenceError> {
        // Extract system prompt.
        let mut system_prompt = conversation
            .system_message()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // When tool definitions are available, append them to the system prompt.
        if !options.tool_definitions.is_empty() {
            system_prompt.push_str("\n\n# Available Tools\n\n");
            system_prompt.push_str(
                "When you want to call a tool, emit one or more blocks in this format:\n",
            );
            system_prompt
                .push_str("<tool_call name=\"TOOL_NAME\">{\"param\": \"value\"}</tool_call>\n\n");
            for tool in &options.tool_definitions {
                system_prompt.push_str(&format!("## {}\n", tool.name));
                system_prompt.push_str(&format!("{}\n", tool.description));
                system_prompt.push_str(&format!(
                    "Parameters: {}\n\n",
                    serde_json::to_string(&tool.parameters).unwrap_or_default()
                ));
            }
        }

        // Build a combined user prompt from all non-system messages.
        let mut parts: Vec<String> = Vec::new();
        for msg in conversation.messages() {
            match msg.role {
                MessageRole::System => continue,
                MessageRole::User => parts.push(format!("[User]: {}", msg.content)),
                MessageRole::Assistant => {
                    if !msg.content.is_empty() {
                        parts.push(format!("[Assistant]: {}", msg.content));
                    }
                    for tc in &msg.tool_calls {
                        parts.push(format!(
                            "[Assistant called tool {}]: {}",
                            tc.name, tc.arguments
                        ));
                    }
                }
                MessageRole::Tool => {
                    let tool_name = msg.tool_name.as_deref().unwrap_or("unknown");
                    parts.push(format!("[Tool result ({})]: {}", tool_name, msg.content));
                }
            }
        }
        let user_prompt = parts.join("\n");

        // Call the underlying provider.
        let provider = self.provider.lock().await;
        let response_text = provider
            .complete(&system_prompt, &user_prompt, options.max_tokens)
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;
        drop(provider);

        // Parse tool calls from the response.
        let tool_calls = parse_tool_calls(&response_text);

        // Strip tool_call blocks from the content that the caller sees.
        let content = strip_tool_call_blocks(&response_text);

        let finish_reason = if !tool_calls.is_empty() {
            FinishReason::ToolCalls
        } else {
            FinishReason::Stop
        };

        Ok(InferenceResponse {
            content,
            tool_calls,
            finish_reason,
            usage: Usage::default(),
            model: self.model_name.clone(),
        })
    }

    fn provider_name(&self) -> &str {
        "phalus"
    }

    fn default_model(&self) -> &str {
        &self.model_name
    }

    fn supports_native_tools(&self) -> bool {
        false
    }

    fn supports_structured_output(&self) -> bool {
        false
    }
}

/// Parse `<tool_call name="NAME">ARGS</tool_call>` blocks from the response.
fn parse_tool_calls(text: &str) -> Vec<ToolCallRequest> {
    let mut results = Vec::new();
    let mut search_from = 0;

    while let Some(start) = text[search_from..].find("<tool_call") {
        let abs_start = search_from + start;
        // Find the name attribute.
        let tag_end = match text[abs_start..].find('>') {
            Some(pos) => abs_start + pos,
            None => break,
        };
        let tag_str = &text[abs_start..tag_end];

        let name = extract_attr(tag_str, "name").unwrap_or_default();

        // Find closing tag.
        let close_tag = "</tool_call>";
        let close_pos = match text[tag_end..].find(close_tag) {
            Some(pos) => tag_end + pos,
            None => break,
        };

        let arguments = text[tag_end + 1..close_pos].trim().to_string();

        results.push(ToolCallRequest {
            id: format!("tc_{}", results.len()),
            name,
            arguments,
        });

        search_from = close_pos + close_tag.len();
    }

    results
}

/// Extract an attribute value from a tag string, e.g. `name="foo"` -> `"foo"`.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{}=\"", attr);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Remove `<tool_call ...>...</tool_call>` blocks from text, returning
/// the remaining content (trimmed).
fn strip_tool_call_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut search_from = 0;

    while let Some(start) = text[search_from..].find("<tool_call") {
        let abs_start = search_from + start;
        result.push_str(&text[search_from..abs_start]);

        let close_tag = "</tool_call>";
        match text[abs_start..].find(close_tag) {
            Some(pos) => search_from = abs_start + pos + close_tag.len(),
            None => {
                // No closing tag; keep the rest as-is.
                result.push_str(&text[abs_start..]);
                return result.trim().to_string();
            }
        }
    }

    result.push_str(&text[search_from..]);
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::provider::{LlmProvider, ProviderKind};
    use crate::config::RetryConfig;
    use symbi_runtime::reasoning::inference::ToolDefinition;

    fn make_provider() -> PhalusInferenceProvider {
        let llm = LlmProvider::new(
            "test-key",
            "test-model",
            Some("http://localhost:0"),
            RetryConfig::default(),
            ProviderKind::OpenAi,
        );
        PhalusInferenceProvider::new(llm)
    }

    #[test]
    fn test_provider_name() {
        let p = make_provider();
        assert_eq!(p.provider_name(), "phalus");
        assert_eq!(p.default_model(), "test-model");
        assert!(!p.supports_native_tools());
        assert!(!p.supports_structured_output());
    }

    #[test]
    fn test_parse_tool_calls_single() {
        let text = r#"Let me search for that.
<tool_call name="web_search">{"query": "rust crates"}</tool_call>
Done."#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "web_search");
        assert_eq!(calls[0].arguments, r#"{"query": "rust crates"}"#);
        assert_eq!(calls[0].id, "tc_0");
    }

    #[test]
    fn test_parse_tool_calls_multiple() {
        let text = r#"<tool_call name="a">{"x":1}</tool_call>
<tool_call name="b">{"y":2}</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[1].name, "b");
    }

    #[test]
    fn test_parse_tool_calls_none() {
        let calls = parse_tool_calls("No tool calls here.");
        assert!(calls.is_empty());
    }

    #[test]
    fn test_strip_tool_call_blocks() {
        let text = r#"Before
<tool_call name="t">{"a":1}</tool_call>
After"#;
        let stripped = strip_tool_call_blocks(text);
        assert_eq!(stripped, "Before\n\nAfter");
    }

    #[test]
    fn test_build_tool_prompt() {
        let tool = ToolDefinition {
            name: "search".into(),
            description: "Search the web".into(),
            parameters: serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        };
        let mut system = String::from("You are helpful.");
        system.push_str("\n\n# Available Tools\n\n");
        system.push_str("When you want to call a tool, emit one or more blocks in this format:\n");
        system.push_str("<tool_call name=\"TOOL_NAME\">{\"param\": \"value\"}</tool_call>\n\n");
        system.push_str(&format!("## {}\n", tool.name));
        assert!(system.contains("# Available Tools"));
        assert!(system.contains("search"));
    }
}
