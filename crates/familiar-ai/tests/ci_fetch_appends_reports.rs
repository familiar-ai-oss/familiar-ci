//! `familiar-ai ci fetch` appends one `ci_report_seen` per finished
//! `familiar-verify` run the record has not seen, oldest first, read from the
//! run's uploaded report; a fake `gh` on PATH stands in for GitHub.

mod common;

use common::Project;
use familiar_ai_facts::{Fact, Record};
use serde_json::{json, Value};

/// A project run from no session, with a fake `gh` first on the PATH: it logs
/// its argv, answers `run list` from runs.json and `run download <id>` from
/// report-<id>.json.
fn project(name: &str) -> Project {
    let b = Project::builder(name).no_session();
    let fake = format!(
        r#"#!/bin/sh
d='{d}'
printf '%s\n' "$*" >> "$d/calls"
if [ -f "$d/fail" ]; then cat "$d/fail" >&2; exit 1; fi
case "$1 $2" in
  "run list") cat "$d/runs.json" ;;
  "run download")
    id="$3"; out=""
    while [ $# -gt 0 ]; do [ "$1" = "-D" ] && out="$2"; shift; done
    if [ -f "$d/report-$id.json" ]; then mkdir -p "$out"; cp "$d/report-$id.json" "$out/verify-report.json"; else echo "no artifact matches" >&2; exit 1; fi ;;
  *) echo "unexpected gh $*" >&2; exit 1 ;;
esac
"#,
        d = b.dir().display()
    );
    b.bin("gh", &fake).create()
}

impl Project {
    fn runs(&self, runs: Value) {
        std::fs::write(self.dir.join("runs.json"), runs.to_string()).expect("runs");
    }

    fn report(&self, id: u64, report: Value) {
        std::fs::write(self.dir.join(format!("report-{id}.json")), report.to_string()).expect("report");
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }

    fn seen(&self) -> Vec<Fact> {
        let r = Record::open(&self.record_path()).expect("record");
        r.read_by_kind("ci_report_seen").expect("read")
    }

    fn record_len(&self) -> u64 {
        Record::open(&self.record_path()).expect("record").len().expect("len")
    }
}

fn text(f: &Fact, k: &str) -> String {
    f.fields.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn lines(f: &Fact) -> Vec<String> {
    f.fields.get("report").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

fn run(id: u64, status: &str, conclusion: &str, updated: &str) -> Value {
    json!({ "databaseId": id, "status": status, "conclusion": conclusion, "updatedAt": updated })
}

#[test]
fn a_green_run_is_appended_proven_with_its_reports_lines() {
    let p = project("proven");
    let head = p.head();
    let label = format!("hosted CI-proven at {}, run 101", &head[..7]);
    p.runs(json!([run(101, "completed", "success", "2026-10-10T12:00:00Z"), run(105, "in_progress", "", "2026-10-10T12:05:00Z")]));
    p.report(101, json!({ "run": { "id": "101", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "greet says hello", "outcome": "proven", "evidence": label }], "exit": 0 }));
    let (code, out, err) = p.run(&["ci", "fetch"], "");
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert_eq!(out, format!("familiar-ai: CI: proven at {}, run 101\n", &head[..7]));
    let calls = p.calls();
    assert!(calls.contains(&format!("run list --commit {head} --workflow familiar-verify --json databaseId,status,conclusion,updatedAt")), "{calls}");
    assert!(calls.contains("run download 101 -n familiar-verify-report -D "), "{calls}");
    assert!(!calls.contains("run download 105"), "a run still going is not read: {calls}");
    let seen = p.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(text(&seen[0], "sha"), head);
    assert_eq!(text(&seen[0], "run_id"), "101");
    assert_eq!(text(&seen[0], "finished_at"), "2026-10-10T12:00:00Z");
    assert_eq!(text(&seen[0], "outcome"), "proven");
    assert_eq!(lines(&seen[0]), [label]);
}

#[test]
fn a_red_run_is_not_proven_and_the_runs_are_appended_oldest_first() {
    let p = project("not-proven");
    let head = p.head();
    let said = "\"greet says hello\" · not proven: its tests fail at the head · next: fix the code";
    p.runs(json!([run(104, "completed", "failure", "2026-10-10T13:00:00Z"), run(103, "completed", "cancelled", "2026-10-10T11:00:00Z")]));
    p.report(104, json!({ "run": { "id": "104", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "greet says hello", "outcome": "not_proven", "evidence": said }], "exit": 1 }));
    let (code, out, _) = p.run(&["ci", "fetch", "--sha", &head], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, format!("familiar-ai: CI: not proven at {}, run 104: {said}\n", &head[..7]));
    let seen = p.seen();
    assert_eq!(seen.iter().map(|f| text(f, "run_id")).collect::<Vec<_>>(), ["103", "104"], "oldest first");
    assert_eq!(text(&seen[0], "outcome"), "cancelled");
    assert_eq!(text(&seen[1], "outcome"), "not_proven");
    assert_eq!(lines(&seen[1]), [said]);
    assert!(!p.calls().contains("run download 103"), "a cancelled run's report is not read");
}

#[test]
fn a_cancelled_run_is_appended_cancelled_with_no_report() {
    let p = project("cancelled");
    p.runs(json!([run(102, "completed", "cancelled", "2026-10-10T12:00:00Z")]));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, "familiar-ai: CI: cancelled (run 102)\n");
    let seen = p.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(text(&seen[0], "outcome"), "cancelled");
    assert!(lines(&seen[0]).is_empty());
    assert!(!p.calls().contains("run download"), "{}", p.calls());
}

#[test]
fn a_timed_out_run_is_appended_timed_out_with_no_report() {
    let p = project("timed-out");
    p.runs(json!([run(106, "completed", "timed_out", "2026-10-10T12:00:00Z")]));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, "familiar-ai: CI: timed out (run 106)\n");
    let seen = p.seen();
    assert_eq!(text(&seen[0], "outcome"), "timed_out");
    assert!(lines(&seen[0]).is_empty());
}

#[test]
fn a_run_with_no_report_to_download_is_read_from_its_conclusion() {
    let p = project("no-report");
    p.runs(json!([run(107, "completed", "success", "2026-10-10T12:00:00Z"), run(108, "completed", "failure", "2026-10-10T12:01:00Z")]));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    let seen = p.seen();
    assert_eq!(seen.iter().map(|f| (text(f, "run_id"), text(f, "outcome"))).collect::<Vec<_>>(), [("107".into(), "proven".into()), ("108".into(), "not_proven".to_string())]);
    assert!(seen.iter().all(|f| lines(f).is_empty()));
    assert_eq!(out, format!("familiar-ai: CI: not proven at {}, run 108\n", &p.head()[..7]));
}

#[test]
fn no_run_writes_nothing_and_says_so() {
    let p = project("no-run");
    p.runs(json!([]));
    let before = p.record_len();
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, format!("familiar-ai: CI: no run for {}\n", &p.head()[..7]));
    assert_eq!(p.record_len(), before, "nothing written");
}

#[test]
fn a_second_fetch_appends_nothing_and_says_the_same() {
    let p = project("again");
    let head = p.head();
    p.runs(json!([run(101, "completed", "success", "2026-10-10T12:00:00Z")]));
    p.report(101, json!({ "run": { "id": "101", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "t", "outcome": "proven", "evidence": "hosted CI-proven" }], "exit": 0 }));
    let first = p.run(&["ci", "fetch"], "");
    let len = p.record_len();
    let second = p.run(&["ci", "fetch"], "");
    assert_eq!(first, second);
    assert_eq!(p.record_len(), len, "nothing appended");
    assert_eq!(p.seen().len(), 1);
}

#[test]
fn gh_failing_or_missing_is_one_line_exit_1_and_nothing_written() {
    let p = project("gh-fails");
    std::fs::write(p.dir.join("fail"), "HTTP 401: Bad credentials\n").expect("fail");
    let before = p.record_len();
    let (code, out, err) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 1, "{out}{err}");
    let said = format!("{out}{err}");
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(said.starts_with("familiar-ai: CI: could not ask GitHub ("), "{said}");
    assert!(said.contains("HTTP 401: Bad credentials"), "{said}");
    assert_eq!(p.record_len(), before);
    // no gh on PATH at all
    std::fs::remove_file(p.dir.join("bin/gh")).expect("rm");
    p.set_env("PATH", Some(&p.dir.join("bin").to_string_lossy()));
    let (code, out, err) = p.run(&["ci", "fetch"], "");
    let said = format!("{out}{err}");
    assert_eq!(code, 1, "{said}");
    assert!(said.starts_with("familiar-ai: CI: could not ask GitHub (") && said.lines().count() == 1, "{said}");
    assert_eq!(p.record_len(), before);
}

#[test]
fn a_forged_report_under_a_failed_run_never_reads_as_hosted_ci_proven() {
    let p = project("forged");
    let head = p.head();
    let label = format!("hosted CI-proven at {}, run 201", &head[..7]);
    // the run failed; the report it uploaded (a committed one) says exit 0
    p.runs(json!([run(201, "completed", "failure", "2026-10-10T12:00:00Z")]));
    p.report(201, json!({ "run": { "id": "201", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "greet says hello", "outcome": "proven", "evidence": label }], "exit": 0 }));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("hosted CI-proven"), "{out}");
    let seen = p.seen();
    assert_eq!(text(&seen[0], "outcome"), "not_proven", "the run's conclusion, never the report's exit");
    assert!(lines(&seen[0]).is_empty(), "a report that disagrees with its run is dropped: {:?}", lines(&seen[0]));
}

#[test]
fn a_report_of_another_sha_or_another_run_is_dropped() {
    let p = project("mismatched");
    let head = p.head();
    let other = "0123456789abcdef0123456789abcdef01234567";
    p.runs(json!([run(202, "completed", "success", "2026-10-10T12:00:00Z"), run(203, "completed", "success", "2026-10-10T12:01:00Z")]));
    p.report(202, json!({ "run": { "id": "202", "at": 1.0, "runner": "r1" }, "sha": other, "plan_hash": null, "criteria": [{ "id": "c1", "text": "t", "outcome": "proven", "evidence": "hosted CI-proven at 0123456, run 202" }], "exit": 0 }));
    p.report(203, json!({ "run": { "id": "999", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "t", "outcome": "proven", "evidence": "hosted CI-proven, run 999" }], "exit": 0 }));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    let seen = p.seen();
    // verify overwrites its report before the upload: a report of another sha
    // or run under a green run means verify did not write it, so nothing is proven
    assert_eq!(seen.iter().map(|f| (text(f, "run_id"), text(f, "outcome"))).collect::<Vec<_>>(), [("202".into(), "not_proven".into()), ("203".into(), "not_proven".to_string())]);
    assert!(seen.iter().all(|f| lines(f).is_empty()), "{:?}", seen.iter().map(lines).collect::<Vec<_>>());
}

#[test]
fn a_green_run_whose_own_report_says_verify_failed_is_not_proven() {
    // the verify step ran under continue-on-error: the job concluded success,
    // the report verify wrote for this sha and this run says exit 1
    let p = project("masked");
    let head = p.head();
    let said = "\"greet says hello\" · not proven: its tests fail at the head · next: fix the code";
    p.runs(json!([run(301, "completed", "success", "2026-10-10T12:00:00Z")]));
    p.report(301, json!({ "run": { "id": "301", "at": 1.0, "runner": "r1" }, "sha": head, "plan_hash": null, "criteria": [{ "id": "c1", "text": "greet says hello", "outcome": "not_proven", "evidence": said }], "exit": 1 }));
    let (code, out, _) = p.run(&["ci", "fetch"], "");
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("CI: proven"), "{out}");
    let seen = p.seen();
    assert_eq!(text(&seen[0], "outcome"), "not_proven", "a report that disagrees with a green run turns it down");
    assert!(lines(&seen[0]).is_empty(), "{:?}", lines(&seen[0]));
}
