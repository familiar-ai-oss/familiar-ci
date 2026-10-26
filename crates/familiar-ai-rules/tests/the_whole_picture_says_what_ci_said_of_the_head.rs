//! The whole picture says what CI said of the head, read from the record
//! (`ci_report_seen`, written by `familiar-ai ci fetch`) and not the network.
//! See docs/decisions.md#ci-proves-the-laptop-guides

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::terminal::{ci_line, status};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

/// A report: sha, run id, outcome, its lines.
type Report<'a> = (&'a str, &'a str, &'a str, &'a [&'a str]);

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// c1 alone; with `reports`, the head k1 (its tests) and the reports.
fn with(reports: Option<&[Report]>) -> Log {
    let mut log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    if let Some(reps) = reports {
        log = ap(&log, "commit_seen", json!({ "commit": "k1", "files": ["tests/test_c1.py"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
        log = ap(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": "k1", "tests_hash": "S1" }));
        for (sha, id, outcome, report) in reps {
            log = ap(&log, "ci_report_seen", json!({ "sha": sha, "run_id": id, "finished_at": "2026-10-10T19:00:00Z", "outcome": outcome, "report": report }));
        }
    }
    log
}

fn line(log: &Log) -> Option<String> {
    ci_line(&View::new(log))
}

#[test]
fn no_run_names_the_head_and_before_any_commit_the_intents_sha() {
    assert_eq!(line(&with(None)).as_deref(), Some("CI: no run for base"));
    assert_eq!(line(&with(Some(&[]))).as_deref(), Some("CI: no run for k1"));
}

#[test]
fn each_outcome_of_the_heads_latest_report_in_the_pinned_words() {
    let first = "\"a right password opens a session\" · not proven: its tests fail at the head";
    let cases: [(&[Report], String); 6] = [
        (&[("k1", "101", "proven", &[])], "CI: proven at k1, run 101".into()),
        (&[("k1", "102", "not_proven", &[first])], format!("CI: not proven at k1, run 102: {first}")),
        (&[("k1", "103", "could_not", &[])], "CI: not proven at k1, run 103".into()),
        (&[("k1", "104", "cancelled", &[])], "CI: cancelled (run 104)".into()),
        (&[("k1", "105", "timed_out", &[])], "CI: timed out (run 105)".into()),
        (&[("k1", "107", "proven", &[]), ("k1", "108", "cancelled", &[])], "CI: cancelled (run 108)".into()),
    ];
    for (reps, want) in cases {
        assert_eq!(line(&with(Some(reps))), Some(want));
    }
}

#[test]
fn another_shas_report_is_stale_and_never_the_heads() {
    assert_eq!(line(&with(Some(&[("a0b1c2d3e4f5", "106", "proven", &[])]))).as_deref(), Some("CI: stale (ran at a0b1c2d)"));
    assert_eq!(line(&with(Some(&[("k1", "109", "proven", &[]), ("a0b1c2d3e4f5", "110", "not_proven", &[])]))).as_deref(), Some("CI: proven at k1, run 109"));
}

#[test]
fn the_whole_picture_ends_with_the_ci_line() {
    let log = with(Some(&[("k1", "101", "proven", &[])]));
    assert_eq!(status(&View::new(&log)).lines().last(), Some("CI: proven at k1, run 101"));
}
