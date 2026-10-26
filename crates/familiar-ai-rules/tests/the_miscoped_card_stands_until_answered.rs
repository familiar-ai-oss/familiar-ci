//! `miscoped` (plan §6.4) reads S9a's re-earn, fixed at t4: c1 crashed, the person asked for a
//! dependency, c3 was drafted before it and went green at G, and c1 passed at G. Once raised the
//! card stands until the person answers it, whatever later happens to c3.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::dependency::{miscoped, restring};
use familiar_ai_rules::sample::{criterion_fixture, LINES_OUTSIDE};
use familiar_ai_rules::{lifecycle, plan, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TOUCHING: &str = "tests/test_login_old.py";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn test_of(c: &str) -> String {
    format!("tests/test_{c}.py")
}

fn place_of(c: &str) -> String {
    if c == "c1" { "src/login.py".to_string() } else { format!("src/dep_{c}.py") }
}

fn hash_of(c: &str) -> String {
    if c == "c1" { "S1".to_string() } else { format!("H{c}") }
}

fn commit(log: &Log, id: &str, files: &[String], tests: bool, code: bool, overlap: &[&str]) -> Log {
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    for c in plan::criteria(&View::new(&log)) {
        let seen = log.iter().any(|f| f.kind == "tests_observed" && f.criterion_id() == Some(c.as_str()));
        if seen || files.contains(&test_of(&c)) {
            log = raw(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash_of(&c) }));
        }
    }
    log
}

fn tests_commit(log: &Log, id: &str, c: &str) -> Log {
    let log = commit(log, id, &[test_of(c)], true, false, &[]);
    raw(&log, "test_written", json!({ "criterion_id": c, "file_path": test_of(c), "test_name": "t1" }))
}

fn code_commit(log: &Log, id: &str, c: &str) -> Log {
    commit(log, id, &[place_of(c)], false, true, &[c])
}

fn run(log: &Log, c: &str, id: &str, label: &str) -> Log {
    let word = if label == "passed" { "passed" } else { "failed" };
    let rid = format!("r-{c}-{id}-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": c, "criterion_rev": "R", "commit": id, "command": "python -m pytest", "resources": ["tests"], "wait": "60", "by": "binary" }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": word, "end": "exited" }));
    log = raw(&log, "test_log_translation", json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" }));
    log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": test_of(c), "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    if c == "c1" {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": TOUCHING, "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    }
    raw(&log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": hash_of(c), "ran_in": "laptop" }))
}

fn start_check(log: &Log, c: &str, lines: &str) -> Log {
    let (symbol, from, to) = if c == "c1" { ("check_password".to_string(), 40, 62) } else { (format!("fn_{c}"), 1, 9) };
    raw(log, "anchor_sha_matched", json!({ "criterion_id": c, "file_path": place_of(c), "symbol": symbol, "line_start": from, "line_end": to, "sha": "base", "lines_hash": lines, "matched": true, "checked_at": "start" }))
}

/// c1 crashed at k1; the person's a_dependency; c3 drafted before c1, red at k2, green at k3;
/// c1 passed at k3, the re-earn commit.
pub fn miscoped_at_g() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = start_check(&log, "c1", "L0");
    let log = tests_commit(&log, "k1", "c1");
    let log = run(&log, "c1", "k1", "crashed");
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    let t2 = log.last().map(|f| f.t).unwrap_or(0);
    let mut log = raw(&log, "dependency_drafted", json!({ "approval": t2, "criterion_id": "c3", "text": "c3: what c1 needs", "test_file": test_of("c3") }));
    log = raw(&log, "criterion", json!({ "criterion_id": "c3", "text": "c3: what c1 needs", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = raw(&log, "place_chosen", json!({ "criterion_id": "c3", "places": [{ "file_path": place_of("c3"), "symbol": "fn_c3", "line_start": 1, "line_end": 9 }] }));
    log = raw(&log, "anchor_sha_matched", json!({ "criterion_id": "c3", "file_path": place_of("c3"), "symbol": "fn_c3", "line_start": 1, "line_end": 9, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c3", "file_path": test_of("c3"), "will": "create" }));
    for (kind, fields) in restring(&View::new(&log), "c3", "c1") {
        log = raw(&log, kind, Value::Object(fields));
    }
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c3", "agent_conversation_id": "a1" }));
    log = raw(&log, "fix_location_category", json!({ "criterion_id": "c3", "category": "this_repository", "where": "" }));
    let log = start_check(&log, "c3", "L0");
    let log = tests_commit(&log, "k2", "c3");
    let log = run(&log, "c3", "k2", "tests_failed");
    let log = code_commit(&log, "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    run(&log, "c1", "k3", "passed")
}

fn held(log: &Log) -> bool {
    lifecycle::whys(&View::new(log), "c1").iter().any(|w| w == "miscoped")
}

#[test]
fn passing_at_the_re_earn_commit_raises_the_card() {
    let log = miscoped_at_g();
    assert!(miscoped(&View::new(&log), "c1"));
    assert!(held(&log), "{:?}", lifecycle::whys(&View::new(&log), "c1"));
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == "miscoped").expect("the card");
    assert!(card.question.contains("criterion c1 passed once criterion c3 was fixed"), "{}", card.question);
}

#[test]
fn the_card_stands_when_the_dependency_falls_from_green() {
    let log = miscoped_at_g();
    // c3's code moved outside this work: c3 is held on anchor_moved, no longer green
    let log = start_check(&log, "c3", LINES_OUTSIDE);
    assert_ne!(lifecycle::state(&View::new(&log), "c3"), "green");
    assert!(held(&log), "a raised card stands until the person answers it: {:?}", lifecycle::whys(&View::new(&log), "c1"));
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "miscoped", "choice": "covered_by_dependency", "typed": "" }));
    assert!(!held(&log));
    assert!(lifecycle::released(&View::new(&log), "c1"));
}
