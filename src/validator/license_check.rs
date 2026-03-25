use std::collections::HashMap;

pub fn check_license_file(files: &HashMap<String, String>) -> bool {
    files.keys().any(|k| {
        let name = k.rsplit('/').next().unwrap_or(k).to_lowercase();
        name == "license"
            || name == "license.md"
            || name == "license.txt"
            || name == "licence"
            || name == "licence.md"
            || name == "licence.txt"
            || name == "copying"
            || name == "copying.txt"
    })
}

pub fn check_license_header(content: &str, license_id: &str) -> bool {
    match license_id {
        "apache-2.0" => content.contains("Licensed under the Apache License"),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_present() {
        let mut files = HashMap::new();
        files.insert("LICENSE".into(), "MIT License\nCopyright...".into());
        files.insert("src/index.js".into(), "module.exports = {}".into());
        assert!(check_license_file(&files));
    }

    #[test]
    fn test_license_missing() {
        let mut files = HashMap::new();
        files.insert("src/index.js".into(), "module.exports = {}".into());
        assert!(!check_license_file(&files));
    }
}
