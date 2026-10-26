//! The dry run's reservation (plan A, lane a-py): a part-less `run_reserved{by:"dry_run"}`
//! holding the `tests` resource. It never serves a part's work, `await-run` never waits on it
//! for a result, and it is announced in its own words, never as the part's tests.

use familiar_ai::door::runs::{eligibility, runs, Eligibility};
use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

fn add(log: &Log, kind: &str, f: Value) -> Log {
    let at = log.len() as f64 + 1.0;
    append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), at, "t").expect(kind)
}

fn dry(log: &Log, id: &str) -> Log {
    add(log, "run_reserved", json!({"run_id": id, "criterion_id": "", "criterion_rev": "", "commit": "K", "command": "x", "resources": ["tests"], "wait": "100", "by": "dry_run"}))
}

#[test]
fn a_live_dry_run_holds_the_tests_resource() {
    let log = dry(&Vec::new(), "d-1");
    assert_eq!(eligibility(&log, "c1", "r", "K", "x", 2.0), Eligibility::Live);
    assert_eq!(runs(&log)[0].by, "dry_run");
}

#[test]
fn an_ended_dry_run_never_settles_a_parts_work_at_the_same_command_and_commit() {
    let log = dry(&Vec::new(), "d-1");
    let log = add(&log, "run_ended", json!({"run_id": "d-1", "outcome": "passed", "end": "exited", "exit_code": 0}));
    assert_eq!(eligibility(&log, "c1", "r", "K", "x", 3.0), Eligibility::Free { attempt: 0, after: None });
}

#[test]
fn await_run_waits_on_the_parts_runs_at_head_never_the_dry_run() {
    let log = dry(&Vec::new(), "d-1");
    let log = add(&log, "run_reserved", json!({"run_id": "r-1", "criterion_id": "c1", "criterion_rev": "r", "commit": "K", "command": "x", "resources": ["tests"], "wait": "100", "by": "binary"}));
    assert_eq!(familiar_ai::door::told::runs_at_head(&log, "K"), ["r-1"]);
    assert!(familiar_ai::door::told::dry_run_live(&log, 3.0), "a dry run before the part's runs keeps await-run looking for them");
}

#[test]
fn a_dry_run_is_announced_in_its_own_words() {
    let before = Vec::new();
    let after = dry(&before, "d-1");
    let lines = familiar_ai::teach::started_lines(&before, &after);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("dry run") && lines[0].contains("d-1") && lines[0].contains(".familiar/adapter"), "{}", lines[0]);
    assert!(!lines[0].contains("tests started"), "not the part's tests: {}", lines[0]);
    let after = add(&after, "run_reserved", json!({"run_id": "r-1", "criterion_id": "c1", "criterion_rev": "r", "commit": "K", "command": "x", "resources": ["tests"], "wait": "100", "by": "binary"}));
    let lines = familiar_ai::teach::started_lines(&before, &after);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[1].contains("tests started — run r-1"), "{}", lines[1]);
}
