//! The marker outside the repository keeps a project set up after its
//! `.familiar` is removed with the plan never committed: setup restores it.
//! With no marker and no plan in git, a project is not set up.

mod common;

use std::io::Write;
use std::process::{Stdio, Command};

use common::{planned_to_line_7, run_binary, stdout};

#[test]
fn removing_the_folder_is_refused_and_a_forced_removal_is_still_a_removal_by_the_marker() {
    let p = planned_to_line_7("marker");
    let hook = |event: &str, rest: &str| {
        let input = format!("{{\"hook_event_name\":\"{event}\",\"cwd\":\"{}\"{rest}}}", p.dir.display());
        let output = run_binary(&["hook", event], &input);
        assert_eq!(common::exit_code(&output), 0, "{event}: {}", common::stderr(&output));
        serde_json::from_str::<serde_json::Value>(&stdout(&output)).expect("json")
    };
    let bash = |cmd: &str| format!(",\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"{}\"}}", cmd.replace('"', "\\\""));
    let edit = format!(",\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}/pydantic/json_schema.py\"}}", p.dir.display());
    let abs = p.dir.join(".familiar");
    for cmd in [
        "git reset --hard HEAD".to_string(),
        "rm -rf .familiar".to_string(),
        format!("rm -rf {}", abs.display()),
        format!("cd {} && rm -rf .familiar", p.dir.display()),
        "mv .familiar .familiar.off".to_string(),
        "echo > .familiar/record".to_string(),
        "git clean -fdx".to_string(),
        "git checkout -- .familiar/plan".to_string(),
    ] {
        let out = hook("PreToolUse", &bash(&cmd));
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{cmd}: {out}");
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
        assert_eq!(reason, "familiar-ai: Familiar's folder stays as it is: it is the person's to remove, not the agent's", "{cmd}");
    }
    for cmd in ["cat .familiar/plan", "ls -la .familiar", "familiar-ai setup", "git status", "rm -rf build"] {
        let out = hook("PreToolUse", &bash(cmd));
        assert!(!out.to_string().contains("not the agent's"), "{cmd}: {out}");
    }
    assert!(abs.is_dir(), "nothing was removed");

    // forced past the courtesy: the folder removed by hand, the plan never committed
    std::fs::remove_dir_all(&abs).expect("rm -rf");
    let out = hook("PreToolUse", &edit);
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(reason.contains("was removed") && reason.contains("familiar-ai setup"), "{reason}");
    assert_eq!(hook("Stop", "")["decision"], "block");
    assert!(!abs.exists(), "nothing is created");
    // setup restores: the folder, nothing in git to bring back; already marked
    let out = hook("PreToolUse", &bash("familiar-ai setup"));
    assert_eq!(out, serde_json::json!({}));
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("made .familiar at ") && out.contains("this project is already marked as set up at "), "{out}");
    assert!(!out.contains("restored"), "{out}");
    let out = hook("PreToolUse", &edit);
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(reason.starts_with("familiar-ai: the plan comes first") && reason.contains("claim it: familiar-ai claim intent_submitted"), "work continues: {reason}");

    // the same project with no marker (another machine) and no plan in git: not set up, allow
    std::fs::remove_dir_all(&abs).expect("rm -rf");
    let input = format!("{{\"hook_event_name\":\"PreToolUse\",\"cwd\":\"{}\"{edit}}}", p.dir.display());
    let output = Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
        .env("FAMILIAR_AI_JUDGE", "off")
        .env("FAMILIAR_AI_INVESTIGATOR", "off")
        .args(["hook", "PreToolUse"])
        .env("FAMILIAR_AI_HOME", common::test_home().join("another-machine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().expect("stdin").write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("run");
    assert_eq!(stdout(&output), "{}\n", "a missing marker never refuses");
}
