pub mod npm;

use crate::ParsedManifest;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("unsupported manifest format: {0}")]
    Unsupported(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn parse_manifest(path: &Path) -> Result<ParsedManifest, ManifestError> {
    let content = std::fs::read_to_string(path)?;
    if npm::NpmParser::detect(path) {
        return npm::NpmParser::parse(&content);
    }
    Err(ManifestError::Unsupported(
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into()),
    ))
}
