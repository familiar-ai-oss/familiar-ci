//! The reserved part `setup` (plan A-4, lane a-surf): the claim door refuses it as a criterion's
//! id, and its card reads plainly for a person: it is "the test setup", its question names the
//! runner and the outcomes that will ask, what happened is said in words, no ask or part is
//! quoted, and each pick tells the agent what comes next.

mod common;

use common::Project;
use familiar_ai::teach::{person_words as pw, picker, words};
use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::reading::SETUP_PART;
use familiar_ai_rules::sample::{criterion_fixture, PROJECT_ROOT, TEST_COMMAND};
use familiar_ai_rules::{terminal, View};
use serde_json::{json, Value};

const RESERVED: &str = "setup is the reserved part that carries the project's own cards (the test setup); give the criterion another id, such as c1";

#[test]
fn the_claim_door_refuses_setup_as_a_criterions_id() {
    let p = Project::new("setup-reserved");
    for args in [
        vec!["criterion", "--criterion_id", "setup", "--text", "greet says hello", "--path", "light"],
        vec!["test_planned", "--criterion_id", "setup", "--file_path", "tests/test_greet.sh", "--will", "create"],
        vec!["criterion_head", "--criterion_id", "setup"],
    ] {
        let (code, _out, err) = p.try_claim(&args);
        assert_eq!(code, 1, "{args:?}: {err}");
        assert!(err.starts_with(&format!("familiar-ai: not claimed yet · {RESERVED}")), "{args:?}: {err}");
    }
    // the plan's lines go through the same door
    let (code, _out, err) = p.run(&["plan"], r#"[{"kind":"criterion","criterion_id":"setup","text":"greet says hello","path":"light"}]"#);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(RESERVED), "{err}");
    // another id meets the door as before
    let (_, _, err) = p.try_claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"]);
    assert!(!err.contains(RESERVED), "{err}");
}

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, 0.0, "_default_").expect("the door takes it")
}

/// One part, its stack, and the dry run's choice with gaps P5 and P6.
fn gapped() -> Log {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "_default_").expect("the fixture");
    raw(&log, "reader_chosen", json!({
        "run_id": "d-1", "root": PROJECT_ROOT, "runner": "pytest", "reader": "pytest:tables", "command": TEST_COMMAND,
        "commit": "base", "familiar": "0.1.0", "watched": ["pyproject.toml"], "gaps": ["P5", "P6"], "why": "no_choice",
    }))
}

#[test]
fn the_setup_card_is_the_test_setup_and_names_the_runner_and_the_outcomes_that_ask() {
    let log = gapped();
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.criterion_id == SETUP_PART).expect("the setup card");
    assert_eq!(
        card.question,
        "the test setup: Familiar cannot read every outcome of pytest at src/; a run that ends in one of these will ask you: a pass beside a skipped test, no test selected. What now?"
    );
    assert_eq!(pw::name_of(&pw::asks(&v), SETUP_PART).as_deref(), Some("the test setup"));
    assert_eq!(words::quoted(&v, SETUP_PART), "the test setup");
    let a = pw::asked(&v, &card);
    assert_eq!(a.happened, "Familiar ran small probe tests of its own through the test runner here, and some of their outcomes could not be read.");
    assert!(a.parts.is_empty() && a.said.is_empty(), "{a:?}");
    let lines = pw::lines(&a);
    assert_eq!(lines, vec![format!("{} {}", pw::WHAT_HAPPENED, a.happened)], "no ask and no part on the setup card");
    assert_eq!(a.sentence, "which setting or plugin of the test runner could change what its runs print, if you can tell");
    assert_eq!(
        a.choices,
        vec![
            ("keep_going".to_string(), "go on; each run Familiar cannot read will ask you".to_string()),
            ("try_again".to_string(), "you have changed how the tests run; check how their runs are read again".to_string()),
        ]
    );
    let facts = picker::facts(&v, &card);
    assert!(facts.contains("a question for the person is open for the test setup."), "{facts}");
    assert!(!facts.contains("part 0") && !facts.contains("of what they asked"), "{facts}");
    let shown = words::card(&v, &card);
    assert!(!shown.contains("you asked") && !shown.contains("split that"), "{shown}");
}

#[test]
fn each_pick_on_the_setup_card_tells_the_agent_what_comes_next() {
    let before = gapped();
    for (choice, want) in [
        ("keep_going", "The person chose to go on with the test setup as it is: a run Familiar cannot read asks them on its own card. Carry on with the work."),
        (
            "try_again",
            "The person changed how the tests run: a new dry run reads the runner again and writes its choice to .familiar/adapter, which is committed with the work. Carry on with the work.",
        ),
    ] {
        let after = raw(&before, "hold_answered", json!({ "criterion_id": SETUP_PART, "hold": "reading_gap", "choice": choice, "typed": "" }));
        let line = words::after_pick(&View::new(&before), &View::new(&after), SETUP_PART, "reading_gap", choice);
        assert_eq!(line.as_deref(), Some(want), "{choice}");
        let told = picker::told_after(&View::new(&before), &View::new(&after), SETUP_PART, "reading_gap", choice, "x");
        assert!(told.contains("the person answered the question for the test setup: x."), "{told}");
    }
}

#[test]
fn status_json_names_the_setup_card_for_the_person() {
    let p = Project::builder("setup-card-json").session("5c67ae1d-1111-4222-8333-944455566600").create();
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "python -m pytest"]);
    let record = familiar_ai_facts::Record::open(&p.path(".familiar/record")).expect("the record");
    let f = json!({
        "run_id": "d-9", "root": "", "runner": "pytest", "reader": "pytest:tables", "command": "python -m pytest",
        "commit": "abc", "familiar": "0.1.0", "watched": [], "gaps": ["P7"], "why": "the dry run"
    });
    record.append_raw("reader_chosen", f.as_object().cloned().unwrap(), 1.0e10, "familiar-ai").expect("append");
    drop(record);
    let (code, out, err) = p.run(&["status", "--json"], "");
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("one JSON object");
    let card = v["cards"].as_array().and_then(|c| c.iter().find(|c| c["criterion_id"] == SETUP_PART)).expect("the setup card");
    assert_eq!(card["name"], "the test setup", "{card}");
    assert!(card["question"].as_str().is_some_and(|q| q.contains("pytest") && q.contains("an error raised in the code") && !q.contains("P7")), "{card}");
}
