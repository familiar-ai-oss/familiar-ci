//! `status` builds its CI line from the record's `ci_report_seen` facts and
//! the local HEAD alone; a fake `gh` alone on PATH shows it is never called.
//! See docs/decisions.md#ci-proves-the-laptop-guides

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::{CiReportSeen, Record};
use serde_json::{json, Value};

const SESSION: &str = "4c1d2e3f-5a6b-4c7d-8e9f-0a1b2c3d4e5f";
const OTHER: &str = "0123456789abcdef0123456789abcdef01234567";

impl Project {
    /// setup commits with git: the whole PATH, the fake gh first; after it
    /// the binary runs with only the fake `gh` on PATH.
    fn ci(name: &str) -> Project {
        let b = Project::builder(name).session(SESSION);
        let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 1\n", b.dir().join("gh-called").display());
        let p = b.bin("gh", &fake).create();
        p.set_env("PATH", Some(&p.dir.join("bin").to_string_lossy()));
        p
    }

    fn seen(&self, sha: &str, run_id: &str, outcome: &str, report: &[&str]) {
        let record = Record::open(&self.record_path()).expect("record");
        let f = CiReportSeen { sha: sha.into(), run_id: run_id.into(), finished_at: "2026-10-10T12:00:00Z".into(), outcome: outcome.into(), report: report.iter().map(|s| s.to_string()).collect() };
        record.append(&f, 1_791_000_000.0, "_default_").expect("append");
    }

    /// Asserts `status`, `status --json` and the session's first line say `ci`,
    /// the status line does not, and gh is never called.
    fn says(&self, ci: &str) {
        let (code, status, err) = self.run(&["status"], "");
        assert_eq!(code, 0, "{err}");
        let lines: Vec<&str> = status.lines().collect();
        let want = format!("familiar-ai: {ci}");
        let at = lines.iter().position(|l| *l == want).unwrap_or_else(|| panic!("{want} in:\n{status}"));
        let last_criterion = lines.iter().rposition(|l| l.starts_with("familiar-ai: the plan is") || l.starts_with("familiar-ai: \"")).expect("the plan's line");
        assert_eq!(at, last_criterion + 1, "right after the criteria lines:\n{status}");

        let started = Instant::now();
        let (code, out, err) = self.run(&["status", "--json"], "");
        let wall = started.elapsed();
        assert_eq!(code, 0, "{err}");
        let j: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        assert_eq!(j["ci"], ci, "{j}");
        // a bound on reading the record, not on the machine: 3 s holds at load ~100 (300 ms did not; lane s9d-ci)
        assert!(wall < Duration::from_secs(3), "status --json took {wall:?}");

        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy() }).to_string();
        let (code, line, _) = self.run(&["statusline"], &input);
        assert_eq!(code, 0);
        assert_eq!(line.lines().count(), 1, "{line}");
        assert!(!line.contains("CI"), "the status line never says CI: {line}");

        let (code, out, err) = self.hook_raw("SessionStart", &self.payload("SessionStart", json!({ "source": "startup" })));
        assert_eq!(code, 0, "{err}");
        let v: Value = serde_json::from_str(&out).expect("one JSON object");
        let context = v["hookSpecificOutput"]["additionalContext"].as_str().expect("context");
        assert_eq!(context.lines().next(), Some(format!("familiar-ai: idle · {ci}").as_str()), "{context}");

        assert!(!self.dir.join("gh-called").exists(), "gh was called: {}", std::fs::read_to_string(self.dir.join("gh-called")).unwrap_or_default());
    }
}

#[test]
fn nothing_seen_is_no_run_for_head() {
    let p = Project::ci("no-run");
    p.says(&format!("CI: no run for {}", &p.head()[..7]));
}

#[test]
fn a_proven_run_at_head_is_proven_with_its_run() {
    let p = Project::ci("proven");
    let head = p.head();
    p.seen(&head, "4242", "proven", &[&format!("hosted CI-proven at {}, run 4242", &head[..7])]);
    p.says(&format!("CI: proven at {}, run 4242", &head[..7]));
}

#[test]
fn a_run_that_did_not_prove_it_names_its_first_line() {
    let p = Project::ci("not-proven");
    let head = p.head();
    p.seen(&head, "77", "not_proven", &["\"greet says hello\" · not proven: its tests fail at the head · next: fix it", "second"]);
    p.says(&format!("CI: not proven at {}, run 77: \"greet says hello\" · not proven: its tests fail at the head · next: fix it", &head[..7]));
}

#[test]
fn a_cancelled_run_is_cancelled() {
    let p = Project::ci("cancelled");
    let head = p.head();
    p.seen(&head, "78", "cancelled", &[]);
    p.says("CI: cancelled (run 78)");
}

#[test]
fn a_timed_out_run_is_timed_out() {
    let p = Project::ci("timed-out");
    let head = p.head();
    p.seen(&head, "79", "timed_out", &[]);
    p.says("CI: timed out (run 79)");
}

#[test]
fn a_run_only_for_another_commit_is_stale() {
    let p = Project::ci("stale");
    p.seen(OTHER, "80", "proven", &[]);
    p.says("CI: stale (ran at 0123456)");
}
