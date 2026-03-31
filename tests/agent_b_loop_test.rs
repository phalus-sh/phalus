use phalus::agents::agent_b_executor::{check_imports_impl, write_files_to_dir};
use tempfile::TempDir;

#[test]
fn test_write_files_and_check_imports_all_resolve() {
    let dir = TempDir::new().unwrap();
    let content = r#"===FILE: src/index.js===
const utils = require('./utils');
module.exports = { greet: utils.greet };
===END_FILE===

===FILE: src/utils.js===
function greet(name) { return "Hello, " + name; }
module.exports = { greet };
===END_FILE===

===FILE: package.json===
{"name": "test", "main": "src/index.js"}
===END_FILE==="#;

    let files = write_files_to_dir(dir.path(), content).unwrap();
    assert_eq!(files.len(), 3);

    let report = check_imports_impl(dir.path());
    assert!(
        report.contains("All imports resolve"),
        "Expected all imports to resolve, got: {}",
        report
    );
}

#[test]
fn test_write_files_unresolved_import() {
    let dir = TempDir::new().unwrap();
    let content = r#"===FILE: src/index.js===
const missing = require('./nonexistent');
module.exports = { foo: missing.foo };
===END_FILE==="#;

    write_files_to_dir(dir.path(), content).unwrap();

    let report = check_imports_impl(dir.path());
    assert!(
        report.contains("nonexistent"),
        "Expected unresolved import report, got: {}",
        report
    );
}

#[test]
fn test_write_files_nested_directories() {
    let dir = TempDir::new().unwrap();
    let content = r#"===FILE: lib/core/main.js===
const helper = require('../helpers/utils');
module.exports = helper;
===END_FILE===

===FILE: lib/helpers/utils.js===
module.exports = { help: true };
===END_FILE==="#;

    let files = write_files_to_dir(dir.path(), content).unwrap();
    assert_eq!(files.len(), 2);
    assert!(dir.path().join("lib/core/main.js").exists());
    assert!(dir.path().join("lib/helpers/utils.js").exists());

    let report = check_imports_impl(dir.path());
    assert!(
        report.contains("All imports resolve"),
        "Expected all imports to resolve, got: {}",
        report
    );
}
