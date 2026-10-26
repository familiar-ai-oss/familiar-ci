//! Plan §1.2, Assumption 52 (S8): a card opened by a subagent's hold reaches
//! the orchestrator on the PostToolUse of the Agent call that returned that
//! subagent, not first at the orchestrator's next refusal, prompt or stop.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// c1 planned on greet and held by the subagent `agent`, which has said its fix lives in a dependency.
fn held_by(name: &str, agent: &str) -> Project {
    let p = Project::builder(name).session(SESSION).no_app().file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt").to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", &format!("{SESSION}/{agent}")]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "dependency", "--where", "the greeting library", "--needs", "a release that says hello"]);
    p
}

fn returned(p: &Project, agent: &str) -> Value {
    p.hook(
        "PostToolUse",
        json!({ "tool_name": "Agent", "tool_input": { "description": "take c1", "prompt": "take c1", "subagent_type": "general-purpose" }, "tool_response": { "status": "completed", "agentId": agent } }),
    )
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

#[test]
fn the_returned_holders_open_card_is_told_to_the_orchestrator() {
    let p = held_by("returned", "a7");
    assert_eq!(p.status_json()["counts"]["needs_you"], 1, "the card is open: {}", p.status());
    let told = context(&returned(&p, "a7"));
    assert!(told.contains("a question for the person is open"), "the card's facts reach the orchestrator: {told}");
}

#[test]
fn another_subagents_return_does_not_tell_it() {
    let p = held_by("other", "a7");
    let told = context(&returned(&p, "a8"));
    assert!(!told.contains("a question for the person is open"), "{told}");
}
