//! The person can switch familiar-ai off for a project from their own terminal
//! but not from an agent's shell; while off, hooks allow and write nothing
//! but the person's own prompts. See docs/decisions.md#the-off-switch

mod common;

use std::process::{Command, Stdio};

use common::Project;
use familiar_ai_facts::Record;

const SESSION: &str = "5d1c0e2a-7f0b-4b8e-9f43-2c1d7e6a0b11";
const OFF_LINE: &str = "Familiar is turned off for this project. Every hook allows everything and says nothing. The person turns it on with the button or by running familiar-ai on in a terminal of their own.";

/// The binary runs from the person's terminal unless a test names the agent's session.
fn project(name: &str) -> Project {
    Project::builder(name).no_session().no_app().file("a.py", "x = 1\n").create()
}

impl Project {
    /// Runs `familiar-ai hook <event>` with the tool's input; returns (exit, stdout, stderr).
    fn told(&self, event: &str, tool: &str, tool_input: serde_json::Value) -> (i32, String, String) {
        let payload = self.payload_as(
            SESSION,
            event,
            serde_json::json!({
                "tool_name": tool,
                "tool_input": tool_input,
                "tool_response": { "stdout": "[main abc] x", "exit_code": 0 },
                "prompt": "just poking around",
                "source": "startup",
            }),
        );
        self.hook_raw(event, &payload)
    }

    fn edit(&self) -> (i32, String, String) {
        self.told("PreToolUse", "Edit", serde_json::json!({ "file_path": self.path("a.py").to_string_lossy() }))
    }

    fn record(&self) -> u64 {
        Record::open(&self.record_path()).expect("record").len().expect("len")
    }

    fn switches(&self) -> Vec<String> {
        let r = Record::open(&self.record_path()).expect("record");
        r.read_by_kind("familiar_switch")
            .expect("read")
            .iter()
            .map(|f| f.fields.get("to").and_then(serde_json::Value::as_str).unwrap_or("").to_string())
            .collect()
    }

    fn off_in_status(&self) -> serde_json::Value {
        let (code, stdout, stderr) = self.run(&["status", "--json"], "");
        assert_eq!(code, 0, "{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("status json");
        v["off"].clone()
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
fn off_from_the_persons_terminal_lets_everything_go_silently_and_writes_nothing() {
    let p = project("off");
    // the control: governed, the edit and the stop are refused
    assert!(p.edit().1.contains("familiar-ai: the plan comes first"));
    assert_eq!(p.run(&["claim", "intent_submitted", "--for", "0", "--text", "x"], "").0, 0);
    assert!(p.told("Stop", "", serde_json::json!({})).1.contains("familiar-ai: the plan comes first"));

    let (code, stdout, stderr) = p.run(&["off"], "");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "Familiar is turned off for this project. Every hook allows everything and says nothing until familiar-ai on.\n");
    assert_eq!(p.switches(), ["off"]);

    let before = p.record();
    says_nothing(&p.edit());
    says_nothing(&p.told("Stop", "", serde_json::json!({})));
    says_nothing(&p.told("PostToolUse", "Bash", serde_json::json!({ "command": "git commit -m x" })));
    says_nothing(&p.told("UserPromptSubmit", "", serde_json::json!({})));
    says_nothing(&p.told("PreToolUse", "Bash", serde_json::json!({ "command": "echo y > a.py" })));
    // the person's prompt is still recorded (the owner, 2026-10-12); nothing else is
    assert_eq!(p.record(), before + 1, "a hook wrote to the record while off");
}

#[test]
fn on_again_governs_as_before() {
    let p = project("on");
    assert_eq!(p.run(&["off"], "").0, 0);
    says_nothing(&p.edit());
    let (code, stdout, stderr) = p.run(&["on"], "");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "Familiar is turned on for this project.\n");
    assert_eq!(p.switches(), ["off", "on"]);
    assert!(p.edit().1.contains("familiar-ai: the plan comes first"), "on again: the edit is refused as before");
}

#[test]
fn an_agent_cannot_switch_it_off_or_on() {
    let p = project("agent");
    let before = p.record();
    for to in ["off", "on"] {
        let (code, stdout, stderr) = p.run_as(Some(SESSION), &[to], "");
        assert_eq!(code, 1, "{to}");
        assert_eq!(stdout, "", "{to}");
        assert_eq!(
            stderr,
            format!("familiar-ai: not switched · familiar-ai {to} is the person's switch, not the agent's: the person turns it {to} with the button or by running familiar-ai {to} in a terminal of their own\n"),
            "{to}"
        );
    }
    // a session the binary launched (the investigator) is an agent too
    let mut c = p.command();
    let out = c.env("FAMILIAR_AI_CONVERSATION_ID", format!("{SESSION}/inv")).arg("off").stdin(Stdio::null()).output().expect("run");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(p.record(), before, "an agent's switch was written");
    assert!(p.edit().1.contains("familiar-ai: the plan comes first"), "still governed");
    // nor can it be claimed: it is the binary's own kind
    let (code, _, stderr) = p.run_as(Some(SESSION), &["claim", "familiar_switch", "--to", "off"], "");
    assert_eq!(code, 1);
    assert!(stderr.contains("familiar_switch is written by familiar-ai itself"), "{stderr}");
    let (code, _, _) = p.run(&["claim", "familiar_switch", "--to", "off"], "");
    assert_eq!(code, 1, "the person switches with familiar-ai off, not a claim");
    assert!(p.switches().is_empty());
}

#[test]
fn a_session_start_while_off_is_one_line_and_no_rulebook() {
    let p = project("start");
    let (_, stdout, _) = p.told("SessionStart", "", serde_json::json!({}));
    assert!(stdout.contains("familiar-ai: proof that the work is done"), "the control: {stdout}");
    assert_eq!(p.run(&["off"], "").0, 0);
    let before = p.record();
    let (code, stdout, stderr) = p.told("SessionStart", "", serde_json::json!({}));
    assert_eq!(code, 0, "{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(v["hookSpecificOutput"]["additionalContext"], OFF_LINE, "{stdout}");
    assert!(!stdout.contains("proof that the work is done"), "{stdout}");
    assert!(v.get("decision").is_none() && !stdout.contains("permissionDecision"), "{stdout}");
    assert_eq!(p.record(), before);
}

#[test]
fn status_json_says_whether_it_is_off() {
    let p = project("status");
    assert_eq!(p.off_in_status(), serde_json::json!(false));
    assert_eq!(p.run(&["off"], "").0, 0);
    assert_eq!(p.off_in_status(), serde_json::json!(true));
    assert_eq!(p.run(&["on"], "").0, 0);
    assert_eq!(p.off_in_status(), serde_json::json!(false));
}

#[test]
fn off_outside_a_project_names_the_folder_and_makes_nothing() {
    let dir = common::temp_dir("none");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    for to in ["off", "on"] {
        let out = Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
            .arg(to)
            .current_dir(&dir)
            .env_remove("CLAUDE_PROJECT_DIR")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("FAMILIAR_AI_CONVERSATION_ID")
            .stdin(Stdio::null())
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1), "{to}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(".familiar"), "{to}");
    }
    assert!(!dir.join(".familiar").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
