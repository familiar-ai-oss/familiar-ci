//! The per-test result file (plan A, JSON Lines v1), as Familiar's own pytest reporter writes it,
//! read first match wins into a translation `by: "reporter"` (lane a-py). The texts are the A0
//! spike's recordings under pytest-pretty (`probe_runs/pytest/pretty/P*`), the paths shortened.

use familiar_ai::door::reported::{read, Reported};
use familiar_ai_facts::LocationKind;

const SESSION: &str = r#"{"type": "session", "v": 1, "runner": "pytest", "runner_version": "9.1.1", "language": "python", "language_version": "3.14.4", "plugins": ["pytest-pretty-1.3.0", "pytest-xdist-3.8.0"], "xdist": false}"#;

fn end(exit: i32, passed: u64, failed: u64, skipped: u64) -> String {
    format!(r#"{{"type": "end", "exit": {exit}, "counts": {{"passed": {passed}, "failed": {failed}, "skipped": {skipped}, "deselected": 0, "errors": 0, "xfailed": 0, "xpassed": 0}}}}"#)
}

fn file(lines: &[&str]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn plan(files: &[&str]) -> Vec<String> {
    files.iter().map(|s| s.to_string()).collect()
}

fn got(text: &str, exit: i32, files: &[&str]) -> Reported {
    read(text, Some(exit), &plan(files)).expect("the reporter's file reads")
}

const P1: &str = r#"{"type": "test", "id": "tests/test_p1.py::test_add", "file": "tests/test_p1.py", "line": 4, "outcome": "passed", "skip_reason": null, "error": null}"#;
const P2: &str = r#"{"type": "test", "id": "tests/test_p2.py::test_add", "file": "tests/test_p2.py", "line": 4, "outcome": "failed", "skip_reason": null, "error": {"type": "AssertionError", "message": "assert 4 == 5\n +  where 4 = add(2, 2)", "kind": "assertion", "frames": [{"file": "tests/test_p2.py", "line": 5, "function": "test_add"}]}}"#;
const P3: &str = r#"{"type": "collect_error", "file": "tests/test_p3.py", "error": {"type": "ImportError", "message": "cannot import name 'mul' from 'calc' (/x/calc.py)", "kind": "missing_name", "frames": [{"file": "tests/test_p3.py", "line": 1, "function": "<module>"}]}}"#;
const P4: &str = r#"{"type": "collect_error", "file": "tests/test_p4.py", "error": {"type": "ModuleNotFoundError", "message": "No module named 'nosuch_helper'", "kind": "exception", "frames": [{"file": "tests/test_p4.py", "line": 1, "function": "<module>"}]}}"#;
const P5_PASS: &str = r#"{"type": "test", "id": "tests/test_p5.py::test_add", "file": "tests/test_p5.py", "line": 5, "outcome": "passed", "skip_reason": null, "error": null}"#;
const P5_SKIP: &str = r#"{"type": "test", "id": "tests/test_p5.py::test_later", "file": "tests/test_p5.py", "line": 9, "outcome": "skipped", "skip_reason": "Skipped: later", "error": null}"#;
const P7: &str = r#"{"type": "test", "id": "tests/test_p7.py::test_parse", "file": "tests/test_p7.py", "line": 4, "outcome": "failed", "skip_reason": null, "error": {"type": "ValueError", "message": "invalid literal for int() with base 10: 'x1'", "kind": "exception", "frames": [{"file": "tests/test_p7.py", "line": 5, "function": "test_parse"}, {"file": "calc.py", "line": 6, "function": "parse"}]}}"#;

#[test]
fn a_pass_reads_passed_with_its_counts_and_the_plans_tests() {
    let r = got(&file(&[SESSION, P1, &end(0, 1, 0, 0)]), 0, &["tests/test_p1.py"]);
    assert_eq!((r.translation.by, r.translation.label.as_str()), ("reporter", "passed"));
    assert_eq!(r.translation.counts.passed, Some(1));
    assert_eq!(r.ran_tests, ["tests/test_p1.py::test_add"]);
    assert!(r.skipped_tests.is_empty());
    assert!(r.versions.contains("pytest 9.1.1") && r.versions.contains("pytest-pretty-1.3.0"), "{}", r.versions);
}

#[test]
fn an_assertion_reads_tests_failed_at_the_assert_line() {
    let r = got(&file(&[SESSION, P2, &end(1, 0, 1, 0)]), 1, &["tests/test_p2.py"]);
    assert_eq!(r.translation.label, "tests_failed");
    assert_eq!(r.translation.failing_tests, ["tests/test_p2.py::test_add"]);
    let loc = r.translation.location.expect("the assert line");
    assert_eq!((loc.file_path.as_str(), loc.line, loc.kind), ("tests/test_p2.py", 5, Some(LocationKind::Assertion)));
    assert!(r.translation.evidence.contains("AssertionError"), "{}", r.translation.evidence);
}

#[test]
fn a_missing_name_at_collection_reads_missing_target_symbol_with_the_name() {
    let r = got(&file(&[SESSION, P3, &end(2, 0, 0, 0)]), 2, &["tests/test_p3.py"]);
    assert_eq!(r.translation.label, "missing_target_symbol");
    assert_eq!(r.translation.missing_name.as_deref(), Some("mul"));
}

#[test]
fn a_broken_import_reads_collection_error() {
    let r = got(&file(&[SESSION, P4, &end(2, 0, 0, 0)]), 2, &["tests/test_p4.py"]);
    assert_eq!(r.translation.label, "collection_error");
}

#[test]
fn a_pass_beside_a_skip_reads_passed_and_names_the_skipped_test() {
    let r = got(&file(&[SESSION, P5_PASS, P5_SKIP, &end(0, 1, 0, 1)]), 0, &["tests/test_p5.py"]);
    assert_eq!(r.translation.label, "passed");
    assert_eq!(r.translation.counts.skipped, Some(1));
    assert_eq!(r.ran_tests, ["tests/test_p5.py::test_add"]);
    assert_eq!(r.skipped_tests, ["tests/test_p5.py::test_later"]);
}

#[test]
fn ran_and_skipped_tests_are_filtered_to_the_plans_test_files() {
    let r = got(&file(&[SESSION, P1, P5_PASS, P5_SKIP, &end(0, 2, 0, 1)]), 0, &["tests/test_p1.py"]);
    assert_eq!(r.ran_tests, ["tests/test_p1.py::test_add"]);
    assert!(r.skipped_tests.is_empty());
}

#[test]
fn nothing_selected_reads_no_tests_collected() {
    let r = got(&file(&[SESSION, &end(5, 0, 0, 0)]), 5, &[]);
    assert_eq!(r.translation.label, "no_tests_collected");
}

#[test]
fn an_exception_in_the_code_reads_tests_failed_at_the_deepest_frame_first() {
    let r = got(&file(&[SESSION, P7, &end(1, 0, 1, 0)]), 1, &["tests/test_p7.py"]);
    assert_eq!(r.translation.label, "tests_failed");
    let first = &r.translation.candidates[0];
    assert_eq!((first.file_path.as_str(), first.line, first.kind), ("calc.py", 6, Some(LocationKind::Exception)));
    assert_eq!(r.translation.candidates[1].file_path, "tests/test_p7.py");
}

#[test]
fn no_session_or_no_end_line_is_for_the_next_reader() {
    assert!(read(&file(&[P1, &end(0, 1, 0, 0)]), Some(0), &[]).is_none(), "no session line");
    assert!(read(&file(&[SESSION, P1]), Some(0), &[]).is_none(), "no end line: the run aborted");
    assert!(read("", Some(0), &[]).is_none(), "no file");
    assert!(read(&file(&[SESSION, r#"{"type":"kit_error","message":"x"}"#, P1, &end(0, 1, 0, 0)]), Some(0), &[]).is_none(), "the kit failed");
    assert!(read(&file(&[SESSION, P1, &end(0, 1, 0, 0)]), None, &[]).is_none(), "a signal: the exit code decides");
}

#[test]
fn a_file_contradicting_the_exit_code_reads_cant_tell() {
    let r = got(&file(&[SESSION, P1, &end(0, 1, 0, 0)]), 1, &[]);
    assert_eq!(r.translation.label, "unclassified", "the file says all passed, the run exited 1");
    let r = got(&file(&[SESSION, P2, &end(0, 0, 1, 0)]), 0, &[]);
    assert_eq!(r.translation.label, "unclassified", "a failed test at exit 0");
}

#[test]
fn a_failing_exit_with_no_failure_in_the_file_reads_cant_tell() {
    let r = got(&file(&[SESSION, P1, &end(1, 1, 0, 0)]), 1, &[]);
    assert_eq!(r.translation.label, "unclassified", "exit 1 and every test passed: something the file does not show failed");
    let r = got(&file(&[SESSION, &end(5, 0, 0, 0)]), 5, &[]);
    assert_eq!(r.translation.label, "no_tests_collected", "pytest's 5 with nothing run is explained");
}

#[test]
fn a_truncated_or_doubled_file_reads_cant_tell() {
    let r = got(&file(&[SESSION, P1, P1, &end(0, 1, 0, 0)]), 0, &[]);
    assert_eq!(r.translation.label, "unclassified", "the same test twice");
    let r = got(&format!("{SESSION}\n{{\"type\": \"test\", \"id\"\n{}\n", end(0, 1, 0, 0)), 0, &[]);
    assert_eq!(r.translation.label, "unclassified", "a line that is not JSON");
}
