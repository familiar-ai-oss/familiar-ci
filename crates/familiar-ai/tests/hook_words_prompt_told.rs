//! A prompt typed during an open run is told at that moment: the
//! UserPromptSubmit of the person's own prompt, while a run is open and its plan
//! confirmed, tells the agent that nothing answers it yet, that it holds the stop,
//! and the claims that answer it. While idle nothing is said, since nothing is
//! governed until code changes.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "3f2e1d0c-9b8a-4c7d-8e6f-5a4b3c2d1e0f";

fn plan(p: &Project) {
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
}

/// The `t` of the person's prompt with `text`.
fn t_of(p: &Project, text: &str) -> u64 {
    let said = p.status_json()["said"].clone();
    said.as_array().into_iter().flatten().find(|s| s["text"] == text).and_then(|s| s["for"].as_u64()).unwrap_or_else(|| panic!("{said}"))
}

fn context(out: &Value) -> &str {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("")
}

#[test]
fn a_prompt_during_an_open_run_is_told_with_the_claims_that_answer_it() {
    let p = Project::builder("open").session(SESSION).create();
    plan(&p);
    let said = "also make bye say goodbye";
    let out = p.hook("UserPromptSubmit", json!({ "prompt": said }));
    let t = t_of(&p, said);
    assert_eq!(
        context(&out),
        [
            format!("familiar-ai: the person said \"{said}\" (t {t}); nothing answers it yet; it holds the stop until a criterion names it with --for {t}, one criterion per part of it, or person_said_answered marks it as needing no work"),
            format!("  claim it: familiar-ai claim criterion --criterion_id c2 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1 --for {t}"),
            format!("  or:       familiar-ai claim person_said_answered --for {t} --how no_work"),
        ]
        .join("\n")
    );
    assert!(out.get("decision").is_none(), "a prompt is never blocked: {out}");
}

#[test]
fn a_prompt_answered_already_or_typed_again_is_told_only_when_it_is_new() {
    let p = Project::builder("again").session(SESSION).create();
    plan(&p);
    p.hook("UserPromptSubmit", json!({ "prompt": "is this the greeting?" }));
    let t = t_of(&p, "is this the greeting?");
    p.claim(&["person_said_answered", "--for", &t.to_string(), "--how", "question"]);
    // a later prompt is told on its own, not the earlier answered one
    let out = p.hook("UserPromptSubmit", json!({ "prompt": "thanks" }));
    let told = context(&out);
    assert!(told.starts_with("familiar-ai: the person said \"thanks\" ("), "{told}");
    assert!(!told.contains("is this the greeting?"), "{told}");
}

#[test]
fn while_idle_or_planning_nothing_is_said() {
    let p = Project::builder("idle").session(SESSION).create();
    let out = p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    assert!(!context(&out).contains("nothing answers it yet"), "idle: {out}");
    p.claim(&["intent_submitted", "--for", &t_of(&p, "make greet say hello").to_string(), "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    let out = p.hook("UserPromptSubmit", json!({ "prompt": "and keep it short" }));
    assert!(!context(&out).contains("nothing answers it yet"), "while planning the plan answers it: {out}");
}
