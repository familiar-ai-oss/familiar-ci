//! The hook's decisions around the case-finder (the owner's plan of 2026-10-25, the cases card,
//! §1b) [draft names]: while a part is owed the finder, the main agent's Agent call is rewritten to
//! it, with the binary's brief in place of the agent's prompt; the finder only reads. Every reason
//! states a fact and never gives a command, because the agent paraphrases injected text.
//! The case-finder by layer: decide/ the spawn and its reads · door/ the answer and its fact · teach/ the words.

use familiar_ai_facts::Log;
use familiar_ai_rules::View;
use serde_json::Value;

use crate::decide::decision::Decision;
use crate::decide::investigator::Answer;
use crate::door::case_finder::{is_case_finder, owed_here, CASE_FINDER};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::words::{quoted, LEAD};

/// The environment variable naming the finder's model [draft name].
pub const MODEL_VAR: &str = "FAMILIAR_AI_CASE_FINDER_MODEL";
/// The finder's model when the environment names none (the owner's ruling, 2026-10-26, "keep the
/// new brief on sonnet"; opus for one day: the offline check named the root case in 1 of 4).
pub const DEFAULT_MODEL: &str = "sonnet";
/// The tools the finder may call: it only reads.
pub const READS: [&str; 3] = ["Read", "Grep", "Glob"];

/// The finder's model from the environment's value: the value trimmed, or sonnet when it is blank or missing.
pub fn model_from(value: Option<&str>) -> String {
    value.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(DEFAULT_MODEL).to_string()
}

/// The finder's model as this process's environment names it.
pub fn model() -> String {
    model_from(std::env::var(MODEL_VAR).ok().as_deref())
}

/// Rewrite a spawn to the case-finder while a part is owed it, refuse every call of the finder's
/// but a read (which goes ahead, held on nothing), and let its stop go. See docs/decisions.md#the-case-finder-is-released-by-the-orchestrator.
pub fn decide(input: &HookInput, log: &Log, model: &str) -> Option<Answer> {
    if is_case_finder(input.agent_type.as_deref()) {
        return match input.event {
            HookEvent::PreToolUse => {
                let tool = input.tool_name.as_deref().unwrap_or("");
                Some(Answer::Decided(if READS.contains(&tool) {
                    Decision::allow()
                } else {
                    Decision::Refuse { reason: format!("{LEAD}the case-finder only reads ({}); {tool} is not one of them", READS.join(", ")) }
                }))
            }
            // its stop is not held on the finder it is
            HookEvent::SubagentStop => Some(Answer::Decided(Decision::allow())),
            _ => None,
        };
    }
    if input.event != HookEvent::PreToolUse || !input.is_spawn() {
        return None;
    }
    let me = input.agent_conversation_id()?;
    let v = View::new(log);
    let c = owed_here(&v, &me)?;
    let mut updated = input.tool_input.as_ref().and_then(Value::as_object).cloned().unwrap_or_default();
    updated.insert("prompt".into(), Value::from(crate::teach::case_finder::brief(&v, &c)));
    updated.insert("subagent_type".into(), Value::from(CASE_FINDER));
    updated.insert("model".into(), Value::from(model));
    updated.insert("run_in_background".into(), Value::from(false));
    let q = quoted(&v, &c);
    // [draft words]
    let reason = format!(
        "{LEAD}{q}: its contract is whole, and the case-finder looks for the cases it may miss before its test is written. This spawn runs as {CASE_FINDER} on {model}, with familiar-ai's brief in place of this prompt; it only reads, and its answer is recorded when it returns."
    );
    let message = format!("{LEAD}the case-finder ({model}) is looking for more cases for {q}");
    Some(Answer::Rewrite { input: Value::Object(updated), reason, message })
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use serde_json::json;

    fn add(log: &Log, kind: &str, fields: Value) -> Log {
        append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_else(|e| panic!("{kind}: {e}"))
    }

    /// A full-path part c1 whose contract is whole: the finder is owed.
    fn owed(path: &str) -> Log {
        let log = add(&Vec::new(), "intent_submitted", json!({ "text": "greet with hello", "sha": "abc" }));
        let log = add(&log, "behavior", json!({ "behavior_id": "b1", "text": "the greeting" }));
        let cases = json!([{ "case_id": "n1", "kind": "normal", "given": "Ada", "expect": "hello Ada" }, { "case_id": "o1", "kind": "opposite", "given": "no name", "expect": "no greeting" }]);
        let log = add(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing", "cases": cases, "contract_version": 1 }));
        add(&log, "criterion", json!({ "criterion_id": "c1", "text": "greet says hello", "path": path, "behavior_id": "b1", "promise_id": "p1" }))
    }

    fn input(rest: Value) -> HookInput {
        let mut j = json!({ "session_id": "s1" });
        if let (Some(o), Some(r)) = (j.as_object_mut(), rest.as_object()) {
            o.extend(r.clone());
        }
        let name = j["hook_event_name"].as_str().unwrap_or("PreToolUse").to_string();
        let e = HookEvent::from_name(&name).unwrap_or(HookEvent::PreToolUse);
        HookInput::parse(&j.to_string(), e).unwrap_or_else(|e| panic!("{e}"))
    }

    fn spawn(tool_input: Value) -> HookInput {
        input(json!({ "hook_event_name": "PreToolUse", "tool_name": "Agent", "tool_input": tool_input }))
    }

    #[test]
    fn the_spawn_is_rewritten_to_the_finder_with_the_brief() {
        let log = owed("full");
        let v = View::new(&log);
        match decide(&spawn(json!({ "description": "d", "prompt": "say hi", "subagent_type": "general-purpose" })), &log, "sonnet") {
            Some(Answer::Rewrite { input, reason, message }) => {
                let brief = crate::teach::case_finder::brief(&v, "c1");
                assert_eq!(input, json!({ "description": "d", "prompt": brief, "subagent_type": CASE_FINDER, "model": "sonnet", "run_in_background": false }));
                assert!(reason.starts_with("familiar-ai: \"greet says hello\": ") && !reason.contains("you must"), "{reason}");
                assert_eq!(message, "familiar-ai: the case-finder (sonnet) is looking for more cases for \"greet says hello\"");
            }
            other => panic!("expected the rewrite, got {other:?}"),
        }
        let named = spawn(json!({ "prompt": "mine", "subagent_type": CASE_FINDER, "model": "opus" }));
        match decide(&named, &log, "haiku") {
            Some(Answer::Rewrite { input, .. }) => {
                assert_eq!((input["prompt"].as_str(), input["model"].as_str()), (Some(crate::teach::case_finder::brief(&v, "c1").as_str()), Some("haiku")))
            }
            other => panic!("expected the rewrite, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_rewritten_when_no_finder_is_owed() {
        let spawn = spawn(json!({ "prompt": "p", "subagent_type": "general-purpose" }));
        assert_eq!(decide(&spawn, &owed("light"), "sonnet"), None, "the light path");
        let log = add(&owed("full"), "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "s9" }));
        assert_eq!(decide(&spawn, &log, "sonnet"), None, "another conversation holds the part");
        let log = add(&owed("full"), "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "covered", "model": "m", "tokens": 1 }));
        assert_eq!(decide(&spawn, &log, "sonnet"), None, "the finding stands");
        let edit = input(json!({ "hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": { "file_path": "src/app.sh" } }));
        assert_eq!(decide(&edit, &owed("full"), "sonnet"), None, "the main agent's edit is the rules'");
    }

    #[test]
    fn the_finder_only_reads_and_its_stop_goes() {
        let log = owed("full");
        let finder = |tool: &str| input(json!({ "hook_event_name": "PreToolUse", "agent_id": "f1", "agent_type": CASE_FINDER, "tool_name": tool, "tool_input": {} }));
        for tool in ["Edit", "Write", "MultiEdit", "NotebookEdit", "Bash", "Agent", "Task"] {
            match decide(&finder(tool), &log, "sonnet") {
                Some(Answer::Decided(Decision::Refuse { reason })) => assert!(reason.contains(tool), "{reason}"),
                other => panic!("{tool}: expected the refusal, got {other:?}"),
            }
        }
        for tool in READS {
            assert_eq!(decide(&finder(tool), &log, "sonnet"), Some(Answer::Decided(Decision::allow())), "{tool}: a read goes ahead, held on nothing");
        }
        let bare = input(json!({ "hook_event_name": "PreToolUse", "agent_id": "f1", "agent_type": "case-finder", "tool_name": "Write", "tool_input": {} }));
        assert!(matches!(decide(&bare, &log, "sonnet"), Some(Answer::Decided(Decision::Refuse { .. }))));
        let stop = input(json!({ "hook_event_name": "SubagentStop", "agent_id": "f1", "agent_type": CASE_FINDER }));
        assert_eq!(decide(&stop, &log, "sonnet"), Some(Answer::Decided(Decision::allow())));
    }

    // updated with the definition (the owner's ruling, 2026-10-26, "keep the new brief on sonnet"):
    // the default was opus for one day (2026-10-25), sonnet before
    #[test]
    fn the_model_is_the_environments_or_sonnet() {
        assert_eq!(model_from(Some("haiku")), "haiku");
        assert_eq!(model_from(Some(" sonnet ")), "sonnet");
        assert_eq!(model_from(Some("  ")), "sonnet");
        assert_eq!(model_from(None), "sonnet");
    }
}
