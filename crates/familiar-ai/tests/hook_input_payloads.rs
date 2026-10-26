//! The hook input parses real payloads captured from Claude Code (a Write, an
//! Edit, a shell command and a subagent), only their paths replaced; see
//! `fixtures/`.

mod common;

use common::fixture;
use familiar_ai::hook_input::{HookEvent, HookInput};

fn load(name: &str, event: HookEvent) -> HookInput {
    HookInput::parse(&fixture(name).to_string(), event).unwrap_or_else(|e| panic!("{name}: {e}"))
}

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

#[test]
fn every_captured_payload_parses_as_its_event() {
    let all = [
        ("session_start", HookEvent::SessionStart),
        ("user_prompt_submit", HookEvent::UserPromptSubmit),
        (
            "user_prompt_submit_task_notification",
            HookEvent::UserPromptSubmit,
        ),
        ("pre_tool_use_write", HookEvent::PreToolUse),
        ("pre_tool_use_edit", HookEvent::PreToolUse),
        ("pre_tool_use_bash", HookEvent::PreToolUse),
        ("pre_tool_use_agent", HookEvent::PreToolUse),
        ("pre_tool_use_bash_in_subagent", HookEvent::PreToolUse),
        ("post_tool_use_write", HookEvent::PostToolUse),
        ("post_tool_use_edit", HookEvent::PostToolUse),
        ("post_tool_use_bash", HookEvent::PostToolUse),
        ("post_tool_use_agent", HookEvent::PostToolUse),
        ("post_tool_use_bash_in_subagent", HookEvent::PostToolUse),
        ("stop", HookEvent::Stop),
        ("stop_with_background_task", HookEvent::Stop),
        ("stop_after_a_block", HookEvent::Stop),
        ("subagent_stop", HookEvent::SubagentStop),
    ];
    for (name, event) in all {
        let input = load(name, event);
        assert!(input.session_id.is_some(), "{name}: session_id");
        assert!(input.transcript_path.is_some(), "{name}: transcript_path");
        assert!(input.cwd.is_some(), "{name}: cwd");
    }
}

#[test]
fn the_prompt_is_in_the_field_prompt() {
    let input = load("user_prompt_submit", HookEvent::UserPromptSubmit);
    assert!(input
        .prompt_text()
        .unwrap()
        .starts_with("Do these steps in order."));
}

#[test]
fn a_finished_background_task_also_arrives_as_a_prompt() {
    let input = load(
        "user_prompt_submit_task_notification",
        HookEvent::UserPromptSubmit,
    );
    assert!(input
        .prompt_text()
        .unwrap()
        .starts_with("<task-notification>"));
}

#[test]
fn edit_write_and_bash_give_their_file_path_or_command() {
    let write = load("pre_tool_use_write", HookEvent::PreToolUse);
    assert_eq!(write.tool_name.as_deref(), Some("Write"));
    assert_eq!(write.file_path(), Some("/home/dev/project/hello.txt"));
    assert_eq!(write.command(), None);

    let edit = load("pre_tool_use_edit", HookEvent::PreToolUse);
    assert_eq!(edit.tool_name.as_deref(), Some("Edit"));
    assert_eq!(edit.file_path(), Some("/home/dev/project/hello.txt"));
    let ti = edit.tool_input.as_ref().unwrap();
    assert_eq!(ti["old_string"], "hi");
    assert_eq!(ti["new_string"], "hello");

    let bash = load("pre_tool_use_bash", HookEvent::PreToolUse);
    assert_eq!(bash.tool_name.as_deref(), Some("Bash"));
    assert_eq!(bash.command(), Some("cat hello.txt"));
    assert_eq!(bash.file_path(), None);
}

#[test]
fn the_tool_result_is_in_the_field_tool_response() {
    let bash = load("post_tool_use_bash", HookEvent::PostToolUse);
    assert_eq!(bash.tool_output().unwrap()["stdout"], "hello");
    let pre = load("pre_tool_use_bash", HookEvent::PreToolUse);
    assert!(pre.tool_output().is_none());
    assert_eq!(pre.tool_use_id, bash.tool_use_id);
}

#[test]
fn one_session_id_for_the_whole_session_and_its_subagent() {
    // The subagent's own tool calls and its stop carry the SAME session_id;
    // agent_id is what tells a subagent from the main conversation.
    for (name, event, in_subagent) in [
        ("session_start", HookEvent::SessionStart, false),
        ("user_prompt_submit", HookEvent::UserPromptSubmit, false),
        ("pre_tool_use_bash", HookEvent::PreToolUse, false),
        ("stop", HookEvent::Stop, false),
        ("pre_tool_use_bash_in_subagent", HookEvent::PreToolUse, true),
        (
            "post_tool_use_bash_in_subagent",
            HookEvent::PostToolUse,
            true,
        ),
        ("subagent_stop", HookEvent::SubagentStop, true),
    ] {
        let input = load(name, event);
        assert_eq!(input.session_id.as_deref(), Some(SESSION), "{name}");
        assert_eq!(input.in_subagent(), in_subagent, "{name}");
    }
    let stop = load("subagent_stop", HookEvent::SubagentStop);
    let call = load("pre_tool_use_bash_in_subagent", HookEvent::PreToolUse);
    assert_eq!(stop.agent_id, call.agent_id);
    assert_eq!(stop.agent_type.as_deref(), Some("general-purpose"));
    assert!(stop.agent_transcript_path.is_some());
}

#[test]
fn a_stop_after_a_blocked_stop_says_so() {
    assert_eq!(load("stop", HookEvent::Stop).stop_hook_active, Some(false));
    assert_eq!(
        load("stop_after_a_block", HookEvent::Stop).stop_hook_active,
        Some(true)
    );
}

#[test]
fn session_start_has_no_prompt_id_or_permission_mode() {
    let input = load("session_start", HookEvent::SessionStart);
    assert_eq!(input.source.as_deref(), Some("startup"));
    assert!(input.prompt_id.is_none());
    assert!(input.permission_mode.is_none());
}

#[test]
fn the_agent_conversation_id_tells_a_subagent_from_the_main_conversation() {
    let main_ids: Vec<Option<String>> = [
        ("session_start", HookEvent::SessionStart),
        ("user_prompt_submit", HookEvent::UserPromptSubmit),
        ("pre_tool_use_edit", HookEvent::PreToolUse),
        ("post_tool_use_bash", HookEvent::PostToolUse),
        ("stop", HookEvent::Stop),
    ]
    .into_iter()
    .map(|(name, event)| load(name, event).agent_conversation_id())
    .collect();
    for id in &main_ids {
        assert_eq!(id.as_deref(), Some(SESSION));
    }

    let sub_call =
        load("pre_tool_use_bash_in_subagent", HookEvent::PreToolUse).agent_conversation_id();
    let sub_stop = load("subagent_stop", HookEvent::SubagentStop).agent_conversation_id();
    let want = format!("{SESSION}/a1c4cab2240ac88dd");
    assert_eq!(sub_call.as_deref(), Some(want.as_str()));
    assert_eq!(
        sub_stop, sub_call,
        "a subagent's tool call and its stop are one conversation"
    );
    assert_ne!(sub_call.as_deref(), Some(SESSION));
}

#[test]
fn an_input_that_cannot_give_an_unambiguous_id_gives_none() {
    let parse = |text: &str| {
        HookInput::parse(text, HookEvent::PreToolUse)
            .unwrap()
            .agent_conversation_id()
    };
    assert_eq!(parse(r#"{"hook_event_name":"PreToolUse"}"#), None);
    assert_eq!(
        parse(r#"{"hook_event_name":"PreToolUse","session_id":""}"#),
        None
    );
    assert_eq!(
        parse(r#"{"hook_event_name":"PreToolUse","session_id":"s/x"}"#),
        None
    );
    assert_eq!(
        parse(r#"{"hook_event_name":"PreToolUse","session_id":"s","agent_id":"a/b"}"#),
        None
    );
    assert_eq!(
        parse(r#"{"hook_event_name":"PreToolUse","session_id":"s","agent_id":""}"#),
        None
    );
    assert_eq!(
        parse(r#"{"hook_event_name":"PreToolUse","session_id":"s","agent_id":"a"}"#).as_deref(),
        Some("s/a")
    );
}

#[test]
fn only_a_prompt_the_person_typed_is_the_person_speaking() {
    assert!(load("user_prompt_submit", HookEvent::UserPromptSubmit).is_the_person_speaking());
    assert!(!load(
        "user_prompt_submit_task_notification",
        HookEvent::UserPromptSubmit
    )
    .is_the_person_speaking());
    // no other event is ever the person speaking
    assert!(!load("stop", HookEvent::Stop).is_the_person_speaking());
    assert!(!load("pre_tool_use_agent", HookEvent::PreToolUse).is_the_person_speaking());
    assert!(!load("session_start", HookEvent::SessionStart).is_the_person_speaking());
}

/// Payloads captured from Claude Code 2.1.289, with ids cut to 8 characters
/// and paths replaced.
#[test]
fn the_investigator_payloads_parse_and_agree_on_the_subagents_id() {
    let all = [
        ("pre_tool_use_agent_spawn_on_haiku", HookEvent::PreToolUse),
        ("pre_tool_use_agent_investigator", HookEvent::PreToolUse),
        ("pre_tool_use_agent_in_subagent", HookEvent::PreToolUse),
        ("subagent_start_investigator", HookEvent::SubagentStart),
        ("subagent_start_in_subagent", HookEvent::SubagentStart),
        ("pre_tool_use_bash_in_investigator", HookEvent::PreToolUse),
        ("subagent_stop_investigator", HookEvent::SubagentStop),
        ("post_tool_use_agent_investigator", HookEvent::PostToolUse),
        ("pre_model_switch", HookEvent::PreModelSwitch),
        ("post_model_switch", HookEvent::PostModelSwitch),
    ];
    for (name, event) in all {
        let input = load(name, event);
        assert!(input.session_id.is_some() && input.transcript_path.is_some(), "{name}");
    }
    // the spawn: its input names the agent and the model, and may be rewritten
    let spawn = load("pre_tool_use_agent_spawn_on_haiku", HookEvent::PreToolUse);
    assert!(spawn.is_spawn() && !spawn.in_subagent());
    assert_eq!((spawn.input_text("subagent_type"), spawn.input_text("model")), (Some("general-purpose"), Some("haiku")));
    // SubagentStart carries agent_id and agent_type, no tool_use_id; the
    // returned spawn's agentId is that agent_id: one conversation id
    let start = load("subagent_start_investigator", HookEvent::SubagentStart);
    assert_eq!((start.agent_type.as_deref(), start.tool_use_id.as_deref()), (Some("investigator"), None));
    let back = load("post_tool_use_agent_investigator", HookEvent::PostToolUse);
    assert_eq!(back.spawned_conversation_id(), start.agent_conversation_id());
    assert_eq!(back.spawned_conversation_id().as_deref(), Some("627c0a59/a0693f4e"));
    assert_eq!(back.response_text("resolvedModel"), Some("claude-sonnet-5-5"));
    // the subagent's own tool call and stop carry the same id
    for (name, event) in [("pre_tool_use_bash_in_investigator", HookEvent::PreToolUse), ("subagent_stop_investigator", HookEvent::SubagentStop)] {
        assert_eq!(load(name, event).agent_conversation_id(), start.agent_conversation_id(), "{name}");
    }
    // a subagent's own subagent is session/agent too (flat)
    assert!(load("pre_tool_use_agent_in_subagent", HookEvent::PreToolUse).in_subagent());
    assert_eq!(load("subagent_start_in_subagent", HookEvent::SubagentStart).agent_conversation_id().as_deref(), Some("09633c87/a7829af4"));
    // the switch names both models; the session's own model is in neither SessionStart
    let switched = load("post_model_switch", HookEvent::PostModelSwitch);
    assert_eq!((switched.from_model.as_deref(), switched.to_model.as_deref()), (Some("claude-haiku-4-5-20251001"), Some("claude-sonnet-5-5")));
    assert_eq!(load("session_start", HookEvent::SessionStart).model, None);
}

/// The picker's payloads, captured live: a card asked with "ask" and then
/// picked or typed, and the same card under "allow", which ran no picker.
#[test]
fn the_pickers_question_and_the_persons_answer_are_where_the_hooks_read_them() {
    let pre = load("pre_tool_use_ask_user_question", HookEvent::PreToolUse);
    assert_eq!(pre.tool_name.as_deref(), Some("AskUserQuestion"));
    assert_eq!(pre.tool_input.as_ref().map(|i| i["questions"][0]["question"].clone()), Some(serde_json::json!("Continue?")));
    let q = "\"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?";
    for (name, answer) in [
        ("post_tool_use_ask_user_question_picked", Some("keep going as planned")),
        ("post_tool_use_ask_user_question_typed", Some("run pytest -q first")),
        ("post_tool_use_ask_user_question_allowed_unasked", None),
    ] {
        let post = load(name, HookEvent::PostToolUse);
        let input = post.tool_input.clone().unwrap_or_default();
        assert_eq!((input["questions"][0]["question"].as_str(), input["questions"][0]["header"].as_str()), (Some(q), Some("familiar-ai")), "{name}");
        // the person's answers ride the input on PostToolUse too (none under "allow")
        assert_eq!(input.get("answers").and_then(|a| a.get(q)).and_then(|a| a.as_str()), answer, "{name}");
        assert_eq!(post.tool_response.as_ref().and_then(|r| r["answers"].get(q)).and_then(|a| a.as_str()), answer, "{name}");
    }
}
