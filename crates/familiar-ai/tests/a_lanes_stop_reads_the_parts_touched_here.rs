//! A lane's stop reads the parts this checkout touched (the owner, 2026-10-15; plan §6.8): in a
//! worktree seeded from main's committed plan, an inherited part the lane never touched never
//! keeps it from stopping, while main's stop reads every part; a part the lane claims does keep
//! it. The lane's commits on an inherited part's anchor raise no card on that part.

mod common;

use std::path::{Path, PathBuf};

use common::Project;
use serde_json::{json, Value};

const MAIN: &str = "5d1c2b3a-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const LANE: &str = "7e2d3c4b-5f6a-4b7c-9d8e-0f1a2b3c4d5e";
const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\nbye() { echo bye; }\n";
const WAVE: &str = "#!/bin/sh\nwave() { echo bye; }\n";
const GREET: &str = "#!/bin/sh\ngreet() { echo hello; }\nbye() { echo bye; }\n";

impl Project {
    fn claim_at(&self, at: &Path, args: &[&str]) -> String {
        let mut argv = vec!["claim"];
        argv.extend_from_slice(args);
        let (code, out, err) = self.run_in(at, &argv, "");
        assert_eq!(code, 0, "claim {args:?}: {out}{err}");
        out
    }

    fn hook_at(&self, at: &Path, session: &str, event: &str, fields: Value) -> Value {
        let mut payload = json!({ "hook_event_name": event, "session_id": session, "cwd": at.to_string_lossy() });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        let (code, out, err) = self.run_in(at, &["hook", event], &payload.to_string());
        assert_eq!(code, 0, "{event}: {err}");
        serde_json::from_str(out.trim()).unwrap_or(Value::Null)
    }

    /// The agent's commit at `at` as `session`: everything added, then the hook after the call.
    fn commit_at(&self, at: &Path, session: &str, message: &str) -> Value {
        self.git_in(at, &["add", "-A"]);
        self.git_in(at, &["commit", "-q", "-m", message]);
        self.hook_at(at, session, "PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {} }))
    }

    fn json_at(&self, at: &Path) -> Value {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).expect("status --json")
    }

    /// Whether a Stop at `at` as `session` is refused.
    fn stop_refused(&self, at: &Path, session: &str) -> bool {
        self.hook_at(at, session, "Stop", json!({}))["decision"] == "block"
    }

    /// A fresh worktree of HEAD on branch `lane`, set up there: seeded from the committed plan.
    fn seeded_lane(&self) -> PathBuf {
        let wt = self.dir.join("lane");
        self.git(&["worktree", "add", "-q", "-b", "lane", &wt.to_string_lossy()]);
        let (code, out, err) = self.run_in(&wt, &["setup"], "");
        assert_eq!(code, 0, "setup: {out}{err}");
        assert!(out.contains("starts from the committed plan: 3 parts"), "{out}");
        // the plan file carries no finding and no answer of the cases card, so the seeded record
        // owes it again on the parts not started: passed here as in main (plan cases-card,
        // 2026-10-25) [draft names]
        for c in ["c2", "c3"] {
            common::cases_kept_at(&wt, c);
        }
        wt
    }
}

/// main: a plan of three parts, c1 on greet green, c2 on bye and c3 on wave strung after it, not started,
/// everything committed with the plan.
fn main_with_three_parts(name: &str) -> Project {
    let p = Project::builder(name).session(MAIN).no_app().file("src/app.sh", APP).file("src/wave.sh", WAVE).env("FAMILIAR_AI_BENCH", "0").create();
    let repo = p.repo();
    p.hook_at(&repo, MAIN, "UserPromptSubmit", json!({ "prompt": "greet with hello and say goodbye" }));
    let said = p.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded").to_string();
    for args in [
        vec!["intent_submitted", "--for", &said, "--text", "greet with hello and say goodbye"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["criterion", "--criterion_id", "c2", "--text", "bye says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#],
        vec!["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/app.sh","symbol":"bye","line_start":3,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion", "--criterion_id", "c3", "--text", "wave waves", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c3", "--places", r#"[{"file_path":"src/wave.sh","symbol":"wave","line_start":2,"line_end":2}]"#],
        vec!["test_planned", "--criterion_id", "c3", "--file_path", "tests/test_c3.sh", "--will", "create"],
        vec!["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"],
        vec!["criterion_strung_on_head", "--criterion_id", "c3", "--head", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c2"],
        vec!["criterion_assigned", "--criterion_id", "c3"],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
        vec!["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""],
        vec!["fix_location_category", "--criterion_id", "c3", "--category", "this_repository", "--where", ""],
    ] {
        p.claim_at(&repo, &args);
    }
    for c in ["c1", "c2", "c3"] {
        p.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    }
    p.write("tests/test_c1.sh", ". src/app.sh; greet | grep -qx hello\n");
    p.commit_at(&repo, MAIN, "c1: tests first");
    p.write("src/app.sh", GREET);
    p.commit_at(&repo, MAIN, "c1: greet says hello");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    p.commit_at(&repo, MAIN, "c1: the plan");
    p
}

fn state_at(p: &Project, at: &Path, c: &str) -> (String, Value) {
    let j = p.json_at(at);
    let part = j["criteria"].as_array().and_then(|a| a.iter().find(|x| x["id"] == c).cloned()).unwrap_or(Value::Null);
    (part["state_name"].as_str().unwrap_or("").to_string(), part["holds"].clone())
}

#[test]
fn a_lanes_stop_ignores_an_untouched_inherited_part_and_mains_reads_every_part() {
    let p = main_with_three_parts("stop");
    let wt = p.seeded_lane();
    assert_eq!(state_at(&p, &wt, "c1").0, "proven", "c1 inherited, verified");
    assert_eq!(state_at(&p, &wt, "c2").0, "testing", "c2 inherited, not started");
    assert!(p.stop_refused(&p.repo(), MAIN), "main reads every part: c2's test comes first");
    assert!(!p.stop_refused(&wt, LANE), "the lane touched neither part");
    // the lane claims c2: now its stop reads c2
    p.claim_at(&wt, &["criterion_assigned", "--criterion_id", "c2", "--agent_conversation_id", LANE]);
    assert!(p.stop_refused(&wt, LANE), "c2 is the lane's now");
}

#[test]
fn the_lanes_commits_on_an_inherited_parts_anchor_raise_no_card_on_it() {
    let p = main_with_three_parts("shared");
    let wt = p.seeded_lane();
    p.claim_at(&wt, &["criterion_assigned", "--criterion_id", "c2", "--agent_conversation_id", LANE]);
    // two commits on greet, c1's anchor, keeping its test passing
    for (i, text) in ["#!/bin/sh\ngreet() { echo hello; } # 1\nbye() { echo bye; }\n", "#!/bin/sh\ngreet() { echo hello; } # 2\nbye() { echo bye; }\n"].iter().enumerate() {
        std::fs::write(wt.join("src/app.sh"), text).expect("write");
        p.commit_at(&wt, LANE, &format!("lane: greet tidied {i}"));
    }
    let j = p.json_at(&wt);
    assert!(j["cards"].as_array().is_some_and(|c| c.iter().all(|k| k["criterion_id"] != "c1")), "no card on c1: {}", j["cards"]);
    assert_eq!(state_at(&p, &wt, "c1").1, json!([]), "c1 holds nothing");
}
