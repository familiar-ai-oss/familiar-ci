//! When a person's card is open, every place the agent reads about it tells
//! the agent to ask with AskUserQuestion and not to claim hold_answered.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "d925ef40-8c5a-4ff1-a3f9-84909cb79d71";
#[allow(dead_code)]
const CARD: &str = r#"{"questions":[{"question":"\"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?","header":"familiar-ai","options":[{"label":"plan it again","description":"re-plan this criterion"},{"label":"keep going as planned","description":"keep going as planned"},{"label":"release it","description":"release it: it stops blocking the string"},{"label":"something else","description":"type what to do; it is recorded as your words"}],"multiSelect":false}]}"#;
#[allow(dead_code)]
const FACTS: &str = "familiar-ai: a question for you is open for \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now? The choices are: 1) plan it again 2) keep going as planned 3) release it, or something else. The AskUserQuestion tool presents it to the person; a choice made there is recorded.";

/// No `gh` on the PATH: the push question is not read here.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
}

impl Project {
    /// Claims the plan with the anchored lines edited before the last line, so
    /// c1 holds on anchor_moved.
    fn moved(name: &str) -> Project {
        let p = project(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
        p
    }
}

fn text(v: &Value, path: &[&str]) -> String {
    let mut cur = v;
    for k in path {
        cur = &cur[*k];
    }
    cur.as_str().unwrap_or("").to_string()
}

#[test]
fn the_rulebook_says_to_ask_with_the_picker_and_never_to_claim_hold_answered() {
    let p = project("rulebook");
    let start = p.hook("SessionStart", json!({}));
    let context = text(&start, &["hookSpecificOutput", "additionalContext"]);
    assert!(context.contains("When a card is open, ask the person with AskUserQuestion"), "{context}");
    assert!(!context.contains("familiar-ai claim hold_answered"), "{context}");
}

#[test]
fn an_agents_own_hold_answered_is_told_to_ask_with_the_picker() {
    let p = Project::moved("claim");
    let (code, _, err) = p.run_as(
        Some(SESSION),
        &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "anchor_moved", "--choice", "keep_going", "--typed", ""],
        "",
    );
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: "),
        "{err}"
    );
    assert!(!err.contains("open a terminal of your own"), "{err}");
}

#[test]
fn a_stop_with_a_card_open_is_refused_asking_for_the_picker_first() {
    let p = Project::moved("stop");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let reason = text(&stop, &["reason"]);
    assert_eq!(
        reason.lines().next(),
        Some("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card"),
        "{reason}"
    );
}

#[test]
fn the_cards_facts_say_to_ask_the_person_and_not_to_claim_hold_answered() {
    let p = Project::moved("facts");
    let prompt = p.hook("UserPromptSubmit", json!({ "prompt": "what now?" }));
    let context = text(&prompt, &["hookSpecificOutput", "additionalContext"]);
    assert!(context.contains("Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself."), "{context}");
    assert!(!context.contains("presents it to the person"), "{context}");
}

#[test]
fn the_plan_skill_carries_no_copy_of_the_rulebook() {
    let skill = std::fs::read_to_string(format!("{}/../../plugin/skills/plan/SKILL.md", env!("CARGO_MANIFEST_DIR"))).expect("plan skill");
    assert!(!skill.contains("<!-- discipline-begin -->") && !skill.contains("<!-- discipline-end -->"), "the skill still has the block");
    for line in [
        "Nothing is edited or committed until planning_confirm_complete",
        "The record is a database file: read it with familiar-ai status",
        "Stopping is refused until every criterion is green, proven or released.",
    ] {
        assert!(!skill.contains(line), "the skill carries a rulebook line: {line}");
    }
    let p = project("block");
    let start = p.hook("SessionStart", json!({}));
    let context = text(&start, &["hookSpecificOutput", "additionalContext"]);
    assert!(context.contains("When a card is open, ask the person with AskUserQuestion"), "the rulebook itself keeps the picker rule");
}
