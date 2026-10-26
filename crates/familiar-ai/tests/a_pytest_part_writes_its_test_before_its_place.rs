//! Test first, then pick the spot (the owner's plan of 2026-10-23, changes A and D) [draft names],
//! through the real binary. A part whose tests run under pytest, planned up to its test (lines 1–3
//! and 6–10) with no place yet: its planned test file may be written; the code waits, its refusal
//! naming the next step, familiar-ai try. When the person's words show no way to make the problem
//! happen, the person is asked first ("How do you make it happen? What do you run, and what do you
//! see?"), and the test waits for their answer; words they type are theirs, and reach the judge.
//! The try's listing is the binary's; the agent's reason for a place off it is claimable, not blank.

mod common;

use common::Project;
use familiar_ai_rules::{trace, View};
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";
const SHOWN: &str = "running `sh src/app.sh` prints hi, it should print hello";
const VAGUE: &str = "the greeting is wrong, please fix it";
const CASES: &str = r#"[{"case_id":"n1","kind":"normal","given":"the app run","expect":"hello"},{"case_id":"o1","kind":"opposite","given":"no run","expect":"nothing printed"}]"#;
const TEST: &str = "tests/test_greet.py";

fn project(name: &str, words: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create();
    p.hook("UserPromptSubmit", json!({ "prompt": words }));
    p.claim(&["intent_submitted", "--text", words]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a run", "--gives_back", "hello", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "the app says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST, "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_greet.py"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p
}

/// The refusal of a write of `file` before the tool runs, or `None` when it is allowed.
fn write_refused(p: &Project, file: &str) -> Option<String> {
    let path = p.repo().join(file).to_string_lossy().into_owned();
    let out = p.tool("PreToolUse", "Write", json!({ "file_path": path, "content": "x = 1\n" }));
    (out["hookSpecificOutput"]["permissionDecision"] == "deny").then(|| out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string())
}

#[test]
fn the_planned_test_is_written_before_the_place_and_the_code_waits_for_the_try() {
    let p = project("before-place", SHOWN);
    assert_eq!(write_refused(&p, TEST), None, "the planned test may be written");
    let refused = write_refused(&p, "src/app.sh").expect("the code waits");
    assert!(refused.contains("familiar-ai try"), "{refused}");
    // the listing is the binary's; a reason off the list is the agent's, and says something
    let (code, _, _) = p.try_claim(&["trace_listed", "--criterion_id", "c1", "--run_id", "r", "--sha", "s", "--red", "true", "--path_size", "1", "--ranker", "words", "--query", "q", "--top", "[]"]);
    assert_ne!(code, 0, "trace_listed is the binary's");
    let (code, _, err) = p.try_claim(&["off_list_reason", "--criterion_id", "c1", "--reason", "  "]);
    assert_ne!(code, 0, "a blank reason: {err}");
    let (code, out, err) = p.try_claim(&["off_list_reason"]);
    assert_eq!(code, 0, "its form: {out}{err}");
    assert!(out.contains("--reason"), "{out}");
}

#[test]
fn vague_words_ask_the_person_how_first_and_the_test_waits() {
    let p = project("vague", VAGUE);
    let refused = write_refused(&p, TEST).expect("the test waits on the person");
    assert!(refused.contains(trace::REPRO) || refused.contains("How do you make it happen"), "{refused}");
    let status = p.status_json();
    let cards: Vec<Value> = status["cards"].as_array().cloned().unwrap_or_default().into_iter().filter(|c| c["hold"] == trace::REPRO).collect();
    assert_eq!(cards.len(), 1, "{status}");
    assert_eq!(cards[0]["question"], trace::REPRO_QUESTION);
    assert!(familiar_ai::teach::card::is_card_hold(trace::REPRO));
}

#[test]
fn words_typed_on_the_card_are_the_persons_and_reach_the_judge() {
    let p = project("typed", VAGUE);
    let log = p.facts();
    let mut log: Vec<familiar_ai_facts::Fact> = log;
    let answer = json!({ "criterion_id": "c1", "hold": trace::REPRO, "choice": "typed", "typed": "I run sh src/app.sh and it prints hi" });
    log = familiar_ai_facts::append_raw(&log, "hold_answered", answer.as_object().cloned().unwrap_or_default(), 1.0, "_default_").expect("the person's answer");
    let words = familiar_ai::teach::person::words_and_picks(&log);
    assert!(words.iter().any(|w| w.contains("I run sh src/app.sh and it prints hi")), "{words:?}");
    let asked = familiar_ai::teach::judge::asked_for(&log);
    assert!(asked.iter().any(|w| w.contains("I run sh src/app.sh and it prints hi")), "{asked:?}");
    // the card's options, as the picker shows them
    let v = View::new(&log);
    let before: Vec<familiar_ai_facts::Fact> = log[..log.len() - 1].to_vec();
    let vb = View::new(&before);
    let card = familiar_ai_rules::terminal::cards(&vb).into_iter().find(|c| c.hold == trace::REPRO).expect("the card");
    let labels: Vec<String> = familiar_ai::teach::picker::card_options(&vb, &card).as_array().cloned().unwrap_or_default().iter().map(|o| o["label"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(labels, [trace::TYPE_IT_WORDS, trace::GO_AHEAD_WORDS]);
    assert_eq!(familiar_ai::teach::picker::choice_of(&vb, &card, trace::GO_AHEAD_WORDS).as_deref(), Some(trace::GO_AHEAD));
    assert!(familiar_ai_rules::terminal::cards(&v).iter().all(|c| c.hold != trace::REPRO), "answered");
}
