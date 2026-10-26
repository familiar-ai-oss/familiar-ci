//! Plan §6.7, "The holder ended" (S8 owed): the person's pick on a headless lane's
//! card, written to the lane's record after the lane's session ended, gives the
//! orchestrator the line that resumes it: `familiar-ai launch_lane --brief <file with
//! the CTA> --worktree <the lane> --resume`. Told once (`told {about: the pick's t}` in
//! the lane's record), at the person's prompt and the session's start; the CTA is a
//! file under the lane's `.familiar/runs/lane/` so `--brief` can read it.

mod common;

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
    fn picked_in_main(&self, answer: &str) {
        let card: Value = serde_json::from_str(CARD).expect("card");
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let asks = json!({ "questions": [{ "question": PROSE, "header": "familiar-ai", "options": [{ "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }], "multiSelect": false }] });
        let pre = self.hook_at(&self.repo(), PERSON, "PreToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": asks }));
        assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "ask", "{pre}");
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        self.hook_at(&self.repo(), PERSON, "PostToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": input, "tool_response": response }));
    }

    /// The lane's facts of `kind`, leaving out the cases card's answer and its telling: the card
    /// every full-path plan passes before its test (plan cases-card, 2026-10-25) [draft names].
    fn lane_facts(&self, at: &Path, kind: &str) -> Vec<Fact> {
        let record = Record::open_waiting(&at.join(".familiar/record"), std::time::Duration::from_secs(10)).expect("record");
        let cases: Vec<String> = record.read_by_kind("hold_answered").expect("read").iter().filter(|a| a.fields.get("hold").and_then(Value::as_str) == Some(common::project::CASES_CARD)).map(|a| a.t.to_string()).collect();
        let theirs = |f: &Fact| f.fields.get("hold").and_then(Value::as_str) == Some(common::project::CASES_CARD) || f.fields.get("about").and_then(Value::as_str).is_some_and(|t| cases.iter().any(|c| c == t));
        record.read_by_kind(kind).expect("read").into_iter().filter(|f| !theirs(f)).collect()
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

#[test]
fn an_ended_lanes_answer_gives_the_orchestrator_its_resume_line_once() {
    let p = project("ended");
    let wt = p.lane_with_a_card();
    p.lane_stops(&wt);
    let ended = p.lane_facts(&wt, "subagent_ended");
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].fields["agent_type"], "lane");
    p.picked_in_main("carry on with part 1 on the code as it is now");
    let t = p.lane_facts(&wt, "hold_answered")[0].t.to_string();
    assert!(p.lane_facts(&wt, "told").is_empty(), "nothing told yet");

    let shown = wt.display().to_string();
    let prompt = context(&p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "how is the lane doing?" })));
    let lead = format!("\"greet says hello\"'s lane {shown} has finished. Resume it with: familiar-ai launch_lane --brief ");
    assert!(prompt.contains(&lead), "{prompt}");
    let rest = prompt.split(&lead).nth(1).unwrap_or("");
    let (brief, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    assert_eq!(tail.lines().next().unwrap_or(""), format!("--worktree {shown} --resume"), "{prompt}");
    assert!(Path::new(brief).starts_with(wt.join(".familiar/runs/lane")), "the brief is under the lane's runs folder: {brief}");
    let cta = std::fs::read_to_string(brief).expect("the brief is a file");
    assert!(cta.contains("the person answered the question for \"greet says hello\": keep going as planned"), "{cta}");

    // told once, in the lane's record
    let told = p.lane_facts(&wt, "told");
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].fields["about"], t.as_str());
    let again = context(&p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "and now?" })));
    assert!(!again.contains("has finished"), "once: {again}");
    let start = context(&p.hook_at(&p.repo(), PERSON, "SessionStart", json!({ "source": "startup" })));
    assert!(!start.contains("has finished"), "once, at the start too: {start}");
}

#[test]
fn the_line_comes_at_the_sessions_start_too() {
    let p = project("start");
    let wt = p.lane_with_a_card();
    p.lane_stops(&wt);
    p.picked_in_main("drop it");
    let start = context(&p.hook_at(&p.repo(), PERSON, "SessionStart", json!({ "source": "startup" })));
    assert!(start.contains(&format!("'s lane {} has finished. Resume it with: familiar-ai launch_lane --brief ", wt.display())), "{start}");
    assert_eq!(p.lane_facts(&wt, "told").len(), 1);
}

#[test]
fn a_lane_still_running_is_not_resumed() {
    let p = project("running");
    let wt = p.lane_with_a_card();
    p.picked_in_main("carry on with part 1 on the code as it is now");
    let prompt = context(&p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "how is the lane doing?" })));
    assert!(!prompt.contains("has finished"), "{prompt}");
    assert!(p.lane_facts(&wt, "told").is_empty(), "the lane's own next hook tells it: {:?}", p.lane_facts(&wt, "told"));
}

/// The owner (2026-10-16, S8g): a lane resumed with `launch_lane --resume` starts again in the
/// same session; its SessionStart writes `holder_resumed`, and the orchestrator never offers a
/// running lane a resume line.
#[test]
fn a_resumed_lane_is_running_again_and_is_not_offered_a_resume_line() {
    let p = project("resumed");
    let wt = p.lane_with_a_card();
    p.lane_stops(&wt);
    p.set_env("CLAUDE_CODE_ENTRYPOINT", Some("sdk-cli"));
    p.hook_at(&wt, LANE, "SessionStart", json!({ "source": "resume" }));
    p.set_env("CLAUDE_CODE_ENTRYPOINT", None);
    let resumed = p.lane_facts(&wt, "holder_resumed");
    assert_eq!(resumed.len(), 1, "the resumed session is the holder again");
    assert_eq!(resumed[0].fields["agent_id"], LANE);
    p.picked_in_main("carry on with part 1 on the code as it is now");
    let prompt = context(&p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "how is the lane doing?" })));
    assert!(!prompt.contains("has finished"), "a running lane is not resumed: {prompt}");
    assert!(p.lane_facts(&wt, "told").is_empty(), "the lane's own next hook tells it");
}
