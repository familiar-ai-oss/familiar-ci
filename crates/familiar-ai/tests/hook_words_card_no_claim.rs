//! A card's refusal does not hand the agent the person's claim: while a card
//! hold stands, a refused stop says the person answers its card and
//! carries the card's facts (ask with AskUserQuestion), and no `hold_answered`
//! claim line. The person's own terminal command stays where it was: in the
//! refusal of an agent's own `hold_answered` claim and in `status --json`.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "7a6b5c4d-3e2f-4a1b-9c8d-7e6f5a4b3c2d";

/// c1 planned on src/app.sh greet lines 1–3, the lines edited outside the work before the
/// plan was confirmed: the anchor_moved card is open.
fn moved(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").create();
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

#[test]
fn a_stop_while_the_card_stands_says_the_person_answers_it_and_hands_no_claim() {
    let p = moved("stop");
    let out = p.hook("Stop", json!({}));
    assert_eq!(out["decision"], "block", "{out}");
    let reason = out["reason"].as_str().unwrap_or("");
    assert!(
        reason.starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\nfamiliar-ai: \"greet says hello\" · the code at the plan's place changed after the plan was made · next: the person answers its card: plan it again, keep going as planned or release it\n"),
        "{reason}"
    );
    assert!(reason.contains("Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself."), "the card's facts: {reason}");
    assert!(!reason.contains("familiar-ai claim hold_answered"), "no claim of the person's: {reason}");
    assert!(!reason.lines().any(|l| l.trim_start().starts_with("claim it:") || l.trim_start().starts_with("or:")), "no claim line at all: {reason}");
}

#[test]
fn the_persons_own_command_stays_in_the_agents_refused_claim_and_in_status() {
    let p = moved("persons");
    let (code, _, stderr) = p.run_as(Some(SESSION), &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "anchor_moved", "--choice", "keep_going", "--typed", ""], "");
    assert_eq!(code, 1);
    assert!(stderr.contains("(in a terminal of their own the person can also run: familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed '')"), "{stderr}");
    let status = p.status_json();
    assert_eq!(status["cards"][0]["claim"], "familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice re_plan --typed ''", "{status}");
}
