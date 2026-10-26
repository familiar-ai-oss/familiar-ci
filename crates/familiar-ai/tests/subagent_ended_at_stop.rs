//! Plan §1.2 (S8): every allowed SubagentStop appends `subagent_ended
//! {agent_id, agent_type}`, so "the holder has ended" is read from the record;
//! a headless session's allowed Stop appends `subagent_ended {agent_id: its
//! session id, agent_type: "lane"}`. A refused stop appends nothing.

mod common;

use common::project::DEFAULT_SESSION;
use common::Project;
use serde_json::{json, Value};

fn ended(p: &Project) -> Vec<Value> {
    p.facts_of("subagent_ended")
}

#[test]
fn an_allowed_subagent_stop_records_that_the_subagent_ended() {
    let p = Project::new("substop");
    let out = p.hook("SubagentStop", json!({ "agent_id": "a7", "agent_type": "general-purpose", "stop_hook_active": false }));
    assert!(out.get("decision").is_none(), "allowed: {out}");
    assert_eq!(ended(&p), [json!({ "agent_id": "a7", "agent_type": "general-purpose" })]);
    // the main conversation's stop is no subagent's end
    p.hook("Stop", json!({ "stop_hook_active": false }));
    assert_eq!(ended(&p).len(), 1, "{:?}", ended(&p));
}

#[test]
fn a_headless_sessions_allowed_stop_records_the_lane_ended() {
    let p = Project::builder("lanestop").env("CLAUDE_CODE_ENTRYPOINT", "sdk-cli").create();
    let out = p.hook("Stop", json!({ "stop_hook_active": false }));
    assert!(out.get("decision").is_none(), "allowed: {out}");
    assert_eq!(ended(&p), [json!({ "agent_id": DEFAULT_SESSION, "agent_type": "lane" })]);
}

#[test]
fn a_refused_stop_records_nothing() {
    let p = Project::builder("refused").env("CLAUDE_CODE_ENTRYPOINT", "sdk-cli").create();
    // a planned part not done keeps a stop from going
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt").to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light", "--for", &t]);
    let out = p.hook("Stop", json!({ "stop_hook_active": false }));
    assert_eq!(out["decision"], "block", "{out}");
    let out = p.hook("SubagentStop", json!({ "agent_id": "a7", "agent_type": "general-purpose", "stop_hook_active": false }));
    if out["decision"] == "block" {
        assert!(ended(&p).is_empty(), "{:?}", ended(&p));
    }
    assert!(ended(&p).iter().all(|e| e["agent_type"] != "lane"), "{:?}", ended(&p));
}
