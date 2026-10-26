//! The runs, read from the log: reserved, started, cancelled, ended. Nothing is
//! stored, and the reader supplies `now`.

use familiar_ai_facts::{Fact, Log};
use serde_json::Value;

/// How long a reservation may wait for its child to start before it is void;
/// the child copies the commit first.
pub const LAUNCH_GRACE: f64 = 120.0;
/// How long past its `wait` a started run may live before recovery ends it.
pub const EXPIRY_SLACK: f64 = 15.0;
/// The delay of the one automatic retry.
pub const RETRY_AFTER: f64 = 5.0;
/// The one resource every run holds, so runs go one at a time.
pub const RESOURCE: &str = "tests";

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn num(f: &Fact, k: &str) -> Option<f64> {
    f.fields.get(k).and_then(Value::as_str).and_then(|s| s.parse().ok())
}

#[derive(Debug, Clone)]
pub struct Started {
    pub pgid: i32,
    pub pgroup_start: String,
    pub at: f64,
}

#[derive(Debug, Clone)]
pub struct Ended {
    pub outcome: String,
    pub end: String,
    pub t: u64,
}

#[derive(Debug, Clone)]
pub struct Run {
    pub run_id: String,
    pub criterion_id: String,
    pub criterion_rev: String,
    pub commit: String,
    pub command: String,
    pub resources: Vec<String>,
    pub wait: f64,
    pub after: Option<f64>,
    pub reserved_at: f64,
    pub started: Option<Started>,
    pub cancel_t: Option<u64>,
    pub ended: Option<Ended>,
    /// The label of the run's `test_log_translation` (S4), when it has one.
    pub label: Option<String>,
    /// Who reserved it: `binary` for a part's run, `dry_run` for the adapter's dry run (plan A).
    pub by: String,
}

/// A dry run's `by` (plan A): part-less, it holds the `tests` resource and settles no part.
pub const DRY_RUN: &str = "dry_run";

impl Run {
    /// The adapter's dry run (plan A), not a part's run.
    pub fn dry(&self) -> bool {
        self.by == DRY_RUN
    }
}

impl Run {
    pub fn live(&self) -> bool {
        self.ended.is_none()
    }

    /// Transient (plan §6.1): its translation reads `killed` or `runner_crash`.
    pub fn transient(&self) -> bool {
        matches!(self.label.as_deref(), Some("killed" | "runner_crash"))
    }

    /// Whether the run ended with no cancel before its end, so its result counts.
    pub fn held(&self) -> bool {
        self.ended.as_ref().is_some_and(|e| self.cancel_t.is_none_or(|c| c > e.t))
    }

    pub fn cancelled(&self) -> bool {
        self.cancel_t.is_some()
    }

    fn due(&self) -> f64 {
        self.after.unwrap_or(self.reserved_at).max(self.reserved_at)
    }

    /// Live, never started, and past its launch grace.
    pub fn void(&self, now: f64) -> bool {
        self.live() && self.started.is_none() && now > self.due() + LAUNCH_GRACE
    }

    /// Live, started, and past its wait.
    pub fn expired(&self, now: f64) -> bool {
        self.live() && self.started.as_ref().is_some_and(|s| now > s.at + self.wait + EXPIRY_SLACK)
    }

    pub fn can_start(&self, now: f64) -> bool {
        self.live() && self.started.is_none() && self.cancel_t.is_none() && !self.void(now)
    }
}

/// Every run, in the order reserved.
pub fn runs(log: &Log) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for f in log {
        match f.kind {
            // CI's own runs, written into a record by `verify`, are never the laptop's (S9d)
            "run_reserved" if f.fields.get("by").and_then(Value::as_str) == Some("pipeline") => {}
            "run_reserved" => out.push(Run {
                run_id: text(f, "run_id").to_string(),
                criterion_id: text(f, "criterion_id").to_string(),
                criterion_rev: text(f, "criterion_rev").to_string(),
                commit: text(f, "commit").to_string(),
                command: text(f, "command").to_string(),
                resources: f.fields.get("resources").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                wait: num(f, "wait").unwrap_or(1800.0),
                after: num(f, "after"),
                reserved_at: f.at,
                started: None,
                cancel_t: None,
                ended: None,
                label: None,
                by: text(f, "by").to_string(),
            }),
            "run_started" => {
                if let Some(r) = out.iter_mut().find(|r| r.run_id == text(f, "run_id")) {
                    r.started.get_or_insert(Started { pgid: text(f, "pgid").parse().unwrap_or(0), pgroup_start: text(f, "pgroup_start").to_string(), at: f.at });
                }
            }
            "run_cancel_requested" => {
                if let Some(r) = out.iter_mut().find(|r| r.run_id == text(f, "run_id")) {
                    r.cancel_t.get_or_insert(f.t);
                }
            }
            "run_ended" => {
                if let Some(r) = out.iter_mut().find(|r| r.run_id == text(f, "run_id")) {
                    r.ended.get_or_insert(Ended { outcome: text(f, "outcome").to_string(), end: text(f, "end").to_string(), t: f.t });
                }
            }
            "test_log_translation" => {
                if let Some(r) = out.iter_mut().find(|r| r.run_id == text(f, "run_id")) {
                    r.label.get_or_insert(text(f, "label").to_string());
                }
            }
            _ => {}
        }
    }
    out
}

/// Whether any run is live and neither void nor expired at `now`.
pub fn any_live(log: &Log, now: f64) -> bool {
    runs(log).iter().any(|r| r.live() && !r.void(now) && !r.expired(now))
}

pub fn find(log: &Log, run_id: &str) -> Option<Run> {
    runs(log).into_iter().find(|r| r.run_id == run_id)
}

/// What the work (criterion, rev, commit) with `command` can do next.
#[derive(Debug, Clone, PartialEq)]
pub enum Eligibility {
    /// Reserve it; `attempt` counts earlier reservations, `after` is a retry's delay.
    Free { attempt: usize, after: Option<f64> },
    /// A run for it, or one holding the resource, is live.
    Live,
    /// Ran (or settled without running): write this outcome as the result,
    /// naming the run that settled it.
    Settled { outcome: &'static str, run_id: String },
    /// Cancelled, or otherwise not to be run by itself: nothing is written.
    Stalled,
}

/// Retried once (plan §2, §6.1): it did not start, recovery killed it, or it is Transient.
fn retryable(r: &Run) -> bool {
    (r.ended.as_ref().is_some_and(|e| matches!(e.end.as_str(), "not_started" | "killed")) || r.transient()) && !r.cancelled()
}

/// The `t` of the person's `try_again` that runs `commit` again for `c` (plan §6.7), when one stands.
fn answered_at(log: &Log, c: &str, commit: &str) -> Option<u64> {
    let v = familiar_ai_rules::View::new(log);
    familiar_ai_rules::moves::reruns(&v, c).into_iter().find(|(x, _)| x == commit).map(|(_, a)| a.t)
}

pub fn eligibility(log: &Log, c: &str, rev: &str, commit: &str, command: &str, now: f64) -> Eligibility {
    let all = runs(log);
    if all.iter().any(|r| r.live() && !r.void(now) && !r.expired(now)) {
        return Eligibility::Live;
    }
    // After the person's try_again on this commit, a run that ended before the answer is not
    // read for it: the commit runs again (plan §6.7).
    let answered = answered_at(log, c, commit);
    let current = |r: &Run| answered.is_none_or(|a| r.ended.as_ref().is_none_or(|e| e.t > a));
    // One run per command and commit: another criterion's held pass or fail serves; a Transient
    // run never does (plan §1.1).
    // A run that ran has a translation (S4); its family is read from it by the rules (S6).
    // A dry run (plan A) ran probes, never the part's tests: it serves no part.
    if let Some(r) = all.iter().rev().find(|r| !r.dry() && r.command == command && r.commit == commit && r.held() && (r.label.is_some() || r.ended.as_ref().is_some_and(|e| e.outcome != "did_not_run")) && !r.transient() && current(r)) {
        let outcome = if r.ended.as_ref().is_some_and(|e| e.outcome == "passed") { "passed" } else { "failed" };
        return Eligibility::Settled { outcome, run_id: r.run_id.clone() };
    }
    // The work is the command's too: the part's own run and its touching run at one commit are
    // two runs, never read for each other (the owner's plan of 2026-10-21, step 0).
    let mine: Vec<&Run> = all.iter().filter(|r| r.criterion_id == c && r.criterion_rev == rev && r.commit == commit && r.command == command).collect();
    // The runs read for it; `attempt` counts them all, so a re-run's id is new.
    let since: Vec<&Run> = mine.iter().copied().filter(|r| current(r)).collect();
    let Some(last) = since.last() else {
        return Eligibility::Free { attempt: mine.len(), after: None };
    };
    if last.live() {
        return Eligibility::Live; // void or expired: recovery ends it first
    }
    if last.cancelled() {
        return Eligibility::Stalled;
    }
    if retryable(last) && since.len() == 1 {
        return Eligibility::Free { attempt: mine.len(), after: Some(now + RETRY_AFTER) };
    }
    // Settled on the last run: `failed` when it ran and did not pass (it has a translation, a
    // Transient one included), `did_not_run` only for a run that never ran (plan §6.1, the
    // outcome word).
    let outcome = if last.label.is_some() { "failed" } else { "did_not_run" };
    Eligibility::Settled { outcome, run_id: last.run_id.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::{append_raw, Log};
    use serde_json::json;

    fn add(log: &Log, kind: &str, at: f64, f: Value) -> Log {
        append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), at, "t").expect("append")
    }

    fn reserved(log: &Log, id: &str, at: f64, after: Option<&str>) -> Log {
        let mut f = json!({"run_id": id, "criterion_id": "c1", "criterion_rev": "r", "commit": "K", "command": "x", "resources": ["tests"], "wait": "100", "by": "binary"});
        if let Some(a) = after {
            f["after"] = json!(a);
        }
        add(log, "run_reserved", at, f)
    }

    fn ended(log: &Log, id: &str, at: f64, outcome: &str, end: &str) -> Log {
        add(log, "run_ended", at, json!({"run_id": id, "outcome": outcome, "end": end}))
    }

    fn free(log: &Log, now: f64) -> Eligibility {
        eligibility(log, "c1", "r", "K", "x", now)
    }

    #[test]
    fn a_run_that_could_not_start_is_retried_once_with_a_delay_and_then_settles() {
        let log = reserved(&Vec::new(), "a", 10.0, None);
        assert_eq!(free(&log, 11.0), Eligibility::Live);
        let log = ended(&log, "a", 12.0, "did_not_run", "not_started");
        assert_eq!(free(&log, 13.0), Eligibility::Free { attempt: 1, after: Some(18.0) });
        let log = reserved(&log, "b", 13.0, Some("18"));
        let log = ended(&log, "b", 20.0, "did_not_run", "not_started");
        assert_eq!(free(&log, 21.0), Eligibility::Settled { outcome: "did_not_run", run_id: "b".into() }, "one retry only, settled by the retry");
    }

    #[test]
    fn exit_127_timeout_and_cancel_are_not_retried() {
        for end in ["exited", "timed_out", "empty_command", "no_directory"] {
            let log = ended(&reserved(&Vec::new(), "a", 1.0, None), "a", 2.0, "did_not_run", end);
            assert_eq!(free(&log, 3.0), Eligibility::Settled { outcome: "did_not_run", run_id: "a".into() }, "{end}");
        }
        let log = reserved(&Vec::new(), "a", 1.0, None);
        let log = add(&log, "run_cancel_requested", 2.0, json!({"run_id": "a"}));
        let log = ended(&log, "a", 3.0, "did_not_run", "cancelled");
        assert_eq!(free(&log, 4.0), Eligibility::Stalled);
    }

    #[test]
    fn a_result_after_a_cancel_is_diagnostic_and_a_void_or_expired_run_is_found() {
        let log = reserved(&Vec::new(), "a", 1.0, None);
        let log = add(&log, "run_cancel_requested", 2.0, json!({"run_id": "a"}));
        let log = ended(&log, "a", 3.0, "passed", "exited");
        let r = find(&log, "a").expect("run");
        assert!(r.cancelled() && !r.held(), "no authority: its pass is not a result");
        assert_eq!(free(&log, 4.0), Eligibility::Stalled);
        let log = reserved(&Vec::new(), "v", 1.0, None);
        assert!(find(&log, "v").expect("run").void(1.0 + LAUNCH_GRACE + 1.0));
        let log = add(&log, "run_started", 5.0, json!({"run_id": "v", "pgid": "9", "pgroup_start": "1"}));
        let r = find(&log, "v").expect("run");
        assert!(!r.void(1000.0) && r.expired(5.0 + 100.0 + EXPIRY_SLACK + 1.0) && !r.expired(50.0));
    }

    fn translated(log: &Log, id: &str, at: f64, label: &str) -> Log {
        add(log, "test_log_translation", at, json!({"run_id": id, "criterion_id": "c1", "commit": "K", "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"}))
    }

    #[test]
    fn a_run_that_ran_and_did_not_pass_settles_failed_and_its_family_is_read_from_the_translation() {
        // plan §6.1, the outcome word (S6): `did_not_run` only for a run that never ran
        for (end, label) in [("exited", "environment_error"), ("timed_out", "timed_out"), ("exited", "unclassified"), ("signalled", "crashed")] {
            let log = ended(&reserved(&Vec::new(), "a", 1.0, None), "a", 2.0, "did_not_run", end);
            let log = translated(&log, "a", 2.0, label);
            assert_eq!(free(&log, 3.0), Eligibility::Settled { outcome: "failed", run_id: "a".into() }, "{end} {label}");
        }
        let log = ended(&reserved(&Vec::new(), "a", 1.0, None), "a", 2.0, "did_not_run", "not_started");
        let log = reserved(&log, "b", 3.0, Some("8"));
        let log = ended(&log, "b", 9.0, "did_not_run", "not_started");
        assert_eq!(free(&log, 10.0), Eligibility::Settled { outcome: "did_not_run", run_id: "b".into() }, "never ran");
    }

    #[test]
    fn a_held_result_serves_the_same_command_at_the_same_commit() {
        let log = ended(&add(&reserved(&Vec::new(), "a", 1.0, None), "run_started", 2.0, json!({"run_id": "a", "pgid": "9", "pgroup_start": "1"})), "a", 3.0, "failed", "exited");
        assert_eq!(eligibility(&log, "c2", "other", "K", "x", 4.0), Eligibility::Settled { outcome: "failed", run_id: "a".into() }, "the reused run's own id");
        assert_eq!(eligibility(&log, "c2", "other", "K2", "x", 4.0), Eligibility::Free { attempt: 0, after: None });
    }

    /// The part's own run and its touching run at one commit are two works (the owner's plan of
    /// 2026-10-21, step 0): one held never settles the other, and their retries count apart.
    #[test]
    fn the_own_run_and_the_touching_run_are_never_read_for_each_other() {
        let log = ended(&add(&reserved(&Vec::new(), "a", 1.0, None), "run_started", 2.0, json!({"run_id": "a", "pgid": "9", "pgroup_start": "1"})), "a", 3.0, "passed", "exited");
        assert_eq!(free(&log, 4.0), Eligibility::Settled { outcome: "passed", run_id: "a".into() });
        assert_eq!(eligibility(&log, "c1", "r", "K", "x touching", 4.0), Eligibility::Free { attempt: 0, after: None }, "the touching run is its own");
        let log = add(&log, "run_reserved", 5.0, json!({"run_id": "t", "criterion_id": "c1", "criterion_rev": "r", "commit": "K", "command": "x touching", "resources": ["tests"], "wait": "100", "by": "binary"}));
        let log = ended(&log, "t", 6.0, "did_not_run", "not_started");
        assert_eq!(eligibility(&log, "c1", "r", "K", "x touching", 7.0), Eligibility::Free { attempt: 1, after: Some(12.0) }, "its one retry");
        assert_eq!(free(&log, 7.0), Eligibility::Settled { outcome: "passed", run_id: "a".into() }, "the own run's reading stands");
    }

    /// `verify` run on a laptop writes CI's runs into the record (S9d): they are CI's own, never a
    /// laptop run that settles the work or holds the resource.
    #[test]
    fn a_pipeline_run_is_never_the_laptops() {
        let log = add(&Vec::new(), "run_reserved", 1.0, json!({"run_id": "p", "criterion_id": "c1", "criterion_rev": "r", "commit": "K", "command": "x", "resources": [], "wait": "0", "by": "pipeline"}));
        assert!(runs(&log).is_empty(), "no laptop run");
        assert_eq!(free(&log, 2.0), Eligibility::Free { attempt: 0, after: None }, "a reserved pipeline run holds nothing");
        let log = ended(&log, "p", 2.0, "passed", "exited");
        assert_eq!(free(&log, 3.0), Eligibility::Free { attempt: 0, after: None }, "its pass settles nothing on the laptop");
    }
}
