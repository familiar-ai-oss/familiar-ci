//! The dry run for pytest (plan A, lane a-py): probes with known truths, placed at the planned
//! test's folder in a copy of the commit, run by the planned command re-aimed at each probe,
//! under each reader best first; the first reader that reads every probe right is chosen, else
//! the first that reads none wrong, its can't-tell probes the gaps. A fake `pytest` stands in
//! for the runner: it reads the probe's marker line and writes what the reporter would.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use familiar_ai::door::dry_run::{choose, dry_run, owed, probes, re_aim, Verdict};
use familiar_ai::door::verify_run::Place;
use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

#[test]
fn the_command_is_re_aimed_at_the_probe_keeping_wrappers_and_flags() {
    let probe = ["tests/test_familiar_probe_p1.py".to_string()];
    let cases = [
        ("python -m pytest -q -k greet tests/test_c1.py", "python -m pytest -q tests/test_familiar_probe_p1.py"),
        ("cd sub && .venv/bin/pytest --lf -m slow tests/ --tb=short", "cd sub && .venv/bin/pytest --tb=short tests/test_familiar_probe_p1.py"),
        ("uv run pytest --deselect tests/test_a.py::test_x --sw --ff -kfoo tests/test_a.py::test_y", "uv run pytest tests/test_familiar_probe_p1.py"),
        ("pytest", "pytest tests/test_familiar_probe_p1.py"),
        ("PYTHONHASHSEED=0 python3 -m pytest -x --failed-first --last-failed --stepwise", "PYTHONHASHSEED=0 python3 -m pytest -x tests/test_familiar_probe_p1.py"),
    ];
    for (command, want) in cases {
        assert_eq!(re_aim(command, &probe).as_deref(), Some(want), "{command}");
    }
    assert_eq!(re_aim("make test", &probe), None, "a command naming no runner gets no dry run");
}

#[test]
fn the_probes_carry_their_truths_and_their_markers() {
    let p = probes();
    let truths: Vec<(&str, &str)> = p.iter().map(|p| (p.name, p.truth)).collect();
    assert_eq!(
        truths,
        [
            ("P1", "passed"),
            ("P2", "tests_failed"),
            ("P3", "missing_target_symbol"),
            ("P4", "collection_error"),
            ("P5", "passed"),
            ("P6", "no_tests_collected"),
            ("P3b", "passed"),
            ("P7", "tests_failed"),
        ]
    );
    for probe in &p {
        assert!(probe.test.starts_with(&format!("# familiar-ai probe {}\n", probe.name)), "{}", probe.name);
    }
}

#[test]
fn the_first_reader_right_everywhere_wins_else_the_first_never_wrong() {
    use Verdict::{CantTell as C, Right as R, Wrong as W};
    let all_right = vec![("P1", R), ("P2", R)];
    let (reader, gaps, _) = choose(&[("pytest:reporter", all_right.clone()), ("pytest:tables", vec![("P1", C), ("P2", R)])]);
    assert_eq!((reader.as_str(), gaps.len()), ("pytest:reporter", 0));
    let (reader, gaps, _) = choose(&[("pytest:reporter", vec![("P1", R), ("P2", W)]), ("pytest:tables", vec![("P1", R), ("P2", C)])]);
    assert_eq!((reader.as_str(), gaps), ("pytest:tables", vec!["P2".to_string()]), "a wrong reading rules a reader out");
    let (reader, gaps, why) = choose(&[("pytest:reporter", vec![("P1", R), ("P2", W)]), ("pytest:tables", vec![("P1", W), ("P2", R)])]);
    assert_eq!((reader.as_str(), gaps), ("none", vec!["P1".to_string(), "P2".to_string()]), "under none no run is read: every probe is a gap, so the setup card fires");
    assert!(why.contains("P2") || why.contains("P1"), "the card names the probe that read wrong: {why}");
}

fn add(log: &Log, kind: &str, f: Value) -> Log {
    let at = log.len() as f64 + 1.0;
    append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), at, "t").expect(kind)
}

fn stack(log: &Log, command: &str) -> Log {
    add(log, "test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": "python_pytest", "test_command": command}))
}

#[test]
fn a_dry_run_is_owed_for_a_pytest_command_until_one_is_reserved_and_again_after_a_watched_change() {
    let log = stack(&Vec::new(), "python -m pytest");
    assert_eq!(owed(&log).as_deref(), Some("python -m pytest"), "the claimed command");
    let log = add(&log, "run_reserved", json!({"run_id": "d-1", "criterion_id": "", "criterion_rev": "", "commit": "K", "command": "python -m pytest", "resources": ["tests"], "wait": "60", "by": "dry_run"}));
    assert_eq!(owed(&log), None, "reserved once, never a loop");
    let seen = |commit: &str, file: &str| json!({"commit": commit, "files": [file], "touches_tests": false, "touches_code": true, "anchor_overlap": []});
    let log = add(&log, "commit_seen", seen("K2", "src/app.py"));
    assert_eq!(owed(&log), None, "a commit touching no setup file");
    let log = add(&log, "commit_seen", seen("K3", "tests/conftest.py"));
    assert_eq!(owed(&log).as_deref(), Some("python -m pytest"), "a conftest changed");
    let log = stack(&Vec::new(), "npm test");
    assert_eq!(owed(&log), None, "pytest only in this lane");
}

#[test]
fn no_dry_run_is_owed_to_a_stack_claimed_as_another_runner() {
    // the stack in force is sh; its script happens to be named pytest
    let log = add(&Vec::new(), "test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": "sh", "test_command": "./pytest tests/test_c1.sh"}));
    assert_eq!(owed(&log), None);
    let log = add(&Vec::new(), "test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": "pytest", "test_command": "./pytest tests/test_c1.sh"}));
    assert_eq!(owed(&log).as_deref(), Some("./pytest tests/test_c1.sh"), "a pytest stack");
}

const FAKE: &str = r#"#!/bin/sh
# a fake pytest: reads the probe's marker and writes what Familiar's reporter would
for a in "$@"; do target="$a"; done
probe=$(head -1 "$target" | sed 's/^# familiar-ai probe //')
case "$PYTEST_ADDOPTS" in *"-p familiar_ai_pytest"*) on=1 ;; *) on= ;; esac
[ -n "$on" ] && [ -f "${PYTHONPATH%%:*}/familiar_ai_pytest.py" ] || on=
w() { [ -n "$on" ] && printf '%s\n' "$1" >> "$FAMILIAR_AI_REPORT"; }
t() { w "{\"type\":\"test\",\"id\":\"$target::test_familiar_probe$2\",\"file\":\"$target\",\"line\":5,\"outcome\":\"$1\",\"skip_reason\":null,\"error\":$3}"; }
w '{"type":"session","v":1,"runner":"pytest","runner_version":"9.1.1","language":"python","language_version":"3.14.4","plugins":[]}'
LIE=__LIE__
case "$probe" in
  P1|P3b) t passed "" null; code=0 ;;
  P2) if [ -n "$LIE" ]; then t passed "" null; code=0; else t failed "" '{"type":"AssertionError","message":"assert 4 == 5","kind":"assertion","frames":[]}'; code=1; fi ;;
  P3) w "{\"type\":\"collect_error\",\"file\":\"$target\",\"error\":{\"type\":\"ImportError\",\"message\":\"cannot import name 'mul' from 'familiar_probe_calc'\",\"kind\":\"missing_name\",\"frames\":[]}}"; code=2 ;;
  P4) w "{\"type\":\"collect_error\",\"file\":\"$target\",\"error\":{\"type\":\"ModuleNotFoundError\",\"message\":\"No module named 'familiar_probe_nosuch'\",\"kind\":\"exception\",\"frames\":[]}}"; code=2 ;;
  P5) t passed "" null; t skipped _later null; code=0 ;;
  P6) code=5 ;;
  P7) t failed "" '{"type":"ValueError","message":"invalid literal","kind":"exception","frames":[]}'; code=1 ;;
esac
case $code in 0) p=1 ;; *) p=0 ;; esac
w "{\"type\":\"end\",\"exit\":$code,\"counts\":{\"passed\":$p,\"failed\":0,\"skipped\":0,\"deselected\":0,\"errors\":0}}"
exit $code
"#;

fn repo(name: &str, lie: bool) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-dry-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(dir.join("pytest"), FAKE.replace("__LIE__", if lie { "1" } else { "" })).unwrap();
    std::fs::write(dir.join("tests/test_c1.py"), "def test_c1():\n    assert True\n").unwrap();
    let _ = Command::new("chmod").arg("+x").arg(dir.join("pytest")).status();
    let git = |args: &[&str]| {
        let _ = Command::new("git").args(args).current_dir(&dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").status();
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();
    (dir, head)
}

#[test]
fn a_project_whose_reporter_reads_every_probe_right_chooses_the_reporter() {
    let (dir, head) = repo("honest", false);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let chosen = dry_run(&p, "d-1", "./pytest -q tests/test_c1.py", &head, &["tests/test_c1.py".to_string()], &mut |_| true).expect("the dry run ran");
    assert_eq!((chosen.reader.as_str(), chosen.runner.as_str(), chosen.root.as_str()), ("pytest:reporter", "pytest", ""));
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    assert_eq!(chosen.command, "./pytest -q tests/test_c1.py", "the claimed command, not the re-aimed one");
    assert_eq!(chosen.commit, head);
    assert!(chosen.watched.iter().any(|w| w == "**/conftest.py"), "{:?}", chosen.watched);
    assert!(chosen.versions.as_deref().is_some_and(|v| v.contains("pytest 9.1.1")));
    assert!(!dir.join("tests/test_familiar_probe_p1.py").exists(), "the probes went in the copy, never the work tree");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reader_that_cannot_read_the_command_is_never_chosen() {
    // python -I ignores PYTHONPATH, so the reporter cannot load: only the tables are tried
    let (dir, head) = repo("isolated", false);
    std::fs::copy(dir.join("pytest"), dir.join("python3")).unwrap();
    let _ = Command::new("chmod").arg("+x").arg(dir.join("python3")).status();
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    // the copy is of the commit: python3 is linked in as an untracked top-level entry
    let chosen = dry_run(&p, "d-3", "./python3 -I -m pytest -q", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_ne!(chosen.reader, "pytest:reporter", "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reporter_that_reads_a_probe_wrong_is_not_chosen() {
    let (dir, head) = repo("lying", true);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let chosen = dry_run(&p, "d-2", "./pytest", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_ne!(chosen.reader, "pytest:reporter", "P2 read passed under the reporter: {}", chosen.why);
    assert!(chosen.why.contains("P2"), "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}
