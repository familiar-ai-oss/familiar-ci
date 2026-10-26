//! Plan §1.2 (S8): a commit whose run is still going when the hook answers is
//! told "tests started — run r-…"; a run settled in the hook that is neither
//! green nor a valid red is told its OUTCOMES line, read from its translation
//! (the heuristic `did_not_run` retires).

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

/// c1 planned with `command`, the test file written, the tests-first commit not made.
fn planned(name: &str, inline_ms: &str, command: &str, test_body: &str) -> Project {
    let p = Project::builder(name).session(SESSION).no_app().file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").env("FAMILIAR_AI_RUN_INLINE_MS", inline_ms).create();
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
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", command]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", test_body);
    p
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

fn text<'a>(f: &'a Fact, key: &str) -> &'a str {
    f.fields.get(key).and_then(Value::as_str).unwrap_or("")
}

#[test]
fn a_run_still_going_when_the_hook_answers_is_told_started_with_its_id() {
    let p = planned("started", "0", "sh tests/test_c1.sh", "sleep 4\nexit 1\n");
    let out = p.commit("c1: tests first");
    let facts = p.facts();
    let id = facts.iter().find(|f| f.kind == "run_reserved").map(|f| text(f, "run_id").to_string()).expect("a run is reserved");
    assert!(context(&out).contains(&format!("familiar-ai: tests started — run {id}; the result is told when it ends")), "{out}");
}

#[test]
fn a_run_that_ended_in_the_hook_is_not_told_started() {
    let p = planned("ended", "60000", "sh tests/test_c1.sh", "exit 1\n");
    let out = p.commit("c1: tests first");
    assert!(!context(&out).contains("tests started"), "{out}");
}

#[test]
fn a_command_the_machine_does_not_have_is_told_its_outcome_line() {
    let p = planned("environment", "60000", "pytset-not-here -q", "exit 1\n");
    let out = p.commit("c1: tests first");
    let facts = p.facts();
    let t = facts.iter().find(|f| f.kind == "test_log_translation").expect("the run is translated");
    assert_eq!(text(t, "label"), "environment_error", "{t:?}");
    let line = context(&out);
    assert!(line.contains(&format!("familiar-ai: the test command did not run: {} · next: fix the command or the environment · run {}", text(t, "evidence"), text(t, "run_id"))), "{line}");
}
