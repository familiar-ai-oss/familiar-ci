//! The rules items the private checks crate used to copy are public, and the
//! three `_skipping` readers leave one investigator transfer out, reading the
//! log as if that investigator had not yet been sent. This file compiles only
//! when every item is `pub`.

use familiar_ai_facts::{append_raw, Fact, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{action, criterion, hooks, lifecycle, moves, reader, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_default()
}

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m: Option<Move> = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

fn commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

fn start(model: &str) -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default();
    let log = fact(&log, "model_seen", json!({ "agent_conversation_id": "a1", "model": model }));
    step(&log, "start_check", None)
}

fn failed_twice() -> Log {
    let log = commit(&start("claude-haiku-4-5-20251001"), "tests_only", "failed");
    let log = commit(&log, "code", "failed");
    commit(&log, "code", "failed")
}

fn tries_twice() -> Log {
    let log = commit(&start("claude-haiku-4-5-20251001"), "tests_only", "passed");
    commit(&log, "changed_test", "passed")
}

fn transfer<'a>(v: &View<'a>) -> &'a Fact {
    v.of("criterion_transfer", "c1").last().expect("an investigator transfer")
}

#[test]
fn attempts_skipping_leaves_the_investigators_transfer_out() {
    let log = step(&failed_twice(), "investigator", None);
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts(&v, "c1"), 0, "the investigator starts its own two");
    assert_eq!(lifecycle::attempts_skipping(&v, "c1", None), 0, "None is the plain reader");
    assert_eq!(lifecycle::attempts_skipping(&v, "c1", Some(transfer(&v))), 2, "without the transfer the two failures stand");
}

#[test]
fn tries_not_red_skipping_leaves_the_investigators_transfer_out() {
    // D5 (the owner's announced definition change, 2026-10-16): no investigator is sent for
    // two_tries_not_red, so the skip reader is checked on a plain log
    let log = tries_twice();
    let v = View::new(&log);
    assert!(familiar_ai_rules::moves::criterion_admissible(&v).iter().all(|m| m.label.as_deref() != Some("investigator")), "no investigator for two_tries_not_red");
    assert_eq!(lifecycle::tries_not_red_skipping(&v, "c1", None), 2);
    assert_eq!(lifecycle::tries_not_red(&v, "c1"), 2, "the cached plain answer is untouched by a skip call");
}

#[test]
fn last_investigator_skipping_leaves_the_transfer_out() {
    let log = step(&failed_twice(), "investigator", None);
    let v = View::new(&log);
    let t = transfer(&v);
    assert_eq!(lifecycle::last_investigator(&v, "c1").map(|f| f.t), Some(t.t));
    assert_eq!(lifecycle::last_investigator_skipping(&v, "c1", None).map(|f| f.t), Some(t.t));
    assert!(lifecycle::last_investigator_skipping(&v, "c1", Some(t)).is_none());
}

#[test]
fn the_lifecycle_readers_are_public() {
    let log = failed_twice();
    let v = View::new(&log);
    assert!(lifecycle::last_answer(&v, "c1", "two_failures").is_none());
    let s = v.of("fingerprint_stamped", "c1").last().expect("a stamp");
    assert!(lifecycle::stamp_touches(&v, "c1", s));
    let laptop = criterion::results(&v, "c1", criterion::Place::Laptop, false);
    let own = criterion::own_tests(&v, "c1");
    let file = own.first().map(|t| t.file_path.clone()).expect("an own test");
    let ran = lifecycle::ran_at(&laptop, s, &file).expect("the test ran at the stamp");
    assert_eq!(criterion::tname(ran), file);
    assert!(criterion::outcome_is(Some(ran), "failed"));
    assert!(!criterion::outcome_is(None, "failed"));
}

#[test]
fn the_hook_and_action_items_are_public() {
    let log = failed_twice();
    let v = View::new(&log);
    assert_eq!(hooks::LINE_SUPPLY.len(), 11);
    assert_eq!(hooks::LINE_SUPPLY[0], ["intent_submitted"]);
    assert!(!hooks::test_files(&v, "c1").is_empty());
    let _ = hooks::anchor_file(&v, "c1");
    let seen = v.kind("commit_seen").last().and_then(|f| reader::text(f, "commit")).expect("a commit").to_string();
    assert!(!action::content(&v, &seen).starts_with('?'));
    assert!(action::content(&v, "no-such-commit").starts_with('?'));
    assert!(action::same(&v, &seen, &seen));
    assert!(!action::same(&v, &seen, "no-such-commit"));
}

#[test]
fn the_reader_accessors_are_public() {
    let fields = json!({ "a": "s", "b": true, "c": ["p", "q"] }).as_object().cloned().unwrap_or_default();
    let f = Fact { t: 7, at: 0.0, kind: "x", fields, tenant: TENANT.to_string() };
    assert_eq!(reader::text(&f, "a"), Some("s"));
    assert!(reader::flag(&f, "b"));
    assert!(!reader::flag(&f, "a"));
    assert_eq!(reader::texts(&f, "c"), ["p", "q"]);
    assert!(reader::texts(&f, "a").is_empty());
    assert_eq!(reader::t_of(&f), 7);
}
