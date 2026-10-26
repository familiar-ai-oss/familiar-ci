//! A test command the machine does not have makes the run `did_not_run` (exit
//! 127), not a red, and the PostToolUse of the commit that settles it says so
//! with what the command printed.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use familiar_ai_rules::lifecycle::state;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

#[test]
fn a_command_that_is_not_there_is_told_in_the_hook_that_settles_it() {
    let p = project("told");
    p.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
    let reason = denied(&p.edit("src/a.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "a says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "false"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", "pytset -q\n");
    let told = p.commit("c1: a test whose command is missing");
    let commit = p.head();
    let context = told["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string();
    assert!(context.contains("familiar-ai: the test command did not run: "), "the line says the command did not run: {context}");
    assert!(context.contains("tests/test_c1.sh:"), "with what the shell printed: {context}");
    assert!(context.contains("pytset"), "naming the missing command: {context}");
    assert!(context.contains(" · next: fix the command or the environment"), "and the way out: {context}");

    let log = p.facts();
    let at = |f: &Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let results: Vec<String> = log
        .iter()
        .filter(|f| f.kind == "pass_please_pass" && at(f, "commit") == commit && at(f, "criterion_id") == "c1" && at(f, "file_path") == "tests/test_c1.sh")
        .map(|f| at(f, "outcome"))
        .collect();
    // plan §6.1 (S6): a run that ran and did not pass is `failed`; its family is its translation's
    assert_eq!(results, ["failed"], "the run ran: the shell started the script");
    let label: Vec<String> = log.iter().filter(|f| f.kind == "test_log_translation").map(|f| at(f, "label")).collect();
    assert_eq!(label, ["missing_target_symbol"], "the sh table reads the script's 'command not found' (the owner, 2026-10-15)");
    assert_ne!(state(&View::new(&log), "c1"), "sealed", "no red seal");
}
