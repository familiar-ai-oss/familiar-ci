//! The one question (the owner's plan of 2026-10-23, change D) [draft names]. Before a pytest part's
//! first try, when the person's words (the prompt its intent names, read as the ask's card reads
//! them) show no way to make the problem happen — no code fence, no inline code, no quoted value or
//! call, no command, no quoted output or error — the part holds `repro_unknown`, the person's card:
//! "How do you make it happen? What do you run, and what do you see?", its choices "I'll type it"
//! and "I don't know — go ahead". Any answer closes it, and words typed are the person's. While it
//! is open the planned test waits too. It comes before `reading_unclear`.

mod traced;

use familiar_ai_rules::ask;
use familiar_ai_rules::{moves, terminal, trace, View};
use serde_json::{json, Value};
use traced::*;

fn vague() -> familiar_ai_facts::Log {
    planned_with(Some(VAGUE), "pytest")
}

fn cards(log: &familiar_ai_facts::Log) -> Vec<terminal::Card> {
    terminal::cards(&View::new(log)).into_iter().filter(|c| c.hold == trace::REPRO).collect()
}

fn answered(log: &familiar_ai_facts::Log, choice: &str, typed: &str) -> familiar_ai_facts::Log {
    raw(log, "hold_answered", json!({ "criterion_id": "c1", "hold": trace::REPRO, "choice": choice, "typed": typed }))
}

#[test]
fn words_show_a_way_with_code_a_command_or_quoted_output() {
    for shown in [
        SHOWN,
        "run this:\n```\npython export.py --all\n```",
        "export(rows) repeats them",
        "it says \"row 3 already written\"",
        "$ python -m export --all",
        "when I run python export.py --all every row comes out twice",
        "pytest -k export fails",
        "it crashes with KeyError: 'id'",
        "the export raises ValueError when a row is empty",
        "I see a Traceback from the writer",
    ] {
        assert!(trace::shows_a_way(shown), "{shown}");
    }
    for vague in [VAGUE, "the report is wrong", "please make the search faster for big folders", "fix the error in the export"] {
        assert!(!trace::shows_a_way(vague), "{vague}");
    }
    // the ask's own example reading is the one reused
    assert!(ask::has_example("make `report` shorter") && trace::shows_a_way("make `report` shorter"));
}

#[test]
fn vague_words_open_the_persons_card_before_the_first_try_and_the_test_waits() {
    let log = vague();
    assert!(whys(&log).contains(&trace::REPRO.to_string()), "{:?}", whys(&log));
    let cs = cards(&log);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0].criterion_id, "c1");
    assert_eq!(cs[0].question, trace::REPRO_QUESTION);
    assert_eq!(cs[0].question, "How do you make it happen? What do you run, and what do you see?");
    let words: Vec<&str> = cs[0].choices.iter().map(|(_, w)| w.as_str()).collect();
    assert_eq!(words, ["I'll type it", "I don't know — go ahead"]);
    let choices: Vec<&str> = cs[0].choices.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(choices, [trace::TYPE_IT, trace::GO_AHEAD]);
    assert!(!cs[0].typed.is_empty(), "free text is offered");
    // the planned test waits on the person, and so does everything else
    assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some(trace::REPRO));
    assert_eq!(hold_of(&edit(&log, CODE)).as_deref(), Some(trace::REPRO));
    // the walk offers each answer, the person's
    let offered: Vec<Value> = moves::criterion_admissible(&View::new(&log))
        .into_iter()
        .filter(|m| m.kind == "hold_answered" && m.fields.get("hold") == Some(&json!(trace::REPRO)))
        .map(|m| m.fields["choice"].clone())
        .collect();
    assert_eq!(offered, [json!(trace::TYPE_IT), json!(trace::GO_AHEAD)]);
}

#[test]
fn any_answer_closes_it_and_typed_words_are_the_persons() {
    for (choice, typed) in [(trace::GO_AHEAD, ""), (trace::TYPE_IT, ""), ("typed", "I run export --all and every row is there twice")] {
        let log = answered(&vague(), choice, typed);
        assert!(!whys(&log).contains(&trace::REPRO.to_string()), "{choice}: {:?}", whys(&log));
        assert!(cards(&log).is_empty(), "{choice}");
        assert!(edit(&log, TEST).allowed(), "{choice}");
        assert_eq!(hold_of(&edit(&log, CODE)).as_deref(), Some(trace::TRY_OWED), "{choice}");
    }
    let log = answered(&vague(), "typed", "I run export --all and every row is there twice");
    assert_eq!(trace::answers(&View::new(&log)), ["I run export --all and every row is there twice".to_string()]);
    assert_eq!(trace::answer_words(&View::new(&log), "c1"), Some("I run export --all and every row is there twice".to_string()));
    assert!(trace::answers(&View::new(&answered(&vague(), trace::GO_AHEAD, ""))).is_empty(), "no words given");
}

#[test]
fn no_card_when_the_words_show_a_way_or_there_are_none_or_a_try_ran() {
    assert!(!whys(&planned()).contains(&trace::REPRO.to_string()));
    assert!(!whys(&planned_with(None, "pytest")).contains(&trace::REPRO.to_string()), "a record with no prompt has no words to read");
    assert!(!whys(&tried(&vague(), true, top())).contains(&trace::REPRO.to_string()), "asked before the first try only");
    // another stack has no try, and no question
    assert!(!whys(&planned_with(Some(VAGUE), "cargo")).contains(&trace::REPRO.to_string()));
    // not before the test may be written
    let mut early = vague();
    early.retain(|f| f.kind != "test_planned");
    assert!(!whys(&early).contains(&trace::REPRO.to_string()));
}

#[test]
fn it_comes_before_reading_unclear() {
    // words that fire the ask's triggers and show no way: the person is asked how first
    let log = planned_with(Some("fix it"), "pytest");
    assert!(ask::triggers("fix it").contains(&ask::Trigger::FewWords));
    let w = whys(&log);
    assert_eq!(w.iter().find(|h| *h == trace::REPRO || *h == ask::HOLD).map(String::as_str), Some(trace::REPRO), "{w:?}");
    let cs: Vec<String> = terminal::cards(&View::new(&log)).into_iter().map(|c| c.hold).collect();
    assert_eq!(cs.first().map(String::as_str), Some(trace::REPRO), "{cs:?}");
}
