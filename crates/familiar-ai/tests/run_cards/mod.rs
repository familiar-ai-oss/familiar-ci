//! A project planned to one part, c1 on greet, whose runs a test relabels as the regex would have
//! read another output, and the person's pick on a card through the question card (lane S7).
#![allow(dead_code)]

use std::time::Duration;

use familiar_ai_facts::{Fact, Record};
use familiar_ai_rules::View;
use serde_json::{json, Value};

use crate::common::Project;

pub const SESSION: &str = "5b0c1d2e-3f40-4a5b-8c6d-7e8f90a1b2c3";
pub const TEST_FILE: &str = "tests/test_c1.sh";
/// A test that fails on greet as it is (it prints hi).
pub const RED: &str = "out=$(sh src/app.sh)\n[ \"$out\" = hello ] || { echo \"expected hello, got: $out\"; exit 1; }\n";

/// c1 planned on greet, held by SESSION; judge and investigator off.
pub fn planned(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_JUDGE", "off").env("FAMILIAR_AI_INVESTIGATOR", "off").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", TEST_FILE, "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p
}

/// The agent's commit of everything written, and what its PostToolUse told.
pub fn commit(p: &Project, message: &str) -> Value {
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", message]);
    p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {}, "permission_mode": "bypassPermissions" }))
}

/// c1's test committed alone and run red: sealed.
pub fn sealed(name: &str) -> Project {
    let p = planned(name);
    p.write(TEST_FILE, RED);
    commit(&p, "c1: tests first");
    p
}

pub fn record(p: &Project) -> Record {
    Record::open_waiting(&p.record_path(), Duration::from_secs(10)).expect("record")
}

/// The latest translation's run read as `label` (with `extra` fields), as the regex would have
/// read another output: a second translation of the same run, which the rules read last.
pub fn relabel(p: &Project, label: &str, extra: Value) {
    let facts = p.facts();
    let t: &Fact = facts.iter().rev().find(|f| f.kind == "test_log_translation").expect("a translation");
    let mut fields = t.fields.clone();
    fields.insert("label".into(), Value::from(label));
    fields.insert("evidence".into(), Value::from(format!("the run read {label}")));
    if let Value::Object(x) = extra {
        fields.extend(x);
    }
    record(p).append_raw("test_log_translation", fields, 0.0, familiar_ai::TENANT).expect("appended");
}

/// The open cards, by hold.
pub fn cards(p: &Project) -> Vec<String> {
    let log = p.facts();
    familiar_ai_rules::terminal::cards(&View::new(&log)).into_iter().map(|c| c.hold).collect()
}

/// The agent asks the open card with every piece, the facts of the run quoted as the binary words
/// them (2026-10-20: they may not be reworded); the card the hook places.
pub fn ask(p: &Project) -> Value {
    let log = p.facts();
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().next().expect("an open card");
    let a = familiar_ai::teach::person_words::asked(&v, &card);
    let q = format!(
        "{}\n{} \"the run did not go as planned\"\n{}",
        familiar_ai::teach::person_words::lines(&a).join("\n"),
        familiar_ai::teach::person_words::AGENTS_WORDS,
        familiar_ai::teach::person_words::WHAT_NOW
    );
    let q = q.as_str();
    let input = json!({ "questions": [{ "question": q, "header": "familiar-ai", "options": [{ "label": "a", "description": "" }, { "label": "b", "description": "" }], "multiSelect": false }] });
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": input }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "ask", "{pre}");
    pre["hookSpecificOutput"]["updatedInput"].clone()
}

/// The person picks the option whose label is `answer`; what the PostToolUse told.
pub fn pick(p: &Project, card: &Value, answer: &str) -> Value {
    let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let labels: Vec<&str> = card["questions"][0]["options"].as_array().into_iter().flatten().filter_map(|o| o["label"].as_str()).collect();
    assert!(labels.contains(&answer), "{answer} among {labels:?}");
    let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
    let mut input = card.clone();
    input["answers"] = response["answers"].clone();
    input["annotations"] = json!({});
    p.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
}

/// What the PostToolUse told the agent.
pub fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}
