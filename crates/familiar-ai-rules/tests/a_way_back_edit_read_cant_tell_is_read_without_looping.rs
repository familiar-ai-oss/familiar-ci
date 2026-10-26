//! The checks crate's probe (logs/cases/probe-way-back-cant-tell.log, 2026-10-25) [draft names]:
//! a part sealed on a valid red goes green, its test change is announced, the edited test is
//! committed alone and its run reads can't-tell. Since 23b964c2 ("Confirmed once per part") the
//! rules overflowed the stack on it: the way back (`criterion::mark`) asks whether the edited tests
//! came out red, that asks whether the can't-tell run carries an earlier its_the_red, and that asked
//! for the tests-first commit, which asks the way back again. Read without looping, the record holds
//! the can't-tell card, as it did before 23b964c2: no earlier can't-tell run was confirmed.

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use familiar_ai_rules::{lifecycle, moves, View};
use serde_json::{json, Value};

const TEST: &str = "tests/test_c1.py";
const TOUCHING: &str = "tests/test_login_old.py";

fn step(log: &Log, label: &str) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label}"));
    apply_move(log, &m, 0.0, "_default_").unwrap()
}

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, 0.0, "_default_").unwrap()
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

fn ran(log: &Log, rid: &str, word: &str, extra: Value) -> Log {
    let id = head(log);
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "evidence": "", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    log
}

#[test]
fn a_can_t_tell_edit_on_the_way_back_holds_the_card_and_does_not_loop() {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "_default_").unwrap();
    let log = step(&log, "start_check");
    let log = step(&log, "tests_only");
    let log = step(&log, "test_written");
    let log = step(&ran(&log, "r-a", "failed", json!({"label": "tests_failed", "location": {"file_path": TEST, "line": 7, "kind": "assertion"}})), "fingerprint_stamped");
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "sealed");
    let log = step(&log, "code");
    let log = step(&ran(&log, "r-b", "passed", json!({"label": "passed"})), "fingerprint_stamped");
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "green");
    let log = raw(&log, "test_change_announced", json!({"criterion_id": "c1", "reason": "r"}));
    let log = raw(&log, "commit_seen", json!({"commit": "e2", "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": []}));
    let log = raw(&log, "tests_observed", json!({"criterion_id": "c1", "criterion_rev": "R1", "commit": "e2", "tests_hash": "S2"}));
    let log = ran(&log, "r-c", "failed", json!({"label": "unclassified"}));
    let log = raw(&log, "fingerprint_stamped", json!({"criterion_id": "c1", "commit": "e2", "proof_not_tampered": "S2", "ran_in": "laptop"}));
    let v = View::new(&log);
    assert_eq!(lifecycle::state(&v, "c1"), "held");
    assert_eq!(lifecycle::whys(&v, "c1"), vec!["run_unclassified".to_string()]);
}
