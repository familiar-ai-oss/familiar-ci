//! `familiar-ai status --json` is the same picture `status` prints, as one
//! JSON object, and SessionStart's first line names the mode.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
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

/// Returns the `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().or_else(|| out["systemMessage"].as_str()).map_or_else(|| out.to_string(), str::to_string)
}

#[test]
fn idle_is_one_object_with_no_run() {
    let p = project("idle");
    let j = p.status_json();
    assert_eq!(j["mode"], "idle", "{j}");
    assert!(j["intent"].is_null(), "{j}");
    assert_eq!(j["criteria"], json!([]), "{j}");
    assert_eq!(j["waiting_on_you"], 0, "{j}");
    assert!(context(&p.hook("SessionStart", json!({}))).contains("familiar-ai: idle"));
}

#[test]
fn proving_is_the_picture_status_prints() {
    let p = project("proving");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let reason = denied(&p.edit("src/app.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "greet says hello"]);
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
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");

    let j = p.status_json();
    assert_eq!(j["mode"], "proving", "{j}");
    assert_eq!(j["intent"]["text"], "greet says hello", "{j}");
    assert!(j["intent"]["t"].is_u64() && j["intent"]["sha"].is_string(), "{j}");
    assert_eq!(j["plan"], "confirmed", "{j}");
    let c = &j["criteria"][0];
    assert_eq!(c["id"], "c1", "{j}");
    assert_eq!(c["text"], "greet says hello", "{j}");
    assert_eq!(c["path"], "full", "{j}"); // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path

    // the same word the plain status prints for the criterion
    let (_, plain, _) = p.run(&["status"], "");
    let state = c["state"].as_str().expect("a state");
    assert!(plain.contains(&format!("\"greet says hello\" · {state}")), "{state}: {plain}");

    // the cards are the ones the plain status asks, counted
    let cards = j["cards"].as_array().expect("cards");
    assert_eq!(j["waiting_on_you"], cards.len(), "{j}");
    assert_eq!(plain.matches("a question for you").count(), cards.len(), "{plain}");
    for card in cards {
        assert!(card["criterion_id"].is_string() && card["hold"].is_string() && card["question"].is_string(), "{card}");
    }

    let start = context(&p.hook("SessionStart", json!({})));
    assert!(start.contains("familiar-ai: proving · the intent: \"greet says hello\""), "{start}");
}
