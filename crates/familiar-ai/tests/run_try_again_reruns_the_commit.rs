//! The try_again re-run (plan §6.7, behaviour 2; lane S5): the person's `try_again` on
//! `environment_error` runs the same commit again with a new run id, once per answer, and the
//! re-run's result is what the rules read for that commit afterwards. The hold is raised in S7; the
//! answer is appended raw, as the person's pick is recorded.

mod common;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::Project;
use familiar_ai_facts::{Fact, Log, Record};
use familiar_ai_rules::criterion::{results, Place};
use familiar_ai_rules::lifecycle::ran_at;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const SESSION: &str = "5d2e8c41-7a3b-4f90-b1c6-2e9d0f4a7b13";
const TEST: &str = "tests/test_c1.sh";

/// A project whose binary sees an empty folder first on its PATH: the tool the test calls is
/// installed there later, by the person.
fn project(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
    let bin = b.dir().join("bin");
    std::fs::create_dir_all(&bin).expect("bin folder");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    b.env("PATH", path).create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    /// The person installs the tool the test calls; it runs and fails, so the test is red.
    fn install_the_tool(&self) {
        let tool = self.dir.join("bin").join("greettool");
        std::fs::write(&tool, "#!/bin/sh\necho hi\nexit 1\n").expect("write the tool");
        assert!(std::process::Command::new("chmod").arg("+x").arg(&tool).status().expect("chmod").success());
    }

    /// The person's `try_again` on `environment_error` for c1, as the pick is recorded; its `t`.
    fn try_again(&self) -> u64 {
        let record = Record::open_waiting(&self.record_path(), Duration::from_secs(10)).expect("the record");
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs_f64();
        let fields = json!({ "criterion_id": "c1", "hold": "environment_error", "choice": "try_again", "typed": "" });
        let Value::Object(fields) = fields else { unreachable!() };
        record.append_raw("hold_answered", fields, now, "_default_").expect("the answer").t
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

fn text(f: &Fact, k: &str) -> String {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The runs reserved for c1 at `commit`, in order.
fn runs_at(log: &Log, commit: &str) -> Vec<String> {
    log.iter().filter(|f| f.kind == "run_reserved" && text(f, "commit") == commit && text(f, "criterion_id") == "c1").map(|f| text(f, "run_id")).collect()
}

/// c1's laptop results for the test at `commit` written after `t`.
fn results_after<'a>(log: &'a Log, commit: &str, t: u64) -> Vec<&'a Fact> {
    log.iter()
        .filter(|f| f.kind == "pass_please_pass" && f.t > t && text(f, "commit") == commit && text(f, "criterion_id") == "c1" && text(f, "file_path") == TEST && text(f, "ran_in") == "laptop")
        .collect()
}

/// The record once `ready` holds, waiting at most a minute for the detached run and its chain.
fn waited(p: &Project, what: &str, ready: impl Fn(&Log) -> bool) -> Log {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        let log = p.facts();
        if ready(&log) {
            return log;
        }
        assert!(Instant::now() < until, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// What the rules read for c1's test at `commit`: the latest laptop result there, and the result
/// behind the latest laptop stamp there.
fn read_at(log: &Log, commit: &str) -> (String, String) {
    let v = View::new(log);
    let laptop = results(&v, "c1", Place::Laptop, false);
    let latest = laptop.iter().rev().find(|r| text(r, "commit") == commit && text(r, "file_path") == TEST).map(|r| text(r, "run_id")).unwrap_or_default();
    let stamp = log.iter().rev().find(|f| f.kind == "fingerprint_stamped" && text(f, "criterion_id") == "c1" && text(f, "commit") == commit && text(f, "ran_in") == "laptop");
    let behind = stamp.and_then(|s| ran_at(&laptop, s, TEST)).map(|r| text(r, "run_id")).unwrap_or_default();
    (latest, behind)
}

#[test]
fn a_try_again_on_environment_error_runs_the_commit_again_once_per_answer() {
    let p = project("rerun");
    p.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
    let reason = denied(&p.edit("src/a.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "a says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST, "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "false"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    // the test calls a tool the machine does not have yet: the shell's exit 127
    p.write(TEST, "greettool\n");
    p.commit("c1: tests first");
    let commit = p.head();
    let log = waited(&p, "the first run's result", |log| !results_after(log, &commit, 0).is_empty());
    let first = runs_at(&log, &commit);
    assert_eq!(first.len(), 1, "one run at the commit: {first:?}");
    let r0 = first[0].clone();
    let got = results_after(&log, &commit, 0);
    assert_eq!((text(got[0], "outcome"), text(got[0], "run_id")), ("failed".to_string(), r0.clone()), "the first run ran and did not pass (plan §6.1, S6)");

    // no answer: nothing runs again
    p.run(&["run"], "");
    assert_eq!(runs_at(&p.facts(), &commit), [r0.as_str()], "no answer, no re-run");

    // the person installs the tool and answers try_again: the commit runs again, with a new run id
    p.install_the_tool();
    let a1 = p.try_again();
    p.run(&["run"], "");
    let log = waited(&p, "the re-run's result", |log| !results_after(log, &commit, a1).is_empty());
    let runs = runs_at(&log, &commit);
    assert_eq!(runs.len(), 2, "one re-run: {runs:?}");
    let r1 = runs[1].clone();
    assert_ne!(r1, r0, "the re-run has its own run id");
    let got = results_after(&log, &commit, a1);
    assert_eq!(got.len(), 1, "one result for the re-run: {got:?}");
    assert_eq!((text(got[0], "outcome"), text(got[0], "run_id")), ("failed".to_string(), r1.clone()), "the re-run ran the tests; its result names it, not the failing run");
    let log = waited(&p, "the commit stamped after the re-run", |log| read_at(log, &commit).1 == r1);
    assert_eq!(read_at(&log, &commit), (r1.clone(), r1.clone()), "results and ran_at read the re-run's result for the commit");

    // once per answer
    p.run(&["run"], "");
    assert_eq!(runs_at(&p.facts(), &commit).len(), 2, "the answer is spent: no third run");

    // a second answer: one more re-run
    let a2 = p.try_again();
    p.run(&["run"], "");
    let log = waited(&p, "the second re-run's result", |log| !results_after(log, &commit, a2).is_empty());
    let runs = runs_at(&log, &commit);
    assert_eq!(runs.len(), 3, "one more re-run: {runs:?}");
    let r2 = runs[2].clone();
    assert!(r2 != r0 && r2 != r1, "its own run id again: {runs:?}");
    let got = results_after(&log, &commit, a2);
    assert_eq!((got.len(), text(got[0], "run_id")), (1, r2.clone()), "the second re-run's result names it, not the earlier re-run");
    let log = waited(&p, "the commit stamped after the second re-run", |log| read_at(log, &commit).1 == r2);
    assert_eq!(read_at(&log, &commit), (r2.clone(), r2.clone()));
    p.run(&["run"], "");
    assert_eq!(runs_at(&p.facts(), &commit).len(), 3, "the second answer is spent too");
}
