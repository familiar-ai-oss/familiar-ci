//! The second seal after a rebase before CI's run. CI's history names every
//! commit afresh (k6 becomes k6'), while the plan's stamps keep the laptop's
//! ids. The second seal is found by its content, the stamp's proof matched
//! against CI's own tests-only commits, so a rebase does not lose it and CI
//! does not fall back to the way back's commit. A stamp found by content may
//! bind to an earlier commit of the same content (the accepted limit). See docs/decisions.md#the-way-back

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::{action, View};
use serde_json::{json, Value};

fn way_back() -> Log {
    let log = step(&green(), "announce_after_green", None);
    let log = step(&log, "changed_test", None);
    run(&log, "passed")
}

fn commit(log: &Log, id: &str, tests: bool, hash: &str) -> Log {
    let (files, overlap) = if tests { (json!([TEST]), json!([])) } else { (json!(["src/login.py"]), json!(["c1"])) };
    let log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": !tests, "anchor_overlap": overlap }));
    raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": hash }))
}

fn laptop_run(log: &Log, id: &str, hash: &str, outcome: &str) -> Log {
    let mut log = log.clone();
    for (fp, o) in [(TEST, outcome), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    raw(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": id, "proof_not_tampered": hash, "ran_in": "laptop" }))
}

/// The history after the way back: k4 tests back to the red seal's, k5 code,
/// k6 the edited tests alone, stamped red, k7 code green.
fn history() -> Log {
    let log = commit(&way_back(), "k4", true, "S1");
    let log = commit(&log, "k5", false, "S1");
    let log = commit(&log, "k6", true, "S1x");
    let log = laptop_run(&log, "k6", "S1x", "failed");
    let log = commit(&log, "k7", false, "S1x");
    laptop_run(&log, "k7", "S1x", "passed")
}

/// The same history rebased before CI's run: every `commit_seen` and
/// `tests_observed` id x is x', the laptop's facts left alone.
fn rebased(log: &Log) -> Log {
    let mut log = log.clone();
    for f in log.iter_mut().filter(|f| f.kind == "commit_seen" || f.kind == "tests_observed") {
        if let Some(Value::String(id)) = f.fields.get_mut("commit") {
            id.push('\'');
        }
    }
    log
}

#[test]
fn the_second_seal_is_ci_s_own_commit_of_the_re_stamp_s_content() {
    let log = rebased(&history());
    let v = View::new(&log);
    assert_eq!(action::second_seal(&v, "c1"), Some("k6'"), "the re-stamp found by its content");
    assert_eq!(action::tests_first(&v, "c1"), Some("k6'"));
    assert_eq!(action::mark(&v, "c1"), None, "the way back is spent");
    assert_eq!(action::seal_hash(&v, "c1"), Some("S1x"));
}

/// The checks crate's counterexample: k6 committed after the way back but
/// never stamped, CI's run at k6. The way back's own stamp, renamed by the
/// rebase, is not a re-stamp: no second seal, the way back stands at kw'.
#[test]
fn the_way_back_s_own_stamp_renamed_is_not_a_second_seal() {
    let log = commit(&way_back(), "k4", true, "S1");
    let log = commit(&log, "k5", false, "S1");
    let log = rebased(&commit(&log, "k6", true, "S1x"));
    let v = View::new(&log);
    assert_eq!(action::second_seal(&v, "c1"), None);
    assert!(action::mark(&v, "c1").is_some(), "the way back stands");
}
