//! Plan §1.2, §6.7 (S8, D11): everything owed to an agent is told once. Each
//! teller (inline, the push `familiar-ai await-run`, orchestrator delivery,
//! the backstop) appends `told {about}` first, and only the one whose append
//! lands speaks. `about` is a run id or a pick's `t`.

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

/// c1 planned, held by `holder` (the session when `None`), its test written, nothing committed.
fn planned(name: &str, inline_ms: &str, _test_body: &str, holder: Option<&str>) -> Project {
    let p = Project::builder(name).session(SESSION).no_app().file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").env("FAMILIAR_AI_RUN_INLINE_MS", inline_ms).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt").to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
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
        None => p.claim(&["criterion_assigned", "--criterion_id", "c1"]),
    };
    p
}

/// The holder says c1's fix lives here, its first act before a commit.
fn here(p: &Project) {
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
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

fn commit_payload(p: &Project, command: &str) -> Value {
    p.payload("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": command }, "tool_response": {} }))
}

fn await_run(p: &Project, payload: &Value) -> (i32, String, String, Duration) {
    let started = Instant::now();
    let (code, out, err) = p.run(&["await-run"], &payload.to_string());
    (code, out, err, started.elapsed())
}

fn wait_for_end(p: &Project, id: &str) {
    let until = Instant::now() + Duration::from_secs(30);
    while !p.facts().iter().any(|f| f.kind == "test_log_translation" && text(f, "run_id") == id) {
        assert!(Instant::now() < until, "the run never ended");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A shell call of the main conversation after the run; what it told.
fn next_call(p: &Project, agent: Option<&str>) -> String {
    let mut fields = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    if let Some(a) = agent {
        fields["agent_id"] = json!(a);
        fields["agent_type"] = json!("general-purpose");
    }
    context(&p.hook("PreToolUse", fields))
}

#[test]
fn a_run_told_inline_is_told_once() {
    let p = planned("inline", "60000", "", None);
    here(&p);
    p.write("tests/test_c1.sh", "exit 1\n");
    p.commit("c1: tests first");
    let id = run_id(&p);
    assert_eq!(told_about(&p, &id), 1, "the inline teller wrote told: {:?}", p.facts_of("told"));
    assert!(!next_call(&p, None).contains(&format!("run {id}")), "not told again");
    // the push finds it told: nothing to say
    let (code, _, err, _) = await_run(&p, &commit_payload(&p, "git commit -m 'c1: tests first'"));
    assert_eq!(code, 0, "{err}");
    assert_eq!(told_about(&p, &id), 1);
}

#[test]
fn a_slow_run_is_told_by_the_push() {
    let p = planned("push", "0", "", None);
    here(&p);
    p.write("tests/test_c1.sh", "sleep 2\nexit 1\n");
    let out = p.commit("c1: tests first");
    let id = run_id(&p);
    assert!(context(&out).contains(&format!("tests started — run {id}")), "{out}");
    let (code, _, err, _) = await_run(&p, &commit_payload(&p, "git commit -m 'c1: tests first'"));
    assert_eq!(code, 2, "the push wakes the agent: {err} {:?}", p.facts().iter().map(|f| (f.kind, f.fields.get("commit").cloned())).collect::<Vec<_>>());
    assert!(err.contains(&format!("run {id}")), "{err}");
    assert_eq!(told_about(&p, &id), 1);
    assert!(!next_call(&p, None).contains(&format!("run {id}")), "the backstop does not tell it again");
}

#[test]
fn with_the_push_off_the_backstop_tells_it() {
    let p = planned("backstop", "0", "", None);
    here(&p);
    p.write("tests/test_c1.sh", "sleep 1\nexit 1\n");
    p.commit("c1: tests first");
    let id = run_id(&p);
    wait_for_end(&p, &id);
    assert!(next_call(&p, None).contains(&format!("run {id}")), "the next call tells it");
    assert_eq!(told_about(&p, &id), 1);
    assert!(!next_call(&p, None).contains(&format!("run {id}")), "once");
}

#[test]
fn a_running_subagents_result_is_its_own_and_the_orchestrator_gets_it_when_it_returns() {
    let p = planned("holder", "0", "", Some("a7"));
    here(&p);
    p.write("tests/test_c1.sh", "sleep 1\nexit 1\n");
    p.commit("c1: tests first");
    let id = run_id(&p);
    wait_for_end(&p, &id);
    assert!(!next_call(&p, None).contains(&format!("run {id}")), "the holder is running: not the orchestrator's");
    assert!(next_call(&p, Some("a7")).contains(&format!("run {id}")), "the holder's own next call tells it");
    assert_eq!(told_about(&p, &id), 1);

    // a second run: the holder stops before it is told; the orchestrator's Agent return tells it
    let p = planned("returns", "0", "", Some("a7"));
    here(&p);
    p.write("tests/test_c1.sh", "sleep 1\nexit 1\n");
    p.commit("c1: tests first");
    let id = run_id(&p);
    wait_for_end(&p, &id);
    p.hook("SubagentStop", json!({ "agent_id": "a7", "agent_type": "general-purpose", "stop_hook_active": false }));
    let back = p.hook("PostToolUse", json!({ "tool_name": "Agent", "tool_input": { "prompt": "take c1" }, "tool_response": { "status": "completed", "agentId": "a7" } }));
    assert!(context(&back).contains(&format!("run {id}")), "{back}");
    assert_eq!(told_about(&p, &id), 1);
}

#[test]
fn a_call_that_reserved_no_run_spawns_no_wait() {
    let p = planned("nowait", "60000", "", None);
    let (code, _, err, took) = await_run(&p, &commit_payload(&p, "ls"));
    assert_eq!(code, 0, "{err}");
    assert!(took < Duration::from_secs(2), "at once: {took:?}");
    let empty = Project::new("noplan");
    let (code, _, err, took) = await_run(&empty, &commit_payload(&empty, "git commit -m x"));
    assert_eq!(code, 0, "{err}");
    assert!(took < Duration::from_secs(2), "a commit that reserves no run exits at once: {took:?}");
    assert!(empty.facts_of("told").is_empty());
}

#[test]
fn the_push_entry_waits_only_on_a_commit_and_wakes_the_agent() {
    let hooks: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugin/hooks/hooks.json")).expect("hooks.json")).expect("json");
    let entries: Vec<&Value> = hooks["hooks"]["PostToolUse"].as_array().expect("PostToolUse").iter().flat_map(|e| e["hooks"].as_array().cloned().unwrap_or_default().into_iter().map(move |h| (e, h))).filter(|(_, h)| h["command"].as_str().unwrap_or("").contains("await-run")).map(|(e, _)| e).collect();
    assert_eq!(entries.len(), 1, "{hooks}");
    let e = entries[0];
    assert_eq!(e["matcher"], "Bash");
    let h = &e["hooks"][0];
    assert_eq!(h["if"], "Bash(git commit *)");
    assert_eq!(h["async"], true);
    assert_eq!(h["asyncRewake"], true);
    assert!(h["timeout"].as_u64().unwrap_or(0) >= 1800 + 120, "{h}");
}

/// The holder says c1's fix lives in a dependency: the person's card opens.
fn card(p: &Project) {
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "dependency", "--where", "the greeting library", "--needs", "a release"]);
}

/// The person answers c1's card from a terminal of their own.
fn answered(p: &Project) -> String {
    let (code, out, err) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "fix_elsewhere", "--choice", "re_plan", "--typed", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    p.facts().iter().rev().find(|f| f.kind == "hold_answered").map(|f| f.t.to_string()).expect("the pick")
}

#[test]
fn a_pick_reaches_a_running_holder_once() {
    let p = planned("pick", "60000", "", Some("a7"));
    card(&p);
    let t = answered(&p);
    assert!(!next_call(&p, None).contains("the person answered"), "not the orchestrator's while the holder runs");
    assert!(next_call(&p, Some("a7")).contains("the person answered the question for \"greet says hello\""), "the holder is told");
    assert_eq!(told_about(&p, &t), 1);
    assert!(!next_call(&p, Some("a7")).contains("the person answered"), "once");
}

#[test]
fn a_pick_for_a_finished_holder_gives_the_orchestrator_its_resume_line_once() {
    let p = planned("resume", "60000", "", Some("a7"));
    card(&p);
    // a headless holder stops with only the person's card left (the bench)
    p.set_env("CLAUDE_CODE_ENTRYPOINT", Some("sdk-cli"));
    let stop = p.hook("SubagentStop", json!({ "agent_id": "a7", "agent_type": "general-purpose", "stop_hook_active": false }));
    assert_eq!(p.facts_of("subagent_ended").len(), 1, "{stop}");
    let t = answered(&p);
    p.set_env("CLAUDE_CODE_ENTRYPOINT", None);
    let told = next_call(&p, None);
    assert!(told.contains("\"greet says hello\"'s worker a7 has finished. Resume it with this instruction: "), "{told}");
    assert_eq!(told_about(&p, &t), 1);
    assert!(!next_call(&p, None).contains("has finished"), "once");
}

#[test]
fn the_b4_rewrite_still_carries_what_the_subagent_is_owed() {
    let p = planned("rewrite", "0", "", Some("a7"));
    here(&p);
    p.write("tests/test_c1.sh", "sleep 1\nexit 1\n");
    p.commit("c1: tests first");
    let id = run_id(&p);
    wait_for_end(&p, &id);
    let out = p.hook("PreToolUse", json!({ "agent_id": "a7", "agent_type": "general-purpose", "tool_name": "Bash", "tool_input": { "command": "familiar-ai claim criterion_assigned --criterion_id c1" } }));
    assert!(out["hookSpecificOutput"]["updatedInput"].is_object(), "rewritten: {out}");
    assert!(context(&out).contains(&format!("run {id}")), "the owed result rides the rewrite: {out}");
    assert_eq!(told_about(&p, &id), 1);
}
