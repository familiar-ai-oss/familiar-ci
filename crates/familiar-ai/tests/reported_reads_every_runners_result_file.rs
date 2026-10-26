//! Lane A-wire (plan of record 2026-10-19, A): every runner's run becomes the per-test result file
//! through its reader (`readers::result_file`: the kit's own file, go's test2json and nextest's
//! libtest-json from stdout, node's TAP, JUnit and TRX report files), and `reported::read` reads
//! that file. Each of the A0 spike's recordings (`tests/fixtures/probe_runs/<runner>/<setup>/<probe>`)
//! is laid out as its run left it, read through the real path, and judged against its probe's
//! truth: right or can't tell, never wrong; and as often right as the kit lanes' own oracle.

mod probe_reading;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use familiar_ai::door::kits::spec::TRUTHS;
use familiar_ai::door::readers::{result_file, Left};
use familiar_ai::door::reported::read;
use probe_reading::{probe_runs, recordings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Right,
    CantTell,
    Wrong,
}

/// The plan's rule: the truth is right; `unclassified` or nothing is can't tell; another label, or
/// a failed test of another kind than the truth's, is wrong.
fn verdict(truth: (&str, Option<&str>), label: Option<&str>, kind: Option<&str>) -> Verdict {
    match label {
        None | Some("unclassified") => Verdict::CantTell,
        Some(l) if l != truth.0 => Verdict::Wrong,
        Some(_) => match (truth.1, kind) {
            (Some(want), Some(got)) if want != got => Verdict::Wrong,
            _ => Verdict::Right,
        },
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("awire-reported-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn copy_tree(from: &Path, to: &Path) {
    if from.is_file() {
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(from, to).unwrap();
        return;
    }
    for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
        copy_tree(&e.path(), &to.join(e.file_name()));
    }
}

/// The reader a recording was made under, and where its report files lie in a run: (reader id,
/// [(the recording's report entry, its place in the run's folder or `{REPORT_DIR}`)]).
fn reader_of(runner: &str, setup: &str) -> Option<(&'static str, &'static [(&'static str, &'static str)])> {
    Some(match runner {
        "jest" => ("jest:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "vitest" => ("vitest:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "mocha" => ("mocha:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "node-test" if setup == "person_reporter_flags" => ("node:reporter-flags", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "node-test" => ("node:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "unittest" => ("unittest:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "go-test" => ("go-test:reporter", &[]),
        "nextest" => ("cargo-nextest:report", &[]),
        "phpunit" if setup.ends_with("junit") => ("phpunit:report", &[("junit.xml", "{REPORT_DIR}/junit.xml")]),
        "phpunit" => ("phpunit:reporter", &[("results.jsonl", "{REPORT_DIR}/results.jsonl")]),
        "maven" => ("maven:report", &[("surefire-reports", "target/surefire-reports")]),
        "gradle" => ("gradle:report", &[("test-results", "build/test-results/test")]),
        "dotnet-xunit" => ("dotnet:trx", &[("results.trx", "{REPORT_DIR}/results.trx")]),
        _ => return None,
    })
}

struct Read {
    label: Option<String>,
    kind: Option<String>,
    oracle: probe_reading::Verdict,
    verdict: Verdict,
    file: String,
}

fn read_recording(dir: &Path, runner: &str, setup: &str, probe: &str, started: SystemTime) -> Option<Read> {
    let (id, places) = reader_of(runner, setup)?;
    let work = scratch(&format!("{runner}-{setup}-{probe}"));
    let (run_dir, report_dir) = (work.join("run"), work.join("report"));
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(&report_dir).unwrap();
    for (entry, place) in places {
        let from = dir.join("report").join(entry);
        if !from.exists() {
            continue;
        }
        let to = match place.strip_prefix("{REPORT_DIR}/") {
            Some(rest) => report_dir.join(rest),
            None => run_dir.join(place),
        };
        copy_tree(&from, &to);
    }
    let stdout = std::fs::read_to_string(dir.join("stdout")).unwrap_or_default();
    let exit: Option<i32> = std::fs::read_to_string(dir.join("exit")).ok().and_then(|e| e.trim().parse().ok());
    let left = Left { stdout: Some(&stdout), run_dir: &run_dir, report_dir: &report_dir, started, exit, before: &[] };
    let file = result_file(id, &left).unwrap_or_default();
    let truth = TRUTHS.iter().find(|(p, _)| *p == probe).map(|(_, t)| *t).expect("a probe's truth");
    let reported = read(&file, exit, &[]);
    let label = reported.as_ref().map(|r| r.translation.label.clone());
    let kind = reported.as_ref().and_then(|r| r.translation.location.as_ref()).and_then(|l| l.kind).map(|k| format!("{k:?}").to_lowercase());
    let v = verdict((truth.label, truth.kind), label.as_deref(), kind.as_deref());
    let oracle = probe_reading::verdict(&probe_reading::read(&file, exit), (truth.label, truth.kind.unwrap_or("")));
    let _ = std::fs::remove_dir_all(&work);
    Some(Read { label, kind, oracle, verdict: v, file })
}

fn every_recording() -> Vec<(String, String, String, PathBuf)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(probe_runs()).unwrap().flatten() {
        let runner = e.file_name().to_string_lossy().into_owned();
        for r in recordings(&runner) {
            out.push((runner.clone(), r.setup.clone(), r.probe.clone(), r.dir.clone()));
        }
    }
    out.sort();
    out
}

#[test]
fn every_recording_reads_its_truth_or_cant_tell_through_its_reader_and_never_wrong() {
    let (mut right, mut cant, mut seen) = (0, 0, 0);
    let mut wrong: Vec<String> = Vec::new();
    let mut weaker: Vec<String> = Vec::new();
    for (runner, setup, probe, dir) in every_recording() {
        if setup == "stale-report" {
            continue; // its own test below
        }
        let Some(r) = read_recording(&dir, &runner, &setup, &probe, SystemTime::UNIX_EPOCH) else { continue };
        seen += 1;
        match r.verdict {
            Verdict::Right => right += 1,
            Verdict::CantTell => cant += 1,
            Verdict::Wrong => wrong.push(format!("{runner}/{setup}/{probe}: read {:?} {:?}\n{}", r.label, r.kind, r.file)),
        }
        // the oracle the kit lanes calibrated with: the real reader is never weaker than it
        if r.oracle == probe_reading::Verdict::Right && r.verdict != Verdict::Right {
            weaker.push(format!("{runner}/{setup}/{probe}: the oracle reads it right, reported::read {:?} {:?}\n{}", r.label, r.kind, r.file));
        }
    }
    assert!(wrong.is_empty(), "read wrong:\n{}", wrong.join("\n"));
    assert!(weaker.is_empty(), "weaker than the oracle:\n{}", weaker.join("\n"));
    assert!(seen >= 300, "{seen} recordings read");
    assert!(right >= 300 && right > 2 * cant, "{right} right, {cant} can't tell");
}

#[test]
fn a_report_older_than_the_run_is_not_read() {
    // the spike's stale-report recording: a compile error left the last run's surefire XML
    let dir = probe_runs().join("maven/stale-report/P3");
    let later = SystemTime::now() + Duration::from_secs(3600);
    let r = read_recording(&dir, "maven", "stale-report", "P3", later).expect("a maven reader");
    assert_eq!(r.label, None, "a report from before the run is no structured output: {}", r.file);
    assert_ne!(r.verdict, Verdict::Wrong);
}

#[test]
fn a_cut_stdout_is_no_structured_output() {
    let dir = probe_runs().join("go-test/go125/P2");
    let work = scratch("cut");
    let left = Left { stdout: None, run_dir: &work, report_dir: &work, started: SystemTime::UNIX_EPOCH, exit: Some(1), before: &[] };
    assert_eq!(result_file("go-test:reporter", &left), None);
    let stdout = std::fs::read_to_string(dir.join("stdout")).unwrap();
    let ended_by_signal = Left { stdout: Some(&stdout), exit: None, ..left };
    assert_eq!(result_file("go-test:reporter", &ended_by_signal), None, "a signal or a timeout: the exit code decides");
    assert_eq!(result_file("cargo-test:tables", &Left { stdout: Some(&stdout), ..left }), None, "the tables write no file");
    let _ = std::fs::remove_dir_all(&work);
}
