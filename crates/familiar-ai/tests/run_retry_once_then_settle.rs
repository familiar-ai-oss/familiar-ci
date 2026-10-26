//! Retry once, then settle (plan §2, §6.1's Transient row; lane S5). A run is retried once when it
//! did not start, or when its translation reads it Transient (`killed`, `runner_crash`); the
//! second time it settles, naming the second run: `did_not_run` only for a run that never started,
//! `failed` for one that ran and did not pass. A `crashed` run is never retried.

use familiar_ai::door::runs::{eligibility, Eligibility, RETRY_AFTER};
use familiar_ai_facts::{append, FactFields, Log, RunEnded, RunReserved, RunStarted, TestLogTranslation};

const TENANT: &str = "_default_";
const COMMAND: &str = "pytest -q";

fn add<T: FactFields>(log: &Log, fact: &T, at: f64) -> Log {
    append(log, fact, at, TENANT).unwrap_or_else(|e| panic!("append {}: {e}", T::KIND))
}

/// c1's run `id` at commit K, reserved at `at`, started, and ended with `end`.
fn ran(log: &Log, id: &str, at: f64, end: RunEnded) -> Log {
    let reserved = RunReserved {
        run_id: id.into(),
        criterion_id: "c1".into(),
        criterion_rev: "r".into(),
        commit: "K".into(),
        command: COMMAND.into(),
        resources: vec!["tests".into()],
        wait: "100".into(),
        by: "binary".into(),
        after: None,
    };
    let log = add(log, &reserved, at);
    let log = add(&log, &RunStarted { run_id: id.into(), pgid: "9".into(), pgroup_start: "1".into() }, at + 1.0);
    add(&log, &RunEnded { run_id: id.into(), ..end }, at + 2.0)
}

/// The run's translation, beside its `run_ended` (S4).
fn translated(log: &Log, id: &str, at: f64, label: &str) -> Log {
    let t = TestLogTranslation {
        run_id: id.into(),
        criterion_id: "c1".into(),
        commit: "K".into(),
        by: "regex".into(),
        label: label.into(),
        evidence: String::new(),
        failing_tests: Vec::new(),
        classified_on: "full".into(),
        location: None,
        missing_name: None,
        counts: None,
        ran_tests: None,
        skipped_tests: None,
        versions: None,
        frames: None,
    };
    add(log, &t, at)
}

fn next(log: &Log, now: f64) -> Eligibility {
    eligibility(log, "c1", "r", "K", COMMAND, now)
}

/// Ended by signal `n` from outside: `signalled` since S1, with its (empty) output as evidence.
fn signalled(n: i64) -> RunEnded {
    RunEnded { outcome: "did_not_run".into(), end: "signalled".into(), signal: Some(n), stdout_tail: Some(String::new()), stderr_tail: Some(String::new()), ..Default::default() }
}

/// The runner itself crashed (pytest's INTERNALERROR, exit 3): the run's word is `failed`.
fn runner_crashed() -> RunEnded {
    RunEnded { outcome: "failed".into(), end: "exited".into(), exit_code: Some(3), stdout_tail: Some("INTERNALERROR> boom\n".into()), stderr_tail: Some(String::new()), ..Default::default() }
}

fn not_started() -> RunEnded {
    RunEnded { outcome: "did_not_run".into(), end: "not_started".into(), ..Default::default() }
}

#[test]
fn a_run_killed_by_a_signal_twice_is_retried_once_then_settles_failed_naming_the_second_run() {
    let log = translated(&ran(&Vec::new(), "a", 10.0, signalled(9)), "a", 12.0, "killed");
    assert_eq!(next(&log, 13.0), Eligibility::Free { attempt: 1, after: Some(13.0 + RETRY_AFTER) }, "killed (§2: 9): retried once, after its delay");
    let log = translated(&ran(&log, "b", 20.0, signalled(9)), "b", 22.0, "killed");
    assert_eq!(next(&log, 23.0), Eligibility::Settled { outcome: "failed", run_id: "b".into() }, "the second time it settles on the second run: it ran and did not pass");
}

#[test]
fn a_runner_that_crashed_twice_is_retried_once_then_settles_failed_naming_the_second_run() {
    let log = translated(&ran(&Vec::new(), "a", 10.0, runner_crashed()), "a", 12.0, "runner_crash");
    assert_eq!(next(&log, 13.0), Eligibility::Free { attempt: 1, after: Some(13.0 + RETRY_AFTER) }, "runner_crash: retried once, its failed run not reused");
    let log = translated(&ran(&log, "b", 20.0, runner_crashed()), "b", 22.0, "runner_crash");
    assert_eq!(next(&log, 23.0), Eligibility::Settled { outcome: "failed", run_id: "b".into() }, "the second time it settles on the second run");
}

#[test]
fn a_run_that_did_not_start_twice_is_retried_once_then_settles_did_not_run_naming_the_second_run() {
    let log = add(&Vec::new(), &RunReserved { run_id: "a".into(), criterion_id: "c1".into(), criterion_rev: "r".into(), commit: "K".into(), command: COMMAND.into(), resources: vec!["tests".into()], wait: "100".into(), by: "binary".into(), after: None }, 10.0);
    let log = add(&log, &RunEnded { run_id: "a".into(), ..not_started() }, 11.0);
    assert_eq!(next(&log, 12.0), Eligibility::Free { attempt: 1, after: Some(12.0 + RETRY_AFTER) });
    let log = add(&log, &RunReserved { run_id: "b".into(), criterion_id: "c1".into(), criterion_rev: "r".into(), commit: "K".into(), command: COMMAND.into(), resources: vec!["tests".into()], wait: "100".into(), by: "binary".into(), after: Some("17".into()) }, 12.0);
    let log = add(&log, &RunEnded { run_id: "b".into(), ..not_started() }, 18.0);
    assert_eq!(next(&log, 19.0), Eligibility::Settled { outcome: "did_not_run", run_id: "b".into() }, "it never started: did_not_run, on the second run");
}

#[test]
fn a_crashed_run_is_never_retried() {
    for (n, label) in [(11, "crashed"), (6, "crashed")] {
        let log = translated(&ran(&Vec::new(), "a", 10.0, signalled(n)), "a", 12.0, label);
        let got = next(&log, 13.0);
        assert!(matches!(&got, Eligibility::Settled { run_id, .. } if run_id == "a"), "signal {n} reads crashed: settled at once on its run, got {got:?}");
    }
}
