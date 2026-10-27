//! The door's check on `hold_answered`, the one fact a person writes through a
//! card. The door cannot see who typed a command, so it refuses a card answer
//! from an agent's shell; see docs/decisions.md#holds-are-the-persons

use familiar_ai_rules::criterion::ANSWERS;
use familiar_ai_rules::{terminal, View};
use serde_json::{Map, Value};

use crate::teach::example::Project;
use crate::door::claim::{command_form, ClaimError};

/// The holds a person answers: the rules' `ANSWERS` plus `no_test_stack`.
/// This restates the terminal's private `person_whys`.
pub fn card_holds() -> Vec<&'static str> {
    ANSWERS.iter().map(|(w, _)| *w).chain(std::iter::once("no_test_stack")).collect()
}

/// Whether a person answers `hold` on a card: one of [`card_holds`], or the card of an ask read
/// more than one way (`reading_unclear`) [draft], whose choices are the agent's readings and which
/// is a card only while they stand, so the listing of the card holds leaves it out; or the one
/// question, how to make it happen (`repro_unknown`) [draft], asked before a pytest part's first try.
pub fn is_card_hold(hold: &str) -> bool {
    card_holds().contains(&hold) || hold == familiar_ai_rules::ask::HOLD || hold == familiar_ai_rules::trace::REPRO
}

/// The card holds as a sentence: "anchor_moved, two_failures and no_test_stack".
pub fn card_holds_words() -> String {
    let hs = card_holds();
    match hs.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => (*last).to_string(),
        None => String::new(),
    }
}

/// Refuses a `hold_answered` whose hold is not a card hold, or one sent with a
/// `session_id`, which Claude Code's Bash tool sets and a person's terminal
/// does not. An agent that unsets the variable gets through, and CI is the
/// backstop; there is no `--as-person` flag because the agent would paste it.
pub fn card_check(
    kind: &str,
    fields: &Map<String, Value>,
    session_id: Option<&str>,
    project: &Project,
) -> Result<(), ClaimError> {
    // the person's pick on a question of the lens panel's (the owner's plan of 2026-10-26) is
    // theirs as a card's answer is: recorded by the picker, never from an agent's shell
    let hold = if kind == familiar_ai_rules::cases::QUESTION_ANSWERED {
        familiar_ai_rules::cases::CARD.to_string()
    } else if kind == "hold_answered" {
        let Some(Value::String(hold)) = fields.get("hold") else {
            return Ok(());
        };
        hold.clone()
    } else {
        return Ok(());
    };
    let hold = &hold;
    if !is_card_hold(hold) {
        return Err(ClaimError::NotACard(hold.clone()));
    }
    if session_id.is_none() {
        return Ok(());
    }
    let command = command_form(kind, fields, Some(project)).unwrap_or_default();
    let v = View::new(project.log);
    let criterion = fields.get("criterion_id").and_then(Value::as_str);
    let card = terminal::cards(&v)
        .iter()
        .find(|c| c.hold == *hold && criterion.is_none_or(|id| c.criterion_id == id))
        .map(|c| crate::teach::words::card(&v, c))
        .unwrap_or_default();
    Err(ClaimError::PersonsCard { command, card })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_released_criterion_reads_released_by_you() {
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let release = fields(&[("criterion_id", json!("c1")), ("hold", json!("two_failures")), ("choice", json!("release")), ("typed", json!(""))]);
        let log = familiar_ai_facts::append_raw(&log, "hold_answered", release, 1.0, "t").expect("released");
        let status = crate::teach::words::status_lines(&log, false);
        assert_eq!(status[1], "familiar-ai: \"a right password opens a session\" · released by you", "{status:?}");
    }

    fn fields(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    fn answer(hold: &str) -> Map<String, Value> {
        fields(&[("criterion_id", json!("c1")), ("hold", json!(hold)), ("choice", json!("keep_going")), ("typed", json!(""))])
    }

    #[test]
    fn the_card_holds_are_the_models_answers_and_the_missing_command() {
        assert_eq!(card_holds(), ["anchor_moved", "two_failures", "two_tries_not_red", "fix_elsewhere", "anchor_untouched", "two_test_broken", "two_crashed", "two_timed_out", "environment_error", "run_unclassified", "retry_spent", "crashed", "command_broke", "miscoped", "reading_gap", "setup_changed", "review_disputed", "cases_unconfirmed", "no_test_stack"]);
        // definition change (the owner's plan of 2026-10-25, the cases card) [draft name]: cases_unconfirmed is a card
        assert_eq!(card_holds_words(), "anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, two_test_broken, two_crashed, two_timed_out, environment_error, run_unclassified, retry_spent, crashed, command_broke, miscoped, reading_gap, setup_changed, review_disputed, cases_unconfirmed and no_test_stack");
    }

    #[test]
    fn a_hold_that_is_not_a_card_is_refused_whoever_asks() {
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let root = std::env::temp_dir();
        let project = Project { log: &log, root: &root };
        for hold in ["not_stamped", "anchor_not_found", "tests_not_red", "released", "planning_incomplete"] {
            for session in [None, Some("s")] {
                let err = card_check("hold_answered", &answer(hold), session, &project).expect_err(hold);
                assert_eq!(err, ClaimError::NotACard(hold.into()), "{hold} {session:?}");
                assert_eq!(
                    err.to_string(),
                    format!("hold_answered answers a card, and {hold} is not a card hold; the card holds are anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, two_test_broken, two_crashed, two_timed_out, environment_error, run_unclassified, retry_spent, crashed, command_broke, miscoped, reading_gap, setup_changed, review_disputed, cases_unconfirmed and no_test_stack")
                );
            }
        }
        // Another kind, or a hold that is not text, is left to other checks.
        assert!(card_check("criterion_transfer", &answer("not_stamped"), Some("s"), &project).is_ok());
        assert!(card_check("hold_answered", &fields(&[("hold", json!(7))]), Some("s"), &project).is_ok());
    }

    #[test]
    fn a_card_hold_from_an_agents_shell_is_refused_with_the_command_and_the_card_and_accepted_from_a_terminal() {
        // An outside edit seen at the start check holds c1 on anchor_moved.
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let outside = familiar_ai_rules::moves::criterion_admissible(&View::new(&log))
            .into_iter()
            .find(|m| m.label.as_deref() == Some("outside_edit_seen"))
            .expect("the start check with the lines moved is offered");
        let log = familiar_ai_rules::sample::apply_move(&log, &outside, 1.0, "t").expect("applied");
        let root = std::env::temp_dir();
        let project = Project { log: &log, root: &root };
        let v = View::new(&log);
        let open: Vec<String> = terminal::cards(&v).iter().map(|c| c.hold.clone()).collect();
        assert_eq!(open, ["anchor_moved"], "the fixture opens the anchor_moved card");
        let err = card_check("hold_answered", &answer("anchor_moved"), Some("84be6339"), &project).expect_err("an agent's shell");
        let text = err.to_string();
        assert!(
            text.starts_with("this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed '')\n"),
            "{text}"
        );
        assert!(
            text.contains("familiar-ai: a question for you · \"a right password opens a session\": the code at src/login.py check_password changed after the plan was made. What now?"),
            "{text}"
        );
        // The choices are worded for the person.
        assert!(text.contains("  what now?\n    1) carry on with part 1 on the code as it is now\n    2) have the agent pick the code for part 1 again\n    3) drop it\n"), "{text}");
        assert!(text.ends_with("  > or type your own answer"), "{text}");
        assert!(!text.contains("--as-person"), "{text}");
        // With no session id the command comes from a person's terminal.
        assert!(card_check("hold_answered", &answer("anchor_moved"), None, &project).is_ok());
        // A card hold with no card open is still refused, with no card shown.
        assert_eq!(
            card_check("hold_answered", &answer("two_failures"), Some("s"), &project),
            Err(ClaimError::PersonsCard {
                command: "familiar-ai claim hold_answered --criterion_id c1 --hold two_failures --choice keep_going --typed ''".into(),
                card: String::new(),
            })
        );
    }
}
