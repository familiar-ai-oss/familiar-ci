//! The walks reach S6's facts (plan §8 S6, Assumption 30): the binary's laptop run lands as a
//! run with an id, its end, its translation and its results; and while the person's
//! `fix_what_broke_it` stands, the command is locked (§6.3).

mod common;

use common::{raw, step, AT, TENANT};
use familiar_ai_facts::Log;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, moves, View};
use serde_json::{json, Value};

fn tests_first() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    step(&log, "test_written", None)
}

fn ran(log: &Log, label: &str) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v)
        .into_iter()
        // the own run's translation is the move's first; the touching run's comes after it
        .find(|m| m.label.as_deref() == Some("ran") && m.facts().iter().find(|(k, _)| *k == "test_log_translation").is_some_and(|(_, f)| f.get("label").and_then(Value::as_str) == Some(label)))
        .unwrap_or_else(|| panic!("no ran move for {label}"));
    familiar_ai_rules::sample::apply_move(log, &m, AT, TENANT).expect("lands")
}

/// The definition of 2026-10-21 (the owner's plan, step 0): the own tests' results name the part's
/// own run, the touching tests' a run of their own under the touching command.
#[test]
fn a_laptop_run_lands_with_its_id_end_translation_and_results() {
    let log = ran(&tests_first(), "tests_failed");
    let kinds: Vec<&str> = log.iter().rev().take(8).map(|f| f.kind).collect();
    assert_eq!(kinds, ["pass_please_pass", "test_log_translation", "run_ended", "run_reserved", "pass_please_pass", "test_log_translation", "run_ended", "run_reserved"]);
    let run_of = |file: &str| log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == file).and_then(|f| f.fields.get("run_id")).and_then(Value::as_str).unwrap_or("").to_string();
    let command_of = |id: &str| log.iter().find(|f| f.kind == "run_reserved" && f.fields["run_id"] == id).and_then(|f| f.fields.get("command")).and_then(Value::as_str).unwrap_or("").to_string();
    let (own, touching) = (run_of(common::TEST), run_of(common::TOUCHING));
    assert_ne!(own, touching, "two runs");
    assert_eq!(command_of(&touching), criterion::touching_command(&View::new(&log), "c1").unwrap_or_default());
    assert_ne!(command_of(&own), command_of(&touching));
    let log = step(&log, "fingerprint_stamped", None);
    assert!(criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some());
    for label in ["passed", "missing_target_symbol", "timed_out", "unclassified"] {
        let _ = ran(&tests_first(), label);
    }
}

#[test]
fn the_command_is_locked_while_fix_what_broke_it_stands() {
    let offered = |log: &Log| moves::criterion_admissible(&View::new(log)).iter().any(|m| m.kind == "test_stack_detected");
    let log = step(&ran(&tests_first(), "tests_failed"), "fingerprint_stamped", None);
    assert!(offered(&log), "the agent may change its command");
    let log = raw(&log, "hold_answered", json!({"criterion_id": "c1", "hold": "command_broke", "choice": "fix_what_broke_it", "typed": ""}));
    assert!(lifecycle::command_locked(&View::new(&log), "c1"));
    assert!(!offered(&log), "the command is locked");
    let log = step(&log, "code", None);
    let log = ran(&log, "passed");
    assert!(!lifecycle::command_locked(&View::new(&log), "c1"), "a run that is not the command's own breakage frees it");
    assert!(offered(&step(&log, "fingerprint_stamped", None)));
}

/// Lane H6/H7 (D7): the walks' runs are read as the binary reads them now: every translation
/// carries `counts`; the red is an assertion at the planned test's first line beside one other
/// test that passed; the pass counts both; the others count nothing.
#[test]
fn the_walks_runs_carry_counts_and_the_reds_kind() {
    // the own run's translation: the second from the end, the touching run's after it
    let translation = |log: &Log| log.iter().rev().filter(|f| f.kind == "test_log_translation").nth(1).cloned().expect("a translation");
    let t = translation(&ran(&tests_first(), "tests_failed"));
    assert_eq!(t.fields.get("location"), Some(&json!({"file_path": common::TEST, "line": 1, "kind": "assertion"})));
    assert_eq!(t.fields.get("counts"), Some(&json!({"failed": 1, "passed": 1})));
    assert_eq!(translation(&ran(&tests_first(), "passed")).fields.get("counts"), Some(&json!({"passed": 2})));
    for label in ["missing_target_symbol", "timed_out", "unclassified"] {
        assert_eq!(translation(&ran(&tests_first(), label)).fields.get("counts"), Some(&json!({})), "{label}");
    }
    // the touching run, as pytest's table reads a pass beside a skip: unclassified, exit 0, its
    // counts (the owner's plan of 2026-10-21, step 0)
    let log = ran(&tests_first(), "passed");
    let touching = log.iter().rev().find(|f| f.kind == "test_log_translation").cloned().expect("the touching run's");
    assert_eq!((touching.fields.get("label"), touching.fields.get("counts")), (Some(&json!("unclassified")), Some(&json!({"passed": 1, "skipped": 1}))));
    // sealed on the walk's red, the walk's pass after a code commit is green against it, the
    // touching file's skip beside it
    let log = step(&ran(&tests_first(), "tests_failed"), "fingerprint_stamped", None);
    let log = ran(&step(&log, "code", None), "passed");
    let log = step(&log, "fingerprint_stamped", None);
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "green");
}
