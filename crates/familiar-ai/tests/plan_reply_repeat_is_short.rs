//! A repeat refusal is short (finding 35, the 44 benchmark): in one planning,
//! the first `familiar-ai plan` reply that leaves the plan incomplete shows
//! the whole eleven-line checklist; a later `plan` reply in the same planning
//! shows only the lines still missing, each with the claim to paste, and
//! "N of 11 hold". Whether a checklist was already shown is not a fact in the
//! record, so it is derived from what is there: the intent was claimed before
//! this call. The hook's refusal at an edit keeps the whole checklist.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
}

#[test]
fn the_first_plan_reply_shows_the_whole_checklist_and_a_later_one_only_the_missing_lines() {
    let p = project("repeat");
    p.hook("UserPromptSubmit", json!({ "prompt": "greet with hello" }));
    let first = json!([
        {"kind": "intent_submitted", "text": "greet with hello"},
        {"kind": "behavior", "behavior_id": "b1", "text": "the greeting"},
    ]);
    let (code, out, err) = p.run(&["plan"], &first.to_string());
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("the plan's eleven lines, 1 of 11 hold:"), "{out}");
    assert!(out.contains("  ✓ 1 the intent      \"greet with hello\""), "the first reply shows the whole checklist: {out}");
    assert!(out.contains("    2 a criterion     claim it:"), "{out}");

    let second = json!([{"kind": "promise", "promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"}]);
    let (code, out, err) = p.run(&["plan"], &second.to_string());
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("the plan's eleven lines, 1 of 11 hold:"), "N of 11 hold is still said: {out}");
    assert!(!out.contains('✓'), "a later reply names no line that holds: {out}");
    assert!(out.contains("claim it: familiar-ai claim criterion "), "the missing line comes with its claim: {out}");
    assert!(out.contains("claim it: familiar-ai claim criterion_assigned --criterion_id c1") || out.contains("11 where it lives"), "{out}");
    assert!(out.contains("all at once: familiar-ai plan"), "{out}");
}

#[test]
fn the_hooks_refusal_at_an_edit_keeps_the_whole_checklist_after_a_partial_plan() {
    let p = project("hook");
    p.hook("UserPromptSubmit", json!({ "prompt": "greet with hello" }));
    let first = json!([{"kind": "intent_submitted", "text": "greet with hello"}]);
    let (code, out, err) = p.run(&["plan"], &first.to_string());
    assert_eq!(code, 0, "{out}{err}");
    let path = p.path("src/app.sh").to_string_lossy().into_owned();
    let out: Value = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.contains("  ✓ 1 the intent      \"greet with hello\""), "{reason}");
}
