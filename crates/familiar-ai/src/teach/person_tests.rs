//! Tests that the person's words, the agent's reading and unabsorbed prompts are derived correctly.

use super::*;
use familiar_ai_facts::append_raw;
use familiar_ai_rules::sample::criterion_fixture;
use serde_json::json;

fn add(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// The confirmed fixture (c1 testing), with `first` before it when given.
fn confirmed_plan(first: Option<&str>) -> Log {
    let fixture = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let mut log: Log = Vec::new();
    if let Some(text) = first {
        log = add(&log, "person_said", json!({ "text": text }));
    }
    for f in &fixture {
        log = add(&log, f.kind, Value::Object(f.fields.clone()));
    }
    log
}

#[test]
fn the_persons_words_are_every_prompt_else_the_first_intent() {
    let log = confirmed_plan(None);
    let first_intent = log.iter().find(|f| f.kind == "intent_submitted").and_then(|f| text(f, "text")).unwrap_or("").to_string();
    assert_eq!(words(&log), std::slice::from_ref(&first_intent), "a record from before prompts were recorded");
    assert_eq!(agents_reading(&log), None, "one intent: no reading of its own");
    let log = add(&log, "intent_submitted", json!({ "text": "the agent's own words", "sha": "abc" }));
    assert_eq!(agents_reading(&log), Some("the agent's own words"));
    assert_eq!(words(&log), [first_intent], "still the first");

    let log = confirmed_plan(Some("fix the alias, please"));
    let log = add(&log, "person_said", json!({ "text": "and the docs" }));
    assert_eq!(words(&log), ["fix the alias, please", "and the docs"]);
    // the fixture's intent is not the person's words: it is the agent's reading
    assert!(agents_reading(&log).is_some());
}

#[test]
fn a_prompt_while_planning_is_the_plans_and_a_later_one_must_be_named() {
    let log = confirmed_plan(Some("make greet say hello"));
    assert!(unabsorbed(&log).is_empty(), "before the plan was confirmed: the first criterion answers it");
    assert!(status_lines(&log).is_empty());

    let log = add(&log, "person_said", json!({ "text": "also   make it   shout" }));
    let later = log.len() as u64 - 1;
    assert_eq!(unabsorbed(&log).iter().map(|f| f.t).collect::<Vec<_>>(), [later]);
    assert_eq!(status_lines(&log), ["familiar-ai: the person said \"also make it shout\"; nothing answers it yet"]);
    assert_eq!(intent_line("make greet\n say hello"), "familiar-ai: your request is the intent: \"make greet say hello\"");

    let named = add(&log, "criterion", json!({ "criterion_id": "c2", "text": "greet shouts", "path": "light", "for": later }));
    assert!(unabsorbed(&named).is_empty(), "a criterion names it with for");
    let other = add(&log, "criterion", json!({ "criterion_id": "c2", "text": "greet shouts", "path": "light", "for": later - 1 }));
    assert_eq!(unabsorbed(&other).len(), 1, "a criterion for another prompt does not");
    for how in ["no_work", "question"] {
        let answered = add(&log, "person_said_answered", json!({ "for": later, "how": how }));
        assert!(unabsorbed(&answered).is_empty(), "{how}");
    }
}

#[test]
fn a_prompt_with_no_run_open_is_conversation() {
    let log = add(&Vec::new(), "person_said", json!({ "text": "what does this repo do?" }));
    assert!(unabsorbed(&log).is_empty());
}

// Pins the refusal's exact words.
#[test]
fn the_refusal_names_the_prompt_and_the_two_commands() {
    let log = confirmed_plan(Some("make greet say hello"));
    let long = format!("also make it shout {}", "x".repeat(200));
    let log = add(&log, "person_said", json!({ "text": long }));
    let p = unabsorbed(&log)[0];
    let t = p.t;
    let root = std::env::temp_dir();
    let words = refusal(p, &Project { log: &log, root: &root });
    let shown = format!("also make it shout {}…", "x".repeat(101));
    assert_eq!(
        words,
        format!(
            "familiar-ai: not done yet · the person said \"{shown}\"; no criterion names it yet, and it is not marked as needing no work\n  claim it: familiar-ai claim criterion --criterion_id c2 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1 --for {t}\n  or:       familiar-ai claim person_said_answered --for {t} --how no_work"
        )
    );
}
