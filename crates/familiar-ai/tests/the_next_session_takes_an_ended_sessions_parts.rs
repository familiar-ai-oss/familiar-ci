//! A session's end is recorded, and the next session takes the parts it held (the owner,
//! 2026-10-27): at Claude Code's SessionEnd the binary writes `subagent_ended` with agent_type
//! `session` for a session that holds a part, and the next session's first claim or edit writes
//! `criterion_assigned` to itself for each part whose holder ended. A part whose holder still
//! runs stays held.

mod common;

use common::Project;
use serde_json::{json, Value};

/// c1 planned and held by the project's own session.
fn project(name: &str) -> Project {
    let p = Project::builder(name).file("a.py", "x = 1\n").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1"],
        vec!["criterion_assigned", "--criterion_id", "c1"],
    ] {
        p.claim(&args);
    }
    p
}

impl Project {
    /// The conversation c1's latest assignment names.
    fn holder(&self) -> String {
        let f = self.facts().into_iter().rev().find(|f| f.kind == "criterion_assigned").expect("an assignment");
        f.fields.get("agent_conversation_id").and_then(Value::as_str).unwrap_or("").to_string()
    }

    fn session_ends(&self, session: &str) {
        let (code, out, err) = self.hook_raw("SessionEnd", &self.payload_as(session, "SessionEnd", json!({ "reason": "exit" })));
        assert_eq!(code, 0, "{out}{err}");
    }

    fn edits_as(&self, session: &str) {
        let input = json!({ "file_path": self.path("a.py").to_string_lossy(), "old_string": "x = 1", "new_string": "x = 2" });
        let (code, out, err) = self.hook_raw("PreToolUse", &self.payload_as(session, "PreToolUse", json!({ "tool_name": "Edit", "tool_input": input })));
        assert_eq!(code, 0, "{out}{err}");
    }

    fn ends(&self) -> Vec<(String, String)> {
        let s = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        self.facts().into_iter().filter(|f| f.kind == "subagent_ended").map(|f| (s(&f, "agent_id"), s(&f, "agent_type"))).collect()
    }
}

#[test]
fn a_sessions_end_is_recorded_when_it_holds_a_part() {
    let p = project("end");
    let a = p.holder();
    p.session_ends("s-holds-nothing");
    assert!(p.ends().is_empty(), "a session holding no part: {:?}", p.ends());
    p.session_ends(&a);
    assert_eq!(p.ends(), [(a.clone(), "session".to_string())]);
}

#[test]
fn the_next_sessions_first_edit_takes_the_parts_whose_holder_ended() {
    let p = project("edit");
    let a = p.holder();
    p.edits_as("s-next");
    assert_eq!(p.holder(), a, "its holder still runs");
    p.session_ends(&a);
    p.edits_as("s-next");
    assert_eq!(p.holder(), "s-next");
    let taken = p.facts().into_iter().filter(|f| f.kind == "criterion_assigned").count();
    p.edits_as("s-next");
    assert_eq!(p.facts().into_iter().filter(|f| f.kind == "criterion_assigned").count(), taken, "taken once");
}

#[test]
fn the_next_sessions_first_claim_takes_them_too() {
    let p = project("claim");
    let a = p.holder();
    p.session_ends(&a);
    p.set_env("CLAUDE_CODE_SESSION_ID", Some("s-claims"));
    p.claim(&["behavior", "--behavior_id", "b2", "--text", "the farewell"]);
    assert_eq!(p.holder(), "s-claims");
}

#[test]
fn the_plugin_sends_a_sessions_end_to_the_binary() {
    let hooks: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugin/hooks/hooks.json")).expect("hooks.json")).expect("json");
    let h = &hooks["hooks"]["SessionEnd"][0]["hooks"][0];
    assert_eq!(h["command"], json!("\"${CLAUDE_PLUGIN_ROOT}\"/hooks/run.sh SessionEnd"), "{hooks}");
    assert_eq!(h["type"], json!("command"));
}
