//! What the agent reads around the cases card (the owner's plan of 2026-10-25, §1b, §3) [draft words,
//! draft names]: the holds' words (the case-finder owed names the Agent tool and its subagent type;
//! the card names the person and AskUserQuestion), the plan's line 3 while the finder is owed, the
//! card is open or the cases the person asked for are not in the promise yet, and the line after
//! the person's pick.

mod cases_fixture;

use cases_fixture::*;
use familiar_ai::teach::example::Project;
use familiar_ai::teach::{picker, words};
use familiar_ai_rules::View;

fn line_3(log: &familiar_ai_facts::Log) -> String {
    let root = std::env::temp_dir();
    words::plan_checklist(&Project { log, root: &root })[3].clone()
}

#[test]
fn the_holds_read_as_what_comes_before_the_test() {
    let log = planned();
    let v = View::new(&log);
    assert_eq!(
        words::hold(&v, Some("c1"), "cases_owed"),
        "before the test, a case-finder looks for the checks the planned cases miss: start it with the Agent tool, subagent_type familiar-ai:case-finder; the binary gives it its brief"
    );
    assert_eq!(
        words::hold(&v, Some("c1"), "cases_unconfirmed"),
        "before the test, the person checks the cases it will cover and the ones the case-finder found: ask them with AskUserQuestion"
    );
}

#[test]
fn line_3_waits_on_the_finder_then_the_card_then_the_cases_asked_for() {
    let log = planned();
    let row = line_3(&log);
    assert!(row.starts_with("    3 full path only  "), "{row}");
    assert!(row.ends_with(&format!("\"{PART}\": the case-finder looks for the checks the planned cases miss: start it with the Agent tool, subagent_type familiar-ai:case-finder")), "{row}");
    let log = found(&log, "found", 3);
    assert!(line_3(&log).ends_with(&format!("\"{PART}\": the person checks the cases it will cover: ask them with AskUserQuestion")), "{}", line_3(&log));
    let log = answered(&log, "typed", "1 and 3");
    assert!(
        line_3(&log).ends_with(&format!(
            "\"{PART}\": the person asked for a1 (given an all-hidden group under --tb=short, line and native → expect the message in each style); a3 (given a plain exception whose frames are all hidden → expect the message, no IndexError): claim promise p1 again with each added, its kind and a short line"
        )),
        "{}",
        line_3(&log)
    );
}

#[test]
fn after_the_pick_the_agent_is_told_what_to_claim_or_to_write_the_test() {
    let before = found(&planned(), "found", 3);
    let told = |choice: &str, typed: &str| {
        let after = answered(&before, choice, typed);
        picker::told_after(&View::new(&before), &View::new(&after), "c1", "cases_unconfirmed", choice, "x")
    };
    let head = format!("familiar-ai: the person answered the question for \"{PART}\": x. The answer is recorded. ");
    assert_eq!(
        told("add_all", ""),
        format!("{head}Add the cases the person asked for to the promise of \"{PART}\" and claim it again: a1 (given an all-hidden group under --tb=short, line and native → expect the message in each style); a2 (given a nested group whose frames are all hidden → expect the message, no IndexError); a3 (given a plain exception whose frames are all hidden → expect the message, no IndexError). Then write its test.")
    );
    assert_eq!(
        told("typed", "an empty group prints nothing"),
        format!("{head}Add the cases the person asked for to the promise of \"{PART}\" and claim it again: p1 (the person's words: \"an empty group prints nothing\"). Then write its test.")
    );
    assert_eq!(told("keep_planned", ""), format!("{head}The planned cases stand for \"{PART}\": write its test."));
    let none = found(&planned(), "none", 0);
    let after = answered(&none, "look_again", "");
    assert_eq!(
        picker::told_after(&View::new(&none), &View::new(&after), "c1", "cases_unconfirmed", "look_again", "x"),
        format!("{head}One more case-finder for \"{PART}\": start it with the Agent tool, subagent_type familiar-ai:case-finder.")
    );
}
