//! The first done commit (the owner's CI bar, 2026-10-20; plan warm-popping-key step 5): the
//! head at the first point in the record where at least one part is active (not released) and
//! every part is done by the Stop decision's predicate (green, proven or released). It is history:
//! a later commit, or a part added after it, does not move it.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::criterion::observed;
use familiar_ai_rules::hooks::first_done_commit;
use familiar_ai_rules::lifecycle::state;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const T1: &str = "tests/test_c1.py";
const T2: &str = "tests/test_c2.py";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// The start check of `c` on the sample's place.
fn started(log: &Log, c: &str) -> Log {
    let (file, symbol, a, b) = if c == "c1" { ("src/login.py", "check_password", 40, 62) } else { ("src/mail.py", "reset_link", 10, 30) };
    ap(log, "anchor_sha_matched", json!({ "criterion_id": c, "file_path": file, "symbol": symbol, "line_start": a, "line_end": b, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }))
}

/// The sample's two parts, each the head of its own string, both checked at the start.
fn two_heads() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let log = ap(&log, "criterion_head", json!({ "criterion_id": "c2" }));
    started(&started(&log, "c1"), "c2")
}

/// A commit and the binary's observations beside it, one per part.
fn commit(log: &Log, id: &str, files: &[&str], tests: bool, overlap: &[&str], seen: &[(&str, &str)]) -> Log {
    let mut log = ap(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": !tests, "anchor_overlap": overlap }));
    for (c, h) in seen {
        let rev = if *c == "c1" { "R1" } else { "R2" };
        log = ap(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": rev, "commit": id, "tests_hash": h }));
    }
    log
}

/// The binary's run of `c`'s own test at `id`, stamped with the fingerprint it observed there.
fn run(log: &Log, c: &str, id: &str, outcome: &str) -> Log {
    let file = if c == "c1" { T1 } else { T2 };
    let mut log = log.clone();
    if !log.iter().any(|f| f.kind == "test_written" && f.fields.get("criterion_id") == Some(&json!(c))) {
        log = ap(&log, "test_written", json!({ "criterion_id": c, "file_path": file, "test_name": "t1" }));
    }
    log = ap(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": file, "commit": id, "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }));
    if c == "c1" {
        log = ap(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": "tests/test_login_old.py", "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    let v = View::new(&log);
    let proof = observed(&v, c, id).and_then(|o| o.fields.get("tests_hash")).cloned().expect("observed");
    ap(&log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": proof, "ran_in": "laptop" }))
}

fn first(log: &Log) -> Option<String> {
    first_done_commit(&View::new(log))
}

const BOTH: [(&str, &str); 2] = [("c1", "S1"), ("c2", "S2")];

/// Both parts' tests red at f1; c1 green at A; c2 green at B.
fn green_at_a_then_b() -> (Log, Log) {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &BOTH);
    let log = run(&log, "c1", "f1", "failed");
    let log = run(&log, "c2", "f1", "failed");
    let log = commit(&log, "A", &["src/login.py"], false, &["c1"], &BOTH);
    let log = run(&log, "c1", "A", "passed");
    let log = run(&log, "c2", "A", "failed");
    let at_a = log.clone();
    let log = commit(&log, "B", &["src/mail.py"], false, &["c2"], &BOTH);
    let log = run(&log, "c1", "B", "passed");
    (at_a, run(&log, "c2", "B", "passed"))
}

#[test]
fn two_parts_are_first_done_at_the_commit_where_the_second_went_green() {
    let (at_a, at_b) = green_at_a_then_b();
    let v = View::new(&at_a);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "sealed"));
    assert_eq!(first(&at_a), None, "one part green and one sealed is not done");
    let v = View::new(&at_b);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "green"));
    assert_eq!(first(&at_b).as_deref(), Some("B"));
    // a later commit C, run green for both, does not move it
    let log = commit(&at_b, "C", &["README.md"], false, &[], &BOTH);
    let log = run(&run(&log, "c1", "C", "passed"), "c2", "C", "passed");
    assert_eq!(first(&log).as_deref(), Some("B"), "the first, not the latest");
}

#[test]
fn nothing_is_done_before_any_part_is_planned_or_while_one_is_not_done() {
    assert_eq!(first(&Vec::new()), None, "no part");
    assert_eq!(first(&two_heads()), None, "planned, nothing run");
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &BOTH);
    let log = run(&run(&log, "c1", "f1", "failed"), "c2", "f1", "failed");
    assert_eq!(first(&log), None, "both sealed");
}

fn release(log: &Log, c: &str) -> Log {
    ap(log, "hold_answered", json!({ "criterion_id": c, "hold": "two_failures", "choice": "release", "typed": "" }))
}

#[test]
fn a_released_part_counts_done_but_every_part_released_is_no_done_work() {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &BOTH);
    let log = run(&run(&log, "c1", "f1", "failed"), "c2", "f1", "failed");
    let all_released = release(&release(&log, "c1"), "c2");
    assert_eq!(first(&all_released), None, "no active part: nothing was done");
    // c1 released, c2 green at B: done at B
    let log = release(&log, "c1");
    let log = commit(&log, "B", &["src/mail.py"], false, &["c2"], &BOTH);
    let log = run(&log, "c2", "B", "passed");
    assert_eq!(state(&View::new(&log), "c2"), "green");
    assert_eq!(first(&log).as_deref(), Some("B"));
}

#[test]
fn a_part_added_after_done_leaves_the_first_done_commit_where_it_was() {
    // c1 alone: red at k1, green at B
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = started(&log, "c1");
    let log = commit(&log, "k1", &[T1], true, &[], &[("c1", "S1")]);
    let log = run(&log, "c1", "k1", "failed");
    let log = commit(&log, "B", &["src/login.py"], false, &["c1"], &[("c1", "S1")]);
    let log = run(&log, "c1", "B", "passed");
    assert_eq!(first(&log).as_deref(), Some("B"));
    // c2 added afterwards: its plan lines, as the two-part fixture writes them
    let both = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let mut log = log;
    for f in both.iter().filter(|f| f.fields.get("criterion_id") == Some(&json!("c2"))) {
        log = append_raw(&log, f.kind, f.fields.clone(), AT, TENANT).expect("c2's plan");
    }
    let log = started(&ap(&log, "criterion_head", json!({ "criterion_id": "c2" })), "c2");
    let v = View::new(&log);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "testing"), "a new part not done");
    assert_eq!(first(&log).as_deref(), Some("B"), "history: the first done commit stays");
    // c2 red at k3, then both green at D
    let log = commit(&log, "k3", &[T2], true, &[], &BOTH);
    let log = run(&log, "c2", "k3", "failed");
    let log = commit(&log, "D", &["src/mail.py"], false, &["c2"], &BOTH);
    let log = run(&run(&log, "c1", "D", "passed"), "c2", "D", "passed");
    let v = View::new(&log);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "green"));
    assert_eq!(first(&log).as_deref(), Some("B"), "done again at D, the first is still B");
}
