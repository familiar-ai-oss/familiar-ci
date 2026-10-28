//! The hook's answer as the Claude Code hooks reference reads it: one JSON object on stdout, exit 0.
//! An allow never emits `"permissionDecision": "allow"`, which would skip the person's own permission
//! prompt; only the investigator spawn rewrite does, because the reference pairs a rewrite with it.

use serde_json::{json, Map, Value};

use crate::decide::decision::Decision;
use crate::Outcome;
use crate::input::hook_input::{HookEvent, HookInput};

/// The door's name for a hook's finished answer.
pub type Rendered = Outcome;

/// The hook's answer before it is printed, serialized once by [`Response::finish`].
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    object: Map<String, Value>,
}

impl Response {
    /// Turn an allowed call into a rewrite: `permissionDecision: "allow"` with the whole
    /// `updatedInput`, keeping every line the answer already carries (B-4, plan §6.7).
    pub fn with_rewrite(mut self, updated: &Value, reason: &str) -> Response {
        let mut hso = match self.object.remove("hookSpecificOutput") {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        hso.insert("hookEventName".into(), Value::from("PreToolUse"));
        hso.insert("permissionDecision".into(), Value::from("allow"));
        hso.insert("permissionDecisionReason".into(), Value::from(reason));
        hso.insert("updatedInput".into(), updated.clone());
        self.object.insert("hookSpecificOutput".into(), Value::Object(hso));
        self
    }

    /// Append `message` to `systemMessage`, the text the person sees.
    pub fn with_system_message(mut self, message: &str) -> Response {
        let text = match self.object.get("systemMessage").and_then(Value::as_str) {
            Some(before) => format!("{before}\n{message}"),
            None => message.to_string(),
        };
        self.object.insert("systemMessage".into(), Value::String(text));
        self
    }

    /// Append `text` to `hookSpecificOutput.additionalContext`, the text the agent reads, leaving the decision as it is.
    pub fn with_context(mut self, event: HookEvent, text: &str) -> Response {
        let mut hso = match self.object.remove("hookSpecificOutput") {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        let text = match hso.get("additionalContext").and_then(Value::as_str) {
            Some(before) => format!("{before}\n{text}"),
            None => text.to_string(),
        };
        hso.insert("hookEventName".into(), Value::from(event.name()));
        hso.insert("additionalContext".into(), Value::String(text));
        self.object.insert("hookSpecificOutput".into(), Value::Object(hso));
        self
    }

    /// Print the object as one line of JSON on stdout, exit 0.
    pub fn finish(self) -> Outcome {
        one_object(&Value::Object(self.object))
    }
}

/// The answer for a decision; the state lines go with an allow, and as `systemMessage` beside a refusal.
pub fn response(input: &HookInput, decision: &Decision, lines: &[String]) -> Response {
    Response { object: render_object(input.event, decision, lines) }
}

/// [`response`], printed.
pub fn render(input: &HookInput, decision: &Decision, lines: &[String]) -> Outcome {
    response(input, decision, lines).finish()
}

/// What the person and the agent are told when the record stays in use by another step for the whole wait.
pub const BUSY_MESSAGE: &str = "familiar-ai: busy with another step; try again in a moment";

/// The answer when the record could not be opened in turn.
pub fn render_busy(event: HookEvent) -> Outcome {
    render_cannot_check(event, BUSY_MESSAGE)
}

/// The answer when Familiar cannot check at all: an edit or a shell command is refused with `message`,
/// and every other event passes with it shown, because refusing a prompt or a stop would trap the person or the agent.
pub fn render_cannot_check(event: HookEvent, message: &str) -> Outcome {
    let message = crate::teach::words::lead(message);
    let message = message.as_str();
    let object = match event {
        HookEvent::PreToolUse => json!({
            "systemMessage": message,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": message,
            },
        }),
        _ => json!({ "systemMessage": message }),
    };
    one_object(&object)
}

/// The investigator spawn rewritten.
/// `updatedInput` replaces the whole input, so it carries every field the agent sent.
pub fn render_rewrite(input: &Value, reason: &str, message: &str) -> Outcome {
    one_object(&json!({
        "systemMessage": message,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": reason,
            "updatedInput": input,
        },
    }))
}

/// The picker's question rewritten as `"ask"`, so Claude Code shows it in its own picker.
/// With `"allow"` the tool would run with no picker and empty answers.
pub fn render_ask(input: &Value) -> Outcome {
    one_object(&json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "ask",
            "permissionDecisionReason": "familiar-ai: a question for you",
            "updatedInput": input,
        },
    }))
}

/// `object` as one line of JSON on stdout, exit 0.
/// A serialization failure exits 2, because an empty object would allow.
fn one_object(object: &Value) -> Outcome {
    match serde_json::to_string(object) {
        Ok(mut stdout_text) => {
            stdout_text.push('\n');
            Outcome { exit_code: 0, stdout_text, stderr_text: String::new() }
        }
        Err(_) => Outcome { exit_code: 2, stdout_text: String::new(), stderr_text: crate::commands::STOPPED_WHILE_DECIDING.to_string() },
    }
}

/// The events whose `hookSpecificOutput.additionalContext` the agent reads
/// without it changing the event's course. An allowed PreToolUse tells the agent
/// before the tool runs, and never sets `permissionDecision`.
fn carries_context(event: HookEvent) -> bool {
    matches!(
        event,
        HookEvent::SessionStart
            | HookEvent::UserPromptSubmit
            | HookEvent::PreToolUse
            | HookEvent::PostToolUse
            | HookEvent::PostToolUseFailure
            | HookEvent::SubagentStart
            | HookEvent::PostModelSwitch
    )
}

fn joined(told: Option<&str>, lines: &[String]) -> Option<String> {
    let mut parts: Vec<&str> = told.into_iter().collect();
    parts.extend(lines.iter().map(String::as_str));
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// The one JSON object on stdout for an event and a decision.
fn render_object(event: HookEvent, decision: &Decision, lines: &[String]) -> Map<String, Value> {
    match decision {
        Decision::Allow { told } => {
            let Some(text) = joined(told.as_deref(), lines) else {
                return Map::new();
            };
            let mut object = Map::new();
            if carries_context(event) {
                object.insert(
                    "hookSpecificOutput".into(),
                    json!({ "hookEventName": event.name(), "additionalContext": text }),
                );
            } else {
                object.insert("systemMessage".into(), Value::String(text));
            }
            object
        }
        Decision::Refuse { reason } => {
            let mut top = Map::new();
            if let Some(text) = joined(None, lines) {
                top.insert("systemMessage".into(), Value::String(text));
            }
            match event {
                HookEvent::PreToolUse => {
                    top.insert(
                        "hookSpecificOutput".into(),
                        json!({
                            "hookEventName": "PreToolUse",
                            "permissionDecision": "deny",
                            "permissionDecisionReason": reason,
                        }),
                    );
                }
                HookEvent::UserPromptSubmit
                | HookEvent::PostToolUse
                | HookEvent::Stop
                | HookEvent::SubagentStop => {
                    top.insert("decision".into(), Value::from("block"));
                    top.insert("reason".into(), Value::from(reason.as_str()));
                }
                HookEvent::SessionStart | HookEvent::SubagentStart | HookEvent::PostToolUseFailure | HookEvent::PostModelSwitch => {
                    // These events cannot block, so the words go as context and a message.
                    top.insert("systemMessage".into(), Value::from(reason.as_str()));
                    top.insert(
                        "hookSpecificOutput".into(),
                        json!({ "hookEventName": event.name(), "additionalContext": reason }),
                    );
                }
                HookEvent::PreModelSwitch | HookEvent::SessionEnd => {
                    // The switch is the person's, so the words are shown and it goes ahead; at a
                    // session's end nothing reads context.
                    top.insert("systemMessage".into(), Value::from(reason.as_str()));
                }
            }
            top
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_for(name: &str) -> HookInput {
        let event = HookEvent::from_name(name).expect("known");
        let json = format!("{{\"hook_event_name\": \"{name}\"}}");
        HookInput::parse(&json, event).expect("parse")
    }

    fn refuse() -> Decision {
        Decision::Refuse {
            reason: "refused · the run · held: planning_incomplete\n  the plan is not complete yet".into(),
        }
    }

    fn stdout_json(rendered: &Outcome) -> Value {
        serde_json::from_str(&rendered.stdout_text).expect("stdout is JSON")
    }

    #[test]
    fn a_plain_allow_is_an_empty_object_for_every_event() {
        for name in crate::input::hook_input::EVENT_NAMES {
            let rendered = render(&input_for(name), &Decision::allow(), &[]);
            assert_eq!(rendered.exit_code, 0, "{name}");
            assert_eq!(rendered.stderr_text, "", "{name}");
            assert_eq!(rendered.stdout_text, "{}\n", "{name}");
        }
    }

    #[test]
    fn an_allow_with_words_never_emits_a_permission_decision() {
        let told = Decision::Allow { told: Some("this edit is outside the anchor of c1".into()) };
        for name in crate::input::hook_input::EVENT_NAMES {
            let rendered = render(&input_for(name), &told, &["criterion c1 · sealed".into()]);
            let out = stdout_json(&rendered);
            assert!(!rendered.stdout_text.contains("permissionDecision"), "{name}");
            assert!(out.get("decision").is_none(), "{name}");
            let event = HookEvent::from_name(name).expect("known");
            let text = if carries_context(event) {
                out["hookSpecificOutput"]["additionalContext"].as_str()
            } else {
                out["systemMessage"].as_str()
            }
            .unwrap_or_else(|| panic!("{name}: {}", rendered.stdout_text));
            assert!(text.contains("outside the anchor"), "{name}");
            assert!(text.contains("criterion c1 · sealed"), "{name}");
        }
    }

    #[test]
    fn refuse_on_pre_tool_use_denies_with_the_words() {
        let rendered = render(&input_for("PreToolUse"), &refuse(), &[]);
        assert_eq!(rendered.exit_code, 0);
        let out = stdout_json(&rendered);
        let hso = &out["hookSpecificOutput"];
        assert_eq!(hso["hookEventName"], "PreToolUse");
        assert_eq!(hso["permissionDecision"], "deny");
        let reason = hso["permissionDecisionReason"].as_str().expect("reason");
        assert!(reason.contains("planning_incomplete"), "{reason}");
        assert!(out.get("decision").is_none());
        assert!(out.get("systemMessage").is_none());
    }

    #[test]
    fn refuse_on_prompt_post_stop_subagent_blocks_with_the_words() {
        for name in ["UserPromptSubmit", "PostToolUse", "Stop", "SubagentStop"] {
            let rendered = render(&input_for(name), &refuse(), &["criterion c1 · held".into()]);
            assert_eq!(rendered.exit_code, 0, "{name}");
            let out = stdout_json(&rendered);
            assert_eq!(out["decision"], "block", "{name}");
            let reason = out["reason"].as_str().expect("reason");
            assert!(reason.contains("planning_incomplete"), "{name}: {reason}");
            assert_eq!(out["systemMessage"], "criterion c1 · held", "{name}");
            assert!(!rendered.stdout_text.contains("permissionDecision"), "{name}");
        }
    }

    #[test]
    fn refuse_on_session_start_cannot_block_and_uses_context() {
        let rendered = render(&input_for("SessionStart"), &refuse(), &[]);
        let out = stdout_json(&rendered);
        let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("context");
        assert!(context.contains("planning_incomplete"), "{context}");
        assert!(!rendered.stdout_text.contains("permissionDecision"));
        assert!(out.get("decision").is_none());
    }

    #[test]
    fn stdout_is_exactly_one_json_object_plus_a_newline() {
        let rendered = render(&input_for("Stop"), &Decision::allow(), &[]);
        assert!(rendered.stdout_text.ends_with('\n'));
        assert_eq!(rendered.stdout_text.matches('\n').count(), 1);
    }

    #[test]
    fn the_spawn_rewrite_is_allow_with_the_whole_updated_input_and_the_reason() {
        let input = serde_json::json!({ "prompt": "p", "subagent_type": "familiar-ai:investigator", "model": "sonnet" });
        let r = render_rewrite(&input, "two tries failed", "a stronger model (sonnet) is taking it over");
        assert_eq!(r.exit_code, 0);
        assert_eq!(
            r.stdout_text,
            "{\"systemMessage\":\"a stronger model (sonnet) is taking it over\",\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\",\"permissionDecisionReason\":\"two tries failed\",\"updatedInput\":{\"prompt\":\"p\",\"subagent_type\":\"familiar-ai:investigator\",\"model\":\"sonnet\"}}}\n"
        );
    }

    #[test]
    fn context_is_added_to_an_allowed_stop_without_a_decision() {
        let r = response(&input_for("Stop"), &Decision::allow(), &[]).with_context(HookEvent::Stop, "familiar-ai: proven").finish();
        let out = stdout_json(&r);
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "Stop");
        assert_eq!(out["hookSpecificOutput"]["additionalContext"], "familiar-ai: proven");
        assert!(out.get("decision").is_none());
        let told = Decision::Allow { told: Some("a".into()) };
        let r = response(&input_for("UserPromptSubmit"), &told, &[]).with_context(HookEvent::UserPromptSubmit, "b").finish();
        assert_eq!(stdout_json(&r)["hookSpecificOutput"]["additionalContext"], "a\nb");
    }

    #[test]
    fn the_subagents_start_and_the_switch_carry_their_words_as_context() {
        let told = Decision::Allow { told: Some("criterion c1".into()) };
        for name in ["SubagentStart", "PostModelSwitch"] {
            let out = stdout_json(&render(&input_for(name), &told, &[]));
            assert_eq!(out["hookSpecificOutput"]["hookEventName"], name);
            assert_eq!(out["hookSpecificOutput"]["additionalContext"], "criterion c1");
        }
        let out = stdout_json(&render(&input_for("SubagentStart"), &refuse(), &[]));
        assert!(out.get("decision").is_none(), "a subagent's start cannot be blocked");
    }

    #[test]
    fn busy_refuses_only_the_tool_call_and_names_no_hold() {
        for name in crate::input::hook_input::EVENT_NAMES {
            let event = HookEvent::from_name(name).expect("known");
            let r = render_busy(event);
            assert_eq!(r.exit_code, 0, "{name}");
            let v: Value = serde_json::from_str(&r.stdout_text).expect("json");
            assert_eq!(v["systemMessage"], BUSY_MESSAGE, "{name}");
            let denied = v["hookSpecificOutput"]["permissionDecision"] == "deny";
            assert_eq!(denied, event == HookEvent::PreToolUse, "{name}");
            assert!(
                v.get("decision").is_none(),
                "{name}: a prompt or a stop is never blocked"
            );
            assert!(!r.stdout_text.contains("hold"), "{name}");
        }
    }
}
