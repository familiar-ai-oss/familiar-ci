//! One re-run per `try_again` (plan §6.7, behaviour 2; lane S5): the person's `try_again` on
//! `environment_error`, `run_unclassified` or `retry_spent` makes the commit of the part's latest
//! laptop result before it owed again, until each own and touching test has a laptop result there
//! after the answer: once per answer. The three holds are raised in S7; the answer is appended raw,
//! as the person's pick is recorded.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::moves::owed;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
/// c1's own test and its touching test in the sample.
const OWN: &str = "tests/test_c1.py";
const TOUCHING: &str = "tests/test_login_old.py";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn result(log: &Log, file: &str, id: &str, outcome: &str) -> Log {
    ap(log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": file, "commit": id, "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }))
}

/// A run of c1 at `id`: the own test with `outcome`, the touching test passed.
fn ran(log: &Log, id: &str, outcome: &str) -> Log {
    result(&result(log, OWN, id, outcome), TOUCHING, id, "passed")
}

fn answer(log: &Log, hold: &str, choice: &str) -> Log {
    ap(log, "hold_answered", json!({ "criterion_id": "c1", "hold": hold, "choice": choice, "typed": "" }))
}

fn owed_ids(log: &Log) -> Vec<String> {
    owed(&View::new(log), "c1").into_iter().map(|w| w.commit).collect()
}

/// c1's tests commit k1, both tests run there: nothing owed.
fn ran_at_k1() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = ap(&log, "commit_seen", json!({ "commit": "k1", "files": [OWN], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": OWN, "test_name": "t1" }));
    let log = ran(&log, "k1", "failed");
    assert!(owed_ids(&log).is_empty(), "every test ran at k1");
    log
}

#[test]
fn no_answer_nothing_runs_again() {
    assert!(owed_ids(&ran_at_k1()).is_empty());
}

#[test]
fn a_try_again_makes_the_commit_owed_until_each_test_ran_there_after_it() {
    for hold in ["environment_error", "run_unclassified", "retry_spent"] {
        let log = answer(&ran_at_k1(), hold, "try_again");
        assert_eq!(owed_ids(&log), ["k1"], "{hold}: the commit of the latest result before the answer runs again");
        let log = result(&log, OWN, "k1", "failed");
        assert_eq!(owed_ids(&log), ["k1"], "{hold}: one file's result after the answer does not serve the others");
        let log = result(&log, TOUCHING, "k1", "passed");
        assert!(owed_ids(&log).is_empty(), "{hold}: once per answer");
    }
}

#[test]
fn a_second_answer_runs_it_once_more() {
    let log = answer(&ran_at_k1(), "environment_error", "try_again");
    let log = ran(&log, "k1", "failed");
    assert!(owed_ids(&log).is_empty(), "the first answer is spent");
    let log = answer(&log, "environment_error", "try_again");
    assert_eq!(owed_ids(&log), ["k1"], "a second answer: one more re-run");
    let log = ran(&log, "k1", "passed");
    assert!(owed_ids(&log).is_empty());
}

#[test]
fn a_re_run_that_did_not_run_still_spends_its_answer() {
    let log = answer(&ran_at_k1(), "retry_spent", "try_again");
    let log = result(&result(&log, OWN, "k1", "did_not_run"), TOUCHING, "k1", "did_not_run");
    assert!(owed_ids(&log).is_empty(), "the re-run happened; the next one is the person's again");
}

#[test]
fn only_try_again_on_the_three_holds_runs_it_again() {
    for (hold, choice) in [("two_failures", "try_again"), ("two_tries_not_red", "try_again"), ("retry_spent", "re_plan"), ("environment_error", "release")] {
        let log = answer(&ran_at_k1(), hold, choice);
        assert!(owed_ids(&log).is_empty(), "{hold} {choice} runs nothing again");
    }
}

#[test]
fn the_answer_runs_the_commit_of_the_latest_result_before_it() {
    // k2: code on c1's anchor, run there; the answer re-runs k2, not k1
    let log = ap(&ran_at_k1(), "commit_seen", json!({ "commit": "k2", "files": ["src/login.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": ["c1"] }));
    let log = ran(&log, "k2", "failed");
    assert!(owed_ids(&log).is_empty());
    let log = answer(&log, "run_unclassified", "try_again");
    assert_eq!(owed_ids(&log), ["k2"]);
}

#[test]
fn the_re_run_takes_its_place_among_the_owed_commits_oldest_first() {
    // k2 lands on the anchor and is not run yet when the person answers about k1's run
    let log = ap(&ran_at_k1(), "commit_seen", json!({ "commit": "k2", "files": ["src/login.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": ["c1"] }));
    assert_eq!(owed_ids(&log), ["k2"]);
    let log = answer(&log, "environment_error", "try_again");
    assert_eq!(owed_ids(&log), ["k1", "k2"], "k1 runs again, then k2");
}

#[test]
fn an_answer_before_any_result_runs_nothing_again() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = answer(&log, "environment_error", "try_again");
    let log = ap(&log, "commit_seen", json!({ "commit": "k1", "files": [OWN], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": OWN, "test_name": "t1" }));
    let log = ran(&log, "k1", "failed");
    assert!(owed_ids(&log).is_empty(), "the answer came before k1 ran");
}
