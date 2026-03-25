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

/// Run generated tests inside a Docker container for sandboxed execution.
/// Returns None if Docker is not available.
pub async fn run_tests_in_docker(language: &str, pkg_dir: &Path) -> Option<TestResult> {
    // Check if Docker is available
    let docker_check = tokio::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .ok()?;

    if !docker_check.success() {
        return None;
    }

    let (image, install_cmd, test_cmd) = match language {
        "javascript" | "js" | "npm" => (
            "node:20-slim",
            "npm install --ignore-scripts 2>/dev/null; ",
            "npx --yes jest --passWithNoTests --no-coverage 2>&1 || node --test 2>&1",
        ),
        "typescript" | "ts" => (
            "node:20-slim",
            "npm install --ignore-scripts 2>/dev/null; ",
            "npx --yes jest --passWithNoTests --no-coverage 2>&1",
        ),
        "python" | "py" | "pypi" => (
            "python:3.12-slim",
            "pip install -q pytest 2>/dev/null; ",
            "python -m pytest -v 2>&1",
        ),
        "rust" => ("rust:1.78-slim", "", "cargo test -j2 2>&1"),
        "go" => ("golang:1.22-alpine", "", "go test ./... 2>&1"),
        _ => return None,
    };

    let abs_dir = pkg_dir.canonicalize().ok()?;
    let mount = format!("{}:/workspace", abs_dir.display());
    let full_cmd = format!("cd /workspace && {}{}", install_cmd, test_cmd);

    let output = tokio::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            "none", // No network access for safety
            "--memory",
            "512m", // Memory limit
            "--cpus",
            "1", // CPU limit
            "-v",
            &mount,
            "-w",
            "/workspace",
            image,
            "sh",
            "-c",
            &full_cmd,
        ])
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);

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
