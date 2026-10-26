//! Plan §1.2, §6.7 (S8's live checks): what is owed goes to whoever holds the work.
//! A lane resumed in its own session is its holder running again, never a finished
//! worker; a running subagent's result is its own until it ends or returns; a stop
//! while the part's tests are running is told to wait, not to run them again.

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

/// c1 planned, held by `holder` (this session when `None`), nothing committed.
fn planned(name: &str, inline_ms: &str, holder: Option<&str>) -> Project {
    let p = Project::builder(name).session(SESSION).no_app().file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").env("FAMILIAR_AI_RUN_INLINE_MS", inline_ms).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt").to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    // definition change (the owner's plan of 2026-10-21, step 2) [draft]: a short ask with no example
    // fires a trigger; the agent says it reads one way before the tests
    p.claim(&["one_reading", "--why", "the ask names what must change and how"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    match holder {
        Some(a) => p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", &format!("{SESSION}/{a}")]),
        None => p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION]),
    };
    p
}

fn text<'a>(f: &'a Fact, key: &str) -> &'a str {
    f.fields.get(key).and_then(Value::as_str).unwrap_or("")
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

fn run_id(p: &Project) -> String {
    p.facts().iter().rev().find(|f| f.kind == "run_reserved").map(|f| text(f, "run_id").to_string()).expect("a run is reserved")
}

fn told_about(p: &Project, about: &str) -> usize {
    p.facts_of("told").iter().filter(|t| t["about"] == about).count()
}

fn wait_for_end(p: &Project, id: &str) {
    let until = Instant::now() + Duration::from_secs(30);
    while !p.facts().iter().any(|f| f.kind == "test_log_translation" && text(f, "run_id") == id) {
        assert!(Instant::now() < until, "the run never ended");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A shell call of the main conversation, or of subagent `agent`; what it told.
fn next_call(p: &Project, agent: Option<&str>) -> String {
    let mut fields = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    if let Some(a) = agent {
        fields["agent_id"] = json!(a);
        fields["agent_type"] = json!("general-purpose");
    }
    context(&p.hook("PreToolUse", fields))
}

#[test]
fn a_lane_resumed_in_its_own_session_is_told_its_pick_plainly() {
    let p = planned("resumed", "60000", None);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "dependency", "--where", "the greeting library", "--needs", "a release"]);
    // the lane's headless session stops with only the person's card left
    p.set_env("CLAUDE_CODE_ENTRYPOINT", Some("sdk-cli"));
    p.hook("Stop", json!({ "stop_hook_active": false }));
    assert_eq!(p.facts_of("subagent_ended").len(), 1, "the lane ended");
    let (code, out, err) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "fix_elsewhere", "--choice", "re_plan", "--typed", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    let t = p.facts().iter().rev().find(|f| f.kind == "hold_answered").map(|f| f.t.to_string()).expect("the pick");
    // launch_lane --resume: the same session runs again
    let told = next_call(&p, None);
    p.set_env("CLAUDE_CODE_ENTRYPOINT", None);
    assert!(told.contains("the person answered the question for \"greet says hello\""), "the plain pick: {told}");
    assert!(!told.contains("has finished"), "the holder running again is not a finished worker: {told}");
    assert_eq!(told_about(&p, &t), 1);
}

#[test]
fn a_running_subagents_result_waits_for_its_own_push_not_the_orchestrator() {
    let p = planned("own", "0", Some("a7"));
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", "sleep 1\nexit 1\n");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    let payload = p.payload("PostToolUse", json!({ "agent_id": "a7", "agent_type": "general-purpose", "tool_name": "Bash", "tool_input": { "command": "git commit -m 'c1: tests first'" }, "tool_response": {} }));
    p.hook_with("PostToolUse", &payload);
    let id = run_id(&p);
    wait_for_end(&p, &id);
    assert!(!next_call(&p, None).contains(&format!("run {id}")), "the orchestrator does not tell a running holder's result");
    let prompt = context(&p.hook("UserPromptSubmit", json!({ "prompt": "how is it going" })));
    assert!(!prompt.contains(&format!("run {id}")), "nor at the person's prompt: {prompt}");
    let (code, _, err) = p.run(&["await-run"], &payload.to_string());
    assert_eq!(code, 2, "the holder's own push tells it: {err}");
    assert!(err.contains(&format!("run {id}")), "{err}");
    assert_eq!(told_about(&p, &id), 1);
}

#[test]
fn a_stop_while_the_tests_run_is_told_to_wait_for_them() {
    let p = planned("waits", "0", None);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", "sleep 20\nexit 1\n");
    let out = p.commit("c1: tests first");
    let id = run_id(&p);
    assert!(context(&out).contains(&format!("tests started — run {id}")), "{out}");
    let stop = p.hook("Stop", json!({ "stop_hook_active": false }));
    let reason = stop["reason"].as_str().unwrap_or("");
    assert_eq!(stop["decision"], "block", "{stop}");
    assert!(!reason.contains("next: run its tests"), "not told to run them again: {reason}");
    assert!(reason.contains("the tests are running"), "{reason}");
    assert!(reason.contains("wait"), "{reason}");
}
