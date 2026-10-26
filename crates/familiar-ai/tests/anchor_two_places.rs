//! An anchor of two places: each place is checked at the start and compared
//! with its own planning check (lane two-places, F7). The first edit of the
//! part writes a start check for each place and opens no card; an outside
//! edit of the second place is the card.

mod common;

use common::Project;
use serde_json::{json, Value};

const APP: &str = "#!/bin/sh\ngreet() {\n  echo hi\n}\n";
const BYE: &str = "#!/bin/sh\nfarewell() {\n  echo bye\n}\n";
const SESSION: &str = "8e3a0f4b-2d84-4fb1-9d7a-4c5d6e7f8093";
const MOVED: &str = "the code at the plan's place changed after the plan was made";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/app.sh", APP).file("src/bye.sh", BYE).create()
}

impl Project {
    /// c1 planned on greet and farewell (lines 2–4 of each file); `outside`
    /// edits src/bye.sh after the planning checks, then the plan is confirmed
    /// and the part's first edit lands.
    fn planned_then(&self, outside: Option<&str>) -> String {
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet says hello and farewell says goodbye"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello and farewell says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        let places = json!([
            { "file_path": "src/app.sh", "symbol": "greet", "line_start": 2, "line_end": 4 },
            { "file_path": "src/bye.sh", "symbol": "farewell", "line_start": 2, "line_end": 4 },
        ])
        .to_string();
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", &places]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        assert_eq!(self.checks("c1", "planning").len(), 2, "each place has its planning check before the outside edit");
        if let Some(outside) = outside {
            self.write("src/bye.sh", outside);
        }
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        // the part's first edit: its planned test
        let edit = self.payload(
            "PreToolUse",
            json!({ "tool_name": "Write", "tool_input": { "file_path": self.path("tests/test_c1.sh").to_string_lossy(), "content": ". src/app.sh; greet | grep -qx hello\n" } }),
        );
        let (code, _, err) = self.hook_raw("PreToolUse", &edit);
        assert!(code == 0 || code == 2, "{err}");
        self.status()
    }

    /// c1's checks made `when`, from the plan's export of the record.
    fn checks(&self, c: &str, when: &str) -> Vec<Value> {
        self.read(".familiar/plan")
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|f| f["kind"] == "anchor_sha_matched" && f["criterion_id"] == c && f["checked_at"] == when)
            .collect()
    }

    fn files_checked_at_start(&self) -> Vec<String> {
        let mut files: Vec<String> = self.checks("c1", "start").iter().filter_map(|f| f["file_path"].as_str().map(str::to_string)).collect();
        files.sort();
        files
    }
}

#[test]
fn the_first_edit_writes_a_start_check_for_each_place_and_no_card() {
    let p = project("both");
    let status = p.planned_then(None);
    assert_eq!(p.files_checked_at_start(), ["src/app.sh", "src/bye.sh"], "one start check per place");
    let c1 = p.criterion_json("c1");
    assert_eq!((c1["state_name"].as_str(), &c1["holds"]), (Some("testing"), &json!([])), "no anchor_moved, no card: {c1}");
    assert!(!status.contains(MOVED), "{status}");
}

#[test]
fn an_outside_edit_of_the_second_place_is_the_card() {
    let p = project("second-moved");
    let status = p.planned_then(Some("#!/bin/sh\nfarewell() {\n  echo goodbye\n}\n"));
    assert_eq!(p.files_checked_at_start(), ["src/app.sh", "src/bye.sh"], "one start check per place");
    let c1 = p.criterion_json("c1");
    assert_eq!((c1["state_name"].as_str(), &c1["holds"]), (Some("held"), &json!(["anchor_moved"])), "the second place moved: {c1}");
    assert!(status.contains(&format!("\"greet says hello and farewell says goodbye\" · {MOVED}")), "the card: {status}");
}
