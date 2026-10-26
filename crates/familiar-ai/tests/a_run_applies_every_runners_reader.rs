//! Lane A-wire (plan of record 2026-10-19, A): a laptop run applies the reader the record chose for
//! its runner, whatever the runner: the reader's flags go into Familiar's own run only (the claimed
//! command is what the record keeps), its env ops apply, and the run's per-test result file (the
//! kit's own, go's test2json from stdout, Maven's surefire XML) is read `by: "reporter"`. A report
//! the run did not write (older than the run) is no reading of it. Fakes replay the A0 spike's
//! recordings.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use familiar_ai::door::runner::pipeline_reading_by;
use familiar_ai::door::verify_run::{run_commit_read, Place};
use familiar_ai_facts::LocationKind;
use serde_json::Value;

fn recording(runner: &str, setup: &str, probe: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs").join(runner).join(setup).join(probe)
}

fn repo(name: &str, files: &[(&str, &str)]) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-every-reader-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (p, t) in files {
        let at = dir.join(p);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, t).unwrap();
        if !p.contains('.') {
            let _ = Command::new("chmod").arg("+x").arg(&at).status();
        }
    }
    let git = |args: &[&str]| {
        let _ = Command::new("git").args(args).current_dir(&dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").status();
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();
    (dir, head)
}

fn s<'a>(t: &'a serde_json::Map<String, Value>, k: &str) -> &'a str {
    t.get(k).and_then(Value::as_str).unwrap_or("")
}

/// jest that writes the P2 recording's report only when the reporter flag reached it, and says
/// on stderr which arguments it got.
const FAKE_JEST: &str = r#"#!/bin/sh
echo "args: $*" >&2
for a in "$@"; do case "$a" in --reporters=*/jest/reporter.cjs) cp "__REC__/report/results.jsonl" "$FAMILIAR_AI_REPORT" ;; esac; done
cat "__REC__/stdout"; cat "__REC__/stderr" >&2
exit 1
"#;

#[test]
fn a_jest_run_gets_the_reporter_flags_and_reads_by_the_reporter() {
    let rec = recording("jest", "plain", "P2");
    let (dir, head) = repo("jest", &[("jest", &FAKE_JEST.replace("__REC__", &rec.to_string_lossy())), ("test/p2.test.js", "'use strict';\nconst { add } = require('./calc.js');\n\ntest('adds', () => {\n  expect(add(2, 2)).toBe(5);\n});\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    let command = "./jest test/p2.test.js";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("jest:reporter"), &mut |_| true).expect("ran");
    assert!(ran.out.stderr.contains("args: test/p2.test.js --reporters=default --reporters="), "{}", ran.out.stderr);
    let reported = ran.reported.as_deref().expect("the reporter's file");
    let (_, t) = pipeline_reading_by(&dir, command, &head, &ran.out, &ran.report, Some(reported), &["test/p2.test.js".to_string()]);
    let t = t.expect("a translation");
    assert_eq!((s(&t, "by"), s(&t, "label")), ("reporter", "tests_failed"), "{t:?}");
    let loc: familiar_ai_facts::Location = serde_json::from_value(t.get("location").cloned().unwrap_or_else(|| panic!("a location: {t:?}"))).unwrap();
    assert_eq!((loc.file_path.as_str(), loc.line, loc.kind), ("test/p2.test.js", 5, Some(LocationKind::Assertion)));
    // the tables reader adds no flag and reads the console
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("jest:tables"), &mut |_| true).expect("ran");
    assert!(ran.out.stderr.contains("args: test/p2.test.js\n"), "{}", ran.out.stderr);
    assert_eq!(ran.reported, None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// go that prints the go 1.25 P2 recording's test2json when GOFLAGS or the flag asks for JSON.
const FAKE_GO: &str = r#"#!/bin/sh
json=; case " $GOFLAGS " in *" -json "*) json=1 ;; esac
for a in "$@"; do [ "$a" = -json ] && json=1; done
if [ -n "$json" ]; then cat "__JSON__/stdout"; else cat "__PLAIN__/stdout"; fi
exit 1
"#;

#[test]
fn a_go_run_reads_test2json_from_its_stdout() {
    let fake = FAKE_GO.replace("__JSON__", &recording("go-test", "go125", "P2").to_string_lossy()).replace("__PLAIN__", &recording("go-test", "noinject", "P2").to_string_lossy());
    let (dir, head) = repo("go", &[("go", &fake), ("go.mod", "module example.com/probe\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    for (command, reader) in [("./go test ./...", "go-test:reporter"), ("./go vet ./... ; ./go test ./...", "go-test:reporter-flag")] {
        let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some(reader), &mut |_| true).expect("ran");
        let reported = ran.reported.as_deref().expect("test2json converted");
        assert!(reported.contains("\"converted_from\"") || reported.contains("\"session\""), "{reported}");
        let (_, t) = pipeline_reading_by(&dir, command, &head, &ran.out, &ran.report, Some(reported), &[]);
        let t = t.expect("a translation");
        assert_eq!((s(&t, "by"), s(&t, "label")), ("reporter", "tests_failed"), "{command}: {t:?}");
    }
    // the env route refused where another go call would see GOFLAGS: no reader, the tables
    let command = "./go vet ./... ; ./go test ./...";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("go-test:reporter"), &mut |_| true).expect("ran");
    assert_eq!(ran.reported, None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// mvn that writes the P2 recording's surefire report, unless told the build failed (then it
/// leaves what is there, as a compile error does).
const FAKE_MVN: &str = r#"#!/bin/sh
case "$MAVEN_ARGS" in *-B*) ;; *) echo "no batch mode" >&2; exit 3 ;; esac
if [ -f BROKEN ]; then echo "[ERROR] COMPILATION ERROR"; exit 1; fi
mkdir -p target/surefire-reports
cp "__REC__/report/surefire-reports/TEST-calc.ProbeTest.xml" target/surefire-reports/
cat "__REC__/stdout"
exit 1
"#;

#[test]
fn a_maven_run_reads_its_fresh_surefire_reports_and_never_a_stale_one() {
    let rec = recording("maven", "plain", "P2");
    let (dir, head) = repo("mvn", &[("mvn", &FAKE_MVN.replace("__REC__", &rec.to_string_lossy())), ("pom.xml", "<project/>\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    let command = "./mvn -q test";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("maven:report"), &mut |_| true).expect("ran");
    let reported = ran.reported.as_deref().expect("the surefire XML converted");
    let (_, t) = pipeline_reading_by(&dir, command, &head, &ran.out, &ran.report, Some(reported), &[]);
    let t = t.expect("a translation");
    assert_eq!((s(&t, "by"), s(&t, "label")), ("reporter", "tests_failed"), "{t:?}");
    // a compile error leaves an older report in place: it is not this run's
    let (dir2, head2) = repo("mvn-stale", &[("mvn", &FAKE_MVN.replace("__REC__", &rec.to_string_lossy())), ("pom.xml", "<project/>\n"), ("BROKEN", "1\n"), ("target/surefire-reports/TEST-calc.ProbeTest.xml", &std::fs::read_to_string(rec.join("report/surefire-reports/TEST-calc.ProbeTest.xml")).unwrap())]);
    let p2 = Place { worktree: &dir2, root: &dir2, head: &head2, wait: Duration::from_secs(30) };
    let ran = run_commit_read(&p2, command, &head2, familiar_ai_run::details_for(command), Some("maven:report"), &mut |_| true).expect("ran");
    let (_, t) = pipeline_reading_by(&dir2, command, &head2, &ran.out, &ran.report, ran.reported.as_deref(), &[]);
    let t = t.expect("a translation");
    assert_ne!(s(&t, "by"), "reporter", "the old report is no reading of this run: {t:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}
