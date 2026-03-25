use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEvent {
    ManifestParsed {
        manifest_hash: String,
        package_count: usize,
    },
    DocsFetched {
        package: String,
        urls_accessed: Vec<String>,
        content_hashes: HashMap<String, String>,
    },
    SourceCodeBlocked {
        package: String,
        path: String,
        reason: String,
    },
    SpecGenerated {
        package: String,
        document_hashes: HashMap<String, String>,
        model: String,
        prompt_hash: String,
        symbiont_journal_hash: Option<String>,
    },
    FirewallCrossing {
        package: String,
        documents_transferred: Vec<String>,
        sha256_checksums: HashMap<String, String>,
        isolation_mode: String,
        source_code_accessed: bool,
    },
    ImplementationGenerated {
        package: String,
        file_hashes: HashMap<String, String>,
        model: String,
        prompt_hash: String,
        symbiont_journal_hash: Option<String>,
    },
    ValidationCompleted {
        package: String,
        syntax_ok: bool,
        tests_passed: Option<u32>,
        tests_failed: Option<u32>,
        similarity_score: f64,
        verdict: String,
    },
    SpecCacheHit {
        package: String,
        spec_hashes: HashMap<String, String>,
    },
    JobCompleted {
        packages_processed: usize,
        packages_failed: usize,
        total_elapsed_secs: f64,
        audit_log_hash: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub seq: u64,
    pub event: AuditEvent,
}

pub struct AuditLogger {
    path: PathBuf,
    writer: BufWriter<File>,
    seq: u64,
}

impl AuditLogger {
    pub fn new(path: PathBuf) -> Result<Self, AuditError> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let writer = BufWriter::new(file);
        Ok(Self {
            path,
            writer,
            seq: 0,
        })
    }

    pub fn log(&mut self, event: AuditEvent) -> Result<(), AuditError> {
        let entry = AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            seq: self.seq,
            event,
        };
        let line = serde_json::to_string(&entry)?;
        writeln!(self.writer, "{}", line)?;
        self.writer.flush()?;
        self.seq += 1;
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<String, AuditError> {
        self.writer.flush()?;
        let contents = std::fs::read(&self.path)?;
        let mut hasher = Sha256::new();
        hasher.update(&contents);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_log_event_writes_jsonl() {
        let file = NamedTempFile::new().unwrap();
        let mut logger = AuditLogger::new(file.path().to_path_buf()).unwrap();
        logger
            .log(AuditEvent::ManifestParsed {
                manifest_hash: "abc123".into(),
                package_count: 3,
            })
            .unwrap();

        let content = std::fs::read_to_string(file.path()).unwrap();
        let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(entry["event"]["type"], "manifest_parsed");
        assert_eq!(entry["event"]["package_count"], 3);
        assert!(entry["timestamp"].is_string());
        assert_eq!(entry["seq"], 0);
    }

    #[test]
    fn test_sequence_numbers_increment() {
        let file = NamedTempFile::new().unwrap();
        let mut logger = AuditLogger::new(file.path().to_path_buf()).unwrap();
        logger
            .log(AuditEvent::ManifestParsed {
                manifest_hash: "a".into(),
                package_count: 1,
            })
            .unwrap();
        logger
            .log(AuditEvent::ManifestParsed {
                manifest_hash: "b".into(),
                package_count: 2,
            })
            .unwrap();

        let content = std::fs::read_to_string(file.path()).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let e0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let e1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(e0["seq"], 0);
        assert_eq!(e1["seq"], 1);
    }

    #[test]
    fn test_finalize_produces_hash() {
        let file = NamedTempFile::new().unwrap();
        let mut logger = AuditLogger::new(file.path().to_path_buf()).unwrap();
        logger
            .log(AuditEvent::ManifestParsed {
                manifest_hash: "abc".into(),
                package_count: 1,
            })
            .unwrap();
        let hash = logger.finalize().unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex
    }
}
