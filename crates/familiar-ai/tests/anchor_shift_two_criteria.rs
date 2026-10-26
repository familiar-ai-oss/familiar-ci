//! The start check follows the planned lines to where edits since the plan put
//! them, committed or not, so lines added above raise no `anchor_moved`; an edit
//! of the lines themselves does. A commit of the criterion before it that
//! changes the lines themselves is that criterion's work, not this one's code
//! (the owner, 2026-10-11): it goes on and is told whose work it was.

mod common;

use common::Project;
use serde_json::{json, Value};

const BASE: &str = "#!/bin/sh\ngreet() { echo hi; }\nfarewell() { echo bye; }\n";
const BYE: &str = "#!/bin/sh\nfarewell() { echo bye; }\n";

const SESSION: &str = "8e3a0f4b-2d84-4fb1-9d7a-4c5d6e7f8091";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/app.sh", BASE).file("src/bye.sh", BYE).create()
}

impl Project {
    /// The agent's commit of `paths` only, then the hook after the tool call; what it told.
    fn commit_only(&self, message: &str, paths: &[&str]) -> Value {
        let mut add = vec!["add"];
        add.extend(paths);
        self.git(&add);
        self.git(&["commit", "-q", "-m", message]);
        self.bash("PostToolUse", &format!("git commit -m '{message}'"))
    }

    /// Any hook writes the checks owed; a Read decides nothing else.
    fn look(&self) {
        let payload = self.payload("PreToolUse", json!({ "tool_name": "Read", "tool_input": { "file_path": self.path("src/app.sh").to_string_lossy() } }));
        let (code, _, err) = self.hook_raw("PreToolUse", &payload);
        assert_eq!(code, 0, "{err}");
    }

    /// c's start check against its planning check, from the plan's export:
    /// (the same lines_hash, the start check's lines).
    fn start_check(&self, c: &str) -> (bool, u64, u64) {
        let plan = std::fs::read_to_string(self.path(".familiar/plan")).expect("the plan's export");
        let checks: Vec<Value> = plan
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|f| f["kind"] == "anchor_sha_matched" && f["criterion_id"] == c)
            .collect();
        let at = |when: &str| checks.iter().rev().find(|f| f["checked_at"] == when).cloned().unwrap_or_else(|| panic!("no {when} check for {c}: {plan}"));
        let (planning, start) = (at("planning"), at("start"));
        (planning["lines_hash"] == start["lines_hash"], start["line_start"].as_u64().unwrap_or(0), start["line_end"].as_u64().unwrap_or(0))
    }

    /// c1 on greet; c2 strung after it on `c2_place` (file, symbol, line).
    fn two_criteria(&self, c2_place: (&str, &str, u64)) {
        let (file, symbol, line) = c2_place;
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet says hello, farewell says goodbye"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c2", "--text", "farewell says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        let places = json!([{ "file_path": file, "symbol": symbol, "line_start": line, "line_end": line }]).to_string();
        self.claim(&["place_chosen", "--criterion_id", "c2", "--places", &places]);
        self.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"]);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        for c in ["c1", "c2"] {
            self.claim(&["criterion_assigned", "--criterion_id", c]);
            self.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
        }
        // definition change (the owner's plan of 2026-10-21, step 3) [draft]: c2's place names
        // farewell, which the other file defines too; that site is classified before its tests
        common::classify_places(self);
        self.write("tests/test_c1.sh", ". src/app.sh; greet | grep -qx hello\n");
        let sealed = self.commit("c1: tests first");
        assert!(sealed.to_string().contains("they fail, as they should before the fix · stamped"), "{sealed}");
    }

    /// c1's fix, committed from `paths`; c2's turn comes, and its start check is written.
    fn c1_green(&self, app: &str, paths: &[&str]) -> String {
        self.write("src/app.sh", app);
        let green = self.commit_only("c1: greet says hello", paths);
        assert!(green.to_string().contains("they pass · stamped"), "{green}");
        self.look();
        self.status()
    }
}

const MOVED: &str = "the code at the plan's place changed after the plan was made";
const GREET_GREW: &str = "#!/bin/sh\ngreet() {\n  echo hello\n}\nfarewell() { echo bye; }\n";

#[test]
fn lines_added_above_the_place_raise_no_anchor_moved() {
    let p = project("above");
    p.two_criteria(("src/app.sh", "farewell", 3));
    let status = p.c1_green(GREET_GREW, &["-A"]);
    assert!(status.contains("\"farewell says goodbye\" · the test comes first"), "the same text, two lines down: {status}");
    assert!(!status.contains(MOVED), "{status}");
    assert_eq!(p.start_check("c2"), (true, 5, 5), "the start check reads farewell where it is now");
}

// Definition change (the owner, 2026-10-11): changes made by this plan's own
// earlier criteria are not outside edits, and another criterion's commit that
// also names c2's lines before c2's start is not c2's code (was:
// a_commit_that_changes_the_lines_is_still_caught, caught as c2's code,
// landed before its test was sealed).
#[test]
fn a_commit_of_the_criterion_before_that_changes_the_lines_goes_on_and_is_told() {
    let p = project("changed");
    p.two_criteria(("src/app.sh", "farewell", 3));
    let status = p.c1_green("#!/bin/sh\ngreet() {\n  echo hello\n}\nfarewell() { echo goodbye; }\n", &["-A"]);
    assert!(
        status.contains("\"farewell says goodbye\" · the test comes first · its lines were changed by the work on \"greet says hello\"; they now sit at src/app.sh"),
        "{status}"
    );
    assert!(!status.contains("the code landed before its test was sealed"), "{status}");
    assert!(!status.contains(MOVED), "{status}");
}

#[test]
fn an_edit_not_yet_committed_above_the_place_only_shifts_it() {
    let p = project("uncommitted-above");
    p.two_criteria(("src/bye.sh", "farewell", 2));
    p.write("src/bye.sh", "#!/bin/sh\n# says goodbye\n\nfarewell() { echo bye; }\n");
    let status = p.c1_green(GREET_GREW, &["src/app.sh"]);
    assert!(status.contains("\"farewell says goodbye\" · the test comes first"), "the same text, two lines down: {status}");
    assert!(!status.contains(MOVED), "{status}");
    assert_eq!(p.start_check("c2"), (true, 4, 4));
}

#[test]
fn an_edit_not_yet_committed_of_the_lines_is_anchor_moved() {
    let p = project("uncommitted-lines");
    p.two_criteria(("src/bye.sh", "farewell", 2));
    p.write("src/bye.sh", "#!/bin/sh\n# says goodbye\nfarewell() { echo goodbye; }\n");
    let status = p.c1_green(GREET_GREW, &["src/app.sh"]);
    assert!(status.contains(&format!("\"farewell says goodbye\" · {MOVED}")), "someone changed the place: {status}");
    assert!(!p.start_check("c2").0);
}
