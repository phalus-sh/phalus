//! Orchestrates the symbi-runtime `ReasoningLoopRunner` for Agent B.
//!
//! Provides a single public function [`run_agent_b_loop`] that wires up the
//! inference provider, tool executor, and reasoning loop, then collects the
//! generated files into an [`Implementation`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use tracing::{info, warn};

use symbi_runtime::reasoning::circuit_breaker::CircuitBreakerRegistry;
use symbi_runtime::reasoning::context_manager::DefaultContextManager;
use symbi_runtime::reasoning::conversation::{Conversation, ConversationMessage};
use symbi_runtime::reasoning::executor::ActionExecutor;
use symbi_runtime::reasoning::loop_types::{
    BufferedJournal, LoopConfig, RecoveryStrategy, TerminationReason,
};
use symbi_runtime::reasoning::policy_bridge::DefaultPolicyGate;
use symbi_runtime::reasoning::reasoning_loop::ReasoningLoopRunner;
use symbi_runtime::types::AgentId;

use crate::agents::agent_b_executor::AgentBExecutor;
use crate::agents::builder;
use crate::agents::provider::{LlmProvider, ProviderKind};
use crate::agents::symbiont_provider::PhalusInferenceProvider;
use crate::config::PhalusConfig;
use crate::{CspSpec, Implementation, TargetLanguage};

/// Run Agent B's agentic code-generation loop using symbi-runtime.
///
/// The loop uses three tools (`write_files`, `check_completeness`, `check_imports`)
/// to iteratively generate a complete package implementation from a CSP specification.
pub async fn run_agent_b_loop(
    csp: &CspSpec,
    license: &str,
    target_lang: &TargetLanguage,
    config: &PhalusConfig,
    output_dir: &Path,
) -> anyhow::Result<Implementation> {
    // 1. Validate API key
    if config.llm.agent_b_api_key.is_empty() {
        bail!("agent_b_api_key is not set; cannot run Agent B loop");
    }

    // 2. Create package output directory
    let pkg_dir = output_dir.join(&csp.package_name);
    std::fs::create_dir_all(&pkg_dir)
        .with_context(|| format!("failed to create package directory: {}", pkg_dir.display()))?;

    // 3. Extract api-surface document content for the completeness checker
    let api_surface_json = csp
        .documents
        .iter()
        .find(|d| d.filename.contains("02-api-surface"))
        .map(|d| d.content.clone())
        .unwrap_or_else(|| "{}".to_string());

    // 4. Build inference provider
    let base_url = if config.llm.agent_b_base_url.is_empty() {
        None
    } else {
        Some(config.llm.agent_b_base_url.as_str())
    };
    let kind = ProviderKind::parse(&config.llm.agent_b_provider);
    let llm = LlmProvider::new(
        &config.llm.agent_b_api_key,
        &config.llm.agent_b_model,
        base_url,
        config.llm.retry.clone(),
        kind,
    );
    let provider = Arc::new(PhalusInferenceProvider::new(llm));

    // 5. Build executor
    let executor = Arc::new(AgentBExecutor::new(pkg_dir.clone(), api_surface_json));

    // 6. Get tool definitions before moving executor into runner
    let tool_defs = executor.tool_definitions();

    // 7. Build the reasoning loop runner
    let runner = ReasoningLoopRunner::builder()
        .provider(provider as Arc<dyn symbi_runtime::reasoning::inference::InferenceProvider>)
        .executor(executor as Arc<dyn ActionExecutor>)
        .policy_gate(Arc::new(DefaultPolicyGate::permissive()) as _)
        .context_manager(Arc::new(DefaultContextManager::default()) as _)
        .circuit_breakers(Arc::new(CircuitBreakerRegistry::default()))
        .journal(Arc::new(BufferedJournal::new(256)) as _)
        .build();

    // 8. Build conversation
    let system = format!(
        "{}\n\n\
         # Agentic Workflow\n\n\
         You have three tools available:\n\
         - **write_files**: Write source files using ===FILE: path===...===END_FILE=== delimiters.\n\
         - **check_completeness**: Verify all API surface names are implemented.\n\
         - **check_imports**: Verify all local imports resolve.\n\n\
         Follow this iterative workflow:\n\
         1. Generate the initial implementation using write_files.\n\
         2. Call check_completeness to find missing exports.\n\
         3. Call check_imports to find unresolved imports.\n\
         4. If issues are found, fix them with write_files and re-check.\n\
         5. Repeat until both checks pass.\n\n\
         Always write complete, production-quality code.",
        builder::system_prompt()
    );

    let mut conversation = Conversation::with_system(system);
    let user_msg = builder::build_builder_prompt(csp, license, target_lang);
    conversation.push(ConversationMessage::user(user_msg));

    // 9. Configure loop
    let loop_config = LoopConfig {
        max_iterations: 10,
        max_total_tokens: config.llm.agent_b_max_tokens * 10,
        timeout: Duration::from_secs(config.llm.retry.timeout_secs * 10),
        tool_timeout: Duration::from_secs(30),
        max_concurrent_tools: 1,
        context_token_budget: 64_000,
        tool_definitions: tool_defs,
        default_recovery: RecoveryStrategy::Retry {
            max_attempts: 2,
            base_delay: Duration::from_secs(1),
        },
    };

    // 10. Run the loop
    let agent_id = AgentId::new();
    info!(
        package = %csp.package_name,
        "Starting Agent B reasoning loop"
    );

    let result = runner.run(agent_id, conversation, loop_config).await;

    // 11. Log result
    info!(
        iterations = result.iterations,
        termination = ?result.termination_reason,
        duration_secs = result.duration.as_secs(),
        "Agent B loop completed"
    );

    // 12. Check for errors
    if let TerminationReason::Error { message } = &result.termination_reason {
        bail!("Agent B loop failed: {message}");
    }

    // 13. Collect files from pkg_dir
    let files = collect_files(&pkg_dir).with_context(|| {
        format!(
            "failed to collect generated files from {}",
            pkg_dir.display()
        )
    })?;

    if files.is_empty() {
        warn!("Agent B loop produced no files");
    }

    info!(file_count = files.len(), "Collected generated files");

    // 14. Return implementation
    Ok(Implementation {
        package_name: csp.package_name.clone(),
        files,
        target_language: target_lang.to_string(),
    })
}

/// Recursively collect all files under `dir` into a map of relative-path -> content.
/// Skips directories whose name starts with `.` (e.g. `.cleanroom`).
fn collect_files(dir: &Path) -> anyhow::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    collect_files_recursive(dir, dir, &mut map)?;
    Ok(map)
}

fn collect_files_recursive(
    base: &Path,
    current: &Path,
    map: &mut HashMap<String, String>,
) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(current)
        .with_context(|| format!("failed to read directory: {}", current.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden directories
        if path.is_dir() && name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_files_recursive(base, &path, map)?;
        } else {
            let rel_path = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read file: {}", path.display()))?;
            map.insert(rel_path, content);
        }
    }

    Ok(())
}
