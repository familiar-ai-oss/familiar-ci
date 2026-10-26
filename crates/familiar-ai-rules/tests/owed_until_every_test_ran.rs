//! A commit stays owed until every own and touching test of the criterion
//! has a laptop result there that ran: one file's result does not serve the
//! others, so a late run at an older commit can still be stamped there.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::moves::owed;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn result(log: &Log, file: &str, id: &str, outcome: &str) -> Log {
    ap(log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": file, "commit": id, "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }))
}

fn owed_ids(log: &Log) -> Vec<String> {
    owed(&View::new(log), "c1").into_iter().map(|w| w.commit).collect()
}

#[test]
fn a_commit_is_owed_until_each_own_and_touching_test_has_run_there() {
    // c1: own test tests/test_c1.py (written), touching test tests/test_login_old.py
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = ap(&log, "commit_seen", json!({ "commit": "k1", "files": ["tests/test_c1.py"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "test_name": "t1" }));
    let log = ap(&log, "commit_seen", json!({ "commit": "k2", "files": ["src/login.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": ["c1"] }));
    assert_eq!(owed_ids(&log), ["k1", "k2"]);
    // the own test's result at k1 alone: k1 is still owed, the touching test has not run there
    let log = result(&log, "tests/test_c1.py", "k1", "failed");
    assert_eq!(owed_ids(&log), ["k1", "k2"], "one file's result does not serve the others");
    // a touching test that did not run is no result
    let log = result(&log, "tests/test_login_old.py", "k1", "did_not_run");
    assert_eq!(owed_ids(&log), ["k1", "k2"]);
    let log = result(&log, "tests/test_login_old.py", "k1", "passed");
    assert_eq!(owed_ids(&log), ["k2"], "every test ran at k1");
    // a partial result at the newest does not drop it either
    let log = result(&log, "tests/test_login_old.py", "k2", "passed");
    assert_eq!(owed_ids(&log), ["k2"]);
    let log = result(&log, "tests/test_c1.py", "k2", "passed");
    assert!(owed_ids(&log).is_empty());
}
