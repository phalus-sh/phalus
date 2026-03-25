use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

fn default_agent_provider() -> String {
    "anthropic".to_string()
}

fn default_agent_a_model() -> String {
    "claude-sonnet-4-6".to_string()
}

fn default_agent_b_model() -> String {
    "claude-sonnet-4-6".to_string()
}

fn default_isolation_mode() -> String {
    "context".to_string()
}

fn default_max_packages_per_job() -> u32 {
    50
}

fn default_max_package_size_mb() -> u32 {
    10
}

fn default_concurrency() -> u32 {
    3
}

fn default_similarity_threshold() -> f64 {
    0.70
}

fn default_run_tests() -> bool {
    true
}

fn default_syntax_check() -> bool {
    true
}

fn default_license() -> String {
    "mit".to_string()
}

fn default_output_dir() -> String {
    "./phalus-output".to_string()
}

fn default_include_csp() -> bool {
    true
}

fn default_include_audit() -> bool {
    true
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_max_readme_size_kb() -> u32 {
    500
}

fn default_max_type_def_size_kb() -> u32 {
    200
}

fn default_max_code_example_lines() -> u32 {
    10
}

fn default_empty_string() -> String {
    String::new()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    #[serde(default = "default_agent_provider")]
    pub agent_a_provider: String,
    #[serde(default = "default_agent_a_model")]
    pub agent_a_model: String,
    #[serde(default = "default_empty_string")]
    pub agent_a_api_key: String,
    #[serde(default = "default_empty_string")]
    pub agent_a_base_url: String,
    #[serde(default = "default_agent_provider")]
    pub agent_b_provider: String,
    #[serde(default = "default_agent_b_model")]
    pub agent_b_model: String,
    #[serde(default = "default_empty_string")]
    pub agent_b_api_key: String,
    #[serde(default = "default_empty_string")]
    pub agent_b_base_url: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            agent_a_provider: default_agent_provider(),
            agent_a_model: default_agent_a_model(),
            agent_a_api_key: default_empty_string(),
            agent_a_base_url: default_empty_string(),
            agent_b_provider: default_agent_provider(),
            agent_b_model: default_agent_b_model(),
            agent_b_api_key: default_empty_string(),
            agent_b_base_url: default_empty_string(),
        }
    }
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("agent_a_provider", &self.agent_a_provider)
            .field("agent_a_model", &self.agent_a_model)
            .field(
                "agent_a_api_key",
                &if self.agent_a_api_key.is_empty() {
                    "(empty)"
                } else {
                    "***"
                },
            )
            .field("agent_a_base_url", &self.agent_a_base_url)
            .field("agent_b_provider", &self.agent_b_provider)
            .field("agent_b_model", &self.agent_b_model)
            .field(
                "agent_b_api_key",
                &if self.agent_b_api_key.is_empty() {
                    "(empty)"
                } else {
                    "***"
                },
            )
            .field("agent_b_base_url", &self.agent_b_base_url)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IsolationConfig {
    #[serde(default = "default_isolation_mode")]
    pub mode: String,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            mode: default_isolation_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    #[serde(default = "default_max_packages_per_job")]
    pub max_packages_per_job: u32,
    #[serde(default = "default_max_package_size_mb")]
    pub max_package_size_mb: u32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_packages_per_job: default_max_packages_per_job(),
            max_package_size_mb: default_max_package_size_mb(),
            concurrency: default_concurrency(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ValidationConfig {
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f64,
    #[serde(default = "default_run_tests")]
    pub run_tests: bool,
    #[serde(default = "default_syntax_check")]
    pub syntax_check: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: default_similarity_threshold(),
            run_tests: default_run_tests(),
            syntax_check: default_syntax_check(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    #[serde(default = "default_license")]
    pub default_license: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_include_csp")]
    pub include_csp: bool,
    #[serde(default = "default_include_audit")]
    pub include_audit: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            default_license: default_license(),
            output_dir: default_output_dir(),
            include_csp: default_include_csp(),
            include_audit: default_include_audit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    pub enabled: bool,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_port(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocFetcherConfig {
    #[serde(default = "default_max_readme_size_kb")]
    pub max_readme_size_kb: u32,
    #[serde(default = "default_max_type_def_size_kb")]
    pub max_type_def_size_kb: u32,
    #[serde(default = "default_max_code_example_lines")]
    pub max_code_example_lines: u32,
    #[serde(default = "default_empty_string")]
    pub github_token: String,
}

impl Default for DocFetcherConfig {
    fn default() -> Self {
        Self {
            max_readme_size_kb: default_max_readme_size_kb(),
            max_type_def_size_kb: default_max_type_def_size_kb(),
            max_code_example_lines: default_max_code_example_lines(),
            github_token: default_empty_string(),
        }
    }
}

impl std::fmt::Debug for DocFetcherConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocFetcherConfig")
            .field("max_readme_size_kb", &self.max_readme_size_kb)
            .field("max_type_def_size_kb", &self.max_type_def_size_kb)
            .field("max_code_example_lines", &self.max_code_example_lines)
            .field(
                "github_token",
                &if self.github_token.is_empty() {
                    "(empty)"
                } else {
                    "***"
                },
            )
            .finish()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PhalusConfig {
    pub llm: LlmConfig,
    pub isolation: IsolationConfig,
    pub limits: LimitsConfig,
    pub validation: ValidationConfig,
    pub output: OutputConfig,
    pub web: WebConfig,
    pub doc_fetcher: DocFetcherConfig,
}

impl PhalusConfig {
    /// Returns the default config file path: ~/.phalus/config.toml
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".phalus")
            .join("config.toml")
    }

    /// Load config from the default path, falling back to defaults if not found.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::default_path();
        if path.exists() {
            Self::load_from_file(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load config from a specific file path.
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Apply PHALUS_* environment variable overrides.
    ///
    /// Variables use double underscore for nesting, e.g.:
    ///   PHALUS_LLM__AGENT_A_MODEL=gpt-4
    pub fn with_env_overrides(mut config: Self) -> Self {
        for (key, value) in std::env::vars() {
            let Some(rest) = key.strip_prefix("PHALUS_") else {
                continue;
            };
            // Split on __ to get section and field
            let parts: Vec<&str> = rest.splitn(2, "__").collect();
            if parts.len() != 2 {
                continue;
            }
            let section = parts[0].to_ascii_lowercase();
            let field = parts[1].to_ascii_lowercase();

            match section.as_str() {
                "llm" => apply_llm_override(&mut config.llm, &field, &value),
                "isolation" => apply_isolation_override(&mut config.isolation, &field, &value),
                "limits" => apply_limits_override(&mut config.limits, &field, &value),
                "validation" => apply_validation_override(&mut config.validation, &field, &value),
                "output" => apply_output_override(&mut config.output, &field, &value),
                "web" => apply_web_override(&mut config.web, &field, &value),
                "doc_fetcher" => apply_doc_fetcher_override(&mut config.doc_fetcher, &field, &value),
                _ => {}
            }
        }
        config
    }
}

fn apply_llm_override(cfg: &mut LlmConfig, field: &str, value: &str) {
    match field {
        "agent_a_provider" => cfg.agent_a_provider = value.to_string(),
        "agent_a_model" => cfg.agent_a_model = value.to_string(),
        "agent_a_api_key" => cfg.agent_a_api_key = value.to_string(),
        "agent_a_base_url" => cfg.agent_a_base_url = value.to_string(),
        "agent_b_provider" => cfg.agent_b_provider = value.to_string(),
        "agent_b_model" => cfg.agent_b_model = value.to_string(),
        "agent_b_api_key" => cfg.agent_b_api_key = value.to_string(),
        "agent_b_base_url" => cfg.agent_b_base_url = value.to_string(),
        _ => {}
    }
}

fn apply_isolation_override(cfg: &mut IsolationConfig, field: &str, value: &str) {
    if field == "mode" {
        cfg.mode = value.to_string();
    }
}

fn apply_limits_override(cfg: &mut LimitsConfig, field: &str, value: &str) {
    match field {
        "max_packages_per_job" => {
            if let Ok(v) = value.parse() {
                cfg.max_packages_per_job = v;
            }
        }
        "max_package_size_mb" => {
            if let Ok(v) = value.parse() {
                cfg.max_package_size_mb = v;
            }
        }
        "concurrency" => {
            if let Ok(v) = value.parse() {
                cfg.concurrency = v;
            }
        }
        _ => {}
    }
}

fn apply_validation_override(cfg: &mut ValidationConfig, field: &str, value: &str) {
    match field {
        "similarity_threshold" => {
            if let Ok(v) = value.parse() {
                cfg.similarity_threshold = v;
            }
        }
        "run_tests" => {
            if let Ok(v) = value.parse() {
                cfg.run_tests = v;
            }
        }
        "syntax_check" => {
            if let Ok(v) = value.parse() {
                cfg.syntax_check = v;
            }
        }
        _ => {}
    }
}

fn apply_output_override(cfg: &mut OutputConfig, field: &str, value: &str) {
    match field {
        "default_license" => cfg.default_license = value.to_string(),
        "output_dir" => cfg.output_dir = value.to_string(),
        "include_csp" => {
            if let Ok(v) = value.parse() {
                cfg.include_csp = v;
            }
        }
        "include_audit" => {
            if let Ok(v) = value.parse() {
                cfg.include_audit = v;
            }
        }
        _ => {}
    }
}

fn apply_web_override(cfg: &mut WebConfig, field: &str, value: &str) {
    match field {
        "enabled" => {
            if let Ok(v) = value.parse() {
                cfg.enabled = v;
            }
        }
        "host" => cfg.host = value.to_string(),
        "port" => {
            if let Ok(v) = value.parse() {
                cfg.port = v;
            }
        }
        _ => {}
    }
}

fn apply_doc_fetcher_override(cfg: &mut DocFetcherConfig, field: &str, value: &str) {
    match field {
        "max_readme_size_kb" => {
            if let Ok(v) = value.parse() {
                cfg.max_readme_size_kb = v;
            }
        }
        "max_type_def_size_kb" => {
            if let Ok(v) = value.parse() {
                cfg.max_type_def_size_kb = v;
            }
        }
        "max_code_example_lines" => {
            if let Ok(v) = value.parse() {
                cfg.max_code_example_lines = v;
            }
        }
        "github_token" => cfg.github_token = value.to_string(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = PhalusConfig::default();
        assert_eq!(config.llm.agent_a_model, "claude-sonnet-4-6");
        assert_eq!(config.limits.max_packages_per_job, 50);
        assert_eq!(config.validation.similarity_threshold, 0.70);
        assert_eq!(config.output.default_license, "mit");
    }

    #[test]
    fn test_load_from_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"
[llm]
agent_a_model = "gpt-4"

[limits]
max_packages_per_job = 10
"#).unwrap();

        let config = PhalusConfig::load_from_file(&path).unwrap();
        assert_eq!(config.llm.agent_a_model, "gpt-4");
        assert_eq!(config.limits.max_packages_per_job, 10);
        assert_eq!(config.validation.similarity_threshold, 0.70);
    }

    #[test]
    fn test_env_override() {
        unsafe { std::env::set_var("PHALUS_LLM__AGENT_A_MODEL", "test-model"); }
        let config = PhalusConfig::with_env_overrides(PhalusConfig::default());
        assert_eq!(config.llm.agent_a_model, "test-model");
        unsafe { std::env::remove_var("PHALUS_LLM__AGENT_A_MODEL"); }
    }
}
