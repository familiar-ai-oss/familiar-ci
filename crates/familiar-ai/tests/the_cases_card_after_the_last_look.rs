//! The cases card after the last look (the owner's ruling, 2026-10-25: "Keep, or add my own")
//! [draft words]: once the second lens panel found nothing or failed, the card offers keep the
//! planned cases, or add a case of the person's own, so Claude Code's picker, which refuses a
//! question with fewer than two options, can show it (bench click-2956 ended blocked on a card of
//! one option). Words typed are the person's own case, as the one question's "I'll type it" takes
//! words typed under Claude Code's own "Other"; "Add my own" picked with nothing typed is recorded
//! as nothing, and the question stays open, as "something else" picked with nothing typed.

mod cases_fixture;
mod common;

use common::Project;
use familiar_ai::teach::{cases_card, picker};
use familiar_ai_facts::{Fact, Record};
use familiar_ai_rules::terminal::{self, Card};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const CARD: &str = "cases_unconfirmed";

fn the_card(log: &familiar_ai_facts::Log) -> Card {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == CARD).unwrap_or_else(|| panic!("the cases card"))
}

fn labels(log: &familiar_ai_facts::Log) -> Vec<String> {
    let v = View::new(log);
    picker::card_options(&v, &the_card(log)).as_array().cloned().unwrap_or_default().iter().map(|o| o["label"].as_str().unwrap_or("").to_string()).collect()
}

#[test]
fn every_cases_card_on_the_picker_has_two_options_at_least() {
    use cases_fixture::{answered, found, planned};
    let first: Vec<(&str, usize)> = vec![("found", 1), ("found", 2), ("found", 3), ("none", 0), ("failed", 0)];
    for (outcome, n) in &first {
        let log = found(&planned(), outcome, *n);
        assert!(labels(&log).len() >= 2, "{outcome} {n}: {:?}", labels(&log));
    }
    for again in ["none", "failed"] {
        let looked = answered(&found(&planned(), again, 0), "look_again", "");
        for (outcome, n) in &first {
            let log = found(&looked, outcome, *n);
            assert!(labels(&log).len() >= 2, "{again}, then {outcome} {n}: {:?}", labels(&log));
        }
    }
}

#[test]
fn after_the_last_look_the_picker_offers_keep_or_add_my_own() {
    use cases_fixture::{answered, found, planned};
    let looked = answered(&found(&planned(), "none", 0), "look_again", "");
    let log = found(&looked, "none", 0);
    let v = View::new(&log);
    let c = the_card(&log);
    assert_eq!(
        picker::card_options(&v, &c),
        json!([{ "label": "Looks right (Recommended)", "description": "I'll write the test" }, { "label": "Add my own", "description": "pick Other and describe a case to add" }])
    );
    assert_eq!(picker::choice_of(&v, &c, "Add my own").as_deref(), Some("typed"));
    assert_eq!(picker::choice_of(&v, &c, "an empty group prints nothing"), None, "words typed are the picker's typed answer");
    let log = found(&looked, "failed", 0);
    assert_eq!(labels(&log), ["Test these", "Add my own"]);
}

/// A full-path project planned to c1's whole contract, its lens panel answered "none" twice
/// with the person's look_again between, through the hooks: the card after the last look is open.
fn last_look(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_LENS_MODEL").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"the name Ada","expect":"hello Ada"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
    ] {
        p.claim(&args);
    }
    let none = r#"{"cases": [], "none_because": "n1 and o1 cover every path greet has"}"#;
    // updated with the definition (the owner's plan of 2026-10-26, the lens panel): each look is a
    // whole panel, its five lenses and its synthesis
    p.lenses_answer_none();
    p.synthesis_answers(none);
    let record = Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("the record");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0) + 1.0;
    let again = json!({ "criterion_id": "c1", "hold": CARD, "choice": "look_again", "typed": "" });
    record.append_raw("hold_answered", again.as_object().cloned().unwrap_or_default(), now, familiar_ai::TENANT).expect("the person's look_again");
    drop(record);
    p.lenses_answer_none();
    p.synthesis_answers(none);
    assert_eq!(p.facts().iter().filter(|f| f.kind == "cases_found").count(), 2, "two looks");
    p
}

/// A captured payload re-pointed at `p`, `set` over it; the hook's JSON answer.
fn hooked(p: &Project, event: &str, fixture: &str, set: Value) -> Value {
    let (code, out, err) = p.hook_raw(event, &p.fixture(fixture, set));
    assert_eq!(code, 0, "{event} {fixture}: {out}{err}");
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{event} {fixture} printed no JSON ({e}): {out}"))
}

/// The agent asks with the card's lines word for word; the hook's `updatedInput`.
fn asked(p: &Project) -> Value {
    let log = p.facts();
    let v = View::new(&log);
    let question = cases_card::shape(&cases_card::shown(&v, &the_card(&log)));
    let asking = json!({ "tool_input": { "questions": [{ "question": question, "header": "x", "options": [{ "label": "yes", "description": "Continue" }], "multiSelect": false }] } });
    let pre = hooked(p, "PreToolUse", "pre_tool_use_ask_user_question", asking);
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("ask"), "{pre}");
    pre["hookSpecificOutput"]["updatedInput"].clone()
}

/// The person's `answer` on the card the hook asked; the hook's JSON answer.
fn picks(p: &Project, asked: &Value, answer: &str) -> Value {
    let q = asked["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let response = json!({ "questions": asked["questions"], "answers": { q: answer }, "annotations": {} });
    let mut input = asked.clone();
    input["answers"] = response["answers"].clone();
    input["annotations"] = json!({});
    hooked(p, "PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
}

fn answers(p: &Project) -> Vec<Fact> {
    p.facts().into_iter().filter(|f| f.kind == "hold_answered" && f.fields.get("hold").and_then(Value::as_str) == Some(CARD)).collect()
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

#[test]
fn the_hook_asks_the_last_looks_card_with_two_options() {
    let p = last_look("last-look-asked");
    let asked = asked(&p);
    let options = asked["questions"][0]["options"].as_array().cloned().unwrap_or_default();
    assert!(options.len() >= 2, "Claude Code refuses fewer than two: {asked}");
    let labels: Vec<&str> = options.iter().map(|o| o["label"].as_str().unwrap_or("")).collect();
    assert_eq!(labels, ["Looks right (Recommended)", "Add my own"]);
}

#[test]
fn add_my_own_picked_with_nothing_typed_records_nothing_and_the_question_stays_open() {
    let p = last_look("last-look-blank");
    let before = answers(&p).len();
    let asked = asked(&p);
    let out = picks(&p, &asked, "Add my own");
    assert_eq!(answers(&p).len(), before, "nothing is recorded: {out}");
    assert!(out.to_string().contains("the person picked \\\"Add my own\\\" for \\\"greet says hello\\\" and typed nothing; nothing is recorded and the question stays open"), "{out}");
    let log = p.facts();
    assert!(familiar_ai_rules::cases::card_open(&View::new(&log), "c1"), "the card stays open");
    // then the person picks Other and types their case: it is theirs, p1
    let out = picks(&p, &asked, "an empty name greets nobody");
    let after = answers(&p);
    assert_eq!(after.len(), before + 1, "{out}");
    let last = after.last().expect("the answer");
    assert_eq!((text(last, "choice"), text(last, "typed")), ("typed", "an empty name greets nobody"));
    let log = p.facts();
    let v = View::new(&log);
    assert!(!familiar_ai_rules::cases::card_open(&v, "c1"));
    assert_eq!(familiar_ai_rules::cases::missing(&v, "c1"), ["p1"]);
}
