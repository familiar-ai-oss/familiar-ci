//! A lens's call failed (the owner's ruling, 2026-10-25: "Record failed", kept for the lens panel
//! of 2026-10-26): when the main agent's Agent call released as a lens (or the synthesis) never
//! returns normally (a tool error, Esc, a missing agent file), Claude Code sends
//! PostToolUseFailure, not PostToolUse. While that lens is owed, the failure is recorded failed
//! with the error as its why: a lens's as `lens_found` (its one finding of kind `failed`, the
//! error its note), the synthesis's as `cases_found`, so the person's card opens with look again
//! instead of the part staying stuck on cases_owed. The call is matched by its tool_use_id.
//!
//! The event's shape is ASSUMED, not captured: the Claude Agent SDK's
//! PostToolUseFailureHookInput (claude_agent_sdk 0.2.174): the common fields, tool_name,
//! tool_input, tool_use_id, error (a string), is_interrupt and duration_ms optional.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const FIXTURE: &str = "post_tool_use_failure_agent_case_finder";
const ERROR: &str = "Agent type 'familiar-ai:case-finder' not found. Available agents: general-purpose, Explore, Plan";

/// A full-path project planned to c1's whole contract: the lens panel is owed.
fn project(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_LENS_MODEL").create();
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

fn failed(p: &Project, id: &str, set: Value) -> Value {
    let mut set = set;
    set["tool_use_id"] = json!(id);
    let (code, out, err) = p.hook_raw("PostToolUseFailure", &p.fixture(FIXTURE, set));
    assert_eq!(code, 0, "PostToolUseFailure: {out}{err}");
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("PostToolUseFailure printed no JSON ({e}): {out}{err}"))
}

fn of_kind(p: &Project, kind: &str) -> Vec<Fact> {
    p.facts().into_iter().filter(|f| f.kind == kind).collect()
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

#[test]
fn a_lens_whose_call_failed_is_recorded_failed_with_the_error() {
    let p = project("lens");
    let spawn = p.spawn("toolu_a");
    assert_eq!(spawn["hookSpecificOutput"]["updatedInput"]["subagent_type"], json!(common::project::LENS), "{spawn}");
    let out = failed(&p, "toolu_a", json!({}));
    let fs = of_kind(&p, "lens_found");
    assert_eq!(fs.len(), 1, "{out}");
    assert_eq!(
        Value::Object(fs[0].fields.clone()),
        json!({ "criterion_id": "c1", "lens": "ambiguity", "outcome": "failed", "findings": [{ "kind": "failed", "note": ERROR, "file_path": "", "target_symbol": "", "candidate_question": "" }], "model": "sonnet", "tokens": 0 })
    );
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], json!("PostToolUseFailure"), "{out}");
    let told = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(told.starts_with("familiar-ai: the ambiguity lens answered for \"greet says hello\": it could not finish looking."), "{out}");
    // a failed lens is an answer: the next call is the next lens
    let next = p.spawn("toolu_b");
    assert!(next["hookSpecificOutput"]["updatedInput"]["prompt"].as_str().unwrap_or("").contains("Your lens: COMPLETENESS"), "{next}");
    // the same failure again records nothing
    failed(&p, "toolu_a", json!({}));
    assert_eq!(of_kind(&p, "lens_found").len(), 1);
}

#[test]
fn the_synthesis_whose_call_failed_is_recorded_failed_and_the_card_opens() {
    let p = project("synthesis");
    p.lenses_answer_none();
    p.spawn("toolu_synthesis");
    let out = failed(&p, "toolu_synthesis", json!({}));
    let fs = of_kind(&p, "cases_found");
    assert_eq!(fs.len(), 1, "{out}");
    let f = &fs[0];
    assert_eq!((text(f, "criterion_id"), text(f, "outcome"), text(f, "why"), text(f, "model")), ("c1", "failed", ERROR, "sonnet"));
    assert_eq!((f.fields.get("cases"), f.fields.get("tokens")), (Some(&json!([])), Some(&json!(0))));
    let status = p.status_json().to_string();
    assert!(status.contains("cases_unconfirmed") && !status.contains("\"cases_owed\""), "the card, nothing owed: {status}");
    let log = p.facts();
    let told = familiar_ai::teach::lens::after_answer(&familiar_ai_rules::View::new(&log), "c1");
    assert!(out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains(&told), "the agent is told the card is open: {out}");
}

#[test]
fn a_missing_error_is_said_plainly_and_the_task_tool_counts() {
    let p = project("no-error");
    p.spawn("toolu_a");
    let mut payload = p.fixture(FIXTURE, json!({ "tool_name": "Task", "tool_use_id": "toolu_a" }));
    payload.as_object_mut().map(|o| o.remove("error"));
    let (code, out, err) = p.hook_raw("PostToolUseFailure", &payload);
    assert_eq!(code, 0, "{out}{err}");
    let fs = of_kind(&p, "lens_found");
    assert_eq!(fs.len(), 1, "{out}{err}");
    assert_eq!((text(&fs[0], "outcome"), fs[0].fields["findings"][0]["note"].as_str()), ("failed", Some("the lens's call failed")));

    let p = project("not-text");
    p.spawn("toolu_a");
    failed(&p, "toolu_a", json!({ "error": { "message": "not text" } }));
    let fs = of_kind(&p, "lens_found");
    assert_eq!(fs.len(), 1);
    assert!(fs[0].fields["findings"][0]["note"].as_str().unwrap_or("").contains("not text"), "an error that is not text is kept as its JSON: {:?}", fs[0]);
}

#[test]
fn a_call_never_released_or_another_tools_failure_records_nothing() {
    let p = project("other");
    p.spawn("toolu_a");
    failed(&p, "toolu_other", json!({}));
    failed(&p, "toolu_a", json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } }));
    assert!(of_kind(&p, "lens_found").is_empty() && of_kind(&p, "cases_found").is_empty());
    assert!(p.status_json().to_string().contains("\"cases_owed\""), "still owed");
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
