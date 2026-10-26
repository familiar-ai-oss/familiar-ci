//! The investigator's facts: `model_seen` and `criterion_transfer`, to it and back.
//! The investigator by layer: decide/ the spawn and the wait · door/ the facts, the run, the brief · teach/ the words.

use familiar_ai_facts::{append_raw, Log, Record};
use familiar_ai_rules::{ladder, lifecycle, plan, View};
use serde_json::{json, Map, Value};

use crate::door::{cannot, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};

/// The plugin-scoped name of the investigator agent, as `subagent_type` and
/// SubagentStart's `agent_type` carry it.
pub const INVESTIGATOR: &str = "familiar-ai:investigator";

/// True for the investigator's `agent_type`, plugin-scoped or the bare name a
/// project-level copy of the agent carries.
pub fn is_investigator(agent_type: Option<&str>) -> bool {
    matches!(agent_type, Some(INVESTIGATOR) | Some("investigator"))
}

/// The most investigator launches one criterion's climb gets, after which the
/// hold becomes the person's card.
pub const INVESTIGATOR_LAUNCHES_MAX: usize = 2;

pub fn launches_spent(v: &View, c: &str) -> bool {
    ladder::climb_of(v, c).len() >= INVESTIGATOR_LAUNCHES_MAX
}

pub fn rung(v: &View, c: &str) -> Option<&'static str> {
    if launches_spent(v, c) {
        return None;
    }
    ladder::investigator_rung(v, c)
}

/// The first criterion in plan order, held in `session`, that is owed an
/// investigator, with its holder and rung. The rules allow one per session.
pub fn offered(v: &View, session: &str) -> Option<(String, String, &'static str)> {
    plan::criteria(v).into_iter().find_map(|c| {
        let h = plan::assigned_to(v, &c)?;
        if ladder::session_of(Some(h)) != Some(session) {
            return None;
        }
        if !sent_for(v, &c) {
            return None;
        }
        let r = rung(v, &c)?;
        Some((c.clone(), h.to_string(), r))
    })
}

/// Write the facts this hook call carries before anything is decided, and
/// push them onto `log`.
pub(crate) fn hook_facts(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    match input.event {
        HookEvent::SessionStart if input.agent_id.is_none() => {
            if let (Some(conv), Some(m)) = (input.session_conversation_id(), input.model.as_deref()) {
                model_seen(record, log, &conv, m, now)?;
            }
        }
        HookEvent::PostModelSwitch => {
            if let (Some(conv), Some(m)) = (input.agent_conversation_id(), input.to_model.as_deref()) {
                model_seen(record, log, &conv, m, now)?;
            }
        }
        HookEvent::SubagentStart if is_investigator(input.agent_type.as_deref()) => started(record, input, log, now)?,
        HookEvent::PostToolUse if input.is_spawn() => {
            if let Some(conv) = input.spawned_conversation_id() {
                if let Some(m) = input.response_text("resolvedModel") {
                    model_seen(record, log, &conv, m, now)?;
                }
                transfer_back(record, log, &conv, now)?;
            }
        }
        HookEvent::SubagentStop if is_investigator(input.agent_type.as_deref()) => {
            // A background investigator's PostToolUse came at its launch, so
            // its stop is when it can be green.
            if let Some(conv) = input.agent_conversation_id() {
                transfer_back(record, log, &conv, now)?;
            }
        }
        _ => {}
    }
    session_model_from_transcript(record, input, log, now)
}

pub(crate) fn sent_for(v: &View, c: &str) -> bool {
    lifecycle::holds(v, c).iter().any(|h| ladder::SENT_FOR.contains(&h.why.as_str()))
}

fn write(record: &Record, log: &mut Log, kind: &str, fields: Value, now: f64) -> Result<(), String> {
    let fields: Map<String, Value> = fields.as_object().cloned().unwrap_or_default();
    let fact = record.append_raw(kind, fields, now, TENANT).map_err(|e| cannot("write its record", &e))?;
    log.push(fact);
    Ok(())
}

/// Write `model_seen` unless `model` is already the conversation's latest.
pub(crate) fn model_seen(record: &Record, log: &mut Log, conv: &str, model: &str, now: f64) -> Result<(), String> {
    if model.is_empty() || ladder::model_of(&View::new(log), Some(conv)) == Some(model) {
        return Ok(());
    }
    write(record, log, "model_seen", json!({ "agent_conversation_id": conv, "model": model }), now)
}

/// At the investigator's SubagentStart, hand the offered criterion to it,
/// recording the requested rung as its model until `resolvedModel` arrives.
fn started(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    let (Some(new), Some(session)) = (input.agent_conversation_id(), input.session_conversation_id()) else {
        return Ok(());
    };
    let v = View::new(log);
    let named = log.iter().any(|f| {
        ["agent_conversation_id", "from_conversation", "to_conversation"]
            .iter()
            .any(|k| f.fields.get(*k).and_then(Value::as_str) == Some(new.as_str()))
    });
    // A session the binary launched is `<session>/<own>`; take the session part.
    let session = ladder::session_of(Some(&session)).unwrap_or_default().to_string();
    let Some((c, holder, rung)) = offered(&v, &session).filter(|_| !named) else {
        return Ok(()); // a resumed investigator, or nothing held for one
    };
    hand_over(record, log, &c, &holder, &new, rung, now).map(|_| ())
}

/// Write `model_seen` then `criterion_transfer` from `holder` to `new`, and
/// return whether they were written. They are tried on a copy first and
/// written only when the rules read the pair as an investigator transfer.
pub(crate) fn hand_over(record: &Record, log: &mut Log, c: &str, holder: &str, new: &str, rung: &str, now: f64) -> Result<bool, String> {
    let seen = json!({ "agent_conversation_id": new, "model": rung });
    let transfer = json!({ "criterion_id": c, "from_conversation": holder, "to_conversation": new });
    let mut probe = append_raw(log, "model_seen", seen.as_object().cloned().unwrap_or_default(), now, TENANT).map_err(|e| cannot("write its record", &e))?;
    probe = append_raw(&probe, "criterion_transfer", transfer.as_object().cloned().unwrap_or_default(), now, TENANT).map_err(|e| cannot("write its record", &e))?;
    let pv = View::new(&probe);
    let takes = ladder::investigator_of(&pv, c).and_then(|f| f.fields.get("to_conversation")).and_then(Value::as_str) == Some(new);
    if !takes {
        return Ok(false);
    }
    write(record, log, "model_seen", seen, now)?;
    write(record, log, "criterion_transfer", transfer, now)?;
    Ok(true)
}

/// Hand each criterion the investigator `conv` has finished back to the
/// conversation its climb started from.
pub(crate) fn transfer_back(record: &Record, log: &mut Log, conv: &str, now: f64) -> Result<(), String> {
    let backs: Vec<(String, String)> = {
        let v = View::new(log);
        plan::criteria(&v)
            .into_iter()
            .filter_map(|c| {
                let inv = ladder::investigator_of(&v, &c)?;
                if inv.fields.get("to_conversation").and_then(Value::as_str) != Some(conv) || !ladder::hands_back(&v, &c, inv) {
                    return None;
                }
                let first = ladder::climb_of(&v, &c).first()?.fields.get("from_conversation")?.as_str()?.to_string();
                Some((c, first))
            })
            .collect()
    };
    for (c, to) in backs {
        write(record, log, "criterion_transfer", json!({ "criterion_id": c, "from_conversation": conv, "to_conversation": to }), now)?;
    }
    Ok(())
}

/// Record the session's model from its transcript when a criterion it holds
/// is owed an investigator and its model is unknown.
fn session_model_from_transcript(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    from_transcript(record, input, log, now, sent_for)
}

/// Record the session's model from its transcript before the judge runs on a
/// criterion in `sealed`. Headless Claude Code names no model at SessionStart.
pub(crate) fn holder_model_for_the_judge(record: &Record, input: &HookInput, log: &mut Log, now: f64, sealed: &[String]) -> Result<(), String> {
    from_transcript(record, input, log, now, |_, c| sealed.iter().any(|s| s == c))
}

/// Write the transcript's last assistant model for the session when it has no
/// model seen and holds a criterion `needs` names; write nothing if unknown.
fn from_transcript(record: &Record, input: &HookInput, log: &mut Log, now: f64, needs: impl Fn(&View, &str) -> bool) -> Result<(), String> {
    let (Some(session), Some(path)) = (input.session_conversation_id(), input.transcript_path.as_deref()) else {
        return Ok(());
    };
    let needed = {
        let v = View::new(log);
        ladder::model_of(&v, Some(&session)).is_none()
            && plan::criteria(&v).iter().any(|c| plan::assigned_to(&v, c) == Some(session.as_str()) && needs(&v, c))
    };
    if !needed {
        return Ok(());
    }
    match crate::input::transcript::last_assistant_model(std::path::Path::new(path)) {
        Some(m) => model_seen(record, log, &session, &m, now),
        None => Ok(()),
    }
}
