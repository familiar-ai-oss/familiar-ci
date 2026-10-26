//! A second request after a finished run is planned and proved like the
//! first: the next edit of code asks for a new intent, then for a criterion.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

impl Project {
    fn must(&self, args: &[&str]) {
        let mut argv = vec!["claim"];
        argv.extend(args);
        let (code, out, err) = self.run(&argv, "");
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }

    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// Returns the `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

#[test]
fn every_request_is_proved_not_only_the_first() {
    let p = project("again");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let reason = denied(&p.edit("src/app.sh", "echo hi", "echo hello"));
    p.must(&["intent_submitted", "--for", &named_t(&reason), "--text", "greet says hello"]);
    p.must(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.must(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.must(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.must(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.must(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.must(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.must(&["criterion_head", "--criterion_id", "c1"]);
    p.must(&["criterion_assigned", "--criterion_id", "c1"]);
    p.must(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    std::fs::create_dir_all(p.repo().join("tests")).expect("tests");
    std::fs::write(p.repo().join("tests/test_c1.sh"), "sh src/app.sh | grep -qx hello\n").expect("test");
    p.commit("c1: tests first");
    std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hello; }\ngreet\n").expect("fix");
    assert!(p.commit("c1: the fix").to_string().contains("they pass · stamped"));

    // run 1 is done: a message is conversation and the stop goes
    p.hook("UserPromptSubmit", json!({ "prompt": "thanks, looks good" }));
    assert!(p.hook("Stop", json!({})).get("decision").is_none(), "no run is open: the stop goes");
    let (_, status, _) = p.run(&["status"], "");
    assert!(!status.contains("nothing answers it yet"), "nothing must answer conversation: {status}");

    // a new request: the first edit asks for a new intent
    p.hook("UserPromptSubmit", json!({ "prompt": "now say hello world" }));
    let reason = denied(&p.edit("src/app.sh", "echo hello", "echo hello world"));
    assert!(reason.starts_with("familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code\n"), "{reason}");
    assert!(reason.contains("\"now say hello world\""), "{reason}");
    p.must(&["intent_submitted", "--for", &named_t(&reason), "--text", "greet says hello world"]);

    // the plan's lines are still held from run 1, so the new run needs its own criterion
    let reason = denied(&p.edit("src/app.sh", "echo hello", "echo hello world"));
    assert!(reason.starts_with("familiar-ai: the plan comes first · next: write the criterion\n  claim it: familiar-ai claim criterion --criterion_id c2"), "{reason}");

    p.must(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.must(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.must(&["criterion", "--criterion_id", "c2", "--text", "greet says hello world", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.must(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.must(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"]);
    p.must(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    p.must(&["criterion_assigned", "--criterion_id", "c2"]);
    p.must(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
    let out = p.edit("tests/test_c2.sh", "", "");
    assert!(out["hookSpecificOutput"]["permissionDecision"].is_null(), "run 2 is planned: its test may be written: {out}");
}
