//! Every event's input parses with unknown fields ignored, bad input is
//! refused with its reason, and the readers return the right fields.

use super::*;
use serde_json::json;

/// One sample input per event, each carrying an unknown extra field
/// that must be ignored, never refused.
#[test]
fn parses_one_sample_per_event_with_unknown_fields() {
    let samples = [
        (
            "SessionStart",
            json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": "/w", "source": "startup", "model": "m", "brand_new_field": 1}),
        ),
        (
            "UserPromptSubmit",
            json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt": "hello", "permission_mode": "default", "prompt_id": "p1", "surprise": {"x": 2}}),
        ),
        (
            "PreToolUse",
            json!({"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_use_id": "t1", "tool_input": {"file_path": "src/a.rs"}, "unknown": [1, 2]}),
        ),
        (
            "PostToolUse",
            json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_use_id": "t2", "tool_input": {"command": "ls"}, "tool_response": {"exit_code": 0}, "extra": "e"}),
        ),
        (
            "Stop",
            json!({"hook_event_name": "Stop", "last_assistant_message": "done", "stop_hook_active": false, "unknown_bool": true}),
        ),
        (
            "SubagentStop",
            json!({"hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "Explore", "last_assistant_message": "found it", "unknown": null}),
        ),
        (
            "SubagentStart",
            json!({"hook_event_name": "SubagentStart", "session_id": "s1", "agent_id": "a1", "agent_type": "familiar-ai:investigator", "x": 1}),
        ),
        (
            "PreModelSwitch",
            json!({"hook_event_name": "PreModelSwitch", "from_model": "a", "to_model": "b", "requested_model": "sonnet", "pricing": "catalog"}),
        ),
        (
            "PostModelSwitch",
            json!({"hook_event_name": "PostModelSwitch", "from_model": "a", "to_model": "b", "source": "command"}),
        ),
    ];
    for (name, sample) in samples {
        let event = HookEvent::from_name(name).expect("known event");
        let input = HookInput::parse(&sample.to_string(), event)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(input.event, event);
        assert_eq!(input.hook_event_name, name);
    }
}

#[test]
fn invalid_json_is_refused() {
    assert!(matches!(
        HookInput::parse("{not json", HookEvent::Stop),
        Err(InputError::NotJson(_))
    ));
}

#[test]
fn non_object_json_is_refused() {
    for text in ["[1,2]", "\"hello\"", "42", "null"] {
        assert!(
            matches!(
                HookInput::parse(text, HookEvent::Stop),
                Err(InputError::NotAnObject)
            ),
            "{text}"
        );
    }
}

#[test]
fn missing_or_unknown_hook_event_name_is_refused() {
    assert!(matches!(
        HookInput::parse("{}", HookEvent::Stop),
        Err(InputError::MissingEvent)
    ));
    assert!(matches!(
        HookInput::parse(r#"{"hook_event_name": "NotAnEvent"}"#, HookEvent::Stop),
        Err(InputError::MissingEvent)
    ));
}

#[test]
fn event_mismatch_is_refused() {
    let err = HookInput::parse(r#"{"hook_event_name": "Stop"}"#, HookEvent::PreToolUse);
    assert!(matches!(
        err,
        Err(InputError::EventMismatch { ref cli, ref stdin })
            if cli == "PreToolUse" && stdin == "Stop"
    ));
}

#[test]
fn file_path_reader_returns_the_path_or_nothing() {
    let event = HookEvent::PreToolUse;
    let with = HookInput::parse(
        r#"{"hook_event_name": "PreToolUse", "tool_input": {"file_path": "src/x.rs"}}"#,
        event,
    )
    .expect("parse");
    assert_eq!(with.file_path(), Some("src/x.rs"));

    let without = HookInput::parse(
        r#"{"hook_event_name": "PreToolUse", "tool_input": {}}"#,
        event,
    )
    .expect("parse");
    assert_eq!(without.file_path(), None);

    let no_tool =
        HookInput::parse(r#"{"hook_event_name": "PreToolUse"}"#, event).expect("parse");
    assert_eq!(no_tool.file_path(), None);

    let wrong_type = HookInput::parse(
        r#"{"hook_event_name": "PreToolUse", "tool_input": {"file_path": 42}}"#,
        event,
    )
    .expect("parse");
    assert_eq!(wrong_type.file_path(), None);
}

#[test]
fn command_reader_returns_the_command_or_nothing() {
    let event = HookEvent::PreToolUse;
    let with = HookInput::parse(
        r#"{"hook_event_name": "PreToolUse", "tool_input": {"command": "cargo test"}}"#,
        event,
    )
    .expect("parse");
    assert_eq!(with.command(), Some("cargo test"));

    let without = HookInput::parse(
        r#"{"hook_event_name": "PreToolUse", "tool_input": {}}"#,
        event,
    )
    .expect("parse");
    assert_eq!(without.command(), None);

    let no_tool =
        HookInput::parse(r#"{"hook_event_name": "PreToolUse"}"#, event).expect("parse");
    assert_eq!(no_tool.command(), None);
}

#[test]
fn event_names_round_trip() {
    for name in EVENT_NAMES {
        let event = HookEvent::from_name(name).expect("known");
        assert_eq!(event.name(), name);
        assert_eq!(HookEvent::parse_cli(name), Ok(event));
    }
    assert!(HookEvent::from_name("pretooluse").is_none());
}

#[test]
fn the_spawn_is_the_agent_or_the_task_tool_and_its_result_names_the_subagent() {
    let parse = |j: serde_json::Value, e: HookEvent| HookInput::parse(&j.to_string(), e).expect("parse");
    for tool in ["Agent", "Task"] {
        assert!(parse(json!({"hook_event_name": "PreToolUse", "tool_name": tool}), HookEvent::PreToolUse).is_spawn(), "{tool}");
    }
    assert!(!parse(json!({"hook_event_name": "PreToolUse", "tool_name": "Bash"}), HookEvent::PreToolUse).is_spawn());
    let back = parse(
        json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_name": "Agent", "tool_response": {"agentId": "a9", "resolvedModel": "claude-sonnet-5-5"}}),
        HookEvent::PostToolUse,
    );
    assert_eq!(back.spawned_conversation_id().as_deref(), Some("s1/a9"));
    assert_eq!(back.response_text("resolvedModel"), Some("claude-sonnet-5-5"));
    let bad = parse(json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_response": {"agentId": "a/9"}}), HookEvent::PostToolUse);
    assert_eq!(bad.spawned_conversation_id(), None, "never an ambiguous id");
    let switched = parse(json!({"hook_event_name": "PostModelSwitch", "session_id": "s1", "to_model": "m"}), HookEvent::PostModelSwitch);
    assert_eq!((switched.to_model.as_deref(), switched.session_conversation_id().as_deref()), (Some("m"), Some("s1")));
}

#[test]
fn a_session_the_binary_launched_is_the_conversation_it_was_given_and_its_stop_the_investigators() {
    let parse = |j: serde_json::Value, e: HookEvent| HookInput::parse(&j.to_string(), e).expect("parse");
    let stop = parse(json!({"hook_event_name": "Stop", "session_id": "u1"}), HookEvent::Stop);
    let it = stop.clone().launched(Some("s1/u1"), "familiar-ai:investigator");
    assert_eq!((it.event, it.agent_type.as_deref()), (HookEvent::SubagentStop, Some("familiar-ai:investigator")));
    assert_eq!((it.agent_conversation_id().as_deref(), it.session_conversation_id().as_deref()), (Some("s1/u1"), Some("s1/u1")));
    let edit = parse(json!({"hook_event_name": "PreToolUse", "session_id": "u1", "agent_id": "a2"}), HookEvent::PreToolUse).launched(Some("s1/u1"), "x");
    assert_eq!((edit.event, edit.agent_conversation_id().as_deref()), (HookEvent::PreToolUse, Some("s1/u1/a2")));
    let spawned = parse(json!({"hook_event_name": "PostToolUse", "session_id": "u1", "tool_response": {"agentId": "a3"}}), HookEvent::PostToolUse).launched(Some("s1/u1"), "x");
    assert_eq!(spawned.spawned_conversation_id().as_deref(), Some("s1/u1/a3"));
    // Another session, no value or a malformed value leaves the input unchanged.
    for conv in [Some("s1/u2"), None, Some("u1"), Some("/u1"), Some("s1//u1")] {
        let same = stop.clone().launched(conv, "x");
        assert_eq!((same.event, same.agent_conversation_id().as_deref()), (HookEvent::Stop, Some("u1")), "{conv:?}");
    }
}
