//! A laptop run applies the reader the record chose for its runner (plan A, lane a-py): with
//! `pytest:reporter` the run gets the kit and a report path, and its translation is read from
//! the per-test result file `by: "reporter"`; with none, the tables read the console as before.
//! A fake `pytest` stands in for the runner: it writes the A0 spike's P2 recording to
//! `$FAMILIAR_AI_REPORT` only when the kit is on its `PYTHONPATH` and in `PYTEST_ADDOPTS`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use familiar_ai::door::readers::chosen_reader;
use familiar_ai::door::runner::pipeline_reading_by;
use familiar_ai::door::verify_run::{run_commit_read, Place};
use familiar_ai_facts::{Fact, LocationKind};

const FAKE: &str = r#"#!/bin/sh
case "$PYTEST_ADDOPTS" in *"-p familiar_ai_pytest"*) ;; *) exit 1 ;; esac
case "$PYTHONPATH" in *familiar-ai-kit-*/pytest*) ;; *) exit 1 ;; esac
[ -f "${PYTHONPATH%%:*}/familiar_ai_pytest.py" ] || exit 1
cat > "$FAMILIAR_AI_REPORT" <<'EOF'
{"type": "session", "v": 1, "runner": "pytest", "runner_version": "9.1.1", "language": "python", "language_version": "3.14.4", "plugins": ["pytest-pretty-1.3.0"], "xdist": false}
{"type": "test", "id": "tests/test_p2.py::test_add", "file": "tests/test_p2.py", "line": 4, "outcome": "failed", "skip_reason": null, "error": {"type": "AssertionError", "message": "assert 4 == 5", "kind": "assertion", "frames": [{"file": "tests/test_p2.py", "line": 5, "function": "test_add"}]}}
{"type": "end", "exit": 1, "counts": {"passed": 0, "failed": 1, "skipped": 0, "deselected": 0, "errors": 0, "xfailed": 0, "xpassed": 0}}
EOF
exit 1
"#;

const TEST: &str = "from calc import add\n\n\ndef test_add():\n    assert add(2, 2) == 5\n";

fn repo(name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-chosen-reader-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(dir.join("pytest"), FAKE).unwrap();
    std::fs::write(dir.join("tests/test_p2.py"), TEST).unwrap();
    let git = |args: &[&str]| {
        let _ = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status();
    };
    let _ = Command::new("chmod").arg("+x").arg(dir.join("pytest")).status();
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();
    (dir, head)
}

fn chosen(runner: &str, reader: &str, at: f64) -> Fact {
    let fields = serde_json::json!({
        "run_id": "d-1", "root": "", "runner": runner, "reader": reader, "command": "./pytest",
        "commit": "c", "familiar": "0", "watched": [], "gaps": [], "why": "the dry run"
    });
    Fact { t: at as u64, at, kind: "reader_chosen", fields: fields.as_object().unwrap().clone(), tenant: "familiar-ai".into() }
}

#[test]
fn the_latest_choice_for_the_root_and_runner_is_the_one_used() {
    let log = vec![chosen("pytest", "pytest:reporter", 1.0), chosen("pytest", "pytest:tables", 2.0), chosen("jest", "jest:reporter", 3.0)];
    assert_eq!(chosen_reader(&log, "", "pytest").as_deref(), Some("pytest:tables"));
    assert_eq!(chosen_reader(&log[..1], "", "pytest").as_deref(), Some("pytest:reporter"));
    assert_eq!(chosen_reader(&log, "sub", "pytest"), None, "another root");
    assert_eq!(chosen_reader(&[], "", "pytest"), None, "no choice yet");
}

#[test]
fn a_run_with_the_reporter_chosen_reads_by_the_reporter() {
    let (dir, head) = repo("reporter");
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    let command = "./pytest";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("pytest:reporter"), &mut |_| true).expect("ran");
    let reported = ran.reported.as_deref().expect("the reporter's file was read before the copy went");
    let (_, t) = pipeline_reading_by(&dir, command, &head, &ran.out, &ran.report, Some(reported), &["tests/test_p2.py".to_string()]);
    let t = t.expect("a translation");
    assert_eq!(t.get("by").and_then(|v| v.as_str()), Some("reporter"));
    assert_eq!(t.get("label").and_then(|v| v.as_str()), Some("tests_failed"));
    let loc: familiar_ai_facts::Location = serde_json::from_value(t.get("location").cloned().expect("a location")).unwrap();
    assert_eq!((loc.file_path.as_str(), loc.line, loc.kind), ("tests/test_p2.py", 5, Some(LocationKind::Assertion)));
    assert_eq!(loc.function.as_deref(), Some("def test_add():"), "git's header above the line");
    assert_eq!(t.get("ran_tests"), Some(&serde_json::json!(["tests/test_p2.py::test_add"])));
    assert!(t.get("versions").and_then(|v| v.as_str()).is_some_and(|v| v.contains("pytest 9.1.1")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_whose_runner_no_reader_reads_right_reads_cant_tell() {
    // the dry run chose `none`: every reader read a probe wrong, so no run of the runner is read
    let (dir, head) = repo("none");
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    let command = "./pytest";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), Some("none"), &mut |_| true).expect("ran");
    let (_, t) = familiar_ai::door::runner::pipeline_reading_with(&dir, command, &head, &ran.out, &ran.report, Some("none"), ran.reported.as_deref(), &[]);
    let t = t.expect("a translation");
    assert_eq!(t.get("label").and_then(|v| v.as_str()), Some("unclassified"));
    assert_eq!(t.get("by").and_then(|v| v.as_str()), Some("binary"));
    assert!(t.get("evidence").and_then(|v| v.as_str()).is_some_and(|e| e.contains("no reader")), "{t:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_with_no_reader_chosen_reads_by_the_tables() {
    let (dir, head) = repo("tables");
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
    let command = "./pytest";
    let ran = run_commit_read(&p, command, &head, familiar_ai_run::details_for(command), None, &mut |_| true).expect("ran");
    assert_eq!(ran.reported, None, "no reporter, no file");
    let (_, t) = pipeline_reading_by(Path::new(&dir), command, &head, &ran.out, &ran.report, None, &[]);
    assert_eq!(t.unwrap().get("by").and_then(|v| v.as_str()), Some("regex"));
    let _ = std::fs::remove_dir_all(&dir);
}
