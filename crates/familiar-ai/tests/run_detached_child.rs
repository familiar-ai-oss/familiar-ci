//! A test run is reserved by the hook and runs detached, so other processes
//! answer while it is in flight, the child writes its own result, and a
//! cancelled run is not recorded as a pass.

mod common;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

/// Plans criterion c1 with `test_body` and `command`; the tests-first commit
/// is not made yet.
fn planned(name: &str, command: &str, test_body: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_RUN_INLINE_MS", "0").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let reason = denied(&p.edit("src/app.sh", "echo hi", "echo hello"));
    let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", command]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", test_body);
    p
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    /// Makes the tests-first commit and reports it to the hook as Claude Code
    /// does; how long the hook took.
    fn timed_commit(&self, message: &str) -> Duration {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        let started = Instant::now();
        self.bash("PostToolUse", &format!("git commit -m '{message}'"));
        started.elapsed()
    }

    fn wait_for(&self, what: &str, within: Duration, found: impl Fn(&[Fact]) -> bool) -> Vec<Fact> {
        let until = Instant::now() + within;
        loop {
            let facts = self.facts();
            if found(&facts) {
                return facts;
            }
            assert!(Instant::now() < until, "gave up waiting for {what}; kinds so far: {:?}", facts.iter().map(|f| f.kind).collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

fn of<'a>(facts: &'a [Fact], kind: &str) -> Vec<&'a Fact> {
    facts.iter().filter(|f| f.kind == kind).collect()
}

fn text<'a>(f: &'a Fact, key: &str) -> &'a str {
    f.fields.get(key).and_then(Value::as_str).unwrap_or("")
}

fn group_alive(pgid: &str) -> bool {
    Command::new("kill").args(["-0", "--", &format!("-{pgid}")]).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

#[test]
fn a_long_run_leaves_status_answering_and_lands_its_own_result() {
    let p = planned("long", "sh tests/test_c1.sh", "sleep 25\nexit 0\n");
    let hook_took = p.timed_commit("c1: tests first");
    assert!(hook_took < Duration::from_secs(10), "the hook returns while the run is in flight: {hook_took:?}");

    let facts = p.wait_for("the run to start", Duration::from_secs(10), |f| !of(f, "run_started").is_empty());
    assert_eq!(of(&facts, "run_reserved").len(), 1, "one reservation for the commit");
    assert!(of(&facts, "pass_please_pass").is_empty(), "the 25 s run is still going");

    // a second process answers at once while the run is in flight
    let mut best = Duration::from_secs(99);
    for _ in 0..3 {
        let started = Instant::now();
        let (code, out, err) = p.run(&["status"], "");
        let took = started.elapsed();
        assert_eq!(code, 0, "{out}{err}");
        best = best.min(took);
    }
    assert!(best < Duration::from_millis(300), "status took {best:?} while a run was in flight");

    // no hook call from here: the detached child appends the result itself
    let facts = p.wait_for("the result", Duration::from_secs(60), |f| !of(f, "fingerprint_stamped").is_empty());
    let result = of(&facts, "pass_please_pass");
    assert!(!result.is_empty() && result.iter().all(|r| text(r, "outcome") == "passed"), "{result:?}");
    let pos = |kind: &str| facts.iter().position(|f| f.kind == kind).unwrap_or(usize::MAX);
    assert!(pos("run_reserved") < pos("run_started") && pos("run_started") < pos("run_ended") && pos("run_ended") < pos("pass_please_pass"));
    assert_eq!(text(of(&facts, "run_ended")[0], "outcome"), "passed");
}

#[test]
fn cancel_stops_the_run_and_manufactures_no_proof() {
    let p = planned("cancel", "sh tests/test_c1.sh", "sleep 60\nexit 0\n");
    p.timed_commit("c1: tests first");
    let facts = p.wait_for("the run to start", Duration::from_secs(10), |f| !of(f, "run_started").is_empty());
    let started = of(&facts, "run_started")[0];
    let (run_id, pgid) = (text(started, "run_id").to_string(), text(started, "pgid").to_string());
    assert!(group_alive(&pgid), "the command is running in its own group");

    let (code, out, err) = p.run(&["cancel", "--run_id", &run_id], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(!group_alive(&pgid), "the process group is gone when cancel returns");

    let facts = p.wait_for("the run to end", Duration::from_secs(10), |f| !of(f, "run_ended").is_empty());
    let ended = of(&facts, "run_ended");
    assert_eq!((ended.len(), text(ended[0], "end")), (1, "cancelled"), "{ended:?}");
    assert_eq!(of(&facts, "run_cancel_requested").len(), 1);
    std::thread::sleep(Duration::from_secs(2));
    let facts = p.facts();
    assert!(of(&facts, "pass_please_pass").is_empty(), "a cancelled run is no result");
    assert!(of(&facts, "fingerprint_stamped").is_empty(), "and no stamp");
    assert_eq!(of(&facts, "run_reserved").len(), 1, "and it is not run again by itself");

    let (code, out, err) = p.run(&["cancel", "--run_id", &run_id], "");
    assert_ne!(code, 0);
    assert!(format!("{out}{err}").contains("already finished"), "{out}{err}");
}

#[test]
fn a_command_that_cannot_run_is_recorded_as_such_and_not_tried_again() {
    let p = planned("cannot", "no-such-command-familiar-xyz", "exit 0\n");
    p.timed_commit("c1: tests first");
    let facts = p.wait_for("the run to end", Duration::from_secs(20), |f| !of(f, "run_ended").is_empty());
    assert_eq!(text(of(&facts, "run_ended")[0], "outcome"), "did_not_run");
    let facts = p.wait_for("the result", Duration::from_secs(10), |f| !of(f, "pass_please_pass").is_empty());
    // plan §6.1 (S6): the shell ran and said the command is missing: `failed`, read as environment_error
    assert!(of(&facts, "pass_please_pass").iter().all(|r| text(r, "outcome") == "failed"));
    assert!(of(&facts, "test_log_translation").iter().all(|t| text(t, "label") == "environment_error"));

    // later door calls do not run it again
    let _ = p.run(&["status"], "");
    p.hook("Stop", json!({}));
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(of(&p.facts(), "run_reserved").len(), 1, "stalled, not retried");
}
