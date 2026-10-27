//! The cases card in the terminal shows the lens panel's questions (the owner's plan of 2026-10-26,
//! lens-panel.md, approved): below its choices, each of the synthesis's questions still open, with
//! its options numbered; once the card's own choice is answered it offers no choice again and shows
//! only the questions still open. The holds' words name the lens panel, not the case-finder
//! [draft words].

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{cases, terminal, View};
use serde_json::json;
use traced::*;

/// c1 planned, its promise with a whole contract.
fn whole() -> Log {
    let cases = json!([
        { "case_id": "n1", "kind": "normal", "short": "a plain row is written once", "given": "given n1", "expect": "expect n1" },
        { "case_id": "o1", "kind": "opposite", "short": "no rows, nothing written", "given": "given o1", "expect": "expect o1" }
    ]);
    raw(&planned(), "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing", "cases": cases, "contract_version": 1 }))
}

/// The synthesis after the five lenses: one found case and two questions.
fn synthesis(log: &Log) -> Log {
    let log = lenses(log, "c1");
    let cs = json!([{ "case_id": "a1", "short": "found case 1", "given": "given a1", "expect": "expect a1", "why": "the lens saw it" }]);
    let qs = json!([
        { "question_id": "q1", "text": "Should every --tb style show the message?", "options": ["every style", "only long"], "why": "the draft tests long only" },
        { "question_id": "q2", "text": "Should a nested group show it too?", "options": ["yes", "no", "only the outer group"], "why": "the draft leaves nesting out" }
    ]);
    raw(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "found", "cases": cs, "why": "", "model": "claude-opus-5-5", "tokens": 6000, "questions": qs }))
}

fn card(log: &Log) -> terminal::Card {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == cases::CARD).unwrap_or_else(|| panic!("the cases card: {log:?}"))
}

fn lines(card: &terminal::Card) -> Vec<String> {
    card.text.lines().map(str::to_string).collect()
}

const Q1: [&str; 3] = ["  ? Should every --tb style show the message?", "    1) every style", "    2) only long"];
const Q2: [&str; 4] = ["  ? Should a nested group show it too?", "    1) yes", "    2) no", "    3) only the outer group"];

#[test]
fn the_card_shows_each_open_question_below_its_choices() {
    let log = synthesis(&whole());
    let c = card(&log);
    let ls = lines(&c);
    let typed = ls.iter().position(|l| l.starts_with("  > ")).unwrap_or_else(|| panic!("the typed line: {ls:?}"));
    assert!(ls[..typed].iter().any(|l| l.starts_with("  1) add_all")), "the choices first: {ls:?}");
    let want: Vec<String> = Q1.iter().chain(Q2.iter()).map(|l| l.to_string()).collect();
    assert_eq!(ls[typed + 1..], want[..], "{ls:?}");
    assert_eq!(c.choices.iter().map(|(ch, _)| ch.as_str()).collect::<Vec<_>>(), [cases::ADD_ALL, cases::KEEP]);
}

#[test]
fn once_the_choice_is_answered_the_card_shows_only_the_open_questions() {
    let log = raw(&synthesis(&whole()), "hold_answered", json!({ "criterion_id": "c1", "hold": cases::CARD, "choice": cases::KEEP, "typed": "" }));
    let c = card(&log);
    assert!(c.choices.is_empty(), "the choice is not offered again: {:?}", c.choices);
    let ls = lines(&c);
    assert!(!ls.iter().any(|l| l.starts_with("  1) ") || l.starts_with("  > ")), "{ls:?}");
    assert!(ls.ends_with(&Q1.iter().chain(Q2.iter()).map(|l| l.to_string()).collect::<Vec<_>>()), "{ls:?}");

    let log = raw(&log, "question_answered", json!({ "criterion_id": "c1", "question_id": "q1", "choice": "only long", "typed": "" }));
    let ls = lines(&card(&log));
    assert!(ls.ends_with(&Q2.iter().map(|l| l.to_string()).collect::<Vec<_>>()), "{ls:?}");
    assert!(!ls.iter().any(|l| l.contains("--tb style")), "q1 is answered: {ls:?}");
}

#[test]
fn a_card_with_no_questions_is_as_before() {
    let log = lenses(&whole(), "c1");
    let log = raw(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "covered", "model": "m", "tokens": 1 }));
    let ls = lines(&card(&log));
    assert!(ls.last().is_some_and(|l| l.starts_with("  > ")), "{ls:?}");
    assert!(!ls.iter().any(|l| l.starts_with("  ? ")), "{ls:?}");
}

#[test]
fn the_holds_words_name_the_lens_panel() {
    for why in [cases::OWED, cases::CARD] {
        let w = terminal::words_of_why(why);
        assert!(w.contains("lens panel") && !w.contains("case-finder"), "{why}: {w}");
    }
}
