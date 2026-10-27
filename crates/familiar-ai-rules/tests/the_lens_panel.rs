//! The lens panel replaces the case-finder (the owner's plan of 2026-10-26, lens-panel.md, approved
//! with its names). Once a full-path part's contract is whole the panel is owed (`cases::OWED`, the
//! hold kept as it is): first the five lenses (`ambiguity`, `completeness`, `happy_path`,
//! `edge_cases`, `order_and_coupling`), each until its `lens_found` since the panel began (a
//! released lens with no answer is still owed), then the synthesis until its `cases_found`. The
//! card opens after the synthesis as today, with its choices (add_all, keep_planned, look_again);
//! look_again re-runs the whole panel, two panels at most per part. The synthesis may ask up to
//! three questions; the card is answered when its choice and every question has an answer
//! (`question_answered`, the person's pick: one of the question's options, or words typed).

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::{cases, holds, moves, plan, sample, View};
use serde_json::{json, Value};
use traced::*;

/// c1 planned, its promise with a whole contract.
fn whole() -> Log {
    let cases = json!([
        { "case_id": "n1", "kind": "normal", "given": "given n1", "expect": "expect n1" },
        { "case_id": "o1", "kind": "opposite", "given": "given o1", "expect": "expect o1" }
    ]);
    raw(&planned(), "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing", "cases": cases, "contract_version": 1 }))
}

fn released(log: &Log, lens: &str) -> Log {
    raw(log, "lens_released", json!({ "criterion_id": "c1", "lens": lens, "tool_use_id": format!("toolu_{}", log.len()) }))
}

/// The synthesis's finding on c1: `outcome` with `n` cases and the questions `qs` (id, options).
fn synthesis(log: &Log, outcome: &str, n: usize, qs: &[(&str, &[&str])]) -> Log {
    let cs: Vec<Value> = (1..=n)
        .map(|i| json!({ "case_id": format!("a{i}"), "short": format!("found case {i}"), "given": format!("given a{i}"), "expect": format!("expect a{i}"), "why": "the lens saw it" }))
        .collect();
    let mut f = json!({ "criterion_id": "c1", "outcome": outcome, "cases": cs, "why": if n == 0 { "the planned cases cover it" } else { "" }, "model": "claude-opus-5-5", "tokens": 6000 });
    if !qs.is_empty() {
        f["questions"] = qs.iter().map(|(id, options)| json!({ "question_id": id, "text": format!("question {id}"), "options": options, "why": "it decides a case" })).collect();
    }
    raw(log, "cases_found", f)
}

fn answered(log: &Log, choice: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": "c1", "hold": cases::CARD, "choice": choice, "typed": "" }))
}

fn picked(log: &Log, q: &str, choice: &str, typed: &str) -> Log {
    raw(log, "question_answered", json!({ "criterion_id": "c1", "question_id": q, "choice": choice, "typed": typed }))
}

const TWO: &[(&str, &[&str])] = &[("q1", &["every style", "only long"]), ("q2", &["yes", "no"])];

fn v(log: &Log) -> View<'_> {
    View::new(log)
}

#[test]
fn a_whole_contract_owes_the_panel_its_first_lens_first() {
    let log = whole();
    assert!(cases::owed(&v(&log), "c1"));
    assert_eq!(cases::lenses_owed(&v(&log), "c1"), cases::LENSES);
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some("ambiguity"));
    assert_eq!(cases::to_release(&v(&log), "c1"), Some("ambiguity"));
    assert!(!plan::test_may_be_written(&v(&log), "c1"));
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::OWED));
    // no panel on a promise with no contract
    let before = planned();
    assert!(!cases::owed(&v(&before), "c1"));
    assert_eq!(cases::owed_next(&v(&before), "c1"), None);
    assert_eq!(cases::to_release(&v(&before), "c1"), None);
}

#[test]
fn a_released_lens_with_no_answer_is_still_owed_and_the_next_release_skips_it() {
    let mut log = whole();
    for lens in ["ambiguity", "completeness"] {
        log = released(&log, lens);
    }
    assert_eq!(cases::lenses_owed(&v(&log), "c1"), cases::LENSES, "released is not answered");
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some("ambiguity"));
    assert_eq!(cases::to_release(&v(&log), "c1"), Some("happy_path"));
    for lens in ["happy_path", "edge_cases", "order_and_coupling"] {
        log = released(&log, lens);
    }
    assert_eq!(cases::to_release(&v(&log), "c1"), None, "every lens is out: wait for their answers");
    assert!(cases::owed(&v(&log), "c1"));
}

#[test]
fn the_lenses_answer_in_any_order_and_a_failed_lens_is_an_answer() {
    let mut log = whole();
    log = lens(&log, "c1", "edge_cases", "found");
    log = lens(&log, "c1", "ambiguity", "failed");
    assert_eq!(cases::lenses_owed(&v(&log), "c1"), ["completeness", "happy_path", "order_and_coupling"]);
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some("completeness"));
    for x in ["order_and_coupling", "happy_path", "completeness"] {
        log = lens(&log, "c1", x, "none");
    }
    assert!(cases::lenses_owed(&v(&log), "c1").is_empty());
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some(cases::SYNTHESIS));
    assert_eq!(cases::to_release(&v(&log), "c1"), Some(cases::SYNTHESIS));
    assert!(cases::owed(&v(&log), "c1"), "the synthesis is owed");
    assert!(!cases::card_open(&v(&log), "c1"));
    let lensed: Vec<&str> = cases::lens_findings(&v(&log), "c1").iter().filter_map(|f| f.fields.get("lens").and_then(Value::as_str)).collect();
    assert_eq!(lensed, cases::LENSES, "the panel's answers, one per lens, in the lenses' order");
    let log = released(&log, cases::SYNTHESIS);
    assert_eq!(cases::to_release(&v(&log), "c1"), None);
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some(cases::SYNTHESIS), "released, not answered");
}

fn stop(log: &Log) -> Decision {
    decide(&v(log), &Event::Stops { main: true })
}

/// A stop goes while every owed call of the held part's panel is out, so the agent has nothing to
/// start and only waits for the answers (the owner's answer, 2026-10-27: only waiting is left); a
/// background lens's answer comes back after the turn ends. With a lens or the synthesis still to
/// start the stop is refused on the panel as before, and an edit still waits for the panel.
#[test]
fn a_stop_goes_while_only_waiting_is_left_on_the_panel() {
    let mut log = whole();
    assert_eq!(hold_of(&stop(&log)).as_deref(), Some(cases::OWED), "nothing started");
    for l in &cases::LENSES[..4] {
        log = released(&log, l);
    }
    assert_eq!(hold_of(&stop(&log)).as_deref(), Some(cases::OWED), "a lens still to start");
    log = released(&log, "order_and_coupling");
    assert_eq!(hold_of(&stop(&log)), None, "the five out, nothing to start: {:?}", stop(&log));
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::OWED), "an edit still waits for the panel");
    // two answered, the other three still out
    log = lens(&log, "c1", "ambiguity", "none");
    log = lens(&log, "c1", "edge_cases", "failed");
    assert_eq!(hold_of(&stop(&log)), None, "still only waiting");
    // the five answered: the synthesis is to start, then it is out
    let log = lenses(&whole(), "c1");
    assert_eq!(hold_of(&stop(&log)).as_deref(), Some(cases::OWED), "the synthesis to start");
    let log = released(&log, cases::SYNTHESIS);
    assert_eq!(hold_of(&stop(&log)), None, "the synthesis out");
}

#[test]
fn a_synthesis_before_the_five_lenses_closes_nothing() {
    let log = synthesis(&lens(&whole(), "c1", "ambiguity", "none"), "found", 1, &[]);
    assert!(cases::owed(&v(&log), "c1"));
    assert!(!cases::card_open(&v(&log), "c1"));
    assert!(cases::findings(&v(&log), "c1").is_empty());
    assert_eq!(cases::owed_next(&v(&log), "c1"), Some("completeness"));
}

#[test]
fn the_synthesis_closes_the_panel_and_opens_the_card() {
    let log = synthesis(&lenses(&whole(), "c1"), "found", 2, &[]);
    let w = v(&log);
    assert!(!cases::owed(&w, "c1"));
    assert_eq!(cases::owed_next(&w, "c1"), None);
    assert_eq!(cases::to_release(&w, "c1"), None);
    assert!(cases::card_open(&w, "c1"));
    assert_eq!(cases::offered(&w, "c1"), [cases::ADD_ALL, cases::KEEP]);
    assert_eq!(cases::findings(&w, "c1").len(), 1);
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::CARD));
    assert!(cases::questions(&w, "c1").is_empty());
}

#[test]
fn look_again_reruns_the_whole_panel_and_two_panels_are_the_most() {
    let mut log = released(&whole(), "ambiguity");
    log = synthesis(&lenses(&log, "c1"), "none", 0, &[]);
    assert_eq!(cases::offered(&v(&log), "c1"), [cases::KEEP, cases::AGAIN]);
    log = answered(&log, cases::AGAIN);
    let w = v(&log);
    assert!(cases::owed(&w, "c1"));
    assert_eq!(cases::lenses_owed(&w, "c1"), cases::LENSES, "the first panel's answers do not count");
    assert_eq!(cases::owed_next(&w, "c1"), Some("ambiguity"));
    assert_eq!(cases::to_release(&w, "c1"), Some("ambiguity"), "the first panel's releases do not count");
    assert!(cases::lens_findings(&w, "c1").is_empty());
    // a synthesis before the second panel's lenses closes nothing
    let early = synthesis(&log, "found", 1, &[]);
    assert!(cases::owed(&v(&early), "c1"));
    assert_eq!(cases::findings(&v(&early), "c1").len(), 1);
    // the second panel, then its synthesis: no third
    log = synthesis(&lenses(&log, "c1"), "none", 0, &[]);
    let w = v(&log);
    assert!(cases::card_open(&w, "c1"));
    assert_eq!(cases::findings(&w, "c1").len(), 2);
    assert_eq!(cases::offered(&w, "c1"), [cases::KEEP, cases::TYPED]);
}

#[test]
fn the_card_stays_open_until_its_choice_and_every_question_have_an_answer() {
    let log = synthesis(&lenses(&whole(), "c1"), "found", 1, TWO);
    let w = v(&log);
    let qs = cases::questions(&w, "c1");
    assert_eq!(qs.iter().map(|q| q.question_id.as_str()).collect::<Vec<_>>(), ["q1", "q2"]);
    assert_eq!(qs[0].options, ["every style", "only long"]);
    assert_eq!(cases::unanswered_questions(&w, "c1"), ["q1", "q2"]);
    assert!(!cases::choice_answered(&w, "c1"));
    // the choice first: the card stays open on the questions
    let log = answered(&log, cases::ADD_ALL);
    let w = v(&log);
    assert!(cases::choice_answered(&w, "c1"));
    assert!(cases::card_open(&w, "c1"));
    assert!(!plan::test_may_be_written(&w, "c1"));
    let log = picked(&log, "q2", "no", "");
    let w = v(&log);
    assert_eq!(cases::unanswered_questions(&w, "c1"), ["q1"]);
    assert_eq!(cases::question_answer(&w, "c1", "q2").and_then(|f| f.fields.get("choice")).and_then(Value::as_str), Some("no"));
    assert!(cases::question_answer(&w, "c1", "q1").is_none());
    assert!(cases::card_open(&w, "c1"));
    let log = picked(&log, "q1", "typed", "only under --tb=long and short");
    let w = v(&log);
    assert!(cases::unanswered_questions(&w, "c1").is_empty());
    assert!(!cases::card_open(&w, "c1"));
    assert_eq!(cases::missing(&w, "c1"), ["a1"], "add_all asked for the found case");
}

#[test]
fn the_questions_first_then_the_choice_closes_the_card_too() {
    let mut log = synthesis(&lenses(&whole(), "c1"), "none", 0, TWO);
    log = picked(&picked(&log, "q1", "only long", ""), "q2", "yes", "");
    assert!(cases::card_open(&v(&log), "c1"), "the choice is owed");
    assert!(!cases::choice_answered(&v(&log), "c1"));
    log = answered(&log, cases::KEEP);
    assert!(!cases::card_open(&v(&log), "c1"));
    assert!(plan::test_may_be_written(&v(&log), "c1"));
}

#[test]
fn a_pick_outside_the_options_or_typed_with_nothing_typed_is_no_answer() {
    let log = answered(&synthesis(&lenses(&whole(), "c1"), "none", 0, &[("q1", &["yes", "no"])]), cases::KEEP);
    for (choice, typed) in [("maybe", ""), ("typed", ""), ("typed", "   ")] {
        let after = picked(&log, "q1", choice, typed);
        assert!(cases::card_open(&v(&after), "c1"), "{choice:?} {typed:?}");
        assert_eq!(cases::unanswered_questions(&v(&after), "c1"), ["q1"]);
    }
}

#[test]
fn look_again_with_questions_open_reruns_the_panel_and_its_new_questions_need_new_answers() {
    let mut log = synthesis(&lenses(&whole(), "c1"), "none", 0, &[("q1", &["yes", "no"])]);
    log = picked(&log, "q1", "yes", "");
    log = answered(&log, cases::AGAIN);
    assert!(cases::owed(&v(&log), "c1"));
    log = synthesis(&lenses(&log, "c1"), "none", 0, &[("q1", &["yes", "no"])]);
    let w = v(&log);
    assert!(cases::card_open(&w, "c1"));
    assert_eq!(cases::unanswered_questions(&w, "c1"), ["q1"], "the first synthesis's answer is not this one's");
}

#[test]
fn the_cards_clears_name_the_choices_until_answered_then_each_open_question() {
    let log = synthesis(&lenses(&whole(), "c1"), "found", 1, TWO);
    let clears = |log: &Log| -> Vec<(String, String)> {
        holds::cases_card_clears(&v(log), "c1")
            .iter()
            .map(|m| {
                let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                (s("kind"), if s("kind") == "question_answered" { s("question_id") } else { s("choice") })
            })
            .collect()
    };
    let qa = |q: &str| ("question_answered".to_string(), q.to_string());
    let ha = |c: &str| ("hold_answered".to_string(), c.to_string());
    assert_eq!(clears(&log), [ha(cases::ADD_ALL), ha(cases::KEEP), ha(cases::TYPED), qa("q1"), qa("q2")]);
    let log = answered(&picked(&log, "q1", "only long", ""), cases::KEEP);
    assert_eq!(clears(&log), [qa("q2")]);
}

/// The panel's moves over the sample world: while a lens is owed, its answer (the next in order);
/// once the five answered, the synthesis's findings, one with questions; while the card is open, its
/// choices until one is answered, and a pick on the first open question.
#[test]
fn the_panels_moves_follow_what_is_owed() {
    let log = whole();
    let kinds = |log: &Log| -> Vec<(String, String)> {
        moves::cases_moves(&v(log), "c1")
            .iter()
            .map(|m| (m.kind.clone(), m.fields.get("lens").or_else(|| m.fields.get("outcome")).or_else(|| m.fields.get("question_id")).or_else(|| m.fields.get("choice")).and_then(Value::as_str).unwrap_or("").to_string()))
            .collect()
    };
    let lf = |l: &str| ("lens_found".to_string(), l.to_string());
    assert_eq!(kinds(&log), [lf("ambiguity")]);
    let log = lens(&log, "c1", "ambiguity", "none");
    assert_eq!(kinds(&log), [lf("completeness")]);
    let log = lenses(&whole(), "c1");
    let synth = moves::cases_moves(&v(&log), "c1");
    assert!(synth.iter().all(|m| m.kind == cases::FOUND), "{synth:?}");
    let with_questions = synth.iter().find(|m| m.fields.get("questions").and_then(Value::as_array).is_some_and(|q| !q.is_empty())).expect("a synthesis with questions");
    let after = sample::apply_move(&log, with_questions, AT, TENANT).expect("lands");
    assert!(cases::card_open(&v(&after), "c1"));
    let open = kinds(&after);
    assert!(open.contains(&("hold_answered".to_string(), cases::ADD_ALL.to_string())), "{open:?}");
    let first = cases::unanswered_questions(&v(&after), "c1")[0].clone();
    assert!(open.contains(&("question_answered".to_string(), first.clone())), "{open:?}");
    let pick = moves::cases_moves(&v(&after), "c1").into_iter().find(|m| m.kind == "question_answered").expect("a pick");
    let after = sample::apply_move(&after, &pick, AT, TENANT).expect("lands");
    assert!(!cases::unanswered_questions(&v(&after), "c1").contains(&first), "the pick answers its question");
    // once the choice is answered, only the open questions' picks
    let keep = moves::cases_moves(&v(&after), "c1").into_iter().find(|m| m.fields.get("choice").and_then(Value::as_str) == Some(cases::KEEP)).expect("keep");
    let after = sample::apply_move(&after, &keep, AT, TENANT).expect("lands");
    assert!(moves::cases_moves(&v(&after), "c1").iter().all(|m| m.kind == "question_answered"), "{:?}", kinds(&after));
}
