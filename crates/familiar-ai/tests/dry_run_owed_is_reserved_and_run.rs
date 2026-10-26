//! A dry run owed (plan A, lane a-py: a claimed pytest command) is reserved first by the binary's
//! facts, part-less and holding the tests, run by the run's child, and leaves its choice as a
//! `reader_chosen` with the run ended; it is never reserved twice for the same trigger.

use std::process::Command;
use std::time::Duration;

use familiar_ai::door::run::binary_facts;
use familiar_ai::door::runner::execute;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, TENANT};
use familiar_ai_facts::Record;
use serde_json::{json, Value};

/// A fake pytest that reads the probe's marker and writes what Familiar's reporter would.
const FAKE: &str = r#"#!/bin/sh
for a in "$@"; do target="$a"; done
probe=$(head -1 "$target" | sed 's/^# familiar-ai probe //')
w() { printf '%s\n' "$1" >> "$FAMILIAR_AI_REPORT"; }
case "$PYTEST_ADDOPTS" in *"-p familiar_ai_pytest"*) ;; *) exit 9 ;; esac
t() { w "{\"type\":\"test\",\"id\":\"$target::t$2\",\"file\":\"$target\",\"line\":5,\"outcome\":\"$1\",\"skip_reason\":null,\"error\":$3}"; }
w '{"type":"session","v":1,"runner":"pytest","runner_version":"9.1.1","language":"python","language_version":"3.14.4","plugins":[]}'
case "$probe" in
  P1|P3b) t passed "" null; code=0 ;;
  P2) t failed "" '{"type":"AssertionError","message":"x","kind":"assertion","frames":[]}'; code=1 ;;
  P3) w "{\"type\":\"collect_error\",\"file\":\"$target\",\"error\":{\"type\":\"ImportError\",\"message\":\"cannot import name 'mul'\",\"kind\":\"missing_name\",\"frames\":[]}}"; code=2 ;;
  P4) w "{\"type\":\"collect_error\",\"file\":\"$target\",\"error\":{\"type\":\"ModuleNotFoundError\",\"message\":\"x\",\"kind\":\"exception\",\"frames\":[]}}"; code=2 ;;
  P5) t passed "" null; t skipped _later null; code=0 ;;
  P6) code=5 ;;
  P7) t failed "" '{"type":"ValueError","message":"x","kind":"exception","frames":[]}'; code=1 ;;
esac
w "{\"type\":\"end\",\"exit\":$code,\"counts\":{}}"
exit $code
"#;

fn around(dir: &std::path::Path) -> Surroundings {
    Surroundings {
        now: 1.0,
        project_dir: Some(dir.to_path_buf()),
        cwd: Some(dir.to_path_buf()),
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_secs(5),
        test_run_wait: Duration::from_secs(60),
        run_inline: Duration::ZERO,
        runner: None,
        judge_dir: None,
        judge_wait: Duration::ZERO,
        investigator: None,
        conversation: None,
        bench: false,
    }
}

#[test]
fn a_claimed_pytest_command_owes_a_dry_run_that_chooses_the_reporter() {
    let dir = std::env::temp_dir().join(format!("familiar-ai-dry-run-owed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(FAMILIAR_FOLDER)).unwrap();
    std::fs::write(dir.join("pytest"), FAKE).unwrap();
    let _ = Command::new("chmod").arg("+x").arg(dir.join("pytest")).status();
    let git = |args: &[&str]| {
        let _ = Command::new("git").args(args).current_dir(&dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").status();
    };
    git(&["init", "-q"]);
    git(&["add", "pytest"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();

    let record = Record::open(&dir.join(FAMILIAR_FOLDER).join("record")).unwrap();
    let add = |kind: &str, f: Value| {
        record.append_raw(kind, f.as_object().cloned().unwrap(), 1.0, TENANT).expect(kind);
    };
    add("test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": "python_pytest", "test_command": "./pytest -q"}));
    add("commit_seen", json!({"commit": head, "files": ["pytest"], "touches_tests": false, "touches_code": true, "anchor_overlap": []}));

    let a = around(&dir);
    let mut log = record.read_all().unwrap();
    let moves = binary_facts(&record, &mut log, 2.0, TENANT, &a).expect("the binary's facts");
    assert_eq!(moves.reserved.len(), 1, "the dry run is reserved: {:?}", moves.reserved);
    let id = moves.reserved[0].clone();
    assert!(id.starts_with("d-"), "{id}");
    let reserved = log.iter().find(|f| f.kind == "run_reserved").expect("reserved");
    let field = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).cloned().unwrap_or(Value::Null);
    assert_eq!(field(reserved, "by"), json!("dry_run"));
    assert_eq!(field(reserved, "criterion_id"), json!(""), "part-less");
    assert_eq!(field(reserved, "resources"), json!(["tests"]));
    assert_eq!(field(reserved, "command"), json!("./pytest -q"), "the claimed command");

    // never twice for the same trigger
    let again = binary_facts(&record, &mut log, 3.0, TENANT, &a).expect("again");
    assert!(again.reserved.is_empty(), "{:?}", again.reserved);

    // the hook releases the record before the child runs (record.while_released)
    drop(record);
    execute(&dir, &id, &a).expect("the run's child");
    let log = Record::open(&dir.join(FAMILIAR_FOLDER).join("record")).unwrap().read_all().unwrap();
    let chosen = log.iter().find(|f| f.kind == "reader_chosen").expect("a choice");
    assert_eq!(field(chosen, "reader"), json!("pytest:reporter"), "{:?}", chosen.fields);
    assert_eq!(field(chosen, "run_id"), json!(id));
    assert_eq!(field(chosen, "commit"), json!(head));
    assert!(log.iter().any(|f| f.kind == "run_ended" && field(f, "run_id") == json!(id)), "the dry run ended");
    assert!(!log.iter().any(|f| f.kind == "test_log_translation"), "a dry run writes no part's translation");
    let _ = std::fs::remove_dir_all(&dir);
}
