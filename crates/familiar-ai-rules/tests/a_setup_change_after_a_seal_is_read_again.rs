//! A setup change after a seal (plan A-5, lane a5): each commit after the tests-first commit that
//! changes a file the choice watches owes one run of the tests-first commit's code with that
//! commit's watched files put back (`run_reserved.setup_from`). An honest red keeps the seal;
//! anything else holds the part on `setup_changed` [draft], which pauses and is answered
//! `re_plan` or `release`. Green waits while a re-check is owed. A re-check is never a result and
//! never an attempt. CI runs the same re-check after the teeth, and proves only after it.

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{answers_of, tests_first_commit, PAUSING};
use familiar_ai_rules::reading::{setup_changed, setup_rechecks_owed};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move, PROJECT_ROOT, TEST_COMMAND};
use familiar_ai_rules::{lifecycle, moves, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TEST: &str = "tests/test_c1.py";
const TOUCHING: &str = "tests/test_login_old.py";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

fn step(log: &Log, label: &str) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(label));
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).expect("applied"),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

fn chose(log: &Log, commit: &str) -> Log {
    raw(log, "reader_chosen", json!({
        "run_id": format!("d-{}", log.len()), "root": PROJECT_ROOT, "runner": "pytest", "reader": "pytest:reporter",
        "command": TEST_COMMAND, "commit": commit, "familiar": "0.1.0",
        "watched": ["pyproject.toml", "**/conftest.py"], "gaps": [], "why": "no_choice",
    }))
}

/// The binary's run of c1 at the tip, translated as `label`, its results and the stamp.
fn ran(log: &Log, word: &str, label: &str) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": TEST_COMMAND, "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    log = raw(&log, "test_log_translation", json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"}));
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped")
}

/// A commit of files that are neither tests nor code, with c1's fingerprint as it stands observed
/// beside it, as the binary observes every part at every commit.
fn commit(log: &Log, id: &str, files: &[&str]) -> Log {
    let now = familiar_ai_rules::criterion::current_tests(&View::new(log), "c1").map(str::to_string);
    let log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": false, "touches_code": false, "anchor_overlap": [] }));
    match now {
        Some(h) => raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": h })),
        None => log,
    }
}

/// The re-check of c1's tests-first commit with `x`'s watched files put back, read as `label`.
fn rechecked(log: &Log, x: &str, label: &str) -> Log {
    let tf = tests_first_commit(&View::new(log), "c1").and_then(|t| t.ids.last().cloned()).expect("a tests-first commit");
    let rid = format!("s-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": tf, "command": TEST_COMMAND, "resources": ["tests"], "wait": "60", "by": "binary", "setup_from": x}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": if label == "passed" { "passed" } else { "failed" }, "end": "exited"}));
    raw(&log, "test_log_translation", json!({"run_id": rid, "criterion_id": "c1", "commit": tf, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"}))
}

fn sealed() -> Log {
    let log = chose(&criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture"), "base");
    let log = step(&log, "start_check");
    let log = step(&log, "tests_only");
    let log = step(&log, "test_written");
    ran(&log, "failed", "tests_failed")
}

fn green() -> Log {
    let log = ran(&step(&sealed(), "code"), "passed", "passed");
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "green");
    log
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::whys(&View::new(log), "c1")
}

fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

#[test]
fn the_card_pauses_and_is_answered_re_plan_or_release() {
    assert_eq!(answers_of("setup_changed"), Some(&["re_plan", "release"][..]));
    assert!(PAUSING.contains(&"setup_changed"));
}

#[test]
fn a_watched_file_changed_after_the_seal_owes_a_re_check_and_green_waits_for_it() {
    let log = commit(&green(), "x1", &["src/conftest.py"]);
    let v = View::new(&log);
    assert_eq!(setup_rechecks_owed(&v, "c1"), ["x1"]);
    assert!(!setup_changed(&v, "c1"));
    assert_ne!(state(&log), "green", "green waits while the re-check is owed");
    let ok = rechecked(&log, "x1", "tests_failed");
    assert_eq!(setup_rechecks_owed(&View::new(&ok), "c1"), Vec::<String>::new());
    assert!(!whys(&ok).contains(&"setup_changed".to_string()));
    assert_eq!(state(&ok), "green", "an honest red under the new setup keeps the seal");
}

#[test]
fn a_re_check_that_is_not_an_honest_red_holds_the_part_on_its_card() {
    let log = rechecked(&commit(&green(), "x1", &["src/conftest.py"]), "x1", "passed");
    let v = View::new(&log);
    assert!(setup_changed(&v, "c1"));
    assert!(whys(&log).contains(&"setup_changed".to_string()));
    assert!(lifecycle::paused(&v, "c1"));
    assert_eq!(state(&log), "held");
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == "setup_changed").expect("the card");
    assert_eq!(card.choices.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["re_plan", "release"]);
    // a later setup commit that reads red again does not make the card go away: only the person does
    let later = rechecked(&commit(&log, "x2", &["src/pyproject.toml"]), "x2", "tests_failed");
    assert!(whys(&later).contains(&"setup_changed".to_string()));
    let answered = raw(&log, "hold_answered", json!({"criterion_id": "c1", "hold": "setup_changed", "choice": "re_plan", "typed": ""}));
    assert!(!whys(&answered).contains(&"setup_changed".to_string()));
    let released = raw(&log, "hold_answered", json!({"criterion_id": "c1", "hold": "setup_changed", "choice": "release", "typed": ""}));
    assert_eq!(whys(&released), ["released"]);
}

#[test]
fn only_a_watched_file_after_the_tests_first_commit_owes_one() {
    let log = commit(&green(), "x1", &["src/login.py", "docs/conftest.py"]);
    assert_eq!(setup_rechecks_owed(&View::new(&log), "c1"), Vec::<String>::new());
    // a setup commit before the tests-first commit is the setup the seal was read under
    let log = chose(&criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture"), "base");
    let log = commit(&log, "x0", &["src/conftest.py"]);
    let log = chose(&log, "x0"); // the dry run that commit owes
    let log = step(&log, "start_check");
    let log = step(&log, "tests_only");
    let log = step(&log, "test_written");
    let log = ran(&log, "failed", "tests_failed");
    assert_eq!(setup_rechecks_owed(&View::new(&log), "c1"), Vec::<String>::new());
    // with no choice there is nothing watched
    let none = commit(&ran(&step(&sealed_without_choice(), "code"), "passed", "passed"), "x1", &["src/conftest.py"]);
    assert_eq!(setup_rechecks_owed(&View::new(&none), "c1"), Vec::<String>::new());
    assert_eq!(state(&none), "green");
}

fn sealed_without_choice() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture");
    let log = step(&log, "start_check");
    let log = step(&log, "tests_only");
    let log = step(&log, "test_written");
    ran(&log, "failed", "tests_failed")
}

#[test]
fn a_re_check_is_never_a_result_or_an_attempt() {
    let log = commit(&ran(&step(&sealed(), "code"), "failed", "tests_failed"), "x1", &["src/conftest.py"]);
    let before = lifecycle::attempts(&View::new(&log), "c1");
    let after = rechecked(&log, "x1", "tests_failed");
    let v = View::new(&after);
    assert_eq!(lifecycle::attempts(&v, "c1"), before);
    assert_eq!(after.iter().filter(|f| f.kind == "pass_please_pass").count(), log.iter().filter(|f| f.kind == "pass_please_pass").count());
}

fn more_kinds(m: &Move) -> Vec<String> {
    m.more.iter().flatten().map(|f| f.kind.clone()).collect()
}

#[test]
fn the_binary_runs_the_dry_run_first_then_the_re_check() {
    let log = commit(&green(), "x1", &["src/conftest.py"]);
    let b = moves::binary_moves(&View::new(&log));
    assert!(!b.is_empty() && b.iter().all(|m| m.kind == "reader_chosen"), "{:?}", b.iter().map(|m| &m.kind).collect::<Vec<_>>());
    let log = apply_move(&log, &b[0], AT, TENANT).expect("applied");
    let b = moves::binary_moves(&View::new(&log));
    assert!(!b.is_empty());
    let tf = tests_first_commit(&View::new(&log), "c1").and_then(|t| t.ids.last().cloned()).expect("tf");
    for m in &b {
        assert_eq!(m.kind, "run_reserved");
        assert_eq!(m.fields.get("setup_from").and_then(Value::as_str), Some("x1"));
        assert_eq!(m.fields.get("commit").and_then(Value::as_str), Some(tf.as_str()));
        assert!(!more_kinds(m).contains(&"pass_please_pass".to_string()), "a re-check writes no result");
        assert!(more_kinds(m).contains(&"test_log_translation".to_string()));
    }
}

/// Drive the binary's moves to the end: every run at the tests-first commit, the teeth and every
/// re-check read red, every other run passed.
fn drive(mut log: Log) -> Log {
    for _ in 0..40 {
        let v = View::new(&log);
        let b = moves::binary_moves(&v);
        if b.is_empty() {
            return log;
        }
        let tf = tests_first_commit(&v, "c1").and_then(|t| t.ids.last().cloned()).unwrap_or_default();
        let red = |m: &Move| {
            m.fields.get("setup_from").is_some()
                || m.fields.get("commit").and_then(Value::as_str) == Some(tf.as_str())
                || m.more.iter().flatten().any(|f| f.fields.get("with_anchor_reverted") == Some(&Value::Bool(true)))
        };
        let label = |m: &Move| m.more.iter().flatten().find(|f| f.kind == "test_log_translation").and_then(|f| f.fields.get("label")).and_then(Value::as_str).map(str::to_string);
        let pick = b
            .iter()
            .find(|m| match label(m) {
                None => true,
                Some(l) => (l == "tests_failed") == red(m) && (l == "tests_failed" || l == "passed"),
            })
            .unwrap_or(&b[0]);
        log = apply_move(&log, pick, AT, TENANT).expect("applied");
    }
    panic!("the binary's moves did not end")
}

#[test]
fn ci_re_checks_after_the_teeth_and_proves_only_after_it() {
    let log = rechecked(&commit(&green(), "x1", &["src/conftest.py"]), "x1", "tests_failed");
    let log = chose(&log, "x1");
    let log = raw(&log, "pipeline_run_seen", json!({ "head_commit": head(&log) }));
    let log = drive(log);
    let pipeline_rechecks: Vec<usize> = log
        .iter()
        .filter(|f| f.kind == "run_reserved" && f.fields.get("by").and_then(Value::as_str) == Some("pipeline") && f.fields.get("setup_from").is_some())
        .map(|f| f.t as usize)
        .collect();
    assert_eq!(pipeline_rechecks.len(), 1, "one re-check in CI");
    let teeth = log
        .iter()
        .rfind(|f| f.kind == "pass_please_pass" && f.fields.get("with_anchor_reverted") == Some(&Value::Bool(true)))
        .map(|f| f.t as usize)
        .expect("the teeth ran");
    assert!(pipeline_rechecks[0] > teeth, "after the teeth");
    assert_eq!(state(&log), "proven");
    let before = View::new(&log).prefix(pipeline_rechecks[0]);
    assert_ne!(lifecycle::state(&before, "c1"), "proven", "not proven before the re-check");
}
