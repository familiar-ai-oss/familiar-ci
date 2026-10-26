//! Each criterion is proved on its own: it has its own lane and test command,
//! a commit is charged only to the criterion it touches, and the referee
//! refuses only the criterion that is about to start.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::action;
use familiar_ai_rules::criterion::{command, criterion_rev, derived_command, scoped};
use familiar_ai_rules::lifecycle::{attempts, lane, runnable, runnable_criteria, state, tries_not_red};
use familiar_ai_rules::moves::{owed, Work};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{decide, Event, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// The sample's two criteria, each the head of its own string.
fn two_heads() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    ap(&log, "criterion_head", json!({ "criterion_id": "c2" }))
}

fn commit(log: &Log, id: &str, files: &[&str], tests: bool, code: bool, overlap: &[&str]) -> Log {
    ap(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }))
}

/// A tests commit of one criterion's file, with that criterion's fingerprint
/// observed beside it.
fn tests_commit(log: &Log, id: &str, file: &str, hash: &str) -> Log {
    let c = if file == "tests/test_c1.py" { "c1" } else { "c2" };
    let log = ap(log, "commit_seen", json!({ "commit": id, "files": [file], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    ap(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash }))
}

fn result(log: &Log, c: &str, file: &str, id: &str, outcome: &str) -> Log {
    ap(log, "pass_please_pass", json!({ "criterion_id": c, "file_path": file, "commit": id, "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }))
}

fn stamp(log: &Log, c: &str, id: &str, proof: &str) -> Log {
    ap(log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": proof, "ran_in": "laptop" }))
}

/// One binary run of `c`'s own test at `id`, and its stamp.
fn run(log: &Log, c: &str, id: &str, outcome: &str, proof: &str) -> Log {
    let file = if c == "c1" { "tests/test_c1.py" } else { "tests/test_c2.py" };
    let mut log = result(log, c, file, id, outcome);
    if c == "c1" {
        log = result(&log, c, "tests/test_login_old.py", id, "passed");
    }
    stamp(&log, c, id, proof)
}

/// `c`'s tests-first commit `id`, its test written, seen with `outcome`.
fn tests_first(log: &Log, c: &str, id: &str, outcome: &str) -> Log {
    let (file, hash) = if c == "c1" { ("tests/test_c1.py", "S1") } else { ("tests/test_c2.py", "S2") };
    let log = tests_commit(log, id, file, hash);
    let log = ap(&log, "test_written", json!({ "criterion_id": c, "file_path": file, "test_name": "t1" }));
    run(&log, c, id, outcome, hash)
}

/// Both criteria sealed: each tests-first commit seen red and stamped.
fn both_sealed() -> Log {
    let log = tests_first(&two_heads(), "c1", "k1", "failed");
    tests_first(&log, "c2", "k2", "failed")
}

#[test]
fn two_heads_are_both_runnable_in_plan_order() {
    let log = two_heads();
    let v = View::new(&log);
    assert!(runnable(&v, "c1") && runnable(&v, "c2"));
    assert_eq!(runnable_criteria(&v), ["c1", "c2"]);
    let strung = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let v = View::new(&strung);
    assert_eq!(runnable_criteria(&v), ["c1"], "strung on c1, c2 waits its turn");
    assert!(!runnable(&v, "c2"));
}

#[test]
fn a_lane_is_the_runnable_criterion_its_conversation_holds() {
    let log = ap(&two_heads(), "criterion_transfer", json!({ "criterion_id": "c2", "from_conversation": "a1", "to_conversation": "a2" }));
    let v = View::new(&log);
    assert_eq!(lane(&v, Some("a1")).as_deref(), Some("c1"));
    assert_eq!(lane(&v, Some("a2")).as_deref(), Some("c2"));
    assert_eq!(lane(&v, Some("a3")), None, "no fallback to another conversation's criterion");
    assert_eq!(lane(&v, None), None);
}

#[test]
fn a_criterion_with_its_own_command_is_scoped_and_runs_it() {
    let log = ap(&two_heads(), "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "pytest tests/test_c1.py" }));
    let v = View::new(&log);
    assert_eq!(command(&v, "c1"), Some("pytest tests/test_c1.py".into()));
    assert!(scoped(&v, "c1"));
    assert_eq!(command(&v, "c2"), Some("python -m pytest tests/test_c2.py".into()), "no command of its own: derived from its files and the stack's runner");
    assert!(scoped(&v, "c2"));
}

/// CI reads the same command the laptop does: a lane's own `test_command`, else the stack's.
#[test]
fn the_action_runs_a_criterion_under_its_own_command() {
    let log = ap(&two_heads(), "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "sh tests/test_c1.sh" }));
    let v = View::new(&log);
    assert_eq!(action::command(&v, "c1"), Some("sh tests/test_c1.sh".into()), "its own command, not the whole stack's");
    assert_eq!(action::command(&v, "c2"), Some("python -m pytest tests/test_c2.py".into()), "no command of its own: derived from its files and the stack's runner");
    assert_eq!(action::command(&v, "c1"), command(&v, "c1"), "the laptop and the action agree");
    assert_eq!(action::command(&v, "c2"), command(&v, "c2"));
    // the latest non-empty claim stands; an empty one is no claim
    let log = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "sh tests/test_c1_v2.sh" }));
    let log = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "" }));
    assert_eq!(action::command(&View::new(&log), "c1"), Some("sh tests/test_c1_v2.sh".into()));
}

/// A greenfield criterion with no command of its own derives one from its files and the stack's runner, and never reads a sibling's `test_planned`.
#[test]
fn a_criterion_without_its_own_command_reads_the_stacks_never_a_siblings() {
    let log = ap(&two_heads(), "new_work_no_spot", json!({ "criterion_id": "c2", "integration_point": "src/mail.py:reset_link" }));
    let log = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "sh tests/test_c1.sh" }));
    let v = View::new(&log);
    assert_eq!(action::command(&v, "c1"), Some("sh tests/test_c1.sh".into()), "its own");
    assert_eq!(action::command(&v, "c2"), Some("python -m pytest tests/test_c2.py".into()), "no command of its own: derived from its files and the stack's runner, not c1's");
}

/// A command announced after the run on the head started is not the one it ran under, a criterion's own included.
#[test]
fn a_command_a_criterion_claims_after_the_run_unseats_the_run() {
    let log = commit(&two_heads(), "k1", &["tests/test_c1.py"], true, false, &[]);
    let log = ap(&log, "pipeline_run_seen", json!({ "head_commit": "k1" }));
    assert!(action::run(&View::new(&log)).is_some(), "a run on the head stands");
    let plain = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create" }));
    assert!(action::run(&View::new(&plain)).is_some(), "a plan with no command unseats nothing");
    let own = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create", "test_command": "sh tests/test_c1.sh" }));
    assert!(action::run(&View::new(&own)).is_none(), "an own command announced after the run: the run no longer stands");
}

#[test]
fn a_criterions_rev_reads_its_plan_never_the_position() {
    let log = two_heads();
    let r1 = criterion_rev(&View::new(&log), "c1");
    assert_ne!(r1, criterion_rev(&View::new(&log), "c2"));
    let later = commit(&log, "k9", &["src/other.py"], false, true, &[]);
    assert_eq!(criterion_rev(&View::new(&later), "c1"), r1, "a fact elsewhere changes nothing");
    let other = ap(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "src/", "test_stack": "python_pytest", "test_command": "pytest -q" }));
    assert_ne!(criterion_rev(&View::new(&other), "c1"), r1, "the command it runs is part of it");
    let more = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1_more.py", "will": "create" }));
    assert_ne!(criterion_rev(&View::new(&more), "c1"), r1, "its declared tests are part of it");
}

#[test]
fn both_heads_seal_on_their_own_tests_first_commits() {
    let log = both_sealed();
    let v = View::new(&log);
    assert_eq!(state(&v, "c1"), "sealed");
    assert_eq!(state(&v, "c2"), "sealed");
}

/// A criterion's command is derived from its files and the stack's runner through a table of common runners; an explicit `test_command` wins, and a runner not in the table derives nothing (unscoped).
#[test]
fn a_criterions_command_is_derived_from_its_files_and_the_stacks_runner() {
    let mk = |files: &[&str], root: &str, stack: &str, cmd: &str, own: Option<&str>| {
        let mut log: Log = Vec::new();
        log = ap(&log, "criterion", json!({ "criterion_id": "cX", "text": "x", "path": "full" }));
        log = ap(&log, "place_chosen", json!({ "criterion_id": "cX", "places": [{ "file_path": "src/x.py", "symbol": "x", "line_start": 1, "line_end": 2 }] }));
        log = ap(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": root, "test_stack": stack, "test_command": cmd }));
        for (i, f) in files.iter().enumerate() {
            let mut fl = json!({ "criterion_id": "cX", "file_path": f, "will": "create" });
            if let Some(c) = own {
                if i == 0 {
                    fl["test_command"] = json!(c);
                }
            }
            log = ap(&log, "test_planned", fl);
        }
        log
    };
    // two files: both in the derived command, in plan order; the command stays the stack's; scoped (it derives)
    let log = mk(&["tests/test_c1.py", "tests/test_c2.py"], "src/", "python_pytest", "python -m pytest", None);
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), Some("python -m pytest tests/test_c1.py tests/test_c2.py".into()), "two files, in plan order");
    assert_eq!(command(&v, "cX"), Some("python -m pytest tests/test_c1.py tests/test_c2.py".into()), "no own command: the derived is what runs");
    assert!(scoped(&v, "cX"), "it derives -> scoped");
    // explicit wins: the table's form still stands, but the command is the own
    let log = mk(&["tests/test_c1.py"], "src/", "python_pytest", "python -m pytest", Some("pytest -q tests/test_c1.py"));
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), Some("python -m pytest tests/test_c1.py".into()), "the table's form stands even with an own command");
    assert_eq!(command(&v, "cX"), Some("pytest -q tests/test_c1.py".into()), "explicit wins");
    assert!(scoped(&v, "cX"));
    // cargo: tests/<name>.rs -> --test <name>; a file not under tests/ (unit tests in src/) cannot be expressed
    let log = mk(&["tests/login.rs"], "src/", "rust_cargo", "cargo", None);
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), Some("cargo --test login".into()), "cargo names its integration tests");
    let log = mk(&["src/lib.rs"], "src/", "rust_cargo", "cargo", None);
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), None, "a unit test in src/ cannot be expressed for cargo");
    assert!(!scoped(&v, "cX"), "no own command, no derivation: unscoped");
    // a file under any directory whose last segment is tests (as in crates/familiar-ai/tests/<name>.rs) reads as --test <name>
    let log = mk(&["crates/familiar-ai/tests/login.rs"], "src/", "rust_cargo", "cargo", None);
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), Some("cargo --test login".into()), "the parent directory is tests, anywhere");
    // a runner not in the table: the derived command is null, the command falls to the stack's, unscoped
    let log = mk(&["tests/x.py"], "src/", "mystery_runner", "somecmd", None);
    let v = View::new(&log);
    assert_eq!(derived_command(&v, "cX"), None, "a runner not in the table derives nothing");
    assert_eq!(command(&v, "cX"), Some("somecmd".into()), "falls to the stack's command");
    assert!(!scoped(&v, "cX"));
}

/// The command that runs is the criterion's own, else the derived one, else the stack's, and the action runs the same.
/// See docs/decisions.md#the-command-is-claimed-derived-or-unscoped
#[test]
fn the_derived_command_is_what_runs() {
    let mk = |files: &[&str], root: &str, stack: &str, cmd: &str, own: Option<&str>| {
        let mut log: Log = Vec::new();
        log = ap(&log, "criterion", json!({ "criterion_id": "cX", "text": "x", "path": "full" }));
        log = ap(&log, "place_chosen", json!({ "criterion_id": "cX", "places": [{ "file_path": "src/x.py", "symbol": "x", "line_start": 1, "line_end": 2 }] }));
        log = ap(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": root, "test_stack": stack, "test_command": cmd }));
        for (i, f) in files.iter().enumerate() {
            let mut fl = json!({ "criterion_id": "cX", "file_path": f, "will": "create" });
            if let Some(c) = own {
                if i == 0 {
                    fl["test_command"] = json!(c);
                }
            }
            log = ap(&log, "test_planned", fl);
        }
        log
    };
    // pytest files, no typed command: the derived command is what runs
    let log = mk(&["tests/test_c1.py"], "src/", "python_pytest", "python -m pytest", None);
    let v = View::new(&log);
    assert_eq!(command(&v, "cX"), derived_command(&v, "cX"), "no typed command: the derived is what runs");
    assert_eq!(command(&v, "cX"), Some("python -m pytest tests/test_c1.py".into()));
    assert!(scoped(&v, "cX"));
    // a typed command: it is what runs (not the derived), still scoped
    let log = mk(&["tests/test_c1.py"], "src/", "python_pytest", "python -m pytest", Some("pytest -q tests/test_c1.py"));
    let v = View::new(&log);
    assert_eq!(command(&v, "cX"), Some("pytest -q tests/test_c1.py".into()), "the typed command is what runs, not the derived");
    assert!(scoped(&v, "cX"));
    // an unknown runner: the stack's command is what runs, unscoped
    let log = mk(&["tests/x.py"], "src/", "mystery_runner", "somecmd", None);
    let v = View::new(&log);
    assert_eq!(command(&v, "cX"), Some("somecmd".into()), "unknown runner: the stack's is what runs");
    assert!(!scoped(&v, "cX"), "unknown runner: unscoped");
    // the action runs the same command as the criterion, in all three
    let log = mk(&["tests/test_c1.py"], "src/", "python_pytest", "python -m pytest", None);
    assert_eq!(action::command(&View::new(&log), "cX"), command(&View::new(&log), "cX"), "no typed: the action agrees");
    let log = mk(&["tests/test_c1.py"], "src/", "python_pytest", "python -m pytest", Some("pytest -q tests/test_c1.py"));
    assert_eq!(action::command(&View::new(&log), "cX"), command(&View::new(&log), "cX"), "typed: the action agrees");
    let log = mk(&["tests/x.py"], "src/", "mystery_runner", "somecmd", None);
    assert_eq!(action::command(&View::new(&log), "cX"), command(&View::new(&log), "cX"), "unknown runner: the action agrees");
}

#[test]
fn a_commit_is_charged_only_to_the_criterion_it_touches() {
    let log = both_sealed();
    // c1's fix attempt on its anchor fails; the binary also ran c2 there
    let log = commit(&log, "k3", &["src/login.py"], false, true, &["c1"]);
    let log = run(&log, "c1", "k3", "failed", "S1");
    let log = run(&log, "c2", "k3", "failed", "S2");
    let v = View::new(&log);
    assert_eq!(attempts(&v, "c1"), 1);
    assert_eq!(attempts(&v, "c2"), 0, "c1's commit charges nothing to c2");
    // a commit outside every anchor charges nobody
    let log = commit(&log, "k4", &["src/other.py"], false, true, &[]);
    let log = run(&log, "c1", "k4", "failed", "S1");
    let log = run(&log, "c2", "k4", "failed", "S2");
    let v = View::new(&log);
    assert_eq!(attempts(&v, "c1"), 1);
    assert_eq!(attempts(&v, "c2"), 0);
}

#[test]
fn tries_not_red_count_only_the_criterions_own_commits() {
    // c2's tests-first run passed: one try not red, no seal
    let log = tests_first(&two_heads(), "c2", "k1", "passed");
    assert_eq!(tries_not_red(&View::new(&log), "c2"), 1);
    // c1's code, and code outside every anchor, are no try of c2's
    let log = commit(&log, "k2", &["src/login.py"], false, true, &["c1"]);
    let log = commit(&log, "k3", &["src/other.py"], false, true, &[]);
    assert_eq!(tries_not_red(&View::new(&log), "c2"), 1);
    // c2's own code with no run yet is one
    let log = commit(&log, "k4", &["src/mail.py"], false, true, &["c2"]);
    assert_eq!(tries_not_red(&View::new(&log), "c2"), 2);
}

#[test]
fn the_referee_refuses_only_the_criterion_about_to_start() {
    let log = two_heads();
    let moved = ap(
        &log,
        "anchor_sha_matched",
        json!({ "criterion_id": "c1", "file_path": "src/login.py", "symbol": "check_password", "line_start": 40, "line_end": 62, "sha": "abc1234", "lines_hash": "LX", "matched": false, "checked_at": "start" }),
    );
    let v = View::new(&moved);
    assert_eq!(state(&v, "c1"), "held");
    assert!(!decide(&moved, &Event::AboutToStart { criterion_id: "c1".into() }, AT).allowed());
    assert!(decide(&moved, &Event::AboutToStart { criterion_id: "c2".into() }, AT).allowed(), "c1's moved anchor does not stop c2");
}

#[test]
fn owed_is_each_touching_commit_without_a_result_oldest_first() {
    let log = tests_commit(&two_heads(), "k1", "tests/test_c1.py", "S1");
    let v = View::new(&log);
    let rev = criterion_rev(&v, "c1");
    assert_eq!(owed(&v, "c1"), [Work { criterion_rev: rev.clone(), commit: "k1".into() }]);
    assert!(owed(&v, "c2").is_empty(), "c1's tests are not c2's");
    // a run that did not run is no result: still owed
    let log = result(&log, "c1", "tests/test_c1.py", "k1", "did_not_run");
    assert_eq!(owed(&View::new(&log), "c1").len(), 1);
    let log = result(&log, "c1", "tests/test_c1.py", "k1", "failed");
    // the touching test has not run at k1: still owed (every own and touching test runs there)
    assert_eq!(owed(&View::new(&log), "c1").len(), 1);
    let log = result(&log, "c1", "tests/test_login_old.py", "k1", "passed");
    assert!(owed(&View::new(&log), "c1").is_empty());
    // two commits on c1's anchor and one outside every anchor
    let log = commit(&log, "k2", &["src/login.py"], false, true, &["c1"]);
    let log = commit(&log, "k3", &["src/other.py"], false, true, &[]);
    let log = commit(&log, "k4", &["src/login.py"], false, true, &["c1"]);
    let v = View::new(&log);
    let ids: Vec<String> = owed(&v, "c1").into_iter().map(|w| w.commit).collect();
    assert_eq!(ids, ["k2", "k4"]);
    assert!(owed(&v, "c2").is_empty(), "a commit outside every anchor is nobody's");
    // a result at the newest: nothing before it is owed any more
    let log = result(&log, "c1", "tests/test_c1.py", "k4", "passed");
    let log = result(&log, "c1", "tests/test_login_old.py", "k4", "passed");
    assert!(owed(&View::new(&log), "c1").is_empty());
}

#[test]
fn a_run_that_did_not_run_is_no_result() {
    let log = both_sealed();
    let log = result(&log, "c1", "tests/test_c1.py", "k1", "did_not_run");
    assert_eq!(state(&View::new(&log), "c1"), "sealed", "did_not_run neither passes nor fails");
}
