//! The reporter's file reads a module missing where the project's own code imports it as the
//! environment's (`environment_error`), as the pytest tables do; one the test file imports itself
//! stays the test's (`collection_error`).

use familiar_ai::door::reported::read;

fn file(frames: &str) -> String {
    let session = r#"{"type": "session", "v": 1, "runner": "pytest", "runner_version": "9.1.1", "language": "python", "language_version": "3.14.4", "plugins": []}"#;
    let error = format!(r#"{{"type": "collect_error", "file": "test_greeting.py", "error": {{"type": "ModuleNotFoundError", "message": "No module named 'nosuchdep_zz'", "kind": "exception", "frames": [{frames}]}}}}"#);
    let end = r#"{"type": "end", "exit": 2, "counts": {"passed": 0, "failed": 0, "skipped": 0, "deselected": 0, "errors": 1, "xfailed": 0, "xpassed": 0}}"#;
    format!("{session}\n{error}\n{end}\n")
}

#[test]
fn a_module_the_projects_code_cannot_import_is_the_environment() {
    let r = read(&file(r#"{"file": "test_greeting.py", "line": 1, "function": "<module>"}, {"file": "src/greeting.py", "line": 1, "function": "<module>"}"#), Some(2), &[]).expect("read");
    assert_eq!(r.translation.label, "environment_error");
}

#[test]
fn a_module_the_test_file_cannot_import_is_the_tests() {
    let r = read(&file(r#"{"file": "test_greeting.py", "line": 1, "function": "<module>"}"#), Some(2), &[]).expect("read");
    assert_eq!(r.translation.label, "collection_error");
}

#[test]
fn a_test_file_named_from_the_runners_start_folder_is_still_the_test_file() {
    // unittest discover -s tests names the file from tests/, its frames from the project's root
    let r = read(&file(r#"{"file": "tests/test_greeting.py", "line": 1, "function": "<module>"}"#), Some(2), &[]).expect("read");
    assert_eq!(r.translation.label, "collection_error");
}
