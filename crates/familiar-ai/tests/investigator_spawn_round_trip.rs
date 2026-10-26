//! After two failures, the holder's spawn becomes the sonnet investigator, which
//! alone may edit until green; then the criterion goes back to the holder with
//! the model corrected to `resolvedModel`. Payloads are captured from Claude Code.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

/// The captured session of the investigator run.
const SESSION: &str = "627c0a59";
const AGENT: &str = "a0693f4e";

/// The project planned to c1, held by SESSION.
fn project(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env_remove("CLAUDE_CODE_SESSION_ID").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
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

impl Project {
    /// A captured payload re-pointed at this project and session, with top-level fields overridden; what the hook printed.
    fn captured(&self, event: &str, fixture: &str, set: Value) -> String {
        let (code, out, err) = self.hook_raw(event, &self.fixture(fixture, set));
        assert_eq!(code, 0, "{event} {fixture}: {out}{err}");
        out.trim_end().to_string()
    }

    /// A commit, then the PostToolUse that sees it land (from `agent`, or the main conversation).
    fn commit_by(&self, message: &str, agent: Option<&str>) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        let who = agent.map(|a| json!({ "agent_id": a, "agent_type": "familiar-ai:investigator" })).unwrap_or(json!({}));
        let mut set = json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -am '{message}'") } });
        if let (Some(s), Some(w)) = (set.as_object_mut(), who.as_object()) {
            s.extend(w.clone());
        }
        self.captured("PostToolUse", "post_tool_use_bash", set)
    }

    fn of_kind(&self, kind: &str) -> Vec<Fact> {
        self.facts().into_iter().filter(|f| f.kind == kind).collect()
    }
}

/// `s` as it reads inside the hook's JSON text (its quotes escaped).
fn esc(s: &str) -> String {
    s.replace('"', "\\\"")
}

fn field<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The project with c1 held on two_failures, its holder on haiku.
fn held_twice_on_haiku(name: &str) -> Project {
    let p = project(name);
    // the capture switched haiku to sonnet; here the session switches to haiku
    p.captured("PostModelSwitch", "post_model_switch", json!({ "from_model": "claude-sonnet-5-5", "to_model": "claude-haiku-4-5-20251001" }));
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    assert!(p.commit_by("c1: tests first", None).contains(&esc("familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped")));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    p.commit_by("c1: first try", None);
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo howdy; }\ngreet\n");
    assert!(p.commit_by("c1: second try", None).contains(&esc("familiar-ai: \"greet says hello\" · two tries failed; a stronger model is taking over")));
    p
}

#[test]
fn the_switch_names_the_model_and_the_refusal_names_both_ways_out() {
    let p = held_twice_on_haiku("ways");
    let seen = p.of_kind("model_seen");
    assert_eq!(seen.len(), 1);
    assert_eq!((field(&seen[0], "agent_conversation_id"), field(&seen[0], "model")), (SESSION, "claude-haiku-4-5-20251001"));
    let out = p.captured("Stop", "stop", json!({}));
    assert!(out.contains("the person answers its card: try again, take it over yourself, a dependency comes first or release it; or a stronger model (sonnet) takes it over"), "{out}");
    assert_eq!(p.captured("PreModelSwitch", "pre_model_switch", json!({})), "{}");
}

#[test]
fn the_spawn_goes_to_a_sonnet_investigator_and_the_criterion_comes_back_green() {
    let p = held_twice_on_haiku("walk");
    // the holder's spawn (captured as haiku, general-purpose) is rewritten
    let out = p.captured("PreToolUse", "pre_tool_use_agent_spawn_on_haiku", json!({}));
    let want = json!({ "systemMessage": "familiar-ai: a stronger model (sonnet) is taking \"greet says hello\" over", "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow",
        "permissionDecisionReason": "familiar-ai: \"greet says hello\": two tries failed; a stronger model is taking over. This spawn runs as familiar-ai:investigator on sonnet, one rung up the ladder from this conversation, and \"greet says hello\" goes to it when it starts.",
        "updatedInput": { "description": "Run echo command", "prompt": "Run the shell command: echo hi", "subagent_type": "familiar-ai:investigator", "model": "sonnet", "run_in_background": false }
    }});
    assert_eq!(out, want.to_string(), "the exact rewrite");
    let asked = json!({ "tool_input": { "description": "d", "prompt": "p", "subagent_type": "familiar-ai:investigator", "model": "sonnet" } });
    assert_eq!(p.captured("PreToolUse", "pre_tool_use_agent_investigator", asked), "{}", "already the investigator at the rung");

    // its start: model_seen first, then the transfer; told the criterion as facts
    let started = p.captured("SubagentStart", "subagent_start_investigator", json!({ "agent_type": "familiar-ai:investigator" }));
    let me = format!("{SESSION}/{AGENT}");
    assert!(started.contains(&esc(&format!("familiar-ai: this subagent ({me}, sonnet) is the stronger model taking \"greet says hello\" over from {SESSION}, where two tries failed."))), "{started}");
    assert!(started.contains("its tests: tests/test_c1.sh; they are sealed: familiar-ai runs sh tests/test_c1.sh after each commit"), "{started}");
    assert!(started.contains("familiar-ai: proof that the work is done"), "the discipline: {started}");
    let seen = p.of_kind("model_seen");
    let tr = p.of_kind("criterion_transfer");
    assert_eq!((field(&seen[1], "agent_conversation_id"), field(&seen[1], "model")), (me.as_str(), "sonnet"));
    assert_eq!((field(&tr[0], "from_conversation"), field(&tr[0], "to_conversation")), (SESSION, me.as_str()));
    assert!(seen[1].t < tr[0].t, "model_seen lands first");
    assert!(p.status().contains("familiar-ai: \"greet says hello\" · the test fails, as it should before the fix · the code comes next · with a subagent"), "{}", p.status());

    // one investigator at a time; the holder's edits are refused, the investigator's allowed
    let second = p.captured("PreToolUse", "pre_tool_use_agent_spawn_on_haiku", json!({}));
    assert!(second.contains("\"permissionDecision\":\"deny\"") && second.contains("one runs at a time in a conversation, so this spawn waits"), "{second}");
    let edit = json!({ "tool_name": "Edit", "tool_input": { "file_path": p.path("src/app.sh").to_string_lossy() } });
    let holder = p.captured("PreToolUse", "pre_tool_use_edit", edit.clone());
    assert!(holder.contains("\"permissionDecision\":\"deny\"") && holder.contains("wait for it to come back"), "{holder}");
    let mut by_it = edit;
    by_it["agent_id"] = json!(AGENT);
    assert_eq!(p.captured("PreToolUse", "pre_tool_use_edit", by_it), "{}");

    // its stop waits for green
    let stop = json!({ "agent_type": "familiar-ai:investigator" });
    let blocked = p.captured("SubagentStop", "subagent_stop_investigator", stop.clone());
    assert!(blocked.contains("\"decision\":\"block\"") && blocked.contains("greet says hello"), "{blocked}");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    assert!(p.commit_by("c1: the fix", Some(AGENT)).contains(&esc("familiar-ai: ran the tests of \"greet says hello\": they pass · stamped")));
    assert_eq!(p.captured("SubagentStop", "subagent_stop_investigator", stop), "{}");

    // returned: the model corrected to resolvedModel, the criterion back
    p.captured("PostToolUse", "post_tool_use_agent_investigator", json!({}));
    let seen = p.of_kind("model_seen");
    let last = seen.last().expect("model_seen");
    assert_eq!((field(last, "agent_conversation_id"), field(last, "model")), (me.as_str(), "claude-sonnet-5-5"));
    let tr = p.of_kind("criterion_transfer");
    assert_eq!(tr.len(), 2, "one transfer back");
    assert_eq!((field(&tr[1], "from_conversation"), field(&tr[1], "to_conversation")), (me.as_str(), SESSION));
    assert!(p.status().contains("familiar-ai: \"greet says hello\" · locally green · unscoped · scope the command to prove in parallel\n"), "{}", p.status());
    let stop = p.captured("Stop", "stop", json!({}));
    assert!(!stop.contains("\"decision\"") && stop.contains("The branch is not pushed."), "the holder continues: every criterion green: {stop}");
}

#[test]
fn the_transfer_back_is_written_when_the_spawn_returns_green() {
    let p = held_twice_on_haiku("back");
    p.captured("SubagentStart", "subagent_start_investigator", json!({ "agent_type": "familiar-ai:investigator" }));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit_by("c1: the fix", Some(AGENT));
    assert_eq!(p.of_kind("criterion_transfer").len(), 1);
    p.captured("PostToolUse", "post_tool_use_agent_investigator", json!({}));
    let tr = p.of_kind("criterion_transfer");
    assert_eq!((field(&tr[1], "from_conversation"), field(&tr[1], "to_conversation")), (format!("{SESSION}/{AGENT}").as_str(), SESSION));
}

#[test]
fn the_persons_answer_lets_the_investigator_stop() {
    let p = held_twice_on_haiku("answer");
    p.captured("SubagentStart", "subagent_start_investigator", json!({ "agent_type": "familiar-ai:investigator" }));
    let stop = json!({ "agent_type": "familiar-ai:investigator" });
    assert!(p.captured("SubagentStop", "subagent_stop_investigator", stop.clone()).contains("\"decision\":\"block\""));
    let (code, out, err) = p.run(&["claim", "hold_answered", "--criterion_id", "c1", "--hold", "two_failures", "--choice", "take_over", "--typed", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.captured("SubagentStop", "subagent_stop_investigator", stop), "{}");
    let edit = json!({ "tool_name": "Edit", "tool_input": { "file_path": p.path("src/app.sh").to_string_lossy() } });
    assert!(!p.captured("PreToolUse", "pre_tool_use_edit", edit).contains("wait for it to come back"), "the person decided");
}

#[test]
fn a_spawn_with_nothing_held_is_left_alone_and_writes_only_the_model() {
    let p = project("plain");
    assert_eq!(p.captured("PreToolUse", "pre_tool_use_agent_spawn_on_haiku", json!({})), "{}");
    // with a run open, a plain subagent is told the person's words, and no criterion moves to it
    let started = p.captured("SubagentStart", "subagent_start_investigator", json!({}));
    assert!(started.contains("THE PERSON ASKED (their words):") && !started.contains("taking"), "{started}");
    assert!(p.of_kind("criterion_transfer").is_empty());
    p.captured("PostToolUse", "post_tool_use_agent_investigator", json!({}));
    let seen = p.of_kind("model_seen");
    assert_eq!(seen.len(), 1);
    assert_eq!(field(&seen[0], "model"), "claude-sonnet-5-5");
}
