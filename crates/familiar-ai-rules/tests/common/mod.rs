//! The way back walked by hand: c1 sealed, green, its sealed test edited.
#![allow(dead_code)]

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{lifecycle, moves, terminal, View};
use serde_json::{json, Value};

pub const AT: f64 = 0.0;
pub const TENANT: &str = "_default_";
pub const TEST: &str = "tests/test_c1.py";
pub const TOUCHING: &str = "tests/test_login_old.py";

pub fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m: Option<Move> = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

/// A fact the walk's moves do not offer, through the one door.
pub fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

/// The binary's run of c1 at the tip: its test with `outcome`, the touching
/// test passed, the stamp.
pub fn run(log: &Log, outcome: &str) -> Log {
    let mut log = step(log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

/// A tests-only commit of c1's test written by hand, run and stamped the
/// way the binary does (moves offer no commit of a criterion that is green).
pub fn edit_run(log: &Log, id: &str, hash: &str, outcome: &str) -> Log {
    // the fingerprint is c1's own observation beside the commit
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    log = raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": hash }));
    for (fp, o) in [(TEST, outcome), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    raw(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": id, "proof_not_tampered": hash, "ran_in": "laptop" }))
}

pub fn whys(log: &Log) -> Vec<String> {
    lifecycle::holds(&View::new(log), "c1").iter().map(|h| h.why.to_string()).collect()
}

pub fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

/// c1 sealed red at its tests-first commit.
pub fn sealed() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    run(&log, "failed")
}

/// c1 green: the code on its anchor, its tests passed under the same stamp.
pub fn green() -> Log {
    let log = step(&sealed(), "code", None);
    let log = run(&log, "passed");
    assert_eq!(state(&log), "green");
    log
}
