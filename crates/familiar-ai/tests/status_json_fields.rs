//! `familiar-ai status --json` carries state names, counts, runs and each
//! card's choices and claim line, so every surface reads one picture. See
//! docs/decisions.md#one-source-for-the-tui

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "6c1b0e7a-2d4f-4b8e-9a31-c6b0c6b0c6b0";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("FAMILIAR_AI_RUN_INLINE_MS", "0").create()
}

impl Project {
    /// Claims the intent named by the refusal of a first edit.
    fn intent(&self) {
        self.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
        let path = self.path("src/app.sh").to_string_lossy().into_owned();
        let out = self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
        let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
        self.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
        // definition change (the owner's plan of 2026-10-21, step 2) [draft]: a short ask with no example
        // fires a trigger; the agent says it reads one way before the tests
        self.claim(&["one_reading", "--why", "the ask names what must change and how"]);
    }

    /// Claims one criterion's lines up to its string, on src/app.sh greet 1–3.
    fn criterion(&self, c: &str, text: &str) {
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", c, "--text", text, "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", c, "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        self.claim(&["test_planned", "--criterion_id", c, "--file_path", &format!("tests/test_{c}.sh"), "--will", "create"]);
    }

    fn stack(&self) {
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    }

    fn last_lines(&self, c: &str) {
        self.claim(&["criterion_assigned", "--criterion_id", c]);
        self.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
    }

    fn wait_for(&self, what: &str, within: Duration, found: impl Fn(&[Fact]) -> bool) {
        let until = Instant::now() + within;
        loop {
            let facts = self.facts();
            if found(&facts) {
                return;
            }
            assert!(Instant::now() < until, "gave up waiting for {what}; kinds so far: {:?}", facts.iter().map(|f| f.kind).collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn criterion<'a>(j: &'a Value, c: &str) -> &'a Value {
    j["criteria"].as_array().and_then(|a| a.iter().find(|x| x["id"] == c)).unwrap_or_else(|| panic!("{c} in {j}"))
}

fn has(facts: &[Fact], kind: &str) -> bool {
    facts.iter().any(|f| f.kind == kind)
}

/// c1's slow failing test runs while c2, strung on c1, waits; afterwards c1 is
/// sealed and the run reads failed.
#[test]
fn a_run_in_flight_is_counted_running_and_listed_until_it_ends() {
    let p = project("run");
    p.intent();
    p.criterion("c1", "greet says hello");
    p.stack();
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.last_lines("c1");
    p.criterion("c2", "greet says hello twice");
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    p.last_lines("c2");
    std::fs::create_dir_all(p.repo().join("tests")).expect("tests");
    // a red that shows its check (H-2, lane H5: a silent exit 1 is can't tell)
    std::fs::write(p.repo().join("tests/test_c1.sh"), "sleep 4\ngrep -q hello src/app.sh\n").expect("test");
    p.commit("c1: tests first");
    let commit7 = p.git(&["rev-parse", "--short=7", "HEAD"]);
    p.wait_for("the run to start", Duration::from_secs(10), |f| has(f, "run_started"));

    // in flight: status --json answers under 300 ms, best of three
    let mut best = Duration::from_secs(99);
    let mut j = Value::Null;
    for _ in 0..3 {
        let started = Instant::now();
        j = p.status_json();
        best = best.min(started.elapsed());
    }
    assert!(best < Duration::from_millis(300), "status --json took {best:?}");
    let c1 = criterion(&j, "c1");
    assert_eq!(c1["state_name"], "testing", "{j}");
    assert_eq!(c1["holds"], json!([]), "{j}");
    assert!(c1["state"].is_string(), "the words stay beside the name: {j}");
    let c2 = criterion(&j, "c2");
    assert_eq!(c2["state_name"], "waiting", "{j}");
    assert_eq!(c2["holds"], json!([]), "{j}");
    assert_eq!(j["counts"], json!({ "running": 1, "waiting": 1, "needs_you": 0 }), "{j}");
    let runs = j["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1, "{j}");
    let r = &runs[0];
    assert!(r["run_id"].as_str().is_some_and(|s| !s.is_empty()), "{r}");
    assert_eq!(r["criterion_id"], "c1", "{r}");
    assert_eq!(r["commit"], commit7.as_str(), "{r}");
    assert_eq!(r["in_flight"], true, "{r}");
    assert!(r["outcome"].is_null(), "{r}");

    // ended: not in flight, its outcome named, c1 sealed
    p.wait_for("the seal", Duration::from_secs(60), |f| has(f, "fingerprint_stamped"));
    let j = p.status_json();
    assert_eq!(criterion(&j, "c1")["state_name"], "sealed", "{j}");
    assert_eq!(j["counts"]["running"], 0, "{j}");
    let r = &j["runs"][0];
    assert_eq!(r["in_flight"], false, "{r}");
    assert_eq!(r["outcome"], "failed", "{r}");
    assert_eq!(r["commit"], commit7.as_str(), "{r}");
}

/// The anchored lines are edited before the plan's last line, so c1 holds on
/// anchor_moved and its card carries the choices and the claim line.
#[test]
fn a_held_criterion_names_its_hold_and_its_card_its_choices_and_claim() {
    let p = project("moved");
    p.intent();
    p.criterion("c1", "greet says hello");
    p.stack();
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("edit");
    p.last_lines("c1");
    let j = p.status_json();
    let c1 = criterion(&j, "c1");
    assert_eq!(c1["state_name"], "held", "{j}");
    assert_eq!(c1["holds"], json!(["anchor_moved"]), "{j}");
    assert_eq!(j["counts"], json!({ "running": 0, "waiting": 0, "needs_you": 1 }), "{j}");
    assert_eq!(j["runs"], json!([]), "{j}");
    let card = &j["cards"][0];
    assert_eq!(card["hold"], "anchor_moved", "{j}");
    assert_eq!(card["choices"], json!(["re_plan", "keep_going", "release"]), "{j}");
    assert_eq!(card["claim"], "familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice re_plan --typed ''", "{j}");
    assert!(card["question"].is_string(), "{j}");
}

/// With no test command claimed, c1 is unplanned and its card's one choice is
/// the typed command.
#[test]
fn the_no_test_stack_card_offers_the_typed_command() {
    let p = project("stack");
    p.intent();
    p.criterion("c1", "greet says hello");
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.last_lines("c1");
    let j = p.status_json();
    let c1 = criterion(&j, "c1");
    assert_eq!(c1["state_name"], "unplanned", "{j}");
    assert_eq!(c1["holds"], json!(["planning_incomplete", "no_test_stack"]), "{j}");
    assert_eq!(j["counts"]["needs_you"], 1, "{j}");
    let card = &j["cards"][0];
    assert_eq!(card["hold"], "no_test_stack", "{j}");
    assert_eq!(card["choices"], json!(["typed"]), "{j}");
    assert_eq!(card["claim"], "familiar-ai claim hold_answered --criterion_id c1 --hold no_test_stack --choice typed --typed ''", "{j}");
}
