//! In Claude Code's plan mode every hook allows, says nothing and writes
//! nothing, except that SessionStart still carries the rulebook.
//! See docs/decisions.md#plan-mode-is-never-blocked

mod common;

use common::Project;

/// The session the hook payloads carry; the binary runs with none.
const SESSION: &str = "s1";

fn project(name: &str) -> Project {
    Project::builder(name).no_session().no_app().file("a.py", "x = 1\n").create()
}

impl Project {
    /// Runs `familiar-ai hook <event>` with the tool's input and the mode; returns (exit, stdout, stderr).
    fn in_mode(&self, event: &str, tool: &str, tool_input: serde_json::Value, mode: Option<&str>) -> (i32, String, String) {
        let mut payload = self.payload_as(
            SESSION,
            event,
            serde_json::json!({
                "tool_name": tool,
                "tool_input": tool_input,
                "tool_response": { "stdout": "[main abc] x", "exit_code": 0 },
            }),
        );
        if let Some(mode) = mode {
            payload["permission_mode"] = mode.into();
        }
        self.hook_raw(event, &payload)
    }

    fn edit(&self, mode: Option<&str>) -> (i32, String, String) {
        self.in_mode("PreToolUse", "Edit", serde_json::json!({ "file_path": self.path("a.py").to_string_lossy() }), mode)
    }

    fn record(&self) -> Vec<u8> {
        std::fs::read(self.record_path()).expect("record")
    }
}

/// Asserts the hook allowed with no decision, no message and nothing on stderr.
fn says_nothing(out: &(i32, String, String)) {
    let (code, stdout, stderr) = out;
    assert_eq!(*code, 0, "{stdout}{stderr}");
    assert_eq!(stderr, "", "stderr");
    let v: serde_json::Value = if stdout.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(stdout).expect("json") };
    assert_eq!(v, serde_json::json!({}), "said something: {stdout}");
}

#[test]
fn an_edit_in_plan_mode_is_allowed_and_silent_where_any_other_mode_is_refused() {
    let p = project("edit");
    for mode in [None, Some("default"), Some("acceptEdits")] {
        let (code, stdout, _) = p.edit(mode);
        assert_eq!(code, 0);
        assert!(stdout.contains("familiar-ai: the plan comes first"), "{mode:?}: {stdout}");
    }
    says_nothing(&p.edit(Some("plan")));
}

#[test]
fn a_stop_in_plan_mode_is_allowed_while_the_plan_is_being_written() {
    let p = project("stop");
    assert_eq!(p.run(&["claim", "intent_submitted", "--for", "0", "--text", "x"], "").0, 0);
    let (_, stdout, _) = p.in_mode("Stop", "", serde_json::json!({}), Some("default"));
    assert!(stdout.contains("familiar-ai: the plan comes first"), "{stdout}");
    says_nothing(&p.in_mode("Stop", "", serde_json::json!({}), Some("plan")));
}

#[test]
fn a_fact_that_would_land_in_plan_mode_is_not_written() {
    let p = project("post");
    assert_eq!(p.run(&["claim", "intent_submitted", "--for", "0", "--text", "x"], "").0, 0);
    let commit = serde_json::json!({ "command": "git commit -m x" });
    let before = p.record();
    says_nothing(&p.in_mode("PostToolUse", "Bash", commit.clone(), Some("plan")));
    assert_eq!(p.record(), before, "plan mode wrote to the record");
    // the same event in another mode does write
    p.in_mode("PostToolUse", "Bash", commit, Some("default"));
    assert_ne!(p.record(), before, "the control did not write");
}

#[test]
fn a_session_start_in_plan_mode_still_carries_the_rulebook() {
    let p = project("start");
    let (code, stdout, _) = p.in_mode("SessionStart", "", serde_json::json!({}), Some("plan"));
    assert_eq!(code, 0);
    assert!(stdout.contains("familiar-ai: proof that the work is done"), "{stdout}");
}
