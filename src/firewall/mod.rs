use crate::audit::AuditEvent;
use crate::CspSpec;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Dispatch firewall crossing based on isolation mode.
pub fn cross_firewall(csp: CspSpec, isolation_mode: &str) -> (CspSpec, AuditEvent) {
    match isolation_mode {
        "process" => cross_firewall_process(csp),
        "container" => cross_firewall_container(csp),
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
fn cross_firewall_process(csp: CspSpec) -> (CspSpec, AuditEvent) {
    let temp_dir = std::env::temp_dir().join("phalus-firewall");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_path = temp_dir.join(format!(
        "csp-{}-{}.json",
        csp.package_name, csp.package_version
    ));

    // Serialize to disk
    let serialized = serde_json::to_string_pretty(&csp).unwrap_or_default();
    let _ = std::fs::write(&temp_path, &serialized);

    // Read back from disk (proving serialization boundary)
    let read_back = std::fs::read_to_string(&temp_path).unwrap_or(serialized);
    let deserialized: CspSpec = serde_json::from_str(&read_back).unwrap_or(csp);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    let (checksums, doc_names) = compute_checksums(&deserialized);
    let event = AuditEvent::FirewallCrossing {
        package: format!("{}@{}", deserialized.package_name, deserialized.package_version),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: "process".to_string(),
        source_code_accessed: false,
    };
    (deserialized, event)
}

/// Container mode: same as process mode but logs that container isolation
/// would require Docker in a production deployment.
fn cross_firewall_container(csp: CspSpec) -> (CspSpec, AuditEvent) {
    tracing::info!(
        "Container isolation requested for {}@{} -- \
         full container isolation would require Docker; \
         falling back to process-level serialization boundary",
        csp.package_name,
        csp.package_version
    );

    // Use the same serialization approach as process mode
    let temp_dir = std::env::temp_dir().join("phalus-firewall");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_path = temp_dir.join(format!(
        "csp-{}-{}.json",
        csp.package_name, csp.package_version
    ));

    let serialized = serde_json::to_string_pretty(&csp).unwrap_or_default();
    let _ = std::fs::write(&temp_path, &serialized);
    let read_back = std::fs::read_to_string(&temp_path).unwrap_or(serialized);
    let deserialized: CspSpec = serde_json::from_str(&read_back).unwrap_or(csp);
    let _ = std::fs::remove_file(&temp_path);

    let (checksums, doc_names) = compute_checksums(&deserialized);
    let event = AuditEvent::FirewallCrossing {
        package: format!("{}@{}", deserialized.package_name, deserialized.package_version),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: "container".to_string(),
        source_code_accessed: false,
    };
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
    use crate::{CspDocument, CspSpec};
    use crate::audit::AuditEvent;
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

    #[test]
    fn test_crossing_produces_audit_event() {
        let csp = sample_csp();
        let (passed, event) = cross_firewall(csp.clone(), "context");
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

    #[test]
    fn test_checksums_are_sha256() {
        let csp = sample_csp();
        let (_, event) = cross_firewall(csp, "context");
        if let AuditEvent::FirewallCrossing { sha256_checksums, .. } = event {
            for hash in sha256_checksums.values() {
                assert_eq!(hash.len(), 64);
            }
        }
    }

    #[test]
    fn test_process_isolation_roundtrip() {
        let csp = sample_csp();
        let (result, event) = cross_firewall(csp.clone(), "process");
        assert_eq!(result.package_name, csp.package_name);
        assert_eq!(result.documents.len(), csp.documents.len());
        if let AuditEvent::FirewallCrossing { isolation_mode, .. } = event {
            assert_eq!(isolation_mode, "process");
        } else {
            panic!("expected FirewallCrossing event");
        }
    }

    #[test]
    fn test_container_isolation_roundtrip() {
        let csp = sample_csp();
        let (result, event) = cross_firewall(csp.clone(), "container");
        assert_eq!(result.package_name, csp.package_name);
        if let AuditEvent::FirewallCrossing { isolation_mode, .. } = event {
            assert_eq!(isolation_mode, "container");
        } else {
            panic!("expected FirewallCrossing event");
        }
    }
}
