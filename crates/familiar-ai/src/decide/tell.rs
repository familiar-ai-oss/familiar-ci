//! What a hook call tells after its decision. Nothing here allows or refuses; a stop blocked on an open
//! card is decided in `restrict.rs` and only worded here.

use std::path::Path;

use familiar_ai_facts::Log;
use familiar_ai_rules::{terminal, View};

use crate::decide::decision::Decision;
use crate::decide::hook_output::Response;
use crate::decide::picker;
use crate::decide::restrict::CardOpen;
use crate::input::hook_input::{HookEvent, HookInput};

/// The call to action once the work is proven here: the facts for the agent on an allowed stop or a prompt,
/// else the line for the person. A stop the hook already continued once is left to end.
pub(crate) fn call_to_action(out: Response, hook_input: &HookInput, decided: &Decision, log: &Log, root: &Path, for_person: &mut Vec<String>) -> Response {
    use crate::teach::prove;
    // Only in the person's own session, not a subagent's or one the binary launched.
    let asked = match hook_input.event {
        HookEvent::Stop => matches!(decided, Decision::Allow { .. }) && !hook_input.in_subagent() && hook_input.conversation.is_none(),
        HookEvent::UserPromptSubmit => crate::door::person::is_the_persons(hook_input),
        _ => false,
    };
    let Some(step) = asked.then(|| prove::step(log, root, Some(prove::GH))).flatten() else {
        return out;
    };
    if hook_input.event == HookEvent::UserPromptSubmit {
        return out.with_context(hook_input.event, &prove::facts(&step));
    }
    if prove::to_ask(&step) && hook_input.stop_hook_active != Some(true) {
        return out.with_context(hook_input.event, &prove::facts(&step));
    }
    for_person.push(prove::words(&step));
    out
}

/// True when the work is proven here and the push or the pull request is still to come.
pub(crate) fn push_to_ask(log: &Log, root: &Path) -> bool {
    use crate::teach::prove;
    prove::step(log, root, Some(prove::GH)).is_some_and(|s| prove::to_ask(&s))
}

/// Put the judge's line before what the decision tells, or after a refusal's words.
pub(crate) fn judge_first(decided: Decision, line: Option<&str>) -> Decision {
    let Some(line) = line else {
        return decided;
    };
    match decided {
        Decision::Allow { told } => Decision::Allow {
            told: Some(told.map_or_else(|| line.to_string(), |t| format!("{line}\n{t}"))),
        },
        Decision::Refuse { reason } => Decision::Refuse { reason: format!("{reason}\n{line}") },
    }
}

/// `lines` after a refusal's words; an allow is left as it is.
pub(crate) fn refusal_then(decided: Decision, lines: &[String]) -> Decision {
    match decided {
        Decision::Refuse { reason } if !lines.is_empty() => Decision::Refuse { reason: format!("{reason}\n{}", lines.join("\n")) },
        other => other,
    }
}

/// The open card's facts for this conversation, when one is open.
fn card_facts(hook_input: &HookInput, log: &Log) -> Option<String> {
    let v = View::new(log);
    picker::open_card(&v, hook_input).map(|card| crate::teach::picker::facts(&v, &card))
}

/// The first line of a Stop refused while a card is open.
const ASK_NOW: &str = "familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card";

/// The words of a Stop blocked while a card is open: the call to ask, any refusal that stood, the card's facts.
pub(crate) fn card_open_refusal(log: &Log, open: &CardOpen) -> String {
    let facts = crate::teach::picker::facts(&View::new(log), &open.card);
    match &open.refused {
        Some(reason) => format!("{ASK_NOW}\n{reason}\n{facts}"),
        None => format!("{ASK_NOW}\n{facts}"),
    }
}

/// The cards left to the person, in the person's words, one per line.
pub(crate) fn cards_left(log: &Log, cards: &[terminal::Card]) -> String {
    let v = View::new(log);
    cards.iter().map(|c| crate::teach::words::card(&v, c)).collect::<Vec<_>>().join("\n")
}

/// Add the open card's facts as context on a refusal, a prompt, the session's start, or the call that opened it.
pub(crate) fn card_context(out: Response, hook_input: &HookInput, decided: &Decision, before: &Log, log: &Log) -> Response {
    let event = hook_input.event;
    if !matches!(event, HookEvent::PreToolUse | HookEvent::PostToolUse | HookEvent::UserPromptSubmit | HookEvent::SessionStart) {
        return out;
    }
    let Some(facts) = card_facts(hook_input, log) else {
        return out;
    };
    let opened = {
        let (b, v) = (View::new(before), View::new(log));
        let was: Vec<(String, String)> = terminal::cards(&b).into_iter().map(|c| (c.criterion_id, c.hold)).collect();
        picker::open_card(&v, hook_input).is_some_and(|c| !was.contains(&(c.criterion_id, c.hold)))
    };
    // Plan §1.2 (S8): the orchestrator's PostToolUse of the Agent call that returned the card's holder.
    let returned = event == HookEvent::PostToolUse && !hook_input.in_subagent() && hook_input.is_spawn() && {
        let v = View::new(log);
        let holder = picker::open_card(&v, hook_input).and_then(|c| familiar_ai_rules::plan::assigned_to(&v, &c.criterion_id).map(str::to_string));
        holder.is_some() && holder == hook_input.spawned_conversation_id()
    };
    let tell = matches!(decided, Decision::Refuse { .. }) || matches!(event, HookEvent::UserPromptSubmit | HookEvent::SessionStart) || opened || returned;
    if tell {
        out.with_context(event, &facts)
    } else {
        out
    }
}
