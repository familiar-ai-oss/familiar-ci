//! `familiar-ai verify` on fixture repositories with a committed
//! `.familiar/plan` and no record: each case checks the exit code exactly
//! (0 proven, 1 not proven, 2 could not prove).

mod common;

use std::time::Duration;

use common::Project;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};

const FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
const TEST: &str = "grep -q hello src/app.sh\n";

/// `src/app.sh` with the record ignored, committed as `base`, no setup.
fn project(name: &str) -> Project {
    Project::builder(name).no_setup().file(".gitignore", ".familiar/record\n.familiar/verify-report.json\n").create()
}

impl Project {
    /// The `base` commit, the root of the history.
    fn base(&self) -> String {
        self.git(&["rev-list", "--max-parents=0", "HEAD"])
    }

    /// The plan's claims, written as the binary writes the plan file.
    fn plan(&self, claims: &[(&str, Value)]) {
        let claims: Vec<Claim> = claims
            .iter()
            .map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).unwrap_or_else(|e| panic!("{k}: {e}")))
            .collect();
        let path = self.path(FAMILIAR_FOLDER).join(PLAN_FILE);
        familiar_ai_plan::write(&path, &claims).expect("plan");
    }

    /// verify here, its exit code and what it printed.
    fn verify(&self, wait: Duration) -> (i32, String) {
        let out = familiar_ai::run(["verify"], "", &around(&self.repo(), wait));
        (out.exit_code, format!("{}{}", out.stdout_text, out.stderr_text))
    }
}

fn around(dir: &std::path::Path, wait: Duration) -> Surroundings {
    Surroundings {
        now: 1.0,
        project_dir: Some(dir.to_path_buf()),
        cwd: Some(dir.to_path_buf()),
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_secs(1),
        test_run_wait: wait,
        run_inline: Duration::ZERO,
        runner: None,
        judge_dir: None,
        judge_wait: Duration::ZERO,
        investigator: None,
        conversation: None,
        bench: false,
    }
}

/// The plan of one criterion on src/app.sh's lines, its test, its command.
fn claims(sha: &str, lines: (u64, u64), command: Option<&str>) -> Vec<(&'static str, Value)> {
    let mut out = vec![
        ("intent_submitted", json!({"text": "greet with hello", "sha": sha})),
        ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
        ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
        ("criterion", json!({"criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
        ("place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "greet", "line_start": lines.0, "line_end": lines.1}]})),
        ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
    ];
    if let Some(cmd) = command {
        out.push(("test_stack_detected", json!({"codebase_id": "fx", "project_root": "src/", "test_stack": "sh", "test_command": cmd})));
    }
    out.push(("criterion_head", json!({"criterion_id": "c1"})));
    out.push(("criterion_assigned", json!({"criterion_id": "c1", "agent_conversation_id": "s1"})));
    out.push(("fix_location_category", json!({"criterion_id": "c1", "category": "this_repository", "where": ""})));
    out.push(("test_written", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": ""})));
    out
}

/// The tests-first commit (the test and the plan, alone), then the fix.
fn tests_then_fix(r: &Project, test: &str, lines: (u64, u64), fixed: &str) {
    r.plan(&claims(&r.base(), lines, Some("sh tests/test_c1.sh")));
    r.write("tests/test_c1.sh", test);
    r.land("c1: tests first");
    r.write("src/app.sh", fixed);
    r.land("c1: fix");
}

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn a_correct_change_is_proven_exit_0_and_the_pipelines_facts_are_in_the_record() {
    let r = project("correct");
    tests_then_fix(&r, TEST, (1, 3), FIXED);
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 0, "{out}");
    assert_eq!(out.lines().count(), 1, "one line per criterion: {out}");
    assert!(out.starts_with("familiar-ai: \"greet says hello\" · proven · red at "), "{out}");
    assert!(out.contains("(exit 1) · green at ") && out.contains("fails with the change taken back out (exit 1) · ran: sh tests/test_c1.sh"), "{out}");
    let log = r.facts();
    assert!(log.iter().any(|f| f.kind == "pipeline_run_seen"));
    let pipeline = |k: &str| log.iter().filter(|f| f.kind == k && f.fields.get("ran_in") == Some(&json!("pipeline"))).count();
    assert_eq!((pipeline("pass_please_pass"), pipeline("fingerprint_stamped")), (3, 2), "red, green, teeth; two stamps");
    let v = familiar_ai_rules::View::new(&log);
    assert_eq!(familiar_ai_rules::action::check_state(&v), "passed");
}

#[test]
fn a_test_that_never_failed_at_its_tests_first_commit_is_not_proven_exit_1() {
    let r = project("never-red");
    tests_then_fix(&r, "grep -q greet src/app.sh\n", (1, 3), FIXED);
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 1, "{out}");
    assert!(out.contains("not proven: its tests passed at the tests-first commit") && out.contains("exit 0"), "{out}");
}

#[test]
fn a_sealed_test_changed_after_the_seal_is_not_proven_exit_1() {
    let r = project("tampered");
    tests_then_fix(&r, TEST, (1, 3), FIXED);
    r.write("tests/test_c1.sh", "grep -q hel src/app.sh\n");
    r.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello there; }\ngreet\n");
    r.land("c1: the test eased, with code");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 1, "{out}");
    assert!(out.contains("its sealed tests were changed after the tests-first commit"), "{out}");
}

#[test]
fn a_test_that_still_passes_with_the_anchor_put_back_has_no_teeth_exit_1() {
    let r = project("no-teeth");
    // the fix changes the anchor (line 2) and, apart from it, adds the word below
    tests_then_fix(&r, TEST, (2, 2), "#!/bin/sh\ngreet() { echo hullo; }\ngreet\n# hello\n");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 1, "{out}");
    assert!(out.contains("its tests still pass with the change taken back out") && out.contains("exit 0"), "{out}");
}

#[test]
fn no_plan_committed_could_not_prove_anything_exit_2() {
    let r = project("no-plan");
    r.write("tests/test_c1.sh", TEST);
    r.land("tests");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.starts_with("familiar-ai: verify could not prove anything: no .familiar/plan is committed at "), "{out}");
}

#[test]
fn an_empty_plan_and_a_plan_with_no_test_command_are_exit_2_never_proven() {
    let r = project("empty-plan");
    r.plan(&[]);
    r.land("empty plan");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("is empty"), "{out}");

    let r = project("no-command");
    r.plan(&claims(&r.base(), (1, 3), None));
    r.write("tests/test_c1.sh", TEST);
    r.land("c1: tests first");
    r.write("src/app.sh", FIXED);
    r.land("c1: fix");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: the plan committed at ") && out.contains("claims no test command"), "{out}");
}

#[test]
fn a_history_that_does_not_reach_the_intent_is_exit_2() {
    let r = project("no-history");
    r.plan(&claims("1111111111111111111111111111111111111111", (1, 3), Some("sh tests/test_c1.sh")));
    r.write("tests/test_c1.sh", TEST);
    r.land("c1: tests first");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("the history does not reach the intent's commit") && out.contains("fetch-depth: 0"), "{out}");
}

#[test]
fn a_test_command_that_times_out_at_the_tests_first_commit_is_not_red_exit_2() {
    let r = project("timeout");
    tests_then_fix(&r, "grep -q hello src/app.sh || sleep 30\n", (1, 3), FIXED);
    let (exit, out) = r.verify(Duration::from_secs(1));
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: at the tests-first commit") && out.contains("still running at the time limit (1 s)"), "{out}");
    assert!(out.contains("not a red test"), "{out}");
    let log = r.facts();
    assert!(!log.iter().any(|f| f.kind == "pass_please_pass"), "no red written for a run that did not happen");
}

#[test]
fn a_teeth_run_that_times_out_is_not_a_failing_test_exit_2() {
    let r = project("teeth-timeout");
    // red at the tests-first commit, green at the head; with line 2 put back it hangs
    let test = "if grep -q 'echo hi' src/app.sh && grep -q '# hello' src/app.sh; then sleep 30; fi\ngrep -q hello src/app.sh\n";
    tests_then_fix(&r, test, (2, 2), "#!/bin/sh\ngreet() { echo hullo; }\ngreet\n# hello\n");
    let (exit, out) = r.verify(Duration::from_secs(1));
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: with the change taken back out at") && out.contains("not a failing test"), "{out}");
}

#[test]
fn tests_never_committed_alone_before_the_fix_is_no_tests_first_commit_exit_2() {
    let r = project("no-tests-first");
    r.plan(&claims(&r.base(), (1, 3), Some("sh tests/test_c1.sh")));
    r.write("tests/test_c1.sh", TEST);
    r.write("src/app.sh", FIXED);
    r.land("c1: tests and fix together");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: no tests-first commit") && out.contains("fetch-depth: 0"), "{out}");
}

#[test]
fn a_command_missing_at_the_tests_first_commit_is_not_red_exit_2() {
    let r = project("missing-command");
    // the command's program arrives with the fix: at the tests-first commit it is not found
    r.plan(&claims(&r.base(), (1, 3), Some("PATH=\"$PWD/tools:$PATH\" fxcheck")));
    r.write("tests/test_c1.sh", TEST);
    r.land("c1: tests first");
    r.write("src/app.sh", FIXED);
    r.write("tools/fxcheck", "#!/bin/sh\nexec sh tests/test_c1.sh\n");
    r.git(&["update-index", "--add", "--chmod=+x", "tools/fxcheck"]);
    r.land("c1: fix, and the program the command runs");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: at the tests-first commit") && out.contains("not found (exit 127)") && out.contains("not a red test"), "{out}");
}

#[test]
fn a_signal_at_the_tests_first_commit_is_not_red_exit_2() {
    let r = project("signal");
    // before the fix the test is killed (as an out-of-memory kill would be), not failed
    tests_then_fix(&r, "grep -q hello src/app.sh && exit 0\nkill -9 $$\n", (1, 3), FIXED);
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(out.contains("could not be proven: at the tests-first commit") && out.contains("ended by") && out.contains("signal") && out.contains("not a red test"), "{out}");
}

#[test]
fn a_criterion_added_after_the_first_was_proven_is_not_completed_by_it_exit_1() {
    let r = project("second-ask");
    tests_then_fix(&r, TEST, (1, 3), FIXED);
    let s1 = r.git(&["rev-parse", "HEAD"]);
    let (exit, first) = r.verify(WAIT);
    assert_eq!(exit, 0, "{first}");
    assert!(first.contains(&format!("green at {}", &s1[..7])), "the line names the commit it checked: {first}");
    std::fs::remove_file(r.record_path()).ok();
    // a second criterion B in its own part of the code, its test committed
    // first; no fix for it yet
    r.write("lib/b.sh", "echo b\n");
    r.land("lib");
    let mut plan = claims(&r.base(), (1, 3), Some("sh tests/test_c1.sh"));
    plan.extend([
        ("test_stack_detected", json!({"codebase_id": "fx-lib", "project_root": "lib/", "test_stack": "sh", "test_command": "sh tests/test_c2.sh"})),
        ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
        ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
        ("criterion", json!({"criterion_id": "c2", "text": "b says bee", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
        ("place_chosen", json!({"criterion_id": "c2", "places": [{"file_path": "lib/b.sh", "symbol": "b", "line_start": 1, "line_end": 1}]})),
        ("test_planned", json!({"criterion_id": "c2", "file_path": "tests/test_c2.sh", "will": "create"})),
        ("criterion_head", json!({"criterion_id": "c2"})),
        ("criterion_assigned", json!({"criterion_id": "c2", "agent_conversation_id": "s1"})),
        ("fix_location_category", json!({"criterion_id": "c2", "category": "this_repository", "where": ""})),
        ("test_written", json!({"criterion_id": "c2", "file_path": "tests/test_c2.sh", "test_name": ""})),
    ]);
    r.plan(&plan);
    r.write("tests/test_c2.sh", "grep -q bee lib/b.sh\n");
    let head = r.land("c2: tests first");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 1, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "one line per criterion: {out}");
    assert!(lines[0].contains("\"greet says hello\" · proven · red at ") && lines[0].contains(&format!("green at {}", &s1[..7])), "A stays proven, at its own fix, not the later head: {out}");
    assert!(lines[1].contains("\"b says bee\" · not proven") && lines[1].contains(&head[..7]), "B is not completed by A's proof: {out}");
}

/// A release written by hand into the committed plan, with no hold ever
/// raised, does not end a part that has no tests and no fix: the committed
/// plan holds claims, not outcomes, and holds are the person's
/// (docs/decisions.md). The plan carries no `hold_answered` (review of lane
/// s2-facts, S2; whether CI reads the person's answers is the owner's word).
#[test]
fn a_release_written_by_hand_into_the_committed_plan_is_never_a_pass_exit_2() {
    let r = project("hand-release");
    let plan: Vec<_> = claims(&r.base(), (1, 3), Some("sh tests/test_c1.sh")).into_iter().filter(|(k, _)| *k != "test_written").collect();
    r.plan(&plan);
    let path = r.path(FAMILIAR_FOLDER).join(PLAN_FILE);
    let mut text = std::fs::read_to_string(&path).expect("the plan");
    text.push_str("{\"kind\":\"hold_answered\",\"criterion_id\":\"c1\",\"hold\":\"anchor_moved\",\"choice\":\"release\",\"typed\":\"\"}\n");
    std::fs::write(&path, text).expect("write the plan");
    r.land("c1: the plan, and a release by hand");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 2, "{out}");
    assert!(!out.contains("released by you"), "{out}");
    assert!(out.contains("cannot be read: plan: line 11: kind 'hold_answered' is refused in a plan (claims only)"), "{out}");
}
