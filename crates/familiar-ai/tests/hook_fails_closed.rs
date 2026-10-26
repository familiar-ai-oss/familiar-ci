//! The `hook` command fails closed: every valid input exits 0 with one JSON
//! object and never grants permission; every bad input exits 2. No other
//! exit code (any other lets the action through in Claude Code).

mod common;

use common::{run_binary, stdout};

/// One valid input per event, in Claude Code's shapes, each with an
/// unknown extra field that must be ignored.
const SAMPLES: [(&str, &str); 6] = [
    (
        "SessionStart",
        r#"{"session_id": "s1", "transcript_path": "/tmp/t.jsonl", "cwd": "/w", "hook_event_name": "SessionStart", "source": "startup", "model": "claude", "future_field": {"x": 1}}"#,
    ),
    (
        "UserPromptSubmit",
        r#"{"session_id": "s1", "cwd": "/w", "hook_event_name": "UserPromptSubmit", "permission_mode": "default", "prompt_id": "p1", "prompt": "add the tests", "another_new_field": [1]}"#,
    ),
    (
        "PreToolUse",
        r#"{"session_id": "s1", "cwd": "/w", "hook_event_name": "PreToolUse", "tool_name": "Write", "tool_use_id": "tu1", "tool_input": {"file_path": "src/lib.rs", "content": "..."}, "unknown": true}"#,
    ),
    (
        "PostToolUse",
        r#"{"session_id": "s1", "cwd": "/w", "hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_use_id": "tu2", "tool_input": {"command": "cargo test"}, "tool_response": {"exit_code": 0}, "unknown": 3}"#,
    ),
    (
        "Stop",
        r#"{"session_id": "s1", "cwd": "/w", "hook_event_name": "Stop", "last_assistant_message": "done", "stop_hook_active": false, "unknown": "u"}"#,
    ),
    (
        "SubagentStop",
        r#"{"session_id": "s1", "cwd": "/w", "hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "Explore", "last_assistant_message": "found", "unknown": null}"#,
    ),
];

#[test]
fn each_event_with_valid_input_exits_0_and_prints_one_json_object() {
    // no sample's cwd has a `.familiar` folder: nothing is checked or created;
    // SessionStart names `familiar-ai setup`, every other answer is `{}`
    for (event, sample) in SAMPLES {
        let output = run_binary(&["hook", event], sample);
        assert_eq!(common::exit_code(&output), 0, "{event}: {}", common::stderr(&output));
        let out: serde_json::Value =
            serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| panic!("{event}: {e}"));
        if event == "SessionStart" {
            let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("context");
            assert!(context.contains("not set up") && context.contains("familiar-ai setup"), "{context}");
        } else {
            assert_eq!(out, serde_json::json!({}), "{event}");
        }
        let text = stdout(&output);
        assert!(!text.contains("permissionDecision"), "{event}: {text}");
        assert!(out.get("decision").is_none(), "{event}: {text}");
    }
}

#[test]
fn invalid_json_on_stdin_exits_2_with_a_plain_stderr_message() {
    for bad in ["", "not json at all", "{broken", "[1,2,3]", "42"] {
        let output = run_binary(&["hook", "Stop"], bad);
        assert_eq!(common::exit_code(&output), 2, "stdin {bad:?}");
        assert!(!common::stderr(&output).is_empty(), "stdin {bad:?}");
        assert_eq!(stdout(&output), "", "stdin {bad:?}");
    }
}

#[test]
fn an_event_mismatch_exits_2() {
    let output = run_binary(
        &["hook", "PreToolUse"],
        r#"{"hook_event_name": "Stop", "session_id": "s1"}"#,
    );
    assert_eq!(common::exit_code(&output), 2);
    assert!(common::stderr(&output).contains("mismatch"), "{}", common::stderr(&output));
}

#[test]
fn a_missing_hook_event_name_exits_2() {
    let output = run_binary(&["hook", "Stop"], r#"{"session_id": "s1"}"#);
    assert_eq!(common::exit_code(&output), 2);
    assert!(!common::stderr(&output).is_empty());
}

#[test]
fn an_unknown_event_name_is_a_usage_error() {
    let output = run_binary(&["hook", "BeforeToolUse"], "{}");
    assert_eq!(common::exit_code(&output), 2);
    assert!(!common::stderr(&output).is_empty());
}

#[test]
fn no_subcommand_is_a_usage_error_not_a_crash() {
    let output = run_binary(&[], "");
    assert_eq!(common::exit_code(&output), 2);
}

#[test]
fn the_hook_command_only_ever_exits_0_or_2() {
    let inputs = [
        "",
        "null",
        "[]",
        "42",
        "\"text\"",
        "{",
        "{}",
        "{\"hook_event_name\": 7}",
        "{\"hook_event_name\": \"PreToolUse\", \"tool_input\": 5}",
        "\u{0}\u{1}",
        "{\"hook_event_name\":\"Stop\"}{}",
    ];
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Stop",
        "SubagentStop",
        "Nope",
        "",
    ] {
        for input in inputs {
            let code = run_binary(&["hook", event], input).status.code();
            assert!(
                code == Some(0) || code == Some(2),
                "hook {event:?} with {input:?} exited {code:?}"
            );
        }
    }
    let code = run_binary(&["hook"], "{}").status.code();
    assert_eq!(code, Some(2));
}

#[test]
fn an_allowed_hook_never_grants_permission() {
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Stop",
        "SubagentStop",
    ] {
        let input = format!("{{\"hook_event_name\": \"{event}\", \"tool_name\": \"Bash\", \"tool_input\": {{\"command\": \"ls\"}}}}");
        let out = run_binary(&["hook", event], &input);
        let (code, stdout) = (out.status.code(), stdout(&out));
        assert_eq!(code, Some(0), "{event}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
        assert!(!stdout.contains("permissionDecision"), "{event}: {stdout}");
        assert!(v.get("decision").is_none(), "{event}: {stdout}");
        // no cwd and no project folder: nothing to check; only the
        // session's start says so (and never grants anything)
        if event == "SessionStart" {
            assert!(stdout.contains("not set up"), "{event}: {stdout}");
        } else {
            assert_eq!(v, serde_json::json!({}), "{event}");
        }
    }
}
