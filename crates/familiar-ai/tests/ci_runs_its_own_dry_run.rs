//! Lane A-wire (plan of record 2026-10-19, A: "CI: verify::recheck runs its own dry run per (root,
//! runner) at HEAD, appends its own reader_chosen{ran_in: pipeline}, uses it, and prints one line
//! when the committed file differs (never changes the exit code)"). CI re-checks what the laptop
//! calibrated: a project with no adapter committed is read by the tables as before. Its runs use
//! its own choice. A fake `jest` replays the A0 spike's recordings (the reporter's file only when
//! the reporter's flag reached it).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use familiar_ai::door::verify::pipeline_dry_runs;
use familiar_ai::door::verify_run::{run_at_read, At, Place};
use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

const FAKE_JEST: &str = r#"#!/bin/sh
REC=__REC__
on=; t=
for a in "$@"; do
  case "$a" in
    --reporters=*/jest/reporter.cjs) on=1 ;;
    *.test.js) t="$a" ;;
  esac
done
case "$t" in
  *familiar_probe_p*) p=$(basename "$t" .test.js); p=${p#familiar_probe_p}; P="P$p"
    if [ "$p" = 3 ] && grep -q 'function mul' "$(dirname "$t")/familiar_probe_calc.js"; then P=P3b; fi ;;
  *) P=P2 ;;
esac
[ -n "$on" ] && cp "$REC/$P/report/results.jsonl" "$FAMILIAR_AI_REPORT"
cat "$REC/$P/stdout"; cat "$REC/$P/stderr" >&2
exit $(cat "$REC/$P/exit")
"#;

fn repo(name: &str, adapter: Option<&str>) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-ci-dry-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rec = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs/jest/plain");
    let mut files: Vec<(&str, String)> = vec![("jest", FAKE_JEST.replace("__REC__", &rec.to_string_lossy())), ("test/p2.test.js", "'use strict';\nconst { add } = require('./calc.js');\n\ntest('adds', () => {\n  expect(add(2, 2)).toBe(5);\n});\n".into())];
    if let Some(a) = adapter {
        files.push((".familiar/adapter", a.to_string()));
    }
    for (p, t) in &files {
        let at = dir.join(p);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, t).unwrap();
    }
    let _ = Command::new("chmod").arg("+x").arg(dir.join("jest")).status();
    let git = |args: &[&str]| {
        let _ = Command::new("git").args(args).current_dir(&dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").status();
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();
    (dir, head)
}

fn plan(command: &str) -> Log {
    let add = |log: &Log, kind: &str, f: Value| append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), 1.0, "t").expect(kind);
    let log = add(&Vec::new(), "test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": "node_jest", "test_command": command}));
    add(&log, "test_planned", json!({"criterion_id": "c1", "file_path": "test/p2.test.js", "will": "create", "test_command": command}))
}

fn adapter(reader: &str) -> String {
    format!("{{\"root\":\"\",\"runner\":\"jest\",\"reader\":\"{reader}\",\"familiar\":\"0.1.4-prod\",\"gaps\":[]}}\n")
}

#[test]
fn a_project_with_no_adapter_committed_gets_no_ci_dry_run() {
    let (dir, head) = repo("none", None);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let (choices, notes) = pipeline_dry_runs(&p, &plan("./jest"));
    assert!(choices.is_empty() && notes.is_empty(), "{choices:?} {notes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ci_chooses_its_own_reader_and_says_when_the_committed_adapter_differs() {
    let (dir, head) = repo("differs", Some(&adapter("jest:tables")));
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let (choices, notes) = pipeline_dry_runs(&p, &plan("./jest test/p2.test.js"));
    assert_eq!(choices.len(), 1, "{notes:?}");
    let c = &choices[0];
    assert_eq!((c.runner.as_str(), c.reader.as_str(), c.ran_in.as_deref(), c.commit.as_str()), ("jest", "jest:reporter", Some("pipeline"), head.as_str()), "{}", c.why);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("jest:tables") && notes[0].contains("jest:reporter"), "{}", notes[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ci_says_nothing_when_its_choice_is_the_committed_one() {
    let (dir, head) = repo("same", Some(&adapter("jest:reporter")));
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let (choices, notes) = pipeline_dry_runs(&p, &plan("./jest"));
    assert_eq!(choices.iter().map(|c| c.reader.as_str()).collect::<Vec<_>>(), ["jest:reporter"]);
    assert!(notes.is_empty(), "{notes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ci_run_under_its_chosen_reader_reads_by_the_reporter() {
    let (dir, head) = repo("run", None);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let command = "./jest test/p2.test.js";
    let files = ["test/p2.test.js".to_string()];
    for at in [At::Here, At::Commit(&head)] {
        let ran = run_at_read(&p, command, &at, Some("jest:reporter"), &files);
        let t = ran.translation.expect("a translation");
        let s = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        assert_eq!((s("by").as_str(), s("label").as_str()), ("reporter", "tests_failed"), "{t:?}");
        assert_eq!(t.get("ran_tests"), Some(&json!(["test/p2.test.js::adds"])), "the part's own tests by name");
    }
    let ran = run_at_read(&p, command, &At::Commit(&head), None, &files);
    assert_eq!(ran.translation.and_then(|t| t.get("by").cloned()), Some(json!("regex")), "no choice: the tables, as before");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_laptops_adapter_and_runs_never_take_cis_own_choice() {
    let chosen = |reader: &str, ran_in: &str, at: f64| {
        let fields = json!({"run_id": "d-1", "root": "", "runner": "jest", "reader": reader, "command": "npx jest", "commit": "c", "familiar": "0", "watched": [], "gaps": [], "why": "w", "ran_in": ran_in});
        familiar_ai_facts::Fact { t: at as u64, at, kind: "reader_chosen", fields: fields.as_object().unwrap().clone(), tenant: "familiar-ai".into() }
    };
    let log = vec![chosen("jest:reporter", "laptop", 1.0), chosen("jest:tables", "pipeline", 2.0)];
    assert_eq!(familiar_ai::door::readers::chosen_reader(&log, "", "jest").as_deref(), Some("jest:reporter"));
    let text = familiar_ai::door::adapter_lines(&log);
    assert!(text.contains("\"reader\":\"jest:reporter\"") && !text.contains("jest:tables"), "{text}");
}
