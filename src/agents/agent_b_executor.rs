//! Agent B tool executor for the symbiont reasoning loop.
//!
//! Provides three tools: `write_files`, `check_completeness`, and `check_imports`
//! that Agent B uses during its agentic code-generation loop.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use symbi_runtime::reasoning::circuit_breaker::CircuitBreakerRegistry;
use symbi_runtime::reasoning::executor::ActionExecutor;
use symbi_runtime::reasoning::inference::ToolDefinition;
use symbi_runtime::reasoning::loop_types::{LoopConfig, Observation, ProposedAction};

/// Executor that handles Agent B's three tools: write_files, check_completeness, check_imports.
pub struct AgentBExecutor {
    pub output_dir: PathBuf,
    pub api_surface_json: String,
}

impl AgentBExecutor {
    pub fn new(output_dir: PathBuf, api_surface_json: String) -> Self {
        Self {
            output_dir,
            api_surface_json,
        }
    }
}

#[async_trait]
impl ActionExecutor for AgentBExecutor {
    async fn execute_actions(
        &self,
        actions: &[ProposedAction],
        _config: &LoopConfig,
        _circuit_breakers: &CircuitBreakerRegistry,
    ) -> Vec<Observation> {
        let mut observations = Vec::new();

        for action in actions {
            if let ProposedAction::ToolCall {
                call_id,
                name,
                arguments,
            } = action
            {
                let obs = match name.as_str() {
                    "write_files" => match write_files_to_dir(&self.output_dir, arguments) {
                        Ok(files) => {
                            let write_obs = Observation::tool_result(
                                "write_files",
                                format!("Wrote {} file(s): {}", files.len(), files.join(", ")),
                            )
                            .with_call_id(call_id.clone());
                            observations.push(write_obs);

                            // Auto-run check_completeness and check_imports after writing
                            let completeness =
                                check_completeness_impl(&self.output_dir, &self.api_surface_json);
                            observations.push(Observation::tool_result(
                                "check_completeness",
                                &completeness,
                            ));
                            let imports = check_imports_impl(&self.output_dir);
                            observations.push(Observation::tool_result("check_imports", &imports));
                            continue;
                        }
                        Err(e) => Observation::tool_error("write_files", e),
                    },
                    "check_completeness" => {
                        let result =
                            check_completeness_impl(&self.output_dir, &self.api_surface_json);
                        Observation::tool_result("check_completeness", result)
                    }
                    "check_imports" => {
                        let result = check_imports_impl(&self.output_dir);
                        Observation::tool_result("check_imports", result)
                    }
                    other => Observation::tool_error(other, format!("Unknown tool: {other}")),
                };
                observations.push(obs.with_call_id(call_id.clone()));
            }
        }

        observations
    }

    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "write_files".to_string(),
                description: "Write one or more files. Content uses ===FILE: path===...===END_FILE=== delimiters.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "File content in ===FILE: path===...===END_FILE=== format"
                        }
                    },
                    "required": ["content"]
                }),
            },
            ToolDefinition {
                name: "check_completeness".to_string(),
                description: "Compare generated files against the API surface. Reports missing exports.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                }),
            },
            ToolDefinition {
                name: "check_imports".to_string(),
                description: "Scan generated files for unresolved local imports.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// Public helper functions (usable from integration tests)
// ---------------------------------------------------------------------------

/// Parse `===FILE: path===...===END_FILE===` delimited content and write files.
///
/// Returns the list of relative paths written, or an error if a path contains `..`.
pub fn write_files_to_dir(output_dir: &Path, content: &str) -> Result<Vec<String>, String> {
    let mut files_written = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_content = String::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("===FILE: ") {
            // Flush previous file if any.
            if let Some(ref path) = current_path {
                flush_file(output_dir, path, &current_content, &mut files_written)?;
            }
            let path = rest.trim_end_matches('=').trim().to_string();
            if path.contains("..") {
                return Err(format!("Path traversal rejected: {path}"));
            }
            current_path = Some(path);
            current_content.clear();
        } else if line.starts_with("===END_FILE===") {
            if let Some(ref path) = current_path {
                flush_file(output_dir, path, &current_content, &mut files_written)?;
            }
            current_path = None;
            current_content.clear();
        } else if current_path.is_some() {
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        }
    }

    // Handle trailing file without ===END_FILE===
    if let Some(ref path) = current_path {
        flush_file(output_dir, path, &current_content, &mut files_written)?;
    }

    Ok(files_written)
}

fn flush_file(
    output_dir: &Path,
    rel_path: &str,
    content: &str,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let full = output_dir.join(rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir for {rel_path}: {e}"))?;
    }
    std::fs::write(&full, content).map_err(|e| format!("Failed to write {rel_path}: {e}"))?;
    written.push(rel_path.to_string());
    Ok(())
}

/// Check which API surface names are missing from the generated files.
pub fn check_completeness_impl(output_dir: &Path, api_surface_json: &str) -> String {
    // Collect expected names from the API surface JSON.
    let expected = match serde_json::from_str::<serde_json::Value>(api_surface_json) {
        Ok(val) => collect_names(&val),
        Err(e) => return format!("Failed to parse API surface JSON: {e}"),
    };

    if expected.is_empty() {
        return "No names found in API surface JSON.".to_string();
    }

    // Collect exported names from all .js/.ts files.
    let exported = collect_exports(output_dir);

    let missing: Vec<&String> = expected.iter().filter(|n| !exported.contains(*n)).collect();

    if missing.is_empty() {
        "All API surface names are covered.".to_string()
    } else {
        format!(
            "Missing {} name(s): {}",
            missing.len(),
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Scan for unresolved local imports in .js/.ts files.
pub fn check_imports_impl(output_dir: &Path) -> String {
    let files = collect_js_ts_files(output_dir);
    let mut unresolved: Vec<String> = Vec::new();

    for file_path in &files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let dir = file_path.parent().unwrap_or(output_dir);

        for line in content.lines() {
            if let Some(target) = extract_local_import(line) {
                let candidate = dir.join(&target);
                let candidate_js = dir.join(format!("{target}.js"));
                let candidate_index = dir.join(&target).join("index.js");
                if !candidate.exists() && !candidate_js.exists() && !candidate_index.exists() {
                    let rel = file_path
                        .strip_prefix(output_dir)
                        .unwrap_or(file_path)
                        .display();
                    unresolved.push(format!("{rel} -> {target}"));
                }
            }
        }
    }

    if unresolved.is_empty() {
        "All imports resolve.".to_string()
    } else {
        format!("Unresolved import(s):\n{}", unresolved.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Collect export names from the API surface JSON.
///
/// Tries the structured schema first (an `"exports"` array where each entry
/// has a `"name"` field, with optional `"static_methods"` and `"instance_methods"`
/// sub-arrays in the same shape). Falls back to the legacy approach of
/// recursively collecting all `"name"` string values for cached CSPs that
/// predate the schema change.
fn collect_names(val: &serde_json::Value) -> Vec<String> {
    if let Some(exports) = val.get("exports").and_then(|v| v.as_array()) {
        let mut names = Vec::new();
        collect_names_from_exports(exports, &mut names);
        if !names.is_empty() {
            return names;
        }
    }
    // Fallback: legacy recursive search for any "name" string values.
    let mut names = Vec::new();
    collect_names_recursive(val, &mut names);
    names
}

/// Extract names from the structured exports array, recursing into
/// static_methods and instance_methods.
fn collect_names_from_exports(exports: &[serde_json::Value], names: &mut Vec<String>) {
    for entry in exports {
        if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
            names.push(name.to_string());
        }
        for key in &["static_methods", "instance_methods"] {
            if let Some(methods) = entry.get(*key).and_then(|v| v.as_array()) {
                collect_names_from_exports(methods, names);
            }
        }
    }
}

/// Legacy: recursively collect all `"name"` string values from a JSON value.
fn collect_names_recursive(val: &serde_json::Value, names: &mut Vec<String>) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(n)) = map.get("name") {
                names.push(n.clone());
            }
            for v in map.values() {
                collect_names_recursive(v, names);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_names_recursive(v, names);
            }
        }
        _ => {}
    }
}

/// Collect exported names from .js/.ts files under `dir`.
fn collect_exports(dir: &Path) -> Vec<String> {
    let mut exports = Vec::new();
    for file_path in collect_js_ts_files(dir) {
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for line in content.lines() {
            let trimmed = line.trim();
            // module.exports = { ... } — skip, too complex to parse names
            // exports.X = ...
            if let Some(rest) = trimmed.strip_prefix("exports.") {
                if let Some(name) = rest.split(&['=', ' ', '('][..]).next() {
                    let name = name.trim();
                    if !name.is_empty() {
                        exports.push(name.to_string());
                    }
                }
            }
            // module.exports.X = ...
            if let Some(rest) = trimmed.strip_prefix("module.exports.") {
                if let Some(name) = rest.split(&['=', ' ', '('][..]).next() {
                    let name = name.trim();
                    if !name.is_empty() {
                        exports.push(name.to_string());
                    }
                }
            }
            // export function X, export class X, export const X
            for keyword in &["export function ", "export class ", "export const "] {
                if let Some(rest) = trimmed.strip_prefix(keyword) {
                    if let Some(name) = rest.split(&['(', ' ', '{', '=', ':'][..]).next() {
                        let name = name.trim();
                        if !name.is_empty() {
                            exports.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    exports
}

/// Extract a local import target from a line (require or ES import).
fn extract_local_import(line: &str) -> Option<String> {
    let trimmed = line.trim();

    // require('./...')
    for prefix in &["require('./", "require(\"./"] {
        if let Some(rest) = trimmed.strip_prefix(prefix).or_else(|| {
            // Could appear mid-line: const x = require('./...')
            trimmed.find(prefix).map(|i| &trimmed[i + prefix.len()..])
        }) {
            let closing = if prefix.contains('\'') { '\'' } else { '"' };
            if let Some(end) = rest.find(closing) {
                return Some(format!("./{}", &rest[..end]));
            }
        }
    }

    // import ... from './...'
    if trimmed.starts_with("import ") {
        for delim in &["from './", "from \"./"] {
            if let Some(idx) = trimmed.find(delim) {
                let start = idx + delim.len();
                let rest = &trimmed[start..];
                let closing = if delim.contains('\'') { '\'' } else { '"' };
                if let Some(end) = rest.find(closing) {
                    return Some(format!("./{}", &rest[..end]));
                }
            }
        }
    }

    None
}

/// Recursively collect .js and .ts files under `dir`.
fn collect_js_ts_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(collect_js_ts_files(&path));
            } else if let Some(ext) = path.extension() {
                if ext == "js" || ext == "ts" {
                    result.push(path);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tool_definitions_has_three_tools() {
        let tmp = TempDir::new().unwrap();
        let executor = AgentBExecutor::new(tmp.path().to_path_buf(), "{}".to_string());
        let defs = executor.tool_definitions();
        assert_eq!(defs.len(), 3);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"write_files"));
        assert!(names.contains(&"check_completeness"));
        assert!(names.contains(&"check_imports"));
    }

    #[test]
    fn test_write_files_creates_files() {
        let tmp = TempDir::new().unwrap();
        let content = "\
===FILE: src/index.js===
console.log('hello');
===END_FILE===
===FILE: src/utils.js===
module.exports = {};
===END_FILE===";
        let result = write_files_to_dir(tmp.path(), content).unwrap();
        assert_eq!(result.len(), 2);
        assert!(tmp.path().join("src/index.js").exists());
        assert!(tmp.path().join("src/utils.js").exists());
    }

    #[test]
    fn test_write_files_rejects_path_traversal() {
        let tmp = TempDir::new().unwrap();
        let content = "===FILE: ../etc/passwd===\nroot:x:0:0\n===END_FILE===";
        let result = write_files_to_dir(tmp.path(), content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Path traversal rejected"));
    }

    #[test]
    fn test_check_imports_all_resolve() {
        let tmp = TempDir::new().unwrap();
        let content = "\
===FILE: index.js===
const utils = require('./utils');
===END_FILE===
===FILE: utils.js===
module.exports = {};
===END_FILE===";
        write_files_to_dir(tmp.path(), content).unwrap();
        let result = check_imports_impl(tmp.path());
        assert_eq!(result, "All imports resolve.");
    }

    #[test]
    fn test_check_imports_finds_missing() {
        let tmp = TempDir::new().unwrap();
        let content = "\
===FILE: index.js===
const missing = require('./nonexistent');
===END_FILE===";
        write_files_to_dir(tmp.path(), content).unwrap();
        let result = check_imports_impl(tmp.path());
        assert!(
            result.contains("nonexistent"),
            "Should report missing import, got: {result}"
        );
    }
}
