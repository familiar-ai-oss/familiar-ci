//! Pins the words the agent and the person read around the picker.

use super::*;
use familiar_ai_rules::sample::{apply_move, criterion_fixture};

fn anchor_moved_card() -> (familiar_ai_facts::Log, Card) {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let m = familiar_ai_rules::moves::criterion_admissible(&View::new(&log))
        .into_iter()
        .find(|m| m.label.as_deref() == Some("outside_edit_seen"))
        .unwrap_or_else(|| panic!("the start check with the lines moved"));
    let log = apply_move(&log, &m, 1.0, "t").unwrap_or_default();
    let card = familiar_ai_rules::terminal::cards(&View::new(&log)).into_iter().next().unwrap_or_else(|| panic!("a card"));
    (log, card)
}

#[test]
fn the_card_is_told_to_the_agent_as_facts() {
    let (log, card) = anchor_moved_card();
    let v = View::new(&log);
    let f = facts(&v, &card);
    // the agent gets the pieces, the shape and the order to write one sentence, with no choices to copy.
    assert_eq!(
        f,
        "familiar-ai: a question for the person is open for part 1 of what they asked. Write your question to them as prose, in this shape and order:
  you asked: \"let a person log in and reset a forgotten password\"
  the agent split that into one part:
    1  a right password opens a session
  what happened: someone changed the code the agent had picked for part 1, outside this work.
  In the agent's words: \"<one sentence of yours, in plain words: what the code it had picked for part 1 now does differently>\"
  what now?
Quote the person's words and each part's text exactly as above, write the one sentence yourself, and leave the choices to the hook. The record's card: \"a right password opens a session\": the code at src/login.py check_password changed after the plan was made. What now? Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself."
    );
    assert!(f.contains("Ask the person with AskUserQuestion"), "the picker is required: {f}");
}

#[test]
fn the_persons_and_the_agents_lines_after_a_pick() {
    let (log, _) = anchor_moved_card();
    let v = View::new(&log);
    assert_eq!(recorded(&v, "c1", "keep going as planned"), "familiar-ai: recorded your answer for \"a right password opens a session\": keep going as planned");
    assert_eq!(told(&v, "c1", "release it"), "familiar-ai: the person answered the question for \"a right password opens a session\": release it. The answer is recorded; nothing is left to claim for it.");
    assert_eq!(push_chosen(PUSH_YES), "familiar-ai: the person chose: push and open the pull request");
    assert_eq!(push_chosen(PUSH_NOT_NOW), "familiar-ai: the person chose: not now");
}

/// An ask read more than one way (the owner's plan of 2026-10-21, step 2) [draft words]: the
/// agent's readings are the picker's options, verbatim, then "something else".
#[test]
fn the_readings_are_the_pickers_options_verbatim() {
    use serde_json::json;
    let fixture = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let add = |log: &familiar_ai_facts::Log, kind: &str, fields: Value| familiar_ai_facts::append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
    let mut log = add(&Vec::new(), "person_said", json!({ "text": "make the report shorter" }));
    for f in fixture {
        let mut m = f.fields.clone();
        if f.kind == "intent_submitted" {
            m.insert("for".into(), json!(0));
        }
        log = add(&log, f.kind, Value::Object(m));
    }
    log = add(&log, "reading_alternatives", json!({ "readings": ["the report lists fewer rows", "the report's file is smaller on disk"] }));
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().find(|c| c.hold == "reading_unclear").unwrap_or_else(|| panic!("the card"));
    let labels: Vec<String> = card_options(&v, &card).as_array().cloned().unwrap_or_default().iter().map(|o| o["label"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(labels, ["the report lists fewer rows", "the report's file is smaller on disk", SOMETHING_ELSE]);
    assert_eq!(choice_of(&v, &card, "the report's file is smaller on disk").as_deref(), Some("the report's file is smaller on disk"));
    let a = crate::teach::person_words::asked(&v, &card);
    assert_eq!(a.happened, "your ask can be read more than one way, and the agent will plan for the one you pick.");
    let f = facts(&v, &card);
    assert!(f.contains("): \"make the report shorter\""), "{f}");
    assert!(f.contains("what differs between the readings, and which one you would plan for"), "{f}");
    let shown = terminal(&v, &card).join("\n");
    assert!(shown.contains("    1) the report lists fewer rows") && shown.contains("    2) the report's file is smaller on disk"), "{shown}");
}
