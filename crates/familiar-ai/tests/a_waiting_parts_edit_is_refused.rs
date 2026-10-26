//! S9a's two (the owner, 2026-10-16), the first: a waiting part's plain file edits are refused as
//! its commits are (stop 3 joins `wait_refusal`; Assumption 24 in full). A subagent that holds
//! only the part strung after the one in work is told to wait; the part in work edits freely.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "5b0c1d2e-3f40-4a5b-8c6d-7e8f90a1b2c3";
const AGENT: &str = "a9f0e1d2c3b4a5968";

/// c1 then c2 on greet; c1 held by the session, c2 by its subagent.
fn planned(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    let holder = format!("{SESSION}/{AGENT}");
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello, then wave"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["criterion", "--criterion_id", "c2", "--text", "greet waves", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION],
        vec!["criterion_assigned", "--criterion_id", "c2", "--agent_conversation_id", &holder],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
        vec!["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p
}

fn edit(p: &Project, agent: Option<&str>, file: &str) -> serde_json::Value {
    let mut fields = json!({ "tool_name": "Write", "tool_input": { "file_path": p.path(file).to_string_lossy(), "content": "x\n" } });
    if let Some(a) = agent {
        fields["agent_id"] = json!(a);
        fields["agent_type"] = json!("general-purpose");
    }
    p.hook("PreToolUse", fields)
}

#[test]
fn a_waiting_parts_edit_is_refused_and_the_part_in_work_edits_freely() {
    let p = planned("waiting-edit");
    assert_eq!(p.state_name("c2"), "waiting");
    let out = edit(&p, Some(AGENT), "src/app.sh");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let why = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(why.contains("\"greet says hello\" comes first"), "{why}");
    let test = edit(&p, Some(AGENT), "tests/test_c2.sh");
    assert_eq!(test["hookSpecificOutput"]["permissionDecision"], "deny", "its test too: {test}");
    let own = edit(&p, None, "src/app.sh");
    assert_ne!(own["hookSpecificOutput"]["permissionDecision"], "deny", "the part in work: {own}");
}

#[test]
fn a_waiting_parts_shell_write_is_refused() {
    let p = planned("waiting-shell");
    let fields = json!({ "tool_name": "Bash", "tool_input": { "command": "echo x > src/app.sh" }, "agent_id": AGENT, "agent_type": "general-purpose" });
    let out = p.hook("PreToolUse", fields);
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
}
