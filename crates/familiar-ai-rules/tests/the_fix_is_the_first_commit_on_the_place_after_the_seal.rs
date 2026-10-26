//! The journey's fix step: the first commit after the tests-first one that
//! lands code on the place, the same commits `anchor_touched` counts. A try
//! that still fails is a fix landed; green does not move it; code outside
//! the place is not one.

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::{lifecycle, View};

fn fix(log: &Log) -> Option<String> {
    let v = View::new(log);
    lifecycle::fix_commit(&v, "c1").and_then(|f| f.fields.get("commit")).and_then(|c| c.as_str()).map(str::to_string)
}

fn last_commit(log: &Log) -> Option<String> {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(|c| c.as_str()).map(str::to_string)
}

#[test]
fn sealed_with_no_code_on_the_place_there_is_no_fix() {
    assert_eq!(fix(&sealed()), None);
}

#[test]
fn code_on_the_place_is_the_fix_whether_it_passes_or_not() {
    let log = step(&sealed(), "code", None);
    let first = last_commit(&log);
    assert!(first.is_some());
    assert_eq!(fix(&log), first, "landed, not yet run");
    let failed = run(&log, "failed");
    assert_eq!(fix(&failed), first, "a try that still fails");
    let passed = run(&log, "passed");
    assert_eq!(state(&passed), "green");
    assert_eq!(fix(&passed), first, "green keeps it");
}

#[test]
fn code_outside_the_place_is_not_the_fix() {
    let log = step(&sealed(), "code_outside_anchor", None);
    assert_eq!(fix(&log), None);
}
