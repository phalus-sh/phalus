use crate::audit::AuditEvent;
use crate::CspSpec;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Configuration for container-mode firewall isolation.
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Docker image to use for the isolation container
    pub image: String,
    /// Memory limit passed to `docker run --memory`
    pub memory_limit: String,
    /// CPU limit passed to `docker run --cpus`
    pub cpu_limit: String,
    /// Seconds before the container run is forcibly killed
    pub timeout_secs: u64,
    /// Docker network mode (`none`, `host`, …)
    pub network_mode: String,
    /// Maximum PIDs inside the container
    pub pids_limit: u32,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: "alpine:3".to_string(),
            memory_limit: "256m".to_string(),
            cpu_limit: "1.0".to_string(),
            timeout_secs: 60,
            network_mode: "none".to_string(),
            pids_limit: 64,
        }
    }
}

/// Dispatch firewall crossing based on isolation mode.
pub async fn cross_firewall(
    csp: CspSpec,
    isolation_mode: &str,
    container_cfg: &ContainerConfig,
) -> (CspSpec, AuditEvent) {
    match isolation_mode {
        "process" => cross_firewall_process(csp).await,
        "container" => cross_firewall_container(csp, container_cfg).await,
        _ => cross_firewall_context(csp),
    }
}

/// Context mode: CSP passes through in the same process memory.
fn cross_firewall_context(csp: CspSpec) -> (CspSpec, AuditEvent) {
    let (checksums, doc_names) = compute_checksums(&csp);
    let event = AuditEvent::FirewallCrossing {
        package: format!("{}@{}", csp.package_name, csp.package_version),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: "context".to_string(),
        source_code_accessed: false,
    };
    (csp, event)
}

/// Process mode: serialize CSP to a temp file and read it back, proving
/// the data crosses a serialization boundary (as it would with a real
/// separate-process Agent B).
async fn cross_firewall_process(csp: CspSpec) -> (CspSpec, AuditEvent) {
    let temp_dir = std::env::temp_dir().join("phalus-firewall");
    let _ = tokio::fs::create_dir_all(&temp_dir).await;
    let run_id = uuid::Uuid::new_v4();
    let temp_path = temp_dir.join(format!("csp-{}.json", run_id));

    // Serialize to disk
    let serialized = match serde_json::to_string_pretty(&csp) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("process firewall: failed to serialize CSP: {}", e);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return cross_firewall_context(csp);
        }
    };
    if let Err(e) = tokio::fs::write(&temp_path, &serialized).await {
        tracing::error!("process firewall: failed to write temp file: {}", e);
        return cross_firewall_context(csp);
    }

    // Read back from disk (proving serialization boundary)
    let read_back = match tokio::fs::read_to_string(&temp_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("process firewall: failed to read temp file: {}", e);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return cross_firewall_context(csp);
        }
    };
    let deserialized: CspSpec = match serde_json::from_str(&read_back) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("process firewall: failed to deserialize CSP from disk: {}", e);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return cross_firewall_context(csp);
        }
    };

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    let (checksums, doc_names) = compute_checksums(&deserialized);
    let event = AuditEvent::FirewallCrossing {
        package: format!(
            "{}@{}",
            deserialized.package_name, deserialized.package_version
        ),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: "process".to_string(),
        source_code_accessed: false,
    };
    (deserialized, event)
}

/// Container mode: run a Docker container to prove the CSP crosses a true
/// isolation boundary. The CSP is written to a read-only input volume, the
/// container copies it to a writable output volume, and the host reads it
/// back. Resource limits, network isolation, and a hard timeout are applied.
/// Falls back to process-level isolation when Docker is unavailable.
async fn cross_firewall_container(csp: CspSpec, cfg: &ContainerConfig) -> (CspSpec, AuditEvent) {
    // Check Docker availability before doing any disk work.
    let docker_available = tokio::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if !docker_available {
        tracing::warn!(
            "Docker not available for {}@{} — falling back to process isolation",
            csp.package_name,
            csp.package_version
        );
        let (csp, mut evt) = cross_firewall_process(csp).await;
        if let AuditEvent::FirewallCrossing {
            ref mut isolation_mode,
            ..
        } = evt
        {
            *isolation_mode = "container-fallback".to_string();
        }
        return (csp, evt);
    }

    // Create per-run temp directories using a UUID to avoid collisions.
    let run_id = uuid::Uuid::new_v4();
    let base_dir = std::env::temp_dir().join(format!("phalus-fw-{}", run_id));
    let input_dir = base_dir.join("input");
    let output_dir = base_dir.join("output");

    let cleanup = |base: &std::path::Path| {
        let _ = std::fs::remove_dir_all(base);
    };

    if let Err(e) = tokio::fs::create_dir_all(&input_dir).await {
        tracing::error!("container firewall: failed to create input dir: {}", e);
        return cross_firewall_process(csp).await;
    }
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        tracing::error!("container firewall: failed to create output dir: {}", e);
        cleanup(&base_dir);
        return cross_firewall_process(csp).await;
    }

    let safe_name = csp
        .package_name
        .replace(['/', '\\'], "_")
        .replace("..", "_");
    let safe_version = csp
        .package_version
        .replace(['/', '\\'], "_")
        .replace("..", "_");
    let csp_filename = format!("csp-{}-{}.json", safe_name, safe_version);
    let input_path = input_dir.join(&csp_filename);

    // Serialize CSP into the input volume.
    let serialized = match serde_json::to_string_pretty(&csp) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("container firewall: failed to serialize CSP: {}", e);
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
    };
    if let Err(e) = tokio::fs::write(&input_path, &serialized).await {
        tracing::error!("container firewall: failed to write input volume: {}", e);
        cleanup(&base_dir);
        return cross_firewall_process(csp).await;
    }

    // Docker requires absolute paths for volume mounts.
    let input_abs = match input_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("container firewall: canonicalize input dir: {}", e);
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
    };
    let output_abs = match output_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("container firewall: canonicalize output dir: {}", e);
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
    };

    let input_mount = format!("{}:/input:ro", input_abs.display());
    let output_mount = format!("{}:/output:rw", output_abs.display());
    // Copy exactly the CSP file across the Docker boundary.
    // Pass cp arguments directly (no shell) to prevent shell injection via
    // package name or version containing metacharacters like $(), backticks, etc.
    let input_file = format!("/input/{}", csp_filename);
    let output_file = format!("/output/{}", csp_filename);

    tracing::info!(
        "container firewall: starting container for {}@{} (image={}, network={}, memory={}, cpus={})",
        csp.package_name,
        csp.package_version,
        cfg.image,
        cfg.network_mode,
        cfg.memory_limit,
        cfg.cpu_limit,
    );

    let run_future = tokio::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &input_mount,
            "-v",
            &output_mount,
            "--network",
            &cfg.network_mode,
            "--memory",
            &cfg.memory_limit,
            "--cpus",
            &cfg.cpu_limit,
            "--pids-limit",
            &cfg.pids_limit.to_string(),
            "--stop-timeout",
            &cfg.timeout_secs.to_string(),
            &cfg.image,
            "cp",
            &input_file,
            &output_file,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    let run_result =
        tokio::time::timeout(std::time::Duration::from_secs(cfg.timeout_secs), run_future).await;

    match run_result {
        Err(_elapsed) => {
            tracing::error!(
                "container firewall: container timed out after {}s for {}@{}",
                cfg.timeout_secs,
                csp.package_name,
                csp.package_version,
            );
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
        Ok(Err(e)) => {
            tracing::error!("container firewall: docker run I/O error: {}", e);
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
        Ok(Ok(output)) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(
                "container firewall: container exited {:?} for {}@{}: {}",
                output.status.code(),
                csp.package_name,
                csp.package_version,
                stderr.trim(),
            );
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
        Ok(Ok(_)) => {}
    }

    // Read the CSP back from the container's output volume.
    let output_path = output_dir.join(&csp_filename);
    let content = match tokio::fs::read_to_string(&output_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("container firewall: failed to read output volume: {}", e);
            cleanup(&base_dir);
            return cross_firewall_process(csp).await;
        }
    };
    cleanup(&base_dir);

    let deserialized: CspSpec = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "container firewall: failed to deserialize output CSP: {}",
                e
            );
            return cross_firewall_process(csp).await;
        }
    };

    let (checksums, doc_names) = compute_checksums(&deserialized);
    let event = AuditEvent::FirewallCrossing {
        package: format!(
            "{}@{}",
            deserialized.package_name, deserialized.package_version
        ),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: "container".to_string(),
        source_code_accessed: false,
    };

    tracing::info!(
        "container firewall: CSP for {}@{} transferred through Docker boundary",
        deserialized.package_name,
        deserialized.package_version,
    );

    (deserialized, event)
}

fn compute_checksums(csp: &CspSpec) -> (HashMap<String, String>, Vec<String>) {
    let mut checksums = HashMap::new();
    let mut doc_names = Vec::new();
    for doc in &csp.documents {
        let hash = format!("{:x}", Sha256::digest(doc.content.as_bytes()));
        checksums.insert(doc.filename.clone(), hash);
        doc_names.push(doc.filename.clone());
    }
    (checksums, doc_names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditEvent;
    use crate::{CspDocument, CspSpec};
    use chrono::Utc;

    fn sample_csp() -> CspSpec {
        CspSpec {
            package_name: "lodash".into(),
            package_version: "4.17.21".into(),
            documents: vec![
                CspDocument {
                    filename: "01-overview.md".into(),
                    content: "Lodash utilities".into(),
                    content_hash: "aaa".into(),
                },
                CspDocument {
                    filename: "02-api-surface.json".into(),
                    content: "{}".into(),
                    content_hash: "bbb".into(),
                },
            ],
            generated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_crossing_produces_audit_event() {
        let csp = sample_csp();
        let cfg = ContainerConfig::default();
        let (passed, event) = cross_firewall(csp.clone(), "context", &cfg).await;
        assert_eq!(passed.documents.len(), csp.documents.len());
        match event {
            AuditEvent::FirewallCrossing {
                package,
                documents_transferred,
                sha256_checksums,
                isolation_mode,
                source_code_accessed,
            } => {
                assert_eq!(package, "lodash@4.17.21");
                assert_eq!(documents_transferred.len(), 2);
                assert_eq!(sha256_checksums.len(), 2);
                assert_eq!(isolation_mode, "context");
                assert!(!source_code_accessed);
            }
            _ => panic!("expected FirewallCrossing event"),
        }
    }

    #[tokio::test]
    async fn test_checksums_are_sha256() {
        let csp = sample_csp();
        let cfg = ContainerConfig::default();
        let (_, event) = cross_firewall(csp, "context", &cfg).await;
        if let AuditEvent::FirewallCrossing {
            sha256_checksums, ..
        } = event
        {
            for hash in sha256_checksums.values() {
                assert_eq!(hash.len(), 64);
            }
        }
    }

    #[tokio::test]
    async fn test_process_isolation_roundtrip() {
        let csp = sample_csp();
        let cfg = ContainerConfig::default();
        let (result, event) = cross_firewall(csp.clone(), "process", &cfg).await;
        assert_eq!(result.package_name, csp.package_name);
        assert_eq!(result.documents.len(), csp.documents.len());
        if let AuditEvent::FirewallCrossing { isolation_mode, .. } = event {
            assert_eq!(isolation_mode, "process");
        } else {
            panic!("expected FirewallCrossing event");
        }
    }

    #[tokio::test]
    async fn test_container_isolation_roundtrip() {
        let csp = sample_csp();
        let cfg = ContainerConfig::default();
        let (result, event) = cross_firewall(csp.clone(), "container", &cfg).await;
        assert_eq!(result.package_name, csp.package_name);
        assert_eq!(result.documents.len(), csp.documents.len());
        if let AuditEvent::FirewallCrossing { isolation_mode, .. } = event {
            assert!(
                isolation_mode == "container" || isolation_mode == "container-fallback",
                "unexpected isolation_mode: {}",
                isolation_mode
            );
        } else {
            panic!("expected FirewallCrossing event");
        }
    }

    /// Verify that container mode preserves all CSP document content exactly.
    #[tokio::test]
    async fn test_container_isolation_content_integrity() {
        let csp = sample_csp();
        let cfg = ContainerConfig::default();
        let (result, _event) = cross_firewall(csp.clone(), "container", &cfg).await;
        for (orig, transferred) in csp.documents.iter().zip(result.documents.iter()) {
            assert_eq!(orig.filename, transferred.filename);
            assert_eq!(orig.content, transferred.content);
        }
    }

    /// Verify that container mode with a very short timeout falls back gracefully.
    #[tokio::test]
    async fn test_container_timeout_falls_back() {
        let csp = sample_csp();
        // A 0-second timeout should cause the run to time out immediately when
        // Docker is available, or be ignored during the docker-info check and
        // fall back via the unavailability path.
        let cfg = ContainerConfig {
            timeout_secs: 0,
            ..ContainerConfig::default()
        };
        let (result, event) = cross_firewall(csp.clone(), "container", &cfg).await;
        // Either path should return the original CSP intact.
        assert_eq!(result.package_name, csp.package_name);
        assert_eq!(result.documents.len(), csp.documents.len());
        if let AuditEvent::FirewallCrossing { isolation_mode, .. } = event {
            // Could be container, container-fallback, or process depending on
            // Docker availability and whether the timeout fires first.
            assert!(
                isolation_mode == "container"
                    || isolation_mode == "container-fallback"
                    || isolation_mode == "process",
                "unexpected isolation_mode: {}",
                isolation_mode
            );
        }
    }

    /// When Docker is unavailable, container mode must fall back and still
    /// produce a valid FirewallCrossing event.
    #[tokio::test]
    async fn test_container_fallback_produces_valid_event() {
        let csp = sample_csp();
        // Use a deliberately non-existent image so even if Docker is present
        // the container run will fail and trigger the fallback path.
        let cfg = ContainerConfig {
            image: "phalus-nonexistent-image-for-testing:latest".to_string(),
            ..ContainerConfig::default()
        };
        let (result, event) = cross_firewall(csp.clone(), "container", &cfg).await;
        assert_eq!(result.package_name, csp.package_name);
        if let AuditEvent::FirewallCrossing {
            isolation_mode,
            sha256_checksums,
            documents_transferred,
            ..
        } = event
        {
            // Fallback or real container — either is acceptable.
            assert!(
                isolation_mode == "container"
                    || isolation_mode == "container-fallback"
                    || isolation_mode == "process",
                "unexpected isolation_mode: {}",
                isolation_mode
            );
            assert_eq!(sha256_checksums.len(), csp.documents.len());
            assert_eq!(documents_transferred.len(), csp.documents.len());
        } else {
            panic!("expected FirewallCrossing event");
        }
    }
}
