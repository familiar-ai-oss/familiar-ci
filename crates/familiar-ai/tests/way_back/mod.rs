//! Helpers for the way-back tests: a criterion sealed red and green on the
//! laptop, then its sealed test edited after green, announced or not.
#![allow(dead_code)]

use std::ops::Deref;

use crate::common::Project;

pub const SESSION: &str = "7b2e9c4d-1a3f-4e5b-8c6d-0f9a8b7c6d5e";
/// The sealed test: red on the code as it was, green after the fix.
pub const TEST: &str = "sh src/app.sh | grep -qx hello\n";
/// The same check, its wording changed: it still passes on the fix.
pub const REWORDED: &str = "sh src/app.sh | grep -x -q hello\n";
/// An edit that comes out red on the fix: a new seal.
pub const STRICTER: &str = "sh src/app.sh | grep -qx 'hello there'\n";

/// The shared project, with the way-back steps on top.
pub struct Sealed(Project);

impl Deref for Sealed {
    type Target = Project;

    fn deref(&self) -> &Project {
        &self.0
    }
}

impl Sealed {
    /// main holds `base`; the branch `work` holds setup's commit.
    pub fn new(name: &str) -> Sealed {
        Sealed(Project::builder(name).session(SESSION).setup_on_branch("work").create())
    }

    /// The agent's commit: made, then the hook after the tool call; what it told.
    pub fn commit(&self, message: &str) -> String {
        self.0.commit(message).to_string()
    }

    /// The plan claimed, the test sealed red, the fix green.
    pub fn green(name: &str) -> Sealed {
        let p = Sealed::new(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        p.write("tests/test_c1.sh", TEST);
        assert!(p.commit("c1 tests").contains("they fail, as they should before the fix"));
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
        assert!(p.commit("c1 the fix").contains("they pass"));
        assert!(p.status().contains("\"greet says hello\" · locally green"), "{}", p.status());
        p
    }

    pub fn announce(&self) {
        self.claim(&["test_change_announced", "--criterion_id", "c1", "--reason", "a sibling's wording broke an assertion"]);
    }

    /// The sealed test edited and committed alone; what the hook told.
    pub fn edit_test(&self, text: &str) -> String {
        self.write("tests/test_c1.sh", text);
        self.commit("c1 the test's wording")
    }

    /// Green, announced after green, the test reworded and committed alone.
    pub fn way_back(name: &str) -> Sealed {
        let p = Sealed::green(name);
        p.announce();
        p.edit_test(REWORDED);
        p
    }

    /// The plan as the binary last wrote it, committed alone.
    pub fn commit_plan(&self) {
        self.git(&["add", ".familiar/plan"]);
        if !self.git(&["status", "--porcelain", "--", ".familiar/plan"]).is_empty() {
            self.git(&["commit", "-q", "-m", "the plan"]);
        }
    }

    /// `verify` in a fresh clone of the branch: its exit code and what it said.
    pub fn verify_in_clone(&self, name: &str) -> (i32, String) {
        let clone = self.dir.join(format!("clone-{name}"));
        let _ = std::fs::remove_dir_all(&clone);
        self.git_in(&self.dir, &["clone", "-q", "--branch", "work", self.repo().to_str().expect("path"), clone.to_str().expect("path")]);
        let (code, out, err) = self.run_in(&clone, &["verify"], "");
        (code, format!("{out}{err}"))
    }
}
