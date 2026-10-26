//! The teeth run puts back the fix at the start check's commit and lines, not
//! the plan's, so code moved after planning is still undone. A helper repeating
//! greet's body sits above it, where a plan-to-head diff would pair wrongly.

mod common;

use std::time::Duration;

use common::Project;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};

const BASE: &str = "#!/bin/sh\ngreet() {\n  echo hi\n}\n";
const HELPER: &str = "#!/bin/sh\nwave() {\n  echo hi\n}\ngreet() {\n  echo hi\n}\n";
const FIXED: &str = "#!/bin/sh\nwave() {\n  echo hi\n}\ngreet() {\n  echo hello\n}\n";
const TEST: &str = ". src/app.sh; greet | grep -qx hello\n";

/// `src/app.sh` as BASE with the record ignored, committed as `base`, no setup.
fn project(name: &str) -> Project {
    Project::builder(name).no_app().no_setup().file(".gitignore", ".familiar/record\n").file("src/app.sh", BASE).create()
}

impl Project {
    fn hash(&self, commit: &str, line_start: u64, line_end: u64) -> String {
        familiar_ai_git::lines_hash(&self.repo(), commit, "src/app.sh", line_start, line_end).expect("lines hash")
    }

    /// greet's body planned at the base; the helper commit moves it down and
    /// the start check finds it there; the test committed alone, then the fix.
    fn moved_then_fixed(&self) {
        let base = self.head();
        let planned = self.hash(&base, 3, 3);
        self.write("src/app.sh", HELPER);
        let moved = self.land("a helper above greet");
        assert_eq!(self.hash(&moved, 6, 6), planned, "the helper only moved greet");
        let check = |sha: &str, line_start: u64, line_end: u64, at: &str| {
            json!({"criterion_id": "c1", "file_path": "src/app.sh", "symbol": "echo hi", "line_start": line_start, "line_end": line_end, "sha": sha, "lines_hash": planned, "matched": true, "checked_at": at})
        };
        let claims: Vec<(&str, Value)> = vec![
            ("intent_submitted", json!({"text": "greet says hello", "sha": base})),
            ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
            ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
            ("criterion", json!({"criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
            ("place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "echo hi", "line_start": 3, "line_end": 3}]})),
            ("anchor_sha_matched", check(&base, 3, 3, "planning")),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
            ("test_stack_detected", json!({"codebase_id": "fx", "project_root": "", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"})),
            ("criterion_head", json!({"criterion_id": "c1"})),
            ("criterion_assigned", json!({"criterion_id": "c1", "agent_conversation_id": "s1"})),
            ("fix_location_category", json!({"criterion_id": "c1", "category": "this_repository", "where": ""})),
            ("anchor_sha_matched", check(&moved, 6, 6, "start")),
            ("test_written", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": ""})),
        ];
        let claims: Vec<Claim> = claims
            .iter()
            .map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).unwrap_or_else(|e| panic!("{k}: {e}")))
            .collect();
        familiar_ai_plan::write(&self.path(FAMILIAR_FOLDER).join(PLAN_FILE), &claims).expect("plan");
        self.write("tests/test_c1.sh", TEST);
        self.land("c1: tests first");
        self.write("src/app.sh", FIXED);
        self.land("c1: greet says hello");
    }

    fn verify(&self) -> (i32, String) {
        let around = Surroundings {
            now: 1.0,
            project_dir: Some(self.repo()),
            cwd: Some(self.repo()),
            session_id: None,
            data_dir: None,
            record_wait: Duration::from_secs(1),
            test_run_wait: Duration::from_secs(20),
            run_inline: Duration::ZERO,
            runner: None,
            judge_dir: None,
            judge_wait: Duration::ZERO,
            investigator: None,
            conversation: None,
            bench: false,
        };
        let out = familiar_ai::run(["verify"], "", &around);
        (out.exit_code, format!("{}{}", out.stdout_text, out.stderr_text))
    }
}

#[test]
fn the_fix_inside_code_that_moved_after_planning_is_undone_and_proven_exit_0() {
    let r = project("moved");
    r.moved_then_fixed();
    let (exit, out) = r.verify();
    assert_eq!(exit, 0, "{out}");
    assert!(out.contains("\"greet says hello\" · proven · "), "{out}");
}
