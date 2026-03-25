use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestRunError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub passed: u32,
    pub failed: u32,
    pub output: String,
}

/// Run generated tests for a package. Returns None if no test runner is available.
pub async fn run_generated_tests(language: &str, pkg_dir: &Path) -> Option<TestResult> {
    let (cmd, args) = match language {
        "javascript" | "js" | "npm" => (
            "npx",
            vec![
                "--yes",
                "jest",
                "--passWithNoTests",
                "--no-coverage",
            ],
        ),
        "typescript" | "ts" => (
            "npx",
            vec![
                "--yes",
                "jest",
                "--passWithNoTests",
                "--no-coverage",
            ],
        ),
        "rust" => ("cargo", vec!["test", "-j2", "--no-fail-fast"]),
        "python" | "py" | "pypi" => ("python", vec!["-m", "pytest", "-v"]),
        "go" => ("go", vec!["test", "./..."]),
        _ => return None,
    };

    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .current_dir(pkg_dir)
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);

    // Simple heuristic: count pass/fail from output
    if output.status.success() {
        Some(TestResult {
            passed: 1,
            failed: 0,
            output: combined,
        })
    } else {
        Some(TestResult {
            passed: 0,
            failed: 1,
            output: combined,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unsupported_language_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_generated_tests("cobol", dir.path()).await;
        assert!(result.is_none());
    }
}
