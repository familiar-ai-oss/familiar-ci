//! The plain commands: `verify` with no committed plan never exits 0,
//! `--version` prints the crate's version, and two claim forms read right.

mod common;

use common::{run_binary, run_binary_in};

/// A repository with one commit and no `.familiar/plan`.
fn scratch_repo_with_no_plan(name: &str) -> common::Scratch {
    let repo = common::Scratch::new(name);
    repo.git_init();
    repo.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "base"]);
    repo
}

#[test]
fn verify_with_no_plan_committed_could_not_prove_anything_exit_2_one_line() {
    // this repo commits its plan; a scratch repository with one commit has none
    let repo = scratch_repo_with_no_plan("noplan");
    let output = run_binary_in(Some(&repo.dir), &["verify"], "");
    assert_eq!(common::exit_code(&output), 2);
    let out = common::stdout(&output);
    assert!(out.starts_with("familiar-ai: verify could not prove anything: "), "{out}");
    assert_eq!(out.trim_end_matches('\n').matches('\n').count(), 0, "one line: {out:?}");
}

#[test]
fn verify_never_exits_0_while_it_checks_nothing() {
    // even with hook input on stdin, verify must not pass
    let repo = scratch_repo_with_no_plan("nopass");
    let output = run_binary_in(Some(&repo.dir), &["verify"], r#"{"hook_event_name": "Stop"}"#);
    assert_eq!(common::exit_code(&output), 2);
}

#[test]
fn version_flag_prints_the_crates_version() {
    let output = run_binary(&["--version"], "");
    assert_eq!(common::exit_code(&output), 0);
    assert!(common::stdout(&output).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn line_8_says_a_touching_test_is_run_at_the_head_and_never_edited() {
    let output = run_binary(&["claim", "touching_test_declared"], "");
    let out = common::stdout(&output);
    assert!(
        out.contains("line 8: an existing test the change may break, run at the head and never edited by this criterion (none is allowed)"),
        "{out}"
    );
}

#[test]
fn line_6_says_modify_names_an_existing_test_file_the_tests_first_commit_edits() {
    let output = run_binary(&["claim", "test_planned"], "");
    let out = common::stdout(&output);
    assert!(
        out.contains("line 6: a test file the criterion will create, or modify when it names an existing test file the tests-first commit edits (one per file)"),
        "{out}"
    );
}
