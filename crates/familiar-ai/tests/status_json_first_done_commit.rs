//! `familiar-ai status --json` carries `first_done_commit` [draft name] (the owner's CI bar,
//! 2026-10-20): the commit at the first point in the record where every active part was done,
//! as the rules read it; null until then. It is history: a later commit does not move it.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "0f3e9d21-7c4b-4a58-b6e2-1d9c8a7b6e05";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").create()
}

impl Project {
    /// The intent and one part, c1, on src/a.sh greet.
    fn planned(&self) {
        self.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
        let path = self.path("src/a.sh").to_string_lossy().into_owned();
        let out = self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
        let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
        self.claim(&["intent_submitted", "--for", &t, "--text", "a says hello"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    }
}

fn state_of(j: &Value) -> &Value {
    &j["criteria"][0]["state_name"]
}

#[test]
fn status_json_names_the_first_commit_at_which_every_part_was_done() {
    let p = project("first-done");
    p.planned();
    let j = p.status_json();
    assert!(j.get("first_done_commit").is_some_and(Value::is_null), "planned: the field is there and null: {j}");

    // red: the test sealed failing at its commit, not done
    p.write("tests/test_c1.sh", "sh src/a.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    let j = p.status_json();
    assert_eq!(state_of(&j), "sealed", "{j}");
    assert_eq!(j["first_done_commit"], Value::Null, "{j}");

    // green: the fix lands and passes; that commit is the first done
    p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: the fix");
    let fix = p.head();
    let j = p.status_json();
    assert_eq!(state_of(&j), "green", "{j}");
    assert_eq!(j["first_done_commit"], fix.as_str(), "{j}");

    // a later commit does not move it
    p.write("README.md", "hello\n");
    p.commit("docs: a readme");
    let j = p.status_json();
    assert_ne!(p.head(), fix);
    assert_eq!(j["first_done_commit"], fix.as_str(), "{j}");
}
