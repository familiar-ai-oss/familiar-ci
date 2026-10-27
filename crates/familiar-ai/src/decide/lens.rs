//! The hook's decisions around the lens panel (the owner's plan of 2026-10-26, lens-panel.md,
//! approved with its names): while a part's panel is owed, each Agent call of its holder that the
//! binary released (door::lens::release) is rewritten to the lens agent, with that lens's brief
//! (or the synthesis's) in place of the agent's prompt; a call with nothing left to release is
//! refused while the panel waits; a lens only reads, and the synthesis has no tools. Every reason
//! states a fact and never gives a command, because the agent paraphrases injected text.
//! The lens panel by layer: decide/ the spawn and its reads · door/ the answers and their facts · teach/ the words.

use familiar_ai_facts::Log;
use familiar_ai_rules::{cases, View};
use serde_json::Value;

use crate::decide::decision::Decision;
use crate::decide::investigator::Answer;
use crate::door::lens::{is_lens, only_the_synthesis_out, owed_here, owed_release, LENS};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::lens::{named, next};
use crate::teach::words::{quoted, LEAD};

/// The environment variable naming the panel's model (the owner's plan of 2026-10-26, approved).
pub const MODEL_VAR: &str = "FAMILIAR_AI_LENS_MODEL";
/// The panel's model when the environment names none: opus for every lens and the synthesis, as
/// tested (the owner's answer, 2026-10-26).
pub const DEFAULT_MODEL: &str = "opus";
/// The tools a lens may call: it only reads.
pub const READS: [&str; 3] = ["Read", "Grep", "Glob"];

/// The panel's model from the environment's value: the value trimmed, or opus when it is blank or missing.
pub fn model_from(value: Option<&str>) -> String {
    value.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(DEFAULT_MODEL).to_string()
}

/// The panel's model as this process's environment names it.
pub fn model() -> String {
    model_from(std::env::var(MODEL_VAR).ok().as_deref())
}

/// Rewrite a released spawn to its lens, refuse a spawn of the part's holder while the panel waits
/// with nothing to release, refuse every call of a lens's but a read (every call of the
/// synthesis's, while only syntheses run), and let a lens's stop go.
/// See docs/decisions.md#the-lens-panel-is-released-by-the-orchestrator.
pub fn decide(input: &HookInput, log: &Log, model: &str) -> Option<Answer> {
    let v = View::new(log);
    if is_lens(input.agent_type.as_deref()) {
        return match input.event {
            HookEvent::PreToolUse => {
                let tool = input.tool_name.as_deref().unwrap_or("");
                Some(Answer::Decided(if only_the_synthesis_out(&v) {
                    Decision::Refuse { reason: format!("{LEAD}the synthesis has no tools: it answers from its brief; {tool} is refused") }
                } else if READS.contains(&tool) {
                    Decision::allow()
                } else {
                    Decision::Refuse { reason: format!("{LEAD}a lens only reads ({}); {tool} is not one of them", READS.join(", ")) }
                }))
            }
            // its stop is not held on the lens it is
            HookEvent::SubagentStop => Some(Answer::Decided(Decision::allow())),
            _ => None,
        };
    }
    if input.event != HookEvent::PreToolUse || !input.is_spawn() {
        return None;
    }
    let me = input.agent_conversation_id()?;
    let released = input.tool_use_id.as_deref().and_then(|id| owed_release(&v, id));
    let Some((c, lens)) = released else {
        // the panel waits on its running calls: this spawn is none of them [draft words]
        let c = owed_here(&v, &me)?;
        let what = next(&v, &c).unwrap_or_default();
        return Some(Answer::Decided(Decision::Refuse { reason: format!("{LEAD}{}: the lens panel is reading the plan, and this Agent call is not one of its calls: {what}", quoted(&v, &c)) }));
    };
    let mut updated = input.tool_input.as_ref().and_then(Value::as_object).cloned().unwrap_or_default();
    updated.insert("prompt".into(), Value::from(crate::teach::lens::brief(&v, &c, &lens)));
    updated.insert("subagent_type".into(), Value::from(LENS));
    updated.insert("model".into(), Value::from(model));
    updated.insert("run_in_background".into(), Value::from(false));
    let q = quoted(&v, &c);
    let who = named(&lens);
    // [draft words]
    let (tools, rest) = if lens == cases::SYNTHESIS {
        ("it has no tools".to_string(), String::new())
    } else {
        ("it only reads".to_string(), next(&v, &c).map(|n| format!(" The lens panel: {n}.")).unwrap_or_default())
    };
    let reason = format!(
        "{LEAD}{q}: its contract is whole, and the lens panel reads the plan before its test is written. This spawn runs as {LENS}, {who}, on {model}, with familiar-ai's brief in place of this prompt; {tools}, and its answer is recorded when it returns.{rest}"
    );
    let message = format!("{LEAD}{who} ({model}) is reading the plan for {q}");
    Some(Answer::Rewrite { input: Value::Object(updated), reason, message })
}
