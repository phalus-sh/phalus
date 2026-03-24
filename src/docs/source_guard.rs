pub fn is_source_code(path: &str) -> bool {
    let path = path.replace('\\', "/");

    // Blocked test/spec directory prefixes
    let blocked_dirs = ["test/", "tests/", "__tests__/", "spec/"];
    for dir in &blocked_dirs {
        if path.starts_with(dir) || path.contains(&format!("/{}", dir)) {
            return true;
        }
    }

    // Allow .d.ts files before checking .ts extension
    if path.ends_with(".d.ts") {
        return false;
    }

    // Blocked source extensions
    let blocked_extensions = [
        ".js", ".py", ".rs", ".go", ".java", ".rb", ".php", ".c", ".cpp", ".cc", ".h", ".cs",
        ".ts",
    ];
    for ext in &blocked_extensions {
        if path.ends_with(ext) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_js_files() {
        assert!(is_source_code("lib/index.js"));
        assert!(is_source_code("src/utils.ts"));
        assert!(is_source_code("main.py"));
        assert!(is_source_code("lib.rs"));
        assert!(is_source_code("main.go"));
        assert!(is_source_code("App.java"));
        assert!(is_source_code("helper.rb"));
        assert!(is_source_code("index.php"));
        assert!(is_source_code("main.c"));
        assert!(is_source_code("lib.cpp"));
        assert!(is_source_code("util.cc"));
        assert!(is_source_code("header.h"));
        assert!(is_source_code("Program.cs"));
    }

    #[test]
    fn test_allows_dts_files() {
        assert!(!is_source_code("index.d.ts"));
        assert!(!is_source_code("types/lodash.d.ts"));
    }

    #[test]
    fn test_allows_docs() {
        assert!(!is_source_code("README.md"));
        assert!(!is_source_code("README.rst"));
        assert!(!is_source_code("CHANGELOG.md"));
        assert!(!is_source_code("docs/api.md"));
    }

    #[test]
    fn test_blocks_test_directories() {
        assert!(is_source_code("test/helper.js"));
        assert!(is_source_code("tests/unit.py"));
        assert!(is_source_code("__tests__/foo.js"));
        assert!(is_source_code("spec/bar.rb"));
    }

    #[test]
    fn test_blocks_test_dirs_even_for_non_source() {
        assert!(is_source_code("test/fixtures.json"));
        assert!(is_source_code("__tests__/snapshots/foo.txt"));
    }
}
