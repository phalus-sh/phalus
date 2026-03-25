use std::path::PathBuf;

use thiserror::Error;

use crate::CspSpec;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct CspCache {
    dir: PathBuf,
}

impl CspCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn default_cache() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".phalus")
            .join("cache")
            .join("csp");
        Self { dir }
    }

    fn cache_path(&self, name: &str, version: &str, content_hash: &str) -> PathBuf {
        self.dir.join(format!("{name}@{version}-{content_hash}.json"))
    }

    pub fn get(&self, name: &str, version: &str, content_hash: &str) -> Option<CspSpec> {
        let path = self.cache_path(name, version, content_hash);
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn put(
        &self,
        name: &str,
        version: &str,
        content_hash: &str,
        csp: &CspSpec,
    ) -> Result<(), CacheError> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.cache_path(name, version, content_hash);
        let json = serde_json::to_string_pretty(csp)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CspDocument, CspSpec};
    use chrono::Utc;
    use tempfile::TempDir;

    fn sample_csp() -> CspSpec {
        CspSpec {
            package_name: "test-pkg".into(),
            package_version: "1.0.0".into(),
            documents: vec![CspDocument {
                filename: "01-overview.md".into(),
                content: "test".into(),
                content_hash: "abc".into(),
            }],
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn test_cache_miss() {
        let dir = TempDir::new().unwrap();
        let cache = CspCache::new(dir.path().to_path_buf());
        assert!(cache.get("test-pkg", "1.0.0", "hash123").is_none());
    }

    #[test]
    fn test_cache_hit() {
        let dir = TempDir::new().unwrap();
        let cache = CspCache::new(dir.path().to_path_buf());
        let csp = sample_csp();
        cache.put("test-pkg", "1.0.0", "hash123", &csp).unwrap();
        let cached = cache.get("test-pkg", "1.0.0", "hash123");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().package_name, "test-pkg");
    }

    #[test]
    fn test_cache_different_hash_misses() {
        let dir = TempDir::new().unwrap();
        let cache = CspCache::new(dir.path().to_path_buf());
        let csp = sample_csp();
        cache.put("test-pkg", "1.0.0", "hash123", &csp).unwrap();
        assert!(cache.get("test-pkg", "1.0.0", "different-hash").is_none());
    }
}
