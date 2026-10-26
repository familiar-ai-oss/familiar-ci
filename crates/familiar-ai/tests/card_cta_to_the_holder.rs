//! Plan §6.7 (lane S7 review, item 2): the AFTER_PICK CTA, built from the log after the pick,
//! goes to whoever holds the work: a running subagent at its next hook, an ended lane through its
//! resume brief. The orchestrator's own reply to a pick on a part a subagent holds says only that
//! the holder will be told. (Its reply to a pick on another checkout's card keeps its words:
//! card_from_another_checkout.rs is sealed and pins them; the owner's word is owed.)

mod common;
mod run_cards;

use std::path::{Path, PathBuf};

use common::Project;
use familiar_ai_facts::{Fact, Record};
use serde_json::{json, Value};

const LANE: &str = "9b2c4d6e-1f3a-4b5c-8d7e-0a1b2c3d4e5f";
const PERSON: &str = "3e5f7a9b-2c4d-4e6f-9a1b-3c5d7e9f1a2b";
const PROSE: &str = "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?";
const CARD: &str = r#"{"questions":[{"question":"you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?","header":"familiar-ai","options":[{"label":"carry on with part 1 on the code as it is now","description":"carry on with part 1 on the code as it is now"},{"label":"have the agent pick the code for part 1 again","description":"have the agent pick the code for part 1 again"},{"label":"drop it","description":"drop it"},{"label":"something else","description":"type what to do in your own words"}],"multiSelect":false}]}"#;

impl Project {
    /// A lane in a worktree with a card open on c1, held by the lane's session, which `launch_lane` would have written.
    fn lane_with_a_card(&self) -> PathBuf {
        let wt = self.dir.join("lane");
        self.git(&["worktree", "add", "-q", "-b", "lane", &wt.to_string_lossy()]);
        assert_eq!(self.run_in(&wt, &["setup"], "").0, 0);
        std::fs::create_dir_all(wt.join(".familiar/runs/lane")).expect("the lane's folder");
        std::fs::write(wt.join(".familiar/runs/lane/session"), LANE).expect("the lane's session file");
        assert_eq!(self.run_in(&wt, &["on"], "").0, 0);
        let claim = |args: &[&str]| {
            let mut argv = vec!["claim"];
            argv.extend_from_slice(args);
            let (code, out, err) = self.run_in(&wt, &argv, "");
            assert_eq!(code, 0, "claim {args:?}: {out}{err}");
        };
        claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        common::cases_kept_at(&wt, "c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        claim(&["criterion_head", "--criterion_id", "c1"]);
        std::fs::write(wt.join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("outside edit");
        claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", LANE]);
        claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        wt
    }

    fn hook_at(&self, at: &Path, session: &str, event: &str, fields: Value) -> Value {
        let mut payload = json!({ "hook_event_name": event, "session_id": session, "cwd": at.to_string_lossy() });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        let (code, out, err) = self.run_in(at, &["hook", event], &payload.to_string());
        assert_eq!(code, 0, "{event}: {err}");
        serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{event} printed no JSON ({e}): {out}"))
    }

    /// The lane's headless session stops with only the person's card left: its agent ends.
    fn lane_stops(&self, wt: &Path) {
        self.set_env("CLAUDE_CODE_ENTRYPOINT", Some("sdk-cli"));
        let stop = self.hook_at(wt, LANE, "Stop", json!({}));
        self.set_env("CLAUDE_CODE_ENTRYPOINT", None);
        assert!(stop.get("decision").is_none(), "the headless stop goes: {stop}");
    }

    /// The person's pick on the lane's card, asked and answered in main.
    fn picked_in_main(&self, answer: &str) -> Value {
        let card: Value = serde_json::from_str(CARD).expect("card");
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let asks = json!({ "questions": [{ "question": PROSE, "header": "familiar-ai", "options": [{ "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }], "multiSelect": false }] });
        let pre = self.hook_at(&self.repo(), PERSON, "PreToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": asks }));
        assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "ask", "{pre}");
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        self.hook_at(&self.repo(), PERSON, "PostToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": input, "tool_response": response }))
    }

    fn lane_facts(&self, at: &Path, kind: &str) -> Vec<Fact> {
        let record = Record::open_waiting(&at.join(".familiar/record"), std::time::Duration::from_secs(10)).expect("record");
        record.read_by_kind(kind).expect("read")
    }
}

fn project(name: &str) -> Project {
    let p = Project::builder(name).no_session().env("PATH", "/usr/bin:/bin").env("FAMILIAR_AI_BENCH", "0").create();
    assert_eq!(p.run(&["off"], "").0, 0);
    p
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

const SESSION: &str = "6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f";

/// c1 planned and held by subagent a7 of SESSION, its fix_elsewhere card open.
fn subagent_holds(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", &format!("{SESSION}/a7")]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "dependency", "--where", "the greeting library", "--needs", "a release"]);
    p
}

fn call_of(p: &Project, agent: Option<&str>) -> String {
    let mut fields = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    if let Some(a) = agent {
        fields["agent_id"] = json!(a);
        fields["agent_type"] = json!("general-purpose");
    }
    context(&p.hook("PreToolUse", fields))
}

const RELEASED: &str = "\"greet says hello\" is released. Next: no part is left; the plan is complete.";

#[test]
fn a_running_subagent_holder_gets_the_cta_at_its_next_call() {
    let p = subagent_holds("subagent");
    let (code, out, err) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "fix_elsewhere", "--choice", "release", "--typed", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(!call_of(&p, None).contains(RELEASED), "not the orchestrator's");
    let told = call_of(&p, Some("a7"));
    assert_eq!(told.matches(RELEASED).count(), 1, "{told}");
    assert!(!call_of(&p, Some("a7")).contains(RELEASED), "once");
}

#[test]
fn an_ended_lane_gets_the_cta_in_its_resume_brief_and_the_orchestrator_is_told_it_will_be() {
    let p = project("lane-cta");
    let wt = p.lane_with_a_card();
    p.lane_stops(&wt);
    let reply = context(&p.picked_in_main("drop it"));
    assert!(!reply.contains("is released. Next:"), "the CTA is the holder's: {reply}");
    let prompt = context(&p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "and the lane?" })));
    let lead = "--brief ";
    let brief = prompt.split(lead).nth(1).and_then(|r| r.split(' ').next()).unwrap_or("");
    let cta = std::fs::read_to_string(brief).unwrap_or_else(|e| panic!("{brief}: {e}: {prompt}"));
    assert!(cta.contains(RELEASED), "{cta}");
    let _ = p.lane_facts(&wt, "told");
}

#[test]
fn the_orchestrators_pick_on_a_subagents_part_says_the_holder_will_be_told() {
    let p = run_cards::sealed("picked-for-a7");
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", &format!("{}/a7", run_cards::SESSION)]);
    run_cards::relabel(&p, "crashed", json!({}));
    let reply = run_cards::context(&run_cards::pick(&p, &run_cards::ask(&p), "drop it"));
    assert!(reply.contains("The answer is recorded; the holder will be told what to do next."), "{reply}");
    assert!(!reply.contains("is released. Next:"), "{reply}");
    let told = call_of(&p, Some("a7"));
    assert_eq!(told.matches(RELEASED).count(), 1, "{told}");
}
