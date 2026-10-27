//! What the agent reads around the cases card (the owner's plan of 2026-10-25, §1b, §3) [draft words,
//! draft names]: the holds' words (the lens panel owed names the Agent tool and its subagent type;
//! the card names the person and AskUserQuestion), the plan's line 3 while the panel is owed, the
//! card is open or the cases the person asked for are not in the promise yet, and the line after
//! the person's pick. Updated with the definition (the owner's plan of 2026-10-26, the lens panel,
//! approved): the case-finder's words became the lens panel's.

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
        "before the test, the lens panel reads the plan for the checks the planned cases miss: five lenses, one Agent call each, in parallel, then the synthesis; each runs as familiar-ai:lens, and the binary gives each its brief"
    );
    assert_eq!(
        words::hold(&v, Some("c1"), "cases_unconfirmed"),
        "before the test, the person checks the cases it will cover, the ones the lens panel found and its questions: ask them with AskUserQuestion"
    );
}

#[test]
fn line_3_waits_on_the_panel_then_the_card_then_the_cases_asked_for() {
    let log = planned();
    let row = line_3(&log);
    assert!(row.starts_with("    3 full path only  "), "{row}");
    assert!(
        row.ends_with(&format!("\"{PART}\": the lens panel reads the plan for the checks the planned cases miss: the ambiguity, completeness, happy path, edge cases and order and coupling lenses still to start, one Agent call each, in parallel; each runs as familiar-ai:lens on familiar-ai's brief")),
        "{row}"
    );
    // two lenses out, three answered: the two running
    let out = raw(&raw(&planned(), "lens_released", serde_json::json!({ "criterion_id": "c1", "lens": "ambiguity", "tool_use_id": "t1" })), "lens_released", serde_json::json!({ "criterion_id": "c1", "lens": "completeness", "tool_use_id": "t2" }));
    let out = ["happy_path", "edge_cases", "order_and_coupling"].iter().fold(out, |l, x| raw(&l, "lens_found", serde_json::json!({ "criterion_id": "c1", "lens": x, "outcome": "none", "findings": [], "model": "m", "tokens": 0 })));
    assert!(line_3(&out).ends_with(&format!("\"{PART}\": the lens panel reads the plan for the checks the planned cases miss: the ambiguity and completeness lenses running; each answer is recorded when its call returns")), "{}", line_3(&out));
    // the five answered: the synthesis
    assert!(line_3(&lensed(&planned())).ends_with("the five lenses answered; the synthesis is next, one Agent call; it runs as familiar-ai:lens on familiar-ai's brief, with the five answers and no tools"), "{}", line_3(&lensed(&planned())));
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
        format!("{head}One more lens panel for \"{PART}\": its five lenses, one Agent call each, in parallel, then the synthesis; each runs as familiar-ai:lens on familiar-ai's brief.")
    );
}
