use crate::audit::AuditEvent;
use crate::CspSpec;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn cross_firewall(csp: CspSpec, isolation_mode: &str) -> (CspSpec, AuditEvent) {
    let mut checksums = HashMap::new();
    let mut doc_names = Vec::new();
    for doc in &csp.documents {
        let hash = format!("{:x}", Sha256::digest(doc.content.as_bytes()));
        checksums.insert(doc.filename.clone(), hash);
        doc_names.push(doc.filename.clone());
    }
    let event = AuditEvent::FirewallCrossing {
        package: format!("{}@{}", csp.package_name, csp.package_version),
        documents_transferred: doc_names,
        sha256_checksums: checksums,
        isolation_mode: isolation_mode.to_string(),
        source_code_accessed: false,
    };
    (csp, event)
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
}
