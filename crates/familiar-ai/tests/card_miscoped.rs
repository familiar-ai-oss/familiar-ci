//! A part whose tests pass once the dependency drafted for it is fixed is miscoped (plan §6.4):
//! the card says it was its dependency; covered_by_dependency closes it as covered and names the
//! next part; re_plan asks what it still has to do. The reading is S9a's re-earn (plan §7.1).

use familiar_ai::teach::{picker, words};
use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::dependency::restring;
use familiar_ai_rules::sample::criterion_fixture;
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
fn miscoped() -> Log {
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


fn picked(log: &Log, choice: &str) -> (String, Log) {
    let mut after = raw(log, "hold_answered", json!({"criterion_id": "c1", "hold": "miscoped", "choice": choice, "typed": ""}));
    if choice == "covered_by_dependency" {
        after = raw(&after, "criterion_covered", json!({"criterion_id": "c1", "by_criterion": "c3"}));
    }
    let told = picker::told_after(&View::new(log), &View::new(&after), "c1", "miscoped", choice, &words::choice(choice));
    (told, after)
}

#[test]
fn the_card_says_the_part_was_its_dependency() {
    let log = miscoped();
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == "miscoped").expect("the card");
    let text = words::card(&v, &card);
    assert!(text.contains("passed once"), "{text}");
    assert!(text.contains("the part it depends on was fixed: part"), "{text}");
}

#[test]
fn covered_by_dependency_closes_it_and_names_what_is_next() {
    let (told, after) = picked(&miscoped(), "covered_by_dependency");
    assert!(lifecycle::released(&View::new(&after), "c1"));
    let q1 = words::quoted(&View::new(&after), "c1");
    let q3 = words::quoted(&View::new(&after), "c3");
    assert_eq!(told.matches(&format!("{q1} is closed as covered by {q3}. Next: no part is left; the plan is complete.")).count(), 1, "{told}");
}

#[test]
fn re_plan_asks_what_it_still_has_to_do() {
    let (told, _) = picked(&miscoped(), "re_plan");
    assert!(told.contains(&format!("its behavior turned out to be {}'s", words::quoted(&View::new(&miscoped()), "c3"))), "{told}");
    assert!(told.contains("still has to do."), "{told}");
}
