//! The adapter's triggers (plan A-3, "A · Step 2", lane a5): a dry run is owed for each stack in
//! force when (1) no choice was made for its root, or the command changed; (2) a later commit
//! changed a file the choice watches; (3) the choice was made by another Familiar version; (4) a
//! later run of a part read as `unclassified`; (5) a later translation saw other runner versions;
//! (6) the person's `try_again` on the gap card. Once a choice lands after its trigger, nothing is
//! owed until the next trigger. `reader_at` is the latest choice at or before a commit.

use familiar_ai_facts::Log;
use familiar_ai_rules::reading::{dry_run_owed, reader_at, DryRunOwed, SETUP_PART};
use familiar_ai_rules::sample::{criterion_fixture, PROJECT_ROOT, TEST_COMMAND};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const FAMILIAR: &str = "0.1.0";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

fn start() -> Log {
    criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture")
}

/// The binary's choice for the sample's stack, made at `commit`.
fn chose(log: &Log, command: &str, commit: &str, gaps: &[&str], versions: Option<&str>) -> Log {
    let n = log.iter().filter(|f| f.kind == "reader_chosen").count();
    let mut f = json!({
        "run_id": format!("d-{n}"), "root": PROJECT_ROOT, "runner": "pytest", "reader": "pytest:reporter",
        "command": command, "commit": commit, "familiar": FAMILIAR,
        "watched": ["pyproject.toml", "**/conftest.py"], "gaps": gaps, "why": "test",
    });
    if let Some(x) = versions {
        f["versions"] = json!(x);
    }
    raw(log, "reader_chosen", f)
}

fn commit(log: &Log, id: &str, files: &[&str]) -> Log {
    raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": false, "touches_code": false, "anchor_overlap": [] }))
}

/// A run of c1 at `id` (by the binary, or a dry run), translated as `label`.
fn translated(log: &Log, id: &str, by: &str, label: &str, versions: Option<&str>) -> Log {
    let rid = format!("r-{}", log.len());
    let c = if by == "dry_run" { "" } else { "c1" };
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": c, "criterion_rev": "R1", "commit": id, "command": TEST_COMMAND, "resources": ["tests"], "wait": "60", "by": by }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": "failed", "end": "exited" }));
    let mut t = json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" });
    if let Some(x) = versions {
        t["versions"] = json!(x);
    }
    raw(&log, "test_log_translation", t)
}

fn owed(log: &Log) -> Vec<DryRunOwed> {
    dry_run_owed(&View::new(log), FAMILIAR)
}

fn whys(log: &Log) -> Vec<&'static str> {
    owed(log).into_iter().map(|o| o.why).collect()
}

#[test]
fn no_choice_owes_one_for_the_stack_in_force_and_a_choice_clears_it() {
    let log = start();
    assert_eq!(owed(&log), vec![DryRunOwed { root: PROJECT_ROOT.into(), command: TEST_COMMAND.into(), why: "no_choice" }]);
    let log = chose(&log, TEST_COMMAND, "base", &[], None);
    assert_eq!(owed(&log), Vec::new());
}

#[test]
fn a_new_command_owes_one_and_its_choice_clears_it() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    let log = raw(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": PROJECT_ROOT, "test_stack": "python_pytest", "test_command": "pytest -q" }));
    assert_eq!(owed(&log), vec![DryRunOwed { root: PROJECT_ROOT.into(), command: "pytest -q".into(), why: "command_changed" }]);
    assert_eq!(owed(&chose(&log, "pytest -q", "base", &[], None)), Vec::new());
}

#[test]
fn a_choice_the_pipeline_made_is_not_the_laptops() {
    let log = start();
    let n = log.len();
    let log = raw(&log, "reader_chosen", json!({ "run_id": "d-p", "root": PROJECT_ROOT, "runner": "pytest", "reader": "pytest:reporter", "command": TEST_COMMAND, "commit": "base", "familiar": FAMILIAR, "watched": [], "gaps": [], "why": "ci", "ran_in": "pipeline" }));
    assert!(log.len() > n);
    assert_eq!(whys(&log), ["no_choice"]);
}

#[test]
fn another_familiar_version_owes_one() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    assert_eq!(dry_run_owed(&View::new(&log), FAMILIAR), Vec::new());
    let o = dry_run_owed(&View::new(&log), "0.2.0");
    assert_eq!(o.into_iter().map(|o| o.why).collect::<Vec<_>>(), ["familiar_version"]);
}

#[test]
fn a_later_commit_of_a_watched_file_under_the_root_owes_one() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    assert_eq!(whys(&commit(&log, "k1", &["src/login.py"])), Vec::<&str>::new(), "not a watched file");
    assert_eq!(whys(&commit(&log, "k1", &["other/pyproject.toml"])), Vec::<&str>::new(), "outside the root");
    assert_eq!(whys(&commit(&log, "k1", &["src/pyproject.toml"])), ["setup_files"]);
    assert_eq!(whys(&commit(&log, "k1", &["src/conftest.py"])), ["setup_files"], "** matches no folder");
    assert_eq!(whys(&commit(&log, "k1", &["src/pkg/deep/conftest.py"])), ["setup_files"]);
    assert_eq!(whys(&commit(&log, "k1", &["src/pkg/pyproject.toml"])), Vec::<&str>::new(), "a pattern with no ** is at the root");
    // the choice after the commit read it: nothing owed until the next one
    let log = commit(&log, "k1", &["src/conftest.py"]);
    let log = chose(&log, TEST_COMMAND, "k1", &[], None);
    assert_eq!(owed(&log), Vec::new());
    // a rebase lands no new content: the same files again owe nothing
    assert_eq!(owed(&commit(&log, "k1b", &["src/conftest.py"])), Vec::new());
}

#[test]
fn a_later_unclassified_run_owes_one_once_per_choice() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    let log = commit(&log, "k1", &["src/login.py"]);
    assert_eq!(whys(&translated(&log, "k1", "binary", "tests_failed", None)), Vec::<&str>::new());
    let log = translated(&log, "k1", "binary", "unclassified", None);
    assert_eq!(whys(&log), ["unclassified"]);
    let log = chose(&log, TEST_COMMAND, "k1", &[], None);
    assert_eq!(owed(&log), Vec::new(), "the dry run after it answers it");
    // the dry run's own probes that cannot be read are its gaps, never a trigger
    assert_eq!(owed(&translated(&log, "k1", "dry_run", "unclassified", None)), Vec::new());
}

#[test]
fn other_runner_versions_than_the_choices_owe_one() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], Some("pytest 9.1.1"));
    let log = commit(&log, "k1", &["src/login.py"]);
    assert_eq!(whys(&translated(&log, "k1", "binary", "tests_failed", Some("pytest 9.1.1"))), Vec::<&str>::new());
    assert_eq!(whys(&translated(&log, "k1", "binary", "tests_failed", Some("pytest 9.2.0"))), ["versions_drift"]);
}

#[test]
fn try_again_on_the_gap_card_owes_one_and_keep_going_does_not() {
    let log = chose(&start(), TEST_COMMAND, "base", &["P5"], None);
    assert_eq!(owed(&log), Vec::new());
    let answer = |log: &Log, choice: &str| raw(log, "hold_answered", json!({ "criterion_id": SETUP_PART, "hold": "reading_gap", "choice": choice, "typed": "" }));
    assert_eq!(owed(&answer(&log, "keep_going")), Vec::new());
    let log = answer(&log, "try_again");
    assert_eq!(whys(&log), ["try_again"]);
    assert_eq!(owed(&chose(&log, TEST_COMMAND, "base", &["P5"], None)), Vec::new(), "one dry run per answer");
}

#[test]
fn the_reader_at_a_commit_is_the_latest_choice_at_or_before_it() {
    let log = commit(&start(), "k1", &["src/login.py"]);
    let log = chose(&log, TEST_COMMAND, "k1", &[], None);
    let log = commit(&log, "k2", &["src/conftest.py"]);
    let log = commit(&log, "k3", &["src/login.py"]);
    let log = chose(&log, TEST_COMMAND, "k2", &["P5"], None);
    let v = View::new(&log);
    let run_of = |id: &str| reader_at(&v, PROJECT_ROOT, "pytest", id).and_then(|f| f.fields.get("run_id")).and_then(Value::as_str).map(str::to_string);
    assert_eq!(run_of("k1").as_deref(), Some("d-0"));
    assert_eq!(run_of("k2").as_deref(), Some("d-1"));
    assert_eq!(run_of("k3").as_deref(), Some("d-1"));
    assert_eq!(reader_at(&v, PROJECT_ROOT, "jest", "k3"), None, "another runner has no choice");
    assert_eq!(reader_at(&v, "web/", "pytest", "k3"), None, "another root has no choice");
    // a commit the record has not seen reads the latest choice
    assert_eq!(run_of("unseen").as_deref(), Some("d-1"));
}

/// A run of c1 at `id`: reserved, ended with `word`, translated as `label` with `counts`, and a
/// result for `file` of outcome `word` naming it.
fn ran(log: &Log, id: &str, file: &str, word: &str, label: &str, counts: Value) -> Log {
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": TEST_COMMAND, "resources": ["tests"], "wait": "60", "by": "binary" }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": word, "end": "exited" }));
    log = raw(&log, "test_log_translation", json!({ "run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "counts": counts, "classified_on": "full" }));
    raw(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": file, "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }))
}

#[test]
fn a_touching_run_passed_beside_a_skip_owes_no_dry_run() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    let log = commit(&log, "k1", &["src/login.py"]);
    // a pass beside a skip: the table calls it unclassified, but the touching result reads as a pass
    let log = ran(&log, "k1", "tests/test_login_old.py", "passed", "unclassified", json!({ "passed": 1, "skipped": 1, "failed": 0 }));
    assert_eq!(owed(&log), Vec::new());
}

#[test]
fn a_touching_run_that_failed_reads_unclassified_and_owes_one() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    let log = commit(&log, "k1", &["src/login.py"]);
    let log = ran(&log, "k1", "tests/test_login_old.py", "failed", "unclassified", json!({ "passed": 1, "skipped": 1, "failed": 1 }));
    assert_eq!(whys(&log), ["unclassified"]);
}

#[test]
fn the_parts_own_run_translated_unclassified_still_owes_one() {
    let log = chose(&start(), TEST_COMMAND, "base", &[], None);
    let log = commit(&log, "k1", &["src/login.py"]);
    // the part's own run is not a touching run: its pass beside a skip is the can't-tell of its own counts
    let log = ran(&log, "k1", "tests/test_c1.py", "passed", "unclassified", json!({ "passed": 1, "skipped": 1, "failed": 0 }));
    assert_eq!(whys(&log), ["unclassified"]);
}
