//! Switched off, the person's own prompts are still recorded as `person_said`
//! (the owner, 2026-10-12: "Yes, record my prompts while off"). Nothing else
//! of off changes: the hook says nothing and writes no other fact.

mod common;

use common::Project;
use serde_json::{json, Value};

#[test]
fn a_prompt_while_off_is_recorded_and_nothing_is_said() {
    let p = Project::new("off-prompt");
    assert_eq!(p.run_as(None, &["off"], "").0, 0);
    let before = p.facts().len();

    let (code, out, err) = p.hook_raw("UserPromptSubmit", &p.payload("UserPromptSubmit", json!({ "prompt": "make greet say hello, not hi" })));
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let said: Value = if out.trim().is_empty() { json!({}) } else { serde_json::from_str(out.trim()).expect("json") };
    assert_eq!(said, json!({}), "off says nothing");

    // a finished background task arrives as a prompt too, and is not the person's
    let notice = "<task-notification>\n<task-id>b1</task-id>\n<status>completed</status>\n</task-notification>";
    p.hook_raw("UserPromptSubmit", &p.payload("UserPromptSubmit", json!({ "prompt": notice })));

    let facts = p.facts();
    let new: Vec<&str> = facts[before..].iter().map(|f| f.kind).collect();
    assert_eq!(new, ["person_said"], "only the person's prompt is written");
    assert_eq!(p.facts_of("person_said").last().and_then(|f| f["text"].as_str()), Some("make greet say hello, not hi"));
    assert!(p.status_json()["off"] == json!(true), "still off");
}
