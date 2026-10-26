//! On new work, `verify`'s teeth run puts back the whole file the integration
//! point or new-work spot names, as it was at the plan's sha, and the tests
//! must fail.

mod common;

use std::time::Duration;

use common::Project;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};

/// `src/app.sh` with greet and the record ignored, committed as `base`, no setup.
fn project(name: &str) -> Project {
    Project::builder(name).no_app().no_setup().file(".gitignore", ".familiar/record\n").file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\n").create()
}

impl Project {
    /// Plans one criterion anchored by `exit`, commits its test alone, then
    /// the fix (`fix`: path, text).
    fn tests_then_fix(&self, exit: (&str, Value), test: &str, fix: &[(&str, &str)]) {
        let base = self.head();
        let claims: Vec<(&str, Value)> = vec![
            ("intent_submitted", json!({"text": "say goodbye", "sha": base})),
            ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
            ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
            ("criterion", json!({"criterion_id": "c1", "text": "farewell says goodbye", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
            exit,
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
            ("test_stack_detected", json!({"codebase_id": "fx", "project_root": "", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"})),
            ("criterion_head", json!({"criterion_id": "c1"})),
            ("criterion_assigned", json!({"criterion_id": "c1", "agent_conversation_id": "s1"})),
            ("fix_location_category", json!({"criterion_id": "c1", "category": "this_repository", "where": ""})),
            ("test_written", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": ""})),
        ];
        let claims: Vec<Claim> = claims
            .iter()
            .map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).unwrap_or_else(|e| panic!("{k}: {e}")))
            .collect();
        familiar_ai_plan::write(&self.path(FAMILIAR_FOLDER).join(PLAN_FILE), &claims).expect("plan");
        self.write("tests/test_c1.sh", test);
        self.land("c1: tests first");
        for (path, text) in fix {
            self.write(path, text);
        }
        self.land("c1: the new work");
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

const FAREWELL: &str = "#!/bin/sh\nfarewell() { echo goodbye; }\n";
/// S9d: CI classifies its runs, so the red is a plain `exit 1` with nothing on stderr (the sh
/// runner's tests_failed), not the shell's complaint about a missing file.
const TEST: &str = "[ -f src/farewell.sh ] || exit 1\n. src/farewell.sh; farewell | grep -qx goodbye\n";

#[test]
fn greenfield_whose_test_fails_with_the_integration_file_put_back_is_proven_exit_0() {
    let r = project("gf-teeth");
    r.tests_then_fix(("new_work_no_spot", json!({"criterion_id": "c1", "integration_point": "src/farewell.sh:farewell"})), TEST, &[("src/farewell.sh", FAREWELL)]);
    let (exit, out) = r.verify();
    assert_eq!(exit, 0, "{out}");
    assert!(out.contains("· proven · ") && out.contains("fails with the integration point put back (exit 1)"), "{out}");
}

#[test]
fn greenfield_whose_test_passes_with_the_integration_file_put_back_has_no_teeth_exit_1() {
    let r = project("gf-no-teeth");
    // the integration point names the caller, which the new work leaves as it was
    r.tests_then_fix(("new_work_no_spot", json!({"criterion_id": "c1", "integration_point": "src/app.sh:greet"})), TEST, &[("src/farewell.sh", FAREWELL)]);
    let (exit, out) = r.verify();
    assert_eq!(exit, 1, "{out}");
    assert!(out.contains("its tests still pass with the integration point put back"), "{out}");
}

#[test]
fn a_new_work_spot_whose_test_passes_with_its_file_put_back_has_no_teeth_exit_1() {
    let r = project("spot-no-teeth");
    // the spot is src/app.sh; the work lands there and in a file of its own the test reads
    r.tests_then_fix(
        ("new_work_spot_chosen", json!({"criterion_id": "c1", "file_path": "src/app.sh", "symbol": "farewell", "beside": "greet"})),
        TEST,
        &[("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\n. src/farewell.sh\n"), ("src/farewell.sh", FAREWELL)],
    );
    let (exit, out) = r.verify();
    assert_eq!(exit, 1, "{out}");
    assert!(out.contains("its tests still pass with the file put back"), "{out}");
}
