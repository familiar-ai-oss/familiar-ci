//! A criterion with no command of its own, whose stack's runner the table does
//! not know, is unscoped and `status` says so on its line; with a known runner
//! it is scoped and says nothing.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    /// `familiar-ai status`, which must succeed.
    fn status_text(&self) -> String {
        let (code, out, err) = self.run(&["status"], "");
        assert_eq!(code, 0, "{err}");
        out
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

/// Claim a single criterion `c1`: an intent, a test file of its own, no
/// command of its own, and a stack whose runner is `stack`.
fn claim_c1(p: &Project, stack: &str) {
    p.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
    let reason = denied(&p.edit("src/a.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "a says hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "light"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_greet.py", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", stack, "--test_command", "false"]);
}

#[test]
fn an_unscoped_criterion_says_so() {
    // `mystery_runner` is not in the table, so c1 is unscoped
    let p = project("mystery");
    claim_c1(&p, "mystery_runner");
    let out = p.status_text();
    let line = out.lines().find(|l| l.contains("\"a says hello\"")).expect("c1's line");
    assert!(line.contains("unscoped · scope the command to prove in parallel"), "c1 is unscoped (mystery_runner is not in the table):\n{out}");

    // `pytest` is in the table, so c1 is scoped
    let p = project("pytest");
    claim_c1(&p, "pytest");
    let out = p.status_text();
    let line = out.lines().find(|l| l.contains("\"a says hello\"")).expect("c1's line");
    assert!(!line.contains("unscoped · scope the command to prove in parallel"), "c1 is scoped (pytest is in the table):\n{out}");
}
