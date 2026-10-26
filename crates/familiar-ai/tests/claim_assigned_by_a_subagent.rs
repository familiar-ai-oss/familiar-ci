//! Plan §6.7, B-4 (S8): a subagent's `familiar-ai claim criterion_assigned`
//! is rewritten on its PreToolUse to carry `--agent_conversation_id
//! session/agent_id`, so the part's holder is the subagent and not the
//! session; only when the call is allowed, and an explicit flag is left as typed.

mod common;

use common::project::DEFAULT_SESSION;
use common::Project;
use serde_json::{json, Value};

const CLAIM: &str = "familiar-ai claim criterion_assigned --criterion_id c1";

fn pre(p: &Project, agent: Option<&str>, command: &str) -> Value {
    let mut fields = json!({ "tool_name": "Bash", "tool_input": { "command": command, "description": "take the part" } });
    if let Some(a) = agent {
        fields["agent_id"] = json!(a);
        fields["agent_type"] = json!("general-purpose");
    }
    p.hook("PreToolUse", fields)
}

#[test]
fn a_subagents_claim_is_rewritten_with_its_own_id() {
    let p = Project::new("b4");
    let out = pre(&p, Some("a7"), CLAIM);
    let hso = &out["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "allow", "{out}");
    assert_eq!(hso["updatedInput"]["command"], format!("{CLAIM} --agent_conversation_id {DEFAULT_SESSION}/a7"), "{out}");
    assert_eq!(hso["updatedInput"]["description"], "take the part", "updatedInput carries every field the agent sent: {out}");
}

#[test]
fn an_explicit_flag_and_the_main_conversation_are_left_as_typed() {
    let p = Project::new("b4typed");
    let typed = format!("{CLAIM} --agent_conversation_id {DEFAULT_SESSION}/a9");
    let out = pre(&p, Some("a7"), &typed);
    assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "{out}");
    let out = pre(&p, None, CLAIM);
    assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "{out}");
    let out = pre(&p, Some("a7"), "familiar-ai claim criterion --criterion_id c1 --text x");
    assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "another claim is not rewritten: {out}");
}

#[test]
fn a_refused_claim_stays_refused() {
    let p = Project::new("b4refused");
    let out = pre(&p, Some("a7"), &format!("{CLAIM} && claude -p 'go on'"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "{out}");
    let out = pre(&p, Some("a7"), &format!("{CLAIM} > .familiar/plan"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "{out}");
}

#[test]
fn the_rewritten_claim_stores_the_subagent_as_the_holder() {
    let p = Project::new("b4stored");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", "greet says hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light", "--for", &t.to_string()]);
    let out = pre(&p, Some("a7"), CLAIM);
    let command = out["hookSpecificOutput"]["updatedInput"]["command"].as_str().unwrap_or_else(|| panic!("rewritten: {out}")).to_string();
    let args: Vec<&str> = command.split_whitespace().skip(2).collect();
    p.claim(&args);
    let assigned = p.facts_of("criterion_assigned");
    assert_eq!(assigned.last().map(|a| a["agent_conversation_id"].clone()), Some(json!(format!("{DEFAULT_SESSION}/a7"))), "{assigned:?}");
}

#[test]
fn a_trailing_comment_or_background_is_left_as_typed() {
    let p = Project::new("b4edges");
    for command in [format!("{CLAIM} # take it"), format!("{CLAIM} &")] {
        let out = pre(&p, Some("a7"), &command);
        assert!(out["hookSpecificOutput"].get("updatedInput").is_none(), "{command}: {out}");
    }
}
