//! A main log and its seed, as setup writes one into a fresh checkout's record (the owner,
//! 2026-10-15; the prototype's `seeded()`, plan §6.1): the plan's kinds in order, the walk,
//! verify's pipeline facts at the part's tests-first commit and head, and with the twins the
//! same runs once more as this checkout's laptop evidence, then `record_seeded`.
#![allow(dead_code)]

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::criterion_fixture;
use serde_json::{json, Value};

pub const AT: f64 = 0.0;
pub const TENANT: &str = "_default_";
pub const TEST: &str = "tests/test_c1.py";
pub const TOUCHING: &str = "tests/test_login_old.py";
pub const CODE: &str = "src/login.py";
/// The kinds the plan file carries (`familiar_ai_plan::PLAN_KINDS`, e68d414's 18).
pub const PLAN_KINDS: [&str; 18] = [
    "intent_submitted",
    "behavior",
    "promise",
    "criterion",
    "place_chosen",
    "new_work_spot_chosen",
    "new_work_no_spot",
    "criterion_head",
    "criterion_strung_on_head",
    "test_stack_detected",
    "test_planned",
    "touching_test_declared",
    "test_written",
    "anchor_sha_matched",
    "fingerprint_stamped",
    "criterion_assigned",
    "test_change_announced",
    "fix_location_category",
];

pub fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("{kind}: fields are an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// A commit seen with c1's observation beside it.
pub fn commit(log: &Log, id: &str, files: &[&str], tests: bool, code: bool, overlap: &[&str], hash: &str) -> Log {
    let log = ap(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    ap(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": hash }))
}

pub fn result(log: &Log, file: &str, id: &str, outcome: &str, place: &str, teeth: bool) -> Log {
    ap(log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": file, "commit": id, "outcome": outcome, "ran_in": place, "with_anchor_reverted": teeth }))
}

pub fn stamp(log: &Log, id: &str, proof: &str, place: &str) -> Log {
    ap(log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": id, "proof_not_tampered": proof, "ran_in": place }))
}

/// The laptop's run at `id` as main's runner writes it: the own test, the touching test passed, the stamp.
pub fn laptop_run(log: &Log, id: &str, outcome: &str, proof: &str) -> Log {
    let log = result(log, TEST, id, outcome, "laptop", false);
    let log = result(&log, TOUCHING, id, "passed", "laptop", false);
    stamp(&log, id, proof, "laptop")
}

/// The sample world's c1, planned and confirmed.
pub fn planned() -> Log {
    criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture")
}

/// main: tests-first k1 red (sealed), a failed fix k2, green at k3.
pub fn main_green() -> Log {
    let log = commit(&planned(), "k1", &[TEST], true, false, &[], "S1");
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "t1" }));
    let log = laptop_run(&log, "k1", "failed", "S1");
    let log = commit(&log, "k2", &[CODE], false, true, &["c1"], "S1");
    let log = laptop_run(&log, "k2", "failed", "S1");
    let log = commit(&log, "k3", &[CODE], false, true, &["c1"], "S1");
    laptop_run(&log, "k3", "passed", "S1")
}

/// main: sealed at k1, one failed fix at k2, no green yet.
pub fn main_midwork() -> Log {
    let log = commit(&planned(), "k1", &[TEST], true, false, &[], "S1");
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "t1" }));
    let log = laptop_run(&log, "k1", "failed", "S1");
    let log = commit(&log, "k2", &[CODE], false, true, &["c1"], "S1");
    laptop_run(&log, "k2", "failed", "S1")
}

/// main: sealed at k1, green at k2, then a tests-only commit k3 rewrote c1's test (S1x)
/// unannounced and passed.
pub fn main_tampered() -> Log {
    let log = commit(&planned(), "k1", &[TEST], true, false, &[], "S1");
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "t1" }));
    let log = laptop_run(&log, "k1", "failed", "S1");
    let log = commit(&log, "k2", &[CODE], false, true, &["c1"], "S1");
    let log = laptop_run(&log, "k2", "passed", "S1");
    let log = commit(&log, "k3", &[TEST], true, false, &[], "S1x");
    laptop_run(&log, "k3", "passed", "S1x")
}

/// Setup's seed of `main`: the plan's kinds in order, the walk, then verify's pipeline facts at
/// the tests-first commit `t` and the part's head `h` (its test and the touching test, then the
/// teeth run). With `twins` (the seed of 2026-10-15), the same runs but the teeth once more as
/// this checkout's laptop evidence, then `record_seeded` naming the head and the plan's length;
/// without, the seed as it was before (no laptop result, no mark).
pub fn seeded(main: &Log, t: (&str, &str, &str), h: Option<(&str, &str, &str)>, twins: bool) -> Log {
    let mut log: Log = Vec::new();
    for f in main.iter().filter(|f| PLAN_KINDS.contains(&f.kind)) {
        log = append_raw(&log, f.kind, f.fields.clone(), AT, TENANT).expect("plan");
    }
    let plan_lines = log.len();
    for f in main.iter().filter(|f| f.kind == "commit_seen" || f.kind == "tests_observed") {
        log = append_raw(&log, f.kind, f.fields.clone(), AT, TENANT).expect("walk");
    }
    let (tid, tout, tproof) = t;
    let head = h.map_or(tid, |(id, _, _)| id);
    log = ap(&log, "pipeline_run_seen", json!({ "head_commit": head }));
    for place in if twins { vec!["pipeline", "laptop"] } else { vec!["pipeline"] } {
        log = result(&log, TEST, tid, tout, place, false);
        log = stamp(&log, tid, tproof, place);
        if let Some((hid, hout, hproof)) = h {
            log = result(&log, TEST, hid, hout, place, false);
            log = result(&log, TOUCHING, hid, "passed", place, false);
            log = stamp(&log, hid, hproof, place);
            if place == "pipeline" {
                log = result(&log, TEST, hid, "failed", place, true); // the teeth
            }
        }
    }
    if twins {
        log = ap(&log, "record_seeded", json!({ "commit": head, "plan_lines": plan_lines }));
    }
    log
}

/// A lane commit on c1's anchor made for another part, c1's test run there by the runner.
pub fn lane_code(log: &Log, id: &str, outcome: &str) -> Log {
    let log = commit(log, id, &[CODE], false, true, &["c1"], "S1");
    laptop_run(&log, id, outcome, "S1")
}
