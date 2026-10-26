//! A runnable criterion is run at every tip, even after a commit that touches
//! nothing of it, and no `fingerprint_stamped` is written without a
//! `pass_please_pass` at its commit.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use familiar_ai_rules::lifecycle::state;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const FLAG: &str = "src/flag.txt";
const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").file(FLAG, "no\n").create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

/// Every laptop stamp has a laptop result of the same criterion at its commit.
fn no_stamp_without_a_result(log: &[Fact]) {
    let at = |f: &Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    for s in log.iter().filter(|f| f.kind == "fingerprint_stamped" && at(f, "ran_in") == "laptop") {
        let (c, commit) = (at(s, "criterion_id"), at(s, "commit"));
        let ran = log.iter().any(|r| {
            r.kind == "pass_please_pass"
                && at(r, "criterion_id") == c
                && at(r, "commit") == commit
                && at(r, "ran_in") == "laptop"
                && !r.fields.get("with_anchor_reverted").and_then(Value::as_bool).unwrap_or(false)
        });
        assert!(ran, "{c} is stamped at {commit} with no result there");
    }
}

#[test]
fn a_criterion_is_run_at_every_tip() {
    let p = project("tip");
    p.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
    let reason = denied(&p.edit("src/a.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "a says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    // the test reads the flag, which lies outside the anchor
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "false"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        p.write("tests/test_c1.sh", "sh src/a.sh | grep -qx hello && grep -qx yes src/flag.txt\n");
    p.commit("c1: tests first");
    let v0 = p.facts();
    assert_eq!(state(&View::new(&v0), "c1"), "sealed", "the tests commit is red: sealed");

    // the fix lands, but the flag still says no: the run there is red
    p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: the fix");
    let fix = p.head();
    let log = p.facts();
    assert_ne!(state(&View::new(&log), "c1"), "green", "the fix's run came out red");
    no_stamp_without_a_result(&log);

    // a commit that touches nothing of c1: the tip is run all the same
    p.write(FLAG, "yes\n");
    p.commit("the flag");
    let tip = p.head();
    assert_ne!(tip, fix);
    let log = p.facts();
    no_stamp_without_a_result(&log);
    let ran_at_tip = log.iter().any(|f| {
        f.kind == "pass_please_pass" && f.fields.get("commit").and_then(Value::as_str) == Some(tip.as_str()) && f.fields.get("criterion_id").and_then(Value::as_str) == Some("c1")
    });
    assert!(ran_at_tip, "c1 has a result at the tip, though the tip touches nothing of it");
    assert_eq!(state(&View::new(&log), "c1"), "green", "its test passes at the tip");
}
