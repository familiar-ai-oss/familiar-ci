//! The case-finder's call failed (the owner's ruling, 2026-10-25: "Record failed") [draft names]:
//! when the main agent's Agent call that runs the finder never returns normally (a tool error,
//! Esc, a missing agent file), Claude Code sends PostToolUseFailure, not PostToolUse. While the
//! part is owed the finder, that failure is recorded as `cases_found` with outcome failed and the
//! error as its why, so the person's card opens with look again instead of the part staying
//! stuck on cases_owed.
//!
//! The event's shape is ASSUMED, not captured: the Claude Agent SDK's
//! PostToolUseFailureHookInput (claude_agent_sdk 0.2.174): the common fields, tool_name,
//! tool_input, tool_use_id, error (a string), is_interrupt and duration_ms optional. Every field
//! but hook_event_name is read as optional; tool_name Agent or Task; error read as text (missing:
//! "the finder's call failed"); the spawn matched by tool_input.subagent_type naming the finder.

mod common;

use common::Project;
use familiar_ai_facts::{Fact, Record};
use serde_json::{json, Value};

const FINDER: &str = "familiar-ai:case-finder";
const FIXTURE: &str = "post_tool_use_failure_agent_case_finder";
const ERROR: &str = "Agent type 'familiar-ai:case-finder' not found. Available agents: general-purpose, Explore, Plan";

/// A full-path project planned to c1's whole contract: the finder is owed.
fn project(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_CASE_FINDER_MODEL").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"the name Ada","expect":"hello Ada"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
    ] {
        p.claim(&args);
    }
    p
}

fn failed(p: &Project, set: Value) -> Value {
    let (code, out, err) = p.hook_raw("PostToolUseFailure", &p.fixture(FIXTURE, set));
    assert_eq!(code, 0, "PostToolUseFailure: {out}{err}");
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("PostToolUseFailure printed no JSON ({e}): {out}{err}"))
}

fn found(p: &Project) -> Vec<Fact> {
    p.facts().into_iter().filter(|f| f.kind == "cases_found").collect()
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

#[test]
fn the_finders_failed_call_is_recorded_failed_with_the_error_and_the_card_opens() {
    let p = project("failed");
    let spawn = p.hook_fixture("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    assert_eq!(spawn["hookSpecificOutput"]["updatedInput"]["subagent_type"], json!(FINDER), "{spawn}");
    let out = failed(&p, json!({}));
    let fs = found(&p);
    assert_eq!(fs.len(), 1, "{out}");
    let f = &fs[0];
    assert_eq!((text(f, "criterion_id"), text(f, "outcome"), text(f, "why"), text(f, "model")), ("c1", "failed", ERROR, "sonnet"));
    assert_eq!((f.fields.get("cases"), f.fields.get("tokens")), (Some(&json!([])), Some(&json!(0))));
    let status = p.status_json().to_string();
    assert!(status.contains("cases_unconfirmed") && !status.contains("\"cases_owed\""), "the card, nothing owed: {status}");
    let log = p.facts();
    let told = familiar_ai::teach::case_finder::after_finding(&familiar_ai_rules::View::new(&log), "c1");
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], json!("PostToolUseFailure"), "{out}");
    assert!(out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains(&told), "the agent is told the card is open: {out}");

    // nothing is owed any more: a second failure records nothing
    failed(&p, json!({}));
    assert_eq!(found(&p).len(), 1);
}

#[test]
fn a_missing_error_is_said_plainly_and_the_task_tool_and_the_bare_name_count() {
    let p = project("no-error");
    let mut payload = p.fixture(FIXTURE, json!({ "tool_name": "Task" }));
    payload.as_object_mut().map(|o| o.remove("error"));
    let (code, out, err) = p.hook_raw("PostToolUseFailure", &payload);
    assert_eq!(code, 0, "{out}{err}");
    let fs = found(&p);
    assert_eq!(fs.len(), 1, "{out}{err}");
    assert_eq!((text(&fs[0], "outcome"), text(&fs[0], "why")), ("failed", "the finder's call failed"));

    let p = project("bare");
    let mut input = common::fixture(FIXTURE)["tool_input"].clone();
    input["subagent_type"] = json!("case-finder");
    failed(&p, json!({ "tool_input": input, "error": { "message": "not text" } }));
    let fs = found(&p);
    assert_eq!(fs.len(), 1);
    assert_eq!(text(&fs[0], "outcome"), "failed");
    assert!(text(&fs[0], "why").contains("not text"), "an error that is not text is kept as its JSON: {:?}", fs[0]);
}

#[test]
fn another_agents_failure_or_another_tools_records_nothing() {
    let p = project("other");
    let mut input = common::fixture(FIXTURE)["tool_input"].clone();
    input["subagent_type"] = json!("general-purpose");
    failed(&p, json!({ "tool_input": input }));
    failed(&p, json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } }));
    assert!(found(&p).is_empty());
    assert!(p.status_json().to_string().contains("\"cases_owed\""), "still owed");
}

#[test]
fn look_again_after_a_failed_call_owes_one_more_finder() {
    let p = project("again");
    failed(&p, json!({}));
    let record = Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("the record");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0) + 1.0;
    // the person's pick, as the picker records it
    let fields = json!({ "criterion_id": "c1", "hold": "cases_unconfirmed", "choice": "look_again", "typed": "" });
    record.append_raw("hold_answered", fields.as_object().cloned().unwrap_or_default(), now, familiar_ai::TENANT).expect("the answer");
    drop(record);
    let spawn = p.hook_fixture("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    assert_eq!(spawn["hookSpecificOutput"]["updatedInput"]["subagent_type"], json!(FINDER), "one more finder: {spawn}");
}

#[test]
fn the_plugin_sends_the_spawns_failure_to_the_binary() {
    let hooks: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugin/hooks/hooks.json")).expect("hooks.json")).expect("json");
    let entries = hooks["hooks"]["PostToolUseFailure"].as_array().cloned().unwrap_or_default();
    assert_eq!(entries.len(), 1, "{hooks}");
    assert_eq!(entries[0]["matcher"], json!("Agent|Task"));
    let h = &entries[0]["hooks"][0];
    assert_eq!(h["command"], json!("\"${CLAUDE_PLUGIN_ROOT}\"/hooks/run.sh PostToolUseFailure"));
    assert_eq!(h["type"], json!("command"));
    assert!(h["timeout"].as_u64().is_some_and(|t| t >= 200), "{h}");
}
