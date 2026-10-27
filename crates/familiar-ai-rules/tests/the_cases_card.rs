//! The cases card (the owner's plan of 2026-10-25) [draft names]. Once a full-path part's contract
//! is whole, the case-finder is owed (`cases::OWED`, the agent's hold): the agent releases the
//! subagent and the binary records its answer (`cases_found`). Then the person's card
//! (`cases::CARD`) shows the planned cases and the found ones: `add_all` makes every found case a
//! case the promise must hold, `keep_planned` keeps the plan as it is, `look_again` owes one more
//! finder (two at most per part), and words typed are numbers picking found cases or the person's
//! own case `p1`. After the last look the card offers keep, or add the person's own (the owner's
//! ruling, 2026-10-25: "Keep, or add my own"), and the person's own case picked with nothing typed
//! is no answer. The test waits for all of it: the plan's line 3 holds only when no finder is
//! owed, no card is open and every case the person asked for is in the promise.

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{cases, plan, terminal, View};
use serde_json::{json, Value};
use traced::*;

/// c1 planned up to its test, its promise with a whole contract (two cases).
fn whole() -> Log {
    promise(&planned(), &["n1", "o1"])
}

/// The promise p1 claimed again with the cases `ids` (n1 normal, o1 opposite, any other normal).
fn promise(log: &Log, ids: &[&str]) -> Log {
    let cases: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "case_id": id, "kind": if *id == "o1" { "opposite" } else { "normal" }, "given": format!("given {id}"), "expect": format!("expect {id}") }))
        .collect();
    raw(log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing", "cases": cases, "contract_version": 1 }))
}

// Definition change (the owner's plan of 2026-10-26, the lens panel, approved): the synthesis is
// recorded only after the five lenses answered, so a finding comes after the panel's lenses.
/// The panel's synthesis on c1 after its five lenses: `outcome` with `n` cases a1..an.
fn found(log: &Log, outcome: &str, n: usize) -> Log {
    let log = &lenses(log, "c1");
    let cs: Vec<Value> = (1..=n)
        .map(|i| json!({ "case_id": format!("a{i}"), "short": format!("found case {i}"), "given": format!("given a{i}"), "expect": format!("expect a{i}"), "why": "the code branches here" }))
        .collect();
    raw(log, "cases_found", json!({ "criterion_id": "c1", "outcome": outcome, "cases": cs, "why": if n == 0 { "the planned cases cover it" } else { "" }, "model": "claude-sonnet-5-5", "tokens": 4000 }))
}

fn answered(log: &Log, choice: &str, typed: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": "c1", "hold": cases::CARD, "choice": choice, "typed": typed }))
}

fn card(log: &Log) -> Option<terminal::Card> {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == cases::CARD)
}

fn offered(log: &Log) -> Vec<String> {
    card(log).map(|c| c.choices.into_iter().map(|(ch, _)| ch).collect()).unwrap_or_default()
}

fn may_write(log: &Log) -> bool {
    plan::test_may_be_written(&View::new(log), "c1")
}

#[test]
fn a_whole_contract_on_a_full_part_owes_the_finder_and_the_test_waits() {
    let log = whole();
    let v = View::new(&log);
    assert!(cases::owed(&v, "c1"));
    assert!(!cases::card_open(&v, "c1"));
    assert!(!may_write(&log), "the test waits for the finder");
    assert!(!plan::line_3_of(&v, "c1"));
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::OWED));
    assert!(whys(&log).iter().any(|w| w == cases::OWED));
}

#[test]
fn no_finder_on_the_light_path_or_before_the_contract() {
    // a promise from before the change (no contract): nothing owed, the test as today
    let log = planned();
    assert!(!cases::owed(&View::new(&log), "c1"));
    assert!(may_write(&log));
    // the light path: no promise asked, no finder
    let light = raw(&whole(), "criterion", json!({ "criterion_id": "c1", "text": "the export writes each row once", "path": "light", "behavior_id": "b1", "promise_id": "p1" }));
    assert!(!cases::owed(&View::new(&light), "c1"));
}

#[test]
fn a_finding_with_cases_opens_the_persons_card() {
    let log = found(&whole(), "found", 2);
    let v = View::new(&log);
    assert!(!cases::owed(&v, "c1"));
    assert!(cases::card_open(&v, "c1"));
    assert_eq!(offered(&log), vec![cases::ADD_ALL, cases::KEEP]);
    assert!(!may_write(&log), "the test waits for the person");
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::CARD));
}

#[test]
fn add_all_makes_every_found_case_one_the_promise_must_hold() {
    let log = answered(&found(&whole(), "found", 2), cases::ADD_ALL, "");
    let v = View::new(&log);
    assert!(!cases::card_open(&v, "c1"));
    assert_eq!(cases::missing(&v, "c1"), vec!["a1", "a2"]);
    assert!(!plan::line_3_of(&v, "c1"), "line 3 waits for the cases asked for");
    assert!(!may_write(&log));
    // the agent claims the promise again with them: line 3 holds, and no new finder is owed
    let log = promise(&log, &["n1", "o1", "a1", "a2"]);
    let v = View::new(&log);
    assert!(cases::missing(&v, "c1").is_empty());
    assert!(!cases::owed(&v, "c1"), "a promise claimed again to add the cases owes no new finder");
    assert!(plan::line_3_of(&v, "c1"));
    assert!(may_write(&log));
}

#[test]
fn keep_planned_closes_the_card_and_asks_for_nothing() {
    let log = answered(&found(&whole(), "found", 2), cases::KEEP, "");
    let v = View::new(&log);
    assert!(!cases::card_open(&v, "c1"));
    assert!(cases::missing(&v, "c1").is_empty());
    assert!(may_write(&log));
}

#[test]
fn typed_numbers_pick_found_cases_and_other_words_are_the_persons_own_case() {
    let log = answered(&found(&whole(), "found", 3), "typed", "1 and 3");
    assert_eq!(cases::missing(&View::new(&log), "c1"), vec!["a1", "a3"]);
    let log = answered(&found(&whole(), "found", 3), "typed", "1,3");
    assert_eq!(cases::missing(&View::new(&log), "c1"), vec!["a1", "a3"]);
    let log = answered(&found(&whole(), "found", 2), "typed", "an empty export file writes nothing");
    let v = View::new(&log);
    assert_eq!(cases::missing(&v, "c1"), vec!["p1"]);
    assert_eq!(cases::persons_case(&v, "c1", "p1").as_deref(), Some("an empty export file writes nothing"));
    let log = promise(&log, &["n1", "o1", "p1"]);
    assert!(may_write(&log));
}

#[test]
fn none_found_offers_keep_or_look_again_and_look_again_owes_one_more_finder() {
    let log = found(&whole(), "none", 0);
    assert_eq!(offered(&log), vec![cases::KEEP, cases::AGAIN]);
    let log = answered(&log, cases::AGAIN, "");
    let v = View::new(&log);
    assert!(cases::owed(&v, "c1"), "look again owes another finder");
    assert!(!cases::card_open(&v, "c1"));
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(cases::OWED));
    // the second finding opens the card again, and a third finder is not offered: keep the planned
    // cases, or add the person's own (the owner's ruling, 2026-10-25: "Keep, or add my own")
    let log = found(&log, "none", 0);
    assert!(cases::card_open(&View::new(&log), "c1"));
    assert_eq!(offered(&log), vec![cases::KEEP, cases::TYPED]);
}

#[test]
fn after_the_last_look_the_card_offers_keep_or_the_persons_own_case() {
    for (first, second) in [("none", "none"), ("none", "failed"), ("failed", "none"), ("failed", "failed")] {
        let log = found(&answered(&found(&whole(), first, 0), cases::AGAIN, ""), second, 0);
        assert!(cases::card_open(&View::new(&log), "c1"), "{first}, {second}");
        assert_eq!(offered(&log), vec![cases::KEEP, cases::TYPED], "{first}, {second}");
        // words typed are the person's own case
        let own = answered(&log, cases::TYPED, "an empty export file writes nothing");
        let v = View::new(&own);
        assert!(!cases::card_open(&v, "c1"));
        assert_eq!(cases::missing(&v, "c1"), vec!["p1"]);
        // the person's own case picked with nothing typed is no answer: the card stays open as it was
        let blank = answered(&log, cases::TYPED, "  ");
        let v = View::new(&blank);
        assert!(cases::card_open(&v, "c1"), "{first}, {second}: nothing typed closes nothing");
        assert!(cases::missing(&v, "c1").is_empty());
        assert_eq!(offered(&blank), vec![cases::KEEP, cases::TYPED]);
        assert_eq!(hold_of(&edit(&blank, TEST)).as_deref(), Some(cases::CARD));
        // keep closes it, asking for nothing
        assert!(may_write(&answered(&log, cases::KEEP, "")));
    }
}

/// Every card the cases card can show, over every state its own moves reach from a whole contract
/// (the binary's findings, the person's answers, the promise claimed again): at least two choices,
/// the card the person reads offers exactly those, each once, and each clears its hold; nothing
/// typed is no answer; and an answer that closes the card keeps the plan or asks for a case.
#[test]
fn every_card_the_cases_card_can_show_offers_at_least_two_choices() {
    use familiar_ai_rules::{holds, moves, sample};
    let mut todo = vec![whole()];
    let mut shown = 0;
    while let Some(log) = todo.pop() {
        let v = View::new(&log);
        if cases::card_open(&v, "c1") {
            shown += 1;
            let choices = cases::offered(&v, "c1");
            assert!(choices.len() >= 2, "{choices:?} at:\n{}", terminal::status(&v));
            // Definition change (the lens panel's questions on the card, 2026-10-26): once the choice
            // is answered the card the person reads offers no choice, only its open questions
            let reads: Vec<&str> = if cases::choice_answered(&v, "c1") { Vec::new() } else { choices.clone() };
            assert_eq!(offered(&log), reads, "the card the person reads");
            let clears: Vec<String> = holds::cases_card_clears(&v, "c1").iter().filter_map(|m| m.get("choice").and_then(Value::as_str)).map(str::to_string).collect();
            let mut distinct = clears.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(distinct.len(), clears.len(), "each clears once: {clears:?}");
            // Definition change (the lens panel, approved 2026-10-26): once the choice is answered,
            // the card stays open only on the synthesis's open questions, and only their picks clear it
            if cases::choice_answered(&v, "c1") {
                assert!(clears.is_empty(), "{clears:?}");
                let picks: Vec<String> = holds::cases_card_clears(&v, "c1").iter().filter_map(|m| m.get("question_id").and_then(Value::as_str)).map(str::to_string).collect();
                assert_eq!(picks, cases::unanswered_questions(&v, "c1"));
                assert!(!picks.is_empty());
            } else {
                assert!(choices.iter().all(|ch| clears.iter().any(|x| x == ch)), "{choices:?} clear: {clears:?}");
            }
            for typed in ["", "  "] {
                let blank = answered(&log, cases::TYPED, typed);
                assert!(cases::card_open(&View::new(&blank), "c1"), "{typed:?} typed closes nothing");
                assert_eq!(cases::offered(&View::new(&blank), "c1"), choices);
            }
        }
        for m in moves::cases_moves(&v, "c1") {
            let after = sample::apply_move(&log, &m, AT, TENANT).unwrap_or_else(|e| panic!("{}: {e}", m.kind));
            let w = View::new(&after);
            if m.kind == "hold_answered" && !cases::holds(&w, "c1") {
                let keep = m.fields.get("choice").and_then(Value::as_str) == Some(cases::KEEP);
                assert!(keep || !cases::missing(&w, "c1").is_empty(), "{:?} closed the card with nothing asked for", m.fields);
            }
            todo.push(after);
        }
    }
    // Definition change (the owner's plan of 2026-10-26, the lens panel, approved): the synthesis
    // has a fifth finding, one case and two questions, whose card stays open until its choice and
    // both questions are answered: 15 cards in any order (1 at first, 4 choices x 2 then, 1 pick
    // then 4 choices x 1 or the other pick and 1). At each look: 1 + 1 + 15 + none + failed, and
    // after look again (on none and on failed) the second panel's 1 + 1 + 1 + 1 + 15 = 19 each.
    assert_eq!(shown, 57, "five findings at the first look, five more after each look again (on none and on failed)");
}

#[test]
fn a_failed_finder_offers_keep_or_look_again() {
    let log = found(&whole(), "failed", 0);
    let v = View::new(&log);
    assert!(cases::card_open(&v, "c1"));
    assert_eq!(offered(&log), vec![cases::KEEP, cases::AGAIN]);
    let log = answered(&log, cases::KEEP, "");
    assert!(may_write(&log));
}

#[test]
fn the_card_is_the_persons_and_its_answers_are_its_choices() {
    let log = found(&whole(), "found", 2);
    let c = card(&log).expect("the cases card");
    assert_eq!(c.criterion_id, "c1");
    assert!(whys(&log).iter().any(|w| w == cases::CARD));
}
