//! The restrictions the binary enforces beside the rules crate's decision, each where the rules cannot see
//! what it needs; see docs/decisions.md#pure-rules-in-their-own-crate.
//! Each function decides and none words; the words are `teach`'s and `tell`'s.

use std::path::Path;

use familiar_ai_facts::Log;
use familiar_ai_rules::{lifecycle, plan, terminal, View};

use crate::decide::decision::Decision;
use crate::decide::picker;
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach;
use crate::teach::example::Project;

/// While no run is open, refuse an act of code until the intent is claimed and allow anything else.
/// An act of code is read from the tool call and the disk, which the rules cannot read; see
/// docs/decisions.md#nothing-is-governed-until-code-changes.
pub(crate) fn intent_first(input: &HookInput, log: &Log, root: &Path) -> Decision {
    match crate::door::person::act(input, root).and_then(|a| a.code().cloned().map(|t| (a.landed, t))) {
        Some((landed, code)) => {
            let reason = teach::person::demanded(log, &code.path, code.code.as_ref(), &Project { log, root });
            let shell = input.file_path().is_none() && !landed;
            Decision::Refuse { reason: if shell { format!("{reason}\n{}", crate::door::shell_write::COUNTS_AS_EDIT) } else { reason } }
        }
        None => Decision::allow(),
    }
}

/// In a run after a finished one, refuse an act of code until this run has its own criterion.
/// The plan's lines still hold from the first run, so the rules alone would let it through.
pub(crate) fn criterion_first(decided: Decision, input: &HookInput, log: &Log, root: &Path, idle: bool) -> Decision {
    if !idle
        && matches!(decided, Decision::Allow { .. })
        && !teach::person::run_has_criterion(log)
        && crate::door::person::act(input, root).is_some_and(|a| a.code().is_some())
    {
        return Decision::Refuse { reason: teach::person::criterion_first(log, &Project { log, root }) };
    }
    decided
}

/// In an open run, refuse an allowed stop while a prompt of the person's is unabsorbed.
/// The reference model does not have the person's prompts; see
/// docs/decisions.md#asks-answers-and-notes.
pub(crate) fn unabsorbed_prompt(decided: Decision, input: &HookInput, log: &Log, root: &Path, idle: bool) -> Decision {
    if input.event != HookEvent::Stop || idle || !matches!(decided, Decision::Allow { .. }) {
        return decided;
    }
    match teach::person::unabsorbed(log).first() {
        Some(p) => Decision::Refuse { reason: teach::person::refusal(p, &Project { log, root }) },
        None => decided,
    }
}

/// A stop blocked on an open card: the card, and the refusal that already stood, if one did.
pub(crate) struct CardOpen {
    pub card: terminal::Card,
    pub refused: Option<String>,
}

/// Block a Stop while a card is open for this conversation, whatever the rules decided.
/// The reference model has no question card; see
/// docs/decisions.md#holds-are-the-persons.
pub(crate) fn card_open_at_stop(decided: &Decision, input: &HookInput, log: &Log) -> Option<CardOpen> {
    if input.event != HookEvent::Stop {
        return None;
    }
    let card = picker::open_card(&View::new(log), input)?;
    let refused = match decided {
        Decision::Refuse { reason } => Some(reason.clone()),
        Decision::Allow { .. } => None,
    };
    Some(CardOpen { card, refused })
}

/// On the bench, the person's open cards when every criterion not yet green, proven or released waits on one,
/// so a refused stop can go carrying them. The bench is how the binary runs, not a fact in the record.
/// Off `main` only the parts touched in this checkout are read, as its stop reads them (the owner, 2026-10-15).
pub(crate) fn only_the_persons_cards_left(decided: &Decision, input: &HookInput, log: &Log, main: bool) -> Option<Vec<terminal::Card>> {
    if !matches!(input.event, HookEvent::Stop | HookEvent::SubagentStop) || !matches!(decided, Decision::Refuse { .. }) {
        return None;
    }
    let v = View::new(log);
    let ours = |c: &str| main || lifecycle::touched_here(&v, c);
    let holds = teach::card::card_holds();
    // the one question [draft] is the person's card before the plan is confirmed
    let card_hold = |h: &str| holds.contains(&h) || h == familiar_ai_rules::trace::REPRO;
    let cards: Vec<terminal::Card> = terminal::cards(&v).into_iter().filter(|c| card_hold(c.hold.as_str()) && ours(&c.criterion_id)).collect();
    let done = |c: &str| lifecycle::released(&v, c) || matches!(lifecycle::state(&v, c), "green" | "proven");
    let left = plan::criteria(&v).into_iter().filter(|c| !done(c) && ours(c)).all(|c| cards.iter().any(|k| k.criterion_id == c));
    (left && !cards.is_empty()).then_some(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::tell;

    fn stop() -> HookInput {
        HookInput::parse(r#"{"hook_event_name":"Stop"}"#, HookEvent::Stop).expect("stop")
    }

    /// The rules' fixture with c1 held on anchor_moved after an outside edit, its card open.
    fn held(criteria: &[&str]) -> Log {
        let log = familiar_ai_rules::sample::criterion_fixture(criteria, true, false, 0.0, "t").expect("fixture");
        let outside = familiar_ai_rules::moves::criterion_admissible(&View::new(&log))
            .into_iter()
            .find(|m| m.label.as_deref() == Some("outside_edit_seen"))
            .expect("the start check with the lines moved is offered");
        familiar_ai_rules::sample::apply_move(&log, &outside, 1.0, "t").expect("applied")
    }

    #[test]
    fn a_refused_stop_with_only_the_persons_card_left_carries_the_card() {
        let log = held(&["c1"]);
        let refused = Decision::Refuse { reason: "not done yet".into() };
        let cards = only_the_persons_cards_left(&refused, &stop(), &log, true).expect("only the person's card is left");
        let cards = tell::cards_left(&log, &cards);
        assert!(cards.starts_with("familiar-ai: a question for you · \"a right password opens a session\": the code at src/login.py check_password changed after the plan was made. What now?"), "{cards}");
        // an allowed stop, and another event, are not this one's
        assert_eq!(only_the_persons_cards_left(&Decision::allow(), &stop(), &log, true), None);
        let pre = HookInput::parse(r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{}}"#, HookEvent::PreToolUse).expect("pre");
        assert_eq!(only_the_persons_cards_left(&refused, &pre, &log, true), None);
    }

    #[test]
    fn a_criterion_left_to_the_agent_keeps_the_stop_refused() {
        let refused = Decision::Refuse { reason: "not done yet".into() };
        // c1 held on its card, c2 not done and with no card: the agent has work left
        let log = held(&["c1", "c2"]);
        let v = View::new(&log);
        assert_ne!(lifecycle::state(&v, "c2"), "held", "c2 has no card");
        assert_eq!(only_the_persons_cards_left(&refused, &stop(), &log, true), None);
        // no card at all: refused
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        assert_eq!(only_the_persons_cards_left(&refused, &stop(), &log, true), None);
    }

    /// Off main the bench escape reads the parts touched here (the owner, 2026-10-15): a seeded
    /// lane whose own part waits on the person's card goes, whatever it inherited not done.
    #[test]
    fn off_main_only_the_parts_touched_here_are_read() {
        let refused = Decision::Refuse { reason: "not done yet".into() };
        let planned = familiar_ai_rules::sample::criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture");
        let mark = serde_json::json!({ "commit": "base", "plan_lines": planned.len() });
        let seeded = familiar_ai_facts::append_raw(&planned, "record_seeded", mark.as_object().cloned().unwrap_or_default(), 0.0, "t").expect("the mark");
        let outside = familiar_ai_rules::moves::criterion_admissible(&View::new(&seeded))
            .into_iter()
            .find(|m| m.label.as_deref() == Some("outside_edit_seen"))
            .expect("the start check with the lines moved is offered");
        let held = familiar_ai_rules::sample::apply_move(&seeded, &outside, 1.0, "t").expect("applied");
        let claimed = serde_json::json!({ "criterion_id": "c1", "agent_conversation_id": "lane" });
        let log = familiar_ai_facts::append_raw(&held, "criterion_assigned", claimed.as_object().cloned().unwrap_or_default(), 2.0, "t").expect("claimed");
        // c1 is the lane's and waits on the person's card; c2 is inherited, untouched, not done
        let cards = only_the_persons_cards_left(&refused, &stop(), &log, false).expect("only the person's card is left on the lane's parts");
        assert_eq!(cards.iter().map(|c| c.criterion_id.as_str()).collect::<Vec<_>>(), ["c1"]);
        assert_eq!(only_the_persons_cards_left(&refused, &stop(), &log, true), None, "main reads c2 too");
        // before the lane claimed c1 it touched nothing: no card of its own to carry
        assert_eq!(only_the_persons_cards_left(&refused, &stop(), &held, false), None);
    }
}
