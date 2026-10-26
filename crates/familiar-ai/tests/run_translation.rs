//! Each run that ran gets one `test_log_translation` (plan §1.1, §4, lane S4): the run's child
//! classifies its full cleaned capture in the same open of the record as `run_ended`; the
//! backstop classifies a `run_ended` left without one from its stored tails.

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai::door::runner::translate;
use familiar_ai_facts::{Record, RunEnded, RunReserved, TestLogTranslation};
use serde_json::{json, Value};

const SESSION: &str = "4c1e2a7b-0d93-4f61-9e2a-3b5c7d9f1e20";
const PYTEST_FAILED: &str = include_str!("fixtures/run_logs/pytest/tests_failed/stdout");
const NODE_BIG_RUN: &str = include_str!("fixtures/run_logs/node-test/big_run/stdout");

/// Plans criterion c1 with `command`, whose runner word is a script in the project that runs the
/// test file; the test prints `output` and exits 1. Then the tests-first commit.
fn committed(name: &str, runner: &str, output: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_RUN_INLINE_MS", "0").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let path = p.path("src/app.sh").to_string_lossy().into_owned();
    let out = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
    let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
    let command = format!("./{runner} tests/test_c1.sh");
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", &command]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write(runner, "#!/bin/sh\nexec sh \"$@\"\n");
    p.write("tests/out.txt", output);
    p.write("tests/test_c1.sh", "cat tests/out.txt\nexit 1\n");
    p.git(&["add", "-A"]);
    p.git(&["update-index", "--chmod=+x", "--", runner]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    p.bash("PostToolUse", "git commit -m 'c1: tests first'");
    p
}

fn waited(p: &Project, kind: &str) -> Vec<Value> {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        let found = p.facts_of(kind);
        if !found.is_empty() {
            return found;
        }
        assert!(Instant::now() < until, "gave up waiting for {kind}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_run_is_translated_once_by_its_runners_table() {
    let p = committed("translation", "pytest", PYTEST_FAILED);
    let ended = waited(&p, "run_ended");
    let t = waited(&p, "test_log_translation");
    assert_eq!(t.len(), 1, "{t:?}");
    let t = &t[0];
    assert_eq!(t["run_id"], ended[0]["run_id"]);
    assert_eq!((t["criterion_id"].as_str(), t["by"].as_str(), t["label"].as_str(), t["classified_on"].as_str()), (Some("c1"), Some("regex"), Some("tests_failed"), Some("full")));
    assert_eq!(t["commit"].as_str(), Some(p.head().as_str()));
    let evidence = t["evidence"].as_str().unwrap_or_default();
    assert!(PYTEST_FAILED.lines().any(|l| l == evidence) && !evidence.is_empty(), "one line of the output: {evidence:?}");
    assert_eq!(t["counts"], json!({"failed": 1}), "the summary's count, written on every new translation (lane H3, D7)");
    // a second writer, the backstop of a later call, writes none
    p.run(&["run"], "");
    assert_eq!(p.facts_of("test_log_translation").len(), 1);
}

#[test]
fn a_big_run_is_classified_on_its_full_output_and_stored_cut() {
    let p = committed("translation-big", "node", NODE_BIG_RUN);
    let ended = waited(&p, "run_ended");
    let t = waited(&p, "test_log_translation");
    let tail = ended[0]["stdout_tail"].as_str().unwrap_or_default();
    assert!(tail.contains(" chars dropped]\n"), "the record keeps the cut");
    assert_eq!((t[0]["label"].as_str(), t[0]["classified_on"].as_str()), (Some("tests_failed"), Some("full")));
    let evidence = t[0]["evidence"].as_str().unwrap_or_default();
    assert_eq!(evidence, "not ok 301 - add 2-60", "the deciding line from the middle");
    assert!(!tail.lines().any(|l| l == evidence), "which the cut dropped");
}

#[test]
fn two_writers_give_one_translation() {
    let dir = common::temp_dir("translation-two-writers");
    let _ = std::fs::create_dir_all(&dir);
    let record = Record::open(&dir.join("record")).expect("a record");
    let t = |by: &str, on: &str| TestLogTranslation {
        run_id: "r-1".into(),
        criterion_id: "c1".into(),
        commit: "abc".into(),
        by: by.into(),
        label: "tests_failed".into(),
        evidence: "FAILED tests/test_c1.py::test_greet".into(),
        failing_tests: vec![],
        classified_on: on.into(),
        location: None,
        missing_name: None,
        counts: None,
        ran_tests: None,
        skipped_tests: None,
        versions: None,
        frames: None,
    };
    assert!(translate(&record, &t("regex", "full"), 1.0).expect("written").is_some(), "the run's child");
    assert!(translate(&record, &t("regex", "cut"), 2.0).expect("written").is_none(), "the backstop finds it translated");
    let other = TestLogTranslation { run_id: "r-2".into(), ..t("regex", "cut") };
    assert!(translate(&record, &other, 3.0).expect("written").is_some(), "another run gets its own");
    let kinds: Vec<String> = record.read_all().expect("read").into_iter().filter(|f| f.kind == "test_log_translation").map(|f| f.fields["classified_on"].to_string()).collect();
    assert_eq!(kinds, ["\"full\"", "\"cut\""]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_backstop_translates_a_run_ended_left_without_one() {
    let p = committed("translation-backstop", "pytest", "");
    waited(&p, "test_log_translation");
    let head = p.head();
    {
        let record = Record::open_waiting(&p.record_path(), Duration::from_secs(10)).expect("the record");
        let reserved = RunReserved {
            run_id: "r-backstop".into(),
            criterion_id: "c1".into(),
            criterion_rev: "0".into(),
            commit: head.clone(),
            command: "pytest -q".into(),
            resources: vec![],
            wait: "60".into(),
            by: "binary".into(),
            after: None,
        };
        record.append(&reserved, 1.0, "_default_").expect("append");
        let ended = RunEnded { run_id: "r-backstop".into(), outcome: "failed".into(), end: "exited".into(), exit_code: Some(1), stdout_tail: Some(PYTEST_FAILED.to_string()), stderr_tail: Some(String::new()), ..Default::default() };
        record.append(&ended, 2.0, "_default_").expect("append");
    }
    p.run(&["run"], "");
    let mine: Vec<Value> = p.facts_of("test_log_translation").into_iter().filter(|t| t["run_id"] == "r-backstop").collect();
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!((mine[0]["label"].as_str(), mine[0]["classified_on"].as_str(), mine[0]["by"].as_str()), (Some("tests_failed"), Some("cut"), Some("regex")));
    assert_eq!(mine[0]["commit"].as_str(), Some(head.as_str()));
    assert_eq!(mine[0].get("counts"), None, "the backstop reads the cut: it writes no counts (lane H3)");
    p.run(&["run"], "");
    assert_eq!(p.facts_of("test_log_translation").into_iter().filter(|t| t["run_id"] == "r-backstop").count(), 1, "one per run");
}

/// The draft name `location.function` (the owner, 2026-10-15): the binary writes the function
/// git's hunk header names above the location (`funcname_above`), read at the run's commit, so
/// the place rule (plan §6.2) stays a pure reading.
#[test]
fn the_location_carries_the_function_above_it_at_the_runs_commit() {
    let p = Project::builder("translation-function").session(SESSION).env("FAMILIAR_AI_RUN_INLINE_MS", "0").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let path = p.path("src/app.sh").to_string_lossy().into_owned();
    let out = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
    let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", "check() {\n  farewell\n}\ncheck\n");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    p.bash("PostToolUse", "git commit -m 'c1: tests first'");
    let t = waited(&p, "test_log_translation");
    assert_eq!(t[0]["label"].as_str(), Some("missing_target_symbol"), "{t:?}");
    assert_eq!(t[0]["missing_name"].as_str(), Some("farewell"));
    // lane H3: the location keeps the kind of the row that found it, and counts are written ({} here)
    assert_eq!(t[0]["location"], json!({"file_path": "tests/test_c1.sh", "line": 2, "function": "check() {", "kind": "compile"}));
    assert_eq!(t[0]["counts"], json!({}));
}
