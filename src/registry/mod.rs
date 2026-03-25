pub mod crates;
pub mod golang;
pub mod npm;
pub mod pypi;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("package not found: {name}@{version}")]
    NotFound { name: String, version: String },
    #[error("package too large: {size_mb:.1} MB (limit: {limit_mb} MB)")]
    TooLarge { size_mb: f64, limit_mb: u64 },
    #[error("parse error: {0}")]
    Parse(String),
}
