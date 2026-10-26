//! A clarifying question at plan time, only on a signal (the owner's plan of 2026-10-21, step 2)
//! [draft names]. Fixed, generic triggers read the person's words (the prompt the part's intent
//! names): (a) few content words and no example, (b) a referent with no list it could refer to,
//! (c) two or more open questions. While one fires and the agent has said neither how the ask
//! reads (`one_reading`, with why) nor that it reads more than one way (`reading_alternatives`),
//! the part holds `reading_unclear`, the agent's hold. When the agent lists two or more readings,
//! `reading_unclear` is the person's card, its choices the readings verbatim; the person's pick is
//! recorded as theirs. Either way the tests-first commit waits.

mod common;

use common::{raw, step, AT, TENANT};
use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::ask::{self, Trigger};
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, moves, terminal, View};
use serde_json::{json, Value};

const SHORT: &str = "make the report shorter";
const FIRST: &str = "the report lists fewer rows";
const SECOND: &str = "the report's file is smaller on disk";

/// The confirmed fixture for `criteria`, its intent naming the person's prompt `words`.
fn asked(words: &str) -> Log {
    let fixture = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let mut log = raw(&Vec::new(), "person_said", json!({ "text": words }));
    let said = log[0].t;
    for f in fixture {
        let mut m = f.fields.clone();
        if f.kind == "intent_submitted" {
            m.insert("for".into(), Value::from(said));
        }
        log = append_raw(&log, f.kind, m, AT, TENANT).expect("the door takes it");
    }
    log
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::holds(&View::new(log), "c1").iter().map(|h| h.why.to_string()).collect()
}

fn tests_commit(log: &Log) -> Decision {
    decide(&View::new(log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code: false })
}

fn cards(log: &Log) -> Vec<terminal::Card> {
    terminal::cards(&View::new(log)).into_iter().filter(|c| c.hold == ask::HOLD).collect()
}

fn offered(log: &Log, kind: &str) -> Vec<Value> {
    moves::criterion_admissible(&View::new(log)).into_iter().filter(|m| m.kind == kind).map(|m| Value::Object(m.fields)).collect()
}

#[test]
fn few_content_words_and_no_example_fire_and_an_example_or_more_words_do_not() {
    assert!(ask::triggers(SHORT).contains(&Trigger::FewWords));
    assert!(ask::triggers("please fix the search").contains(&Trigger::FewWords));
    // an example of any kind: a code fence, inline code, a quoted value, a call
    for with_example in [
        "make the report shorter:\n```\nreport --max 10\n```",
        "make `report` shorter",
        "rename the button to \"Save draft\"",
        "rename the button to 'Save draft'",
        "make render(rows) shorter",
    ] {
        assert!(!ask::triggers(with_example).contains(&Trigger::FewWords), "{with_example}");
    }
    let long = "when the nightly export writes the monthly report it repeats every customer row twice, so totals double and the summary page disagrees with the ledger";
    assert!(!ask::triggers(long).contains(&Trigger::FewWords), "{:?}", ask::content_words(long));
    // stop words do not count
    assert_eq!(ask::content_words("make the report a bit shorter for us"), ["make", "report", "bit", "shorter"]);
}

#[test]
fn a_referent_with_no_list_fires_and_one_with_a_list_or_an_idiom_does_not() {
    for dangling in [
        "the export drops the header row; fix the other formats too",
        "handle the remaining cases",
        "fix the first one and the rest",
        "support dates, numbers etc",
        "the parser should accept it as well",
    ] {
        assert!(ask::triggers(dangling).contains(&Trigger::Dangling), "{dangling}");
    }
    for listed in [
        "csv, json and xml drop the header row; the other formats keep it",
        "fix these:\n- the export\n- the import\nand the rest",
        "nothing other than the export changes",
        "the two modules call each other",
        "the export drops the header row as well as the footer",
    ] {
        assert!(!ask::triggers(listed).contains(&Trigger::Dangling), "{listed}");
    }
}

#[test]
fn two_open_questions_fire_and_one_does_not() {
    for open in [
        "Why does the cache grow? I'm not sure it is ever cleared.",
        "what's going on with the export. I don't think it ran",
        "Does the retry stop? Does it log?",
    ] {
        assert!(ask::triggers(open).contains(&Trigger::OpenQuestions), "{open}");
    }
    for one in ["Why does the cache grow without bound when entries expire?", "the cache grows; clear it on expiry", "is `x?` an error? fix it"] {
        assert!(!ask::triggers(one).contains(&Trigger::OpenQuestions), "{one}");
    }
}

#[test]
fn a_trigger_holds_the_part_for_the_agents_word_and_one_reading_clears_it_with_no_card() {
    let log = asked(SHORT);
    assert!(whys(&log).contains(&ask::HOLD.to_string()), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "held");
    // the agent's hold: never a card, the tests-first commit refused, no commit offered
    assert!(cards(&log).is_empty(), "the agent's, not the person's");
    let Decision::Refuse(r) = tests_commit(&log) else { panic!("the tests-first commit is refused") };
    assert_eq!(r.hold.as_deref(), Some(ask::HOLD));
    let words = terminal::refusal(&View::new(&log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code: false }).unwrap_or_default();
    assert!(words.contains("held: reading_unclear"), "{words}");
    assert!(words.contains("few words and no example"), "the trigger in words: {words}");
    assert!(words.contains("(one_reading)") && words.contains("(reading_alternatives)"), "{words}");
    assert!(!moves::criterion_admissible(&View::new(&log)).iter().any(|m| m.label.as_deref() == Some("tests_only")));
    assert_eq!(offered(&log, ask::ONE).len(), 1);
    assert_eq!(offered(&log, ask::ALTERNATIVES).len(), 1);

    // a blank why says nothing
    let blank = raw(&log, ask::ONE, json!({ "why": "  " }));
    assert!(whys(&blank).contains(&ask::HOLD.to_string()));
    // one reading listed is not alternatives: still the agent's to say
    let one = raw(&log, ask::ALTERNATIVES, json!({ "readings": [FIRST] }));
    assert!(whys(&one).contains(&ask::HOLD.to_string()) && cards(&one).is_empty());

    let log = raw(&log, ask::ONE, json!({ "why": "the report is the one page the export writes; shorter means fewer rows" }));
    assert!(!whys(&log).contains(&ask::HOLD.to_string()), "{:?}", whys(&log));
    assert!(cards(&log).is_empty(), "no card");
    assert!(tests_commit(&log).allowed());
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "testing");
    // the start check waited with the part held, and comes now
    step(&step(&log, "start_check", None), "tests_only", None);
}

#[test]
fn readings_listed_are_the_persons_card_until_they_pick_one() {
    let log = raw(&asked(SHORT), ask::ALTERNATIVES, json!({ "readings": [FIRST, SECOND] }));
    assert!(whys(&log).contains(&ask::HOLD.to_string()), "{:?}", whys(&log));
    let cs = cards(&log);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0].criterion_id, "c1");
    let choices: Vec<&str> = cs[0].choices.iter().map(|(ch, _)| ch.as_str()).collect();
    assert_eq!(choices, [FIRST, SECOND], "the readings verbatim");
    assert!(cs[0].text.contains(&format!("1) {FIRST}")), "{}", cs[0].text);
    let Decision::Refuse(r) = tests_commit(&log) else { panic!("the tests-first commit waits on the person") };
    assert_eq!(r.hold.as_deref(), Some(ask::HOLD));
    // the person may pick either reading; the agent is offered no claim of its own
    let picks: Vec<Value> = offered(&log, "hold_answered").into_iter().filter(|f| f["hold"] == ask::HOLD).map(|f| f["choice"].clone()).collect();
    assert_eq!(picks, [json!(FIRST), json!(SECOND)]);
    assert!(offered(&log, ask::ONE).is_empty() && offered(&log, ask::ALTERNATIVES).is_empty());

    let picked = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": ask::HOLD, "choice": SECOND, "typed": "" }));
    assert!(!whys(&picked).contains(&ask::HOLD.to_string()), "{:?}", whys(&picked));
    assert!(cards(&picked).is_empty());
    assert!(tests_commit(&picked).allowed());
    assert_eq!(ask::answers(&View::new(&picked)), [SECOND.to_string()]);
    // the person's own words, typed under "something else", answer it too
    let typed = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": ask::HOLD, "choice": "typed", "typed": "both: fewer rows and a smaller file" }));
    assert!(!whys(&typed).contains(&ask::HOLD.to_string()));
    assert_eq!(ask::answers(&View::new(&typed)), ["both: fewer rows and a smaller file".to_string()]);
}

#[test]
fn readings_listed_without_a_trigger_still_ask_and_at_most_three_are_offered() {
    let clear = "when the nightly export writes the monthly report it repeats every customer row twice, so `totals()` doubles";
    assert!(ask::triggers(clear).is_empty());
    let log = asked(clear);
    assert!(!whys(&log).contains(&ask::HOLD.to_string()), "no trigger, no claim owed");
    let log = raw(&log, ask::ALTERNATIVES, json!({ "readings": [FIRST, SECOND, "both", "neither"] }));
    let cs = cards(&log);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].choices.len(), 3, "the picker's own \"something else\" is the fourth");
}

#[test]
fn a_record_with_no_prompt_of_the_persons_has_nothing_to_read() {
    // the fixture's intent names no prompt: its words are the agent's, and no trigger reads them
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    assert!(!whys(&log).contains(&ask::HOLD.to_string()));
    assert!(tests_commit(&log).allowed());
}

#[test]
fn the_door_takes_two_or_three_distinct_readings_and_a_why() {
    assert_eq!(ask::refused_readings(&[FIRST, SECOND]), None);
    assert_eq!(ask::refused_readings(&[FIRST, SECOND, "both"]), None);
    for bad in [vec![FIRST], vec![FIRST, " "], vec![FIRST, FIRST], vec![FIRST, SECOND, "both", "neither"]] {
        assert!(ask::refused_readings(&bad).is_some(), "{bad:?}");
    }
    assert!(ask::refused_why("  ").is_some());
    assert_eq!(ask::refused_why("the ask names the one thing it changes"), None);
}
