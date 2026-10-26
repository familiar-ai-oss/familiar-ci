//! While no run is open the person's prompts, the agent's commands and its
//! stops all go through, no intent is written, and the status line says `idle`.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5b0d7c1e-9a51-4c8e-8a47-1f2a3b4c5d6e";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

impl Project {
    fn statusline(&self) -> String {
        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "workspace": { "current_dir": self.repo().to_string_lossy(), "project_dir": self.repo().to_string_lossy() } });
        let (code, out, err) = self.run(&["statusline"], &input.to_string());
        assert_eq!((code, err.as_str()), (0, ""));
        // the status line shows the ask and its parts; CI is not shown here
        assert!(!out.contains("CI"), "{out}");
        out
    }

    fn plan_file(&self) -> String {
        self.read(".familiar/plan")
    }
}

fn refused(out: &Value) -> bool {
    out["decision"] == "block" || out["hookSpecificOutput"]["permissionDecision"] == "deny"
}

#[test]
fn the_persons_questions_shell_commands_and_stops_go_through_while_idle() {
    let p = project("talk");
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");

    let asked = p.hook("UserPromptSubmit", json!({ "prompt": "how does the judge pick its model?" }));
    assert!(!refused(&asked), "{asked}");
    assert!(!asked.to_string().contains("intent"), "a question is not an intent: {asked}");

    let bash = json!({ "tool_name": "Bash", "tool_input": { "command": "grep -rn judge src" } });
    let pre = p.hook("PreToolUse", bash.clone());
    assert!(!refused(&pre), "{pre}");
    let mut post = bash;
    post["tool_response"] = json!({ "stdout": "", "stderr": "" });
    let post = p.hook("PostToolUse", post);
    assert!(!refused(&post), "{post}");

    let stop = p.hook("Stop", json!({ "stop_hook_active": false }));
    assert!(!refused(&stop), "a stop while idle goes: {stop}");

    let again = p.hook("UserPromptSubmit", json!({ "prompt": "and what does it cost?" }));
    assert!(!refused(&again), "{again}");
    let stop = p.hook("Stop", json!({ "stop_hook_active": false }));
    assert!(!refused(&stop), "a stop while idle goes, however many prompts: {stop}");

    assert!(!p.plan_file().contains("intent_submitted"), "no intent was written: {}", p.plan_file());
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
}
