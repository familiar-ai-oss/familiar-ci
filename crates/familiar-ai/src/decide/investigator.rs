//! The hook's decisions around the investigator; the rung comes from `familiar_ai_rules::ladder`.
//! Every reason states a fact and never gives a command, because the agent paraphrases injected text.
//! The investigator by layer: decide/ the spawn and the wait · door/ the facts, the run, the brief · teach/ the words.

use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::{ladder, lifecycle, plan, View};
use serde_json::Value;

use crate::decide::decision::{mentions_git_commit, Decision};
use crate::door::investigator::{is_investigator, INVESTIGATOR};
use crate::door::shell_write::written_paths;
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::words::{quoted, LEAD};

/// What the hook answers about the investigator: the spawn rewritten, or a decision.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The whole input as `updatedInput`, with the reason and the person's line.
    Rewrite { input: Value, reason: String, message: String },
    Decided(Decision),
}

fn to_of(f: &Fact) -> Option<&str> {
    f.fields.get("to_conversation").and_then(Value::as_str)
}

fn from_of(f: &Fact) -> Option<&str> {
    f.fields.get("from_conversation").and_then(Value::as_str)
}

/// Rewrite a held holder's spawn to the investigator at its rung, allow one investigator at a time,
/// and hold back the holder's writes while one works. The reference model has none of these, so they live here;
/// see docs/decisions.md#two-failures-send-an-investigator.
pub fn decide(input: &HookInput, log: &Log) -> Option<Answer> {
    let me = input.agent_conversation_id()?;
    let v = View::new(log);
    match input.event {
        HookEvent::PreToolUse if input.is_spawn() => spawn(input, &v, &me),
        HookEvent::PreToolUse if writes(input) => waiting(&v, &me).map(|reason| Answer::Decided(Decision::Refuse { reason })),
        HookEvent::SubagentStop if is_investigator(input.agent_type.as_deref()) => {
            answered_since(&v, &me).then(|| Answer::Decided(Decision::allow()))
        }
        _ => None,
    }
}

/// A criterion this conversation holds that is held on a hold in `ladder::SENT_FOR`, with its rung and the hold.
fn held_here(v: &View, me: &str) -> Option<(String, &'static str, &'static str)> {
    plan::criteria(v).into_iter().find_map(|c| {
        if plan::assigned_to(v, &c) != Some(me) {
            return None;
        }
        let hs = lifecycle::holds(v, &c);
        let why = ladder::SENT_FOR.iter().copied().find(|w| hs.iter().any(|h| h.why == *w))?;
        crate::door::investigator::rung(v, &c).map(|r| (c, r, why))
    })
}

/// An open investigator in this conversation's session: its criterion and the transfer that opened it.
fn open_in_session<'a>(v: &View<'a>, me: &str) -> Option<(String, &'a Fact)> {
    let s = ladder::session_of(Some(me));
    plan::criteria(v).into_iter().find_map(|c| {
        let inv = ladder::investigator_of(v, &c)?;
        (ladder::session_of(to_of(inv)) == s).then_some((c, inv))
    })
}

fn spawn(input: &HookInput, v: &View, me: &str) -> Option<Answer> {
    if let Some((c, r, why)) = held_here(v, me) {
        if input.input_text("subagent_type") == Some(INVESTIGATOR) && input.input_text("model") == Some(r) {
            return None; // already the investigator at the rung
        }
        let mut updated = input.tool_input.as_ref().and_then(Value::as_object).cloned().unwrap_or_default();
        updated.insert("subagent_type".into(), Value::from(INVESTIGATOR));
        updated.insert("model".into(), Value::from(r));
        let q = quoted(v, &c);
        let reason = format!(
            "{LEAD}{q}: {}. This spawn runs as {INVESTIGATOR} on {r}, one rung up the ladder from this conversation, and {q} goes to it when it starts.",
            crate::teach::words::hold(v, Some(&c), why)
        );
        let message = format!("{LEAD}a stronger model ({r}) is taking {q} over");
        return Some(Answer::Rewrite { input: Value::Object(updated), reason, message });
    }
    let (c, inv) = open_in_session(v, me)?;
    let who = to_of(inv).unwrap_or("an investigator");
    let rung = ladder::rung_of(v, to_of(inv)).unwrap_or("unknown");
    let reason = format!(
        "{LEAD}{} is with a stronger model ({rung}, {who}) until its tests pass; one runs at a time in a conversation, so this spawn waits for it",
        quoted(v, &c)
    );
    Some(Answer::Decided(Decision::Refuse { reason }))
}

/// True for what waits while an investigator holds the criterion: an edit, a commit or a shell write.
fn writes(input: &HookInput) -> bool {
    input.file_path().is_some() || input.command().is_some_and(|c| mentions_git_commit(c) || !written_paths(c).is_empty())
}

/// True when the person answered the hold on `c` after the transfer to the investigator `inv`.
fn answered_after(v: &View, c: &str, inv: &Fact) -> bool {
    v.of("hold_answered", c).any(|f| f.t > inv.t && f.fields.get("hold").and_then(Value::as_str).is_some_and(|h| ladder::SENT_FOR.contains(&h)))
}

/// The refusal for a conversation the open investigator took a criterion from: its edits wait
/// for the transfer back, unless the person answered the card since.
fn waiting(v: &View, me: &str) -> Option<String> {
    plan::criteria(v).into_iter().find_map(|c| {
        let chain = ladder::chain_of(v, &c);
        let inv = *chain.last()?;
        let mine = chain.iter().any(|f| from_of(f) == Some(me));
        if !mine || to_of(inv) == Some(me) || answered_after(v, &c, inv) {
            return None;
        }
        let who = to_of(inv).unwrap_or("an investigator");
        let rung = ladder::rung_of(v, to_of(inv)).unwrap_or("unknown");
        Some(format!(
            "{LEAD}{} is with a stronger model ({rung}, {who}) until its tests pass; this conversation's edits and commits wait for it to come back",
            quoted(v, &c)
        ))
    })
}

/// True when the investigator `me` holds a criterion whose card the person answered since the transfer,
/// so its stop is the person's decision and is not refused.
fn answered_since(v: &View, me: &str) -> bool {
    plan::criteria(v).into_iter().any(|c| {
        ladder::investigator_of(v, &c).is_some_and(|inv| to_of(inv) == Some(me) && answered_after(v, &c, inv))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use familiar_ai_rules::moves::criterion_admissible;
    use familiar_ai_rules::sample::{apply_move, criterion_fixture};
    use serde_json::json;

    fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
        let v = View::new(log);
        let m = criterion_admissible(&v)
            .into_iter()
            .find(|m| m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome") == Some(&json!(o))));
        m.and_then(|m| apply_move(log, &m, 0.0, "t").ok()).unwrap_or_else(|| panic!("no {label}"))
    }

    fn commit(log: &Log, label: &str, outcome: &str) -> Log {
        let mut log = step(log, label, None);
        if label == "tests_only" {
            log = step(&log, "test_written", None);
        }
        log = step(&log, "pass_please_pass", Some(outcome));
        log = step(&log, "pass_please_pass", Some("passed"));
        step(&log, "fingerprint_stamped", None)
    }

    /// The sample's c1 held on two_failures; its holder `a1` on haiku.
    fn held() -> Log {
        let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
        let seen = json!({ "agent_conversation_id": "a1", "model": "claude-haiku-4-5-20251001" });
        let log = append_raw(&log, "model_seen", seen.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default();
        let log = step(&log, "start_check", None);
        let log = commit(&log, "tests_only", "failed");
        let log = commit(&log, "code", "failed");
        commit(&log, "code", "failed")
    }

    fn input(event: &str, agent: Option<&str>, rest: Value) -> HookInput {
        let mut j = json!({ "hook_event_name": event, "session_id": "a1" });
        if let Some(a) = agent {
            j["agent_id"] = json!(a);
            j["agent_type"] = json!(INVESTIGATOR);
        }
        if let (Some(o), Some(r)) = (j.as_object_mut(), rest.as_object()) {
            o.extend(r.clone());
        }
        let e = HookEvent::from_name(event).unwrap_or(HookEvent::Stop);
        HookInput::parse(&j.to_string(), e).unwrap_or_else(|e| panic!("{e}"))
    }

    fn spawn_of(subagent_type: &str, model: &str) -> HookInput {
        input("PreToolUse", None, json!({ "tool_name": "Agent", "tool_input": { "prompt": "p", "subagent_type": subagent_type, "model": model } }))
    }

    #[test]
    fn the_holders_spawn_is_rewritten_to_the_rung_and_left_alone_when_it_already_is() {
        let log = held();
        match decide(&spawn_of("general-purpose", "haiku"), &log) {
            Some(Answer::Rewrite { input, reason, message }) => {
                assert_eq!(input, json!({ "prompt": "p", "subagent_type": INVESTIGATOR, "model": "sonnet" }));
                assert!(reason.starts_with("familiar-ai: \"a right password opens a session\": two tries failed; a stronger model is taking over."), "{reason}");
                assert_eq!(message, "familiar-ai: a stronger model (sonnet) is taking \"a right password opens a session\" over");
                assert!(!reason.contains("spawn the") && !reason.contains("you must"), "a fact, not a command: {reason}");
            }
            other => panic!("expected the rewrite, got {other:?}"),
        }
        assert_eq!(decide(&spawn_of(INVESTIGATOR, "sonnet"), &log), None);
        let edit = input("PreToolUse", None, json!({ "tool_name": "Edit", "tool_input": { "file_path": "src/login.py" } }));
        assert_eq!(decide(&edit, &log), None, "nothing is open yet");
    }

    #[test]
    fn while_it_is_open_another_spawn_and_the_holders_edits_are_refused() {
        let log = step(&held(), "investigator", None); // a1 → a1/i1, sonnet
        match decide(&spawn_of("general-purpose", "haiku"), &log) {
            Some(Answer::Decided(Decision::Refuse { reason })) => assert!(reason.contains("one runs at a time in a conversation, so this spawn waits"), "{reason}"),
            other => panic!("expected the refusal, got {other:?}"),
        }
        let edit = |agent| input("PreToolUse", agent, json!({ "tool_name": "Edit", "tool_input": { "file_path": "src/login.py" } }));
        match decide(&edit(None), &log) {
            Some(Answer::Decided(Decision::Refuse { reason })) => {
                assert!(reason.starts_with("familiar-ai: \"a right password opens a session\" is with a stronger model (sonnet, a1/i1)"), "{reason}")
            }
            other => panic!("expected the holder to wait, got {other:?}"),
        }
        assert_eq!(decide(&edit(Some("i1")), &log), None, "the investigator's own edit goes to the rules");
        let read = input("PreToolUse", None, json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } }));
        assert_eq!(decide(&read, &log), None, "a read waits for nothing");
        let stop = input("SubagentStop", Some("i1"), json!({}));
        assert_eq!(decide(&stop, &log), None, "the model's stop 4 decides");
        let answered = append_raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "two_failures", "choice": "try_again", "typed": "" }).as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default();
        assert_eq!(decide(&stop, &answered), Some(Answer::Decided(Decision::allow())));
        assert_eq!(decide(&edit(None), &answered), None, "the person decided");
    }
}
