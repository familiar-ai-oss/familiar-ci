//! A path outside the project's folder (after canonicalising) is never an act
//! of code of this project: an Edit, a Write or a shell write there is allowed
//! and nothing is told, however far the plan has got. Seen 2026-10-12: with
//! the intent claimed and no criterion, an Edit of a scratchpad file and
//! Writes into another repository were refused with "the plan comes first".

mod common;

use std::path::PathBuf;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "4b0d7c1e-9a52-4f3e-8d41-1a2b3c4d5e6f";

fn project(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    // beside the repository: a scratchpad and another repository with code
    std::fs::create_dir_all(p.dir.join("scratchpad")).expect("scratchpad");
    std::fs::create_dir_all(p.dir.join("checks/src")).expect("other repo");
    std::fs::write(p.dir.join("checks/src/check.rs"), "fn check() -> u8 {\n    1\n}\n").expect("other code");
    p
}

impl Project {
    fn edit_at(&self, path: &str, old: &str, new: &str) -> Value {
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    fn write_at(&self, path: &str, content: &str) -> Value {
        self.hook("PreToolUse", json!({ "tool_name": "Write", "tool_input": { "file_path": path, "content": content } }))
    }

    fn outside(&self, rel: &str) -> String {
        self.dir.join(rel).to_string_lossy().into_owned()
    }

    /// The same outside file named through the repository and `..`.
    fn outside_through_repo(&self, rel: &str) -> String {
        let mut p = self.repo();
        p.push("..");
        p.push(rel);
        p.to_string_lossy().into_owned()
    }

    /// Every way the agent reaches outside the project: each must be `{}`.
    fn every_outside_act(&self) -> Vec<(&'static str, Value)> {
        vec![
            ("edit scratchpad", self.write_at(&self.outside("scratchpad/findings.md"), "# findings\n")),
            ("edit other repo's code", self.edit_at(&self.outside("checks/src/check.rs"), "    1", "    2")),
            ("write other repo's code", self.write_at(&self.outside("checks/src/new.rs"), "fn new() {}\n")),
            ("edit through ..", self.edit_at(&self.outside_through_repo("checks/src/check.rs"), "    1", "    3")),
            ("shell write outside", self.bash("PreToolUse", &format!("cat > {} << 'EOF'\nfn x() {{}}\nEOF", self.outside("checks/src/x.rs")))),
            ("shell write through ..", self.bash("PreToolUse", "printf 'fn y() {}\\n' > ../checks/src/y.rs")),
        ]
    }
}

fn denied(out: &Value) -> bool {
    out["hookSpecificOutput"]["permissionDecision"] == "deny" || out.get("decision").is_some()
}

#[test]
fn an_edit_or_shell_write_outside_the_project_goes_through_before_the_intent() {
    let p = project("outside-no-intent");
    p.hook("UserPromptSubmit", json!({ "prompt": "note the findings and fix the checks repo" }));
    for (what, out) in p.every_outside_act() {
        assert_eq!(out, json!({}), "{what}: allowed, nothing told");
    }
    // the same edit inside the project is still an act of code
    let inside = p.edit_at(&p.path("src/app.sh").to_string_lossy(), "echo hi", "echo hello");
    assert!(denied(&inside), "inside is code: {inside}");
    assert!(p.facts_of("intent_submitted").is_empty());
}

#[test]
fn an_edit_or_shell_write_outside_the_project_goes_through_with_the_intent_and_no_criterion() {
    let p = project("outside-intent");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    p.claim(&["intent_submitted", "--text", "make greet say hello"]);
    for (what, out) in p.every_outside_act() {
        assert_eq!(out, json!({}), "{what}: allowed, nothing told");
    }
    let inside = p.edit_at(&p.path("src/app.sh").to_string_lossy(), "echo hi", "echo hello");
    assert!(denied(&inside), "inside is still refused until the plan is whole: {inside}");
    let _: PathBuf = p.repo();
}
