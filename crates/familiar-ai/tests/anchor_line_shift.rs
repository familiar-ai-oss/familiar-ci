//! A pure shift of the anchor's lines raises no `anchor_moved` and tells the
//! agent where they now sit; a changed or deleted line is still the card. See
//! docs/decisions.md#the-anchor-is-a-whole-function-a-shift-is-not-a-move

mod common;

use common::Project;
use serde_json::{json, Value};

const BASE: &str = "#!/bin/sh\ngreet() {\n  echo hi\n}\n";
const SESSION: &str = "8e3a0f4b-2d84-4fb1-9d7a-4c5d6e7f8092";
const MOVED: &str = "the code at the plan's place changed after the plan was made";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/app.sh", BASE).create()
}

impl Project {
    /// c1 planned on greet (lines 2–4); `outside` edits src/app.sh before c1's
    /// start, then the plan is confirmed and the start check written.
    fn planned_then(&self, outside: &str) -> String {
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet says hello"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":4}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        assert!(self.checks("c1").iter().any(|f| f["checked_at"] == "planning"), "the planning check is written before the outside edit");
        self.write("src/app.sh", outside);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        self.status()
    }

    /// c1's checks, from the plan's export of the record.
    fn checks(&self, c: &str) -> Vec<Value> {
        let plan = self.read(".familiar/plan");
        plan.lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|f| f["kind"] == "anchor_sha_matched" && f["criterion_id"] == c)
            .collect()
    }

    /// (planning, start) checks of c.
    fn planning_and_start(&self, c: &str) -> (Value, Value) {
        let checks = self.checks(c);
        let at = |when: &str| checks.iter().rev().find(|f| f["checked_at"] == when).cloned().unwrap_or_else(|| panic!("no {when} check for {c}: {checks:?}"));
        (at("planning"), at("start"))
    }
}

#[test]
fn three_lines_added_above_before_the_start_are_a_pure_shift() {
    let p = project("above");
    let status = p.planned_then("#!/bin/sh\n# greet\n# says hello\n\ngreet() {\n  echo hi\n}\n");
    let (planning, start) = p.planning_and_start("c1");
    assert_eq!(start["matched"], true, "{start}");
    assert_eq!(start["lines_hash"], planning["lines_hash"], "the same bytes at a new place hash the same");
    assert_eq!((start["line_start"].as_u64(), start["line_end"].as_u64()), (Some(5), Some(7)), "the start check carries where the lines now sit");
    let c1 = p.criterion_json("c1");
    assert_eq!((c1["state_name"].as_str(), &c1["holds"]), (Some("testing"), &json!([])), "no anchor_moved, no card: {c1}");
    assert!(status.contains("\"greet says hello\" · the test comes first"), "{status}");
    assert!(status.contains("its lines now sit at src/app.sh 5–7"), "the agent is told where the lines now sit: {status}");
}

#[test]
fn a_line_edited_inside_before_the_start_is_still_the_card() {
    let p = project("inside");
    let status = p.planned_then("#!/bin/sh\ngreet() {\n  echo hello\n}\n");
    let (planning, start) = p.planning_and_start("c1");
    assert_ne!(start["lines_hash"], planning["lines_hash"], "{start}");
    assert!(status.contains(&format!("\"greet says hello\" · {MOVED}")), "the card, as today: {status}");
    let c1 = p.criterion_json("c1");
    assert_eq!((c1["state_name"].as_str(), &c1["holds"]), (Some("held"), &json!(["anchor_moved"])), "the card, as today: {c1}");
    assert!(!status.contains("now sit at"), "{status}");
}

#[test]
fn the_function_deleted_before_the_start_is_not_matched() {
    let p = project("deleted");
    let status = p.planned_then("#!/bin/sh\n");
    let (_, start) = p.planning_and_start("c1");
    assert_eq!((start["matched"].as_bool(), start["lines_hash"].as_str()), (Some(false), Some("")), "{start}");
    let c1 = p.criterion_json("c1");
    assert_eq!((c1["state_name"].as_str(), &c1["holds"]), (Some("held"), &json!(["anchor_moved"])), "the card, as today: {c1}");
    assert!(!status.contains("now sit at"), "{status}");
}
