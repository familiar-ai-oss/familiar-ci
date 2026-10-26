//! The cases card as the person sees it (the owner's plan of 2026-10-25, §3, wording approved by the
//! owner) [draft names]: no ids, no labels, no field names; one line per planned check, the found
//! checks numbered; the options by the finding, the recommended one marked; Claude Code's own
//! "Other" takes words typed, so the card offers no "something else". The agent writes the card's
//! lines word for word, and the hook refuses a question that leaves one out; it is never asked for
//! "you asked", "what happened" or a sentence of its own.

mod cases_fixture;

use cases_fixture::*;
use familiar_ai::teach::{card, person_words as pw, picker, words};
use familiar_ai_rules::terminal::{self, Card};
use familiar_ai_rules::View;
use serde_json::json;

fn the_card(log: &familiar_ai_facts::Log) -> Card {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == "cases_unconfirmed").unwrap_or_else(|| panic!("the cases card"))
}

fn lines_found() -> Vec<String> {
    vec![
        format!("Before I write the test for \"{PART}\", here's what it will check:"),
        String::new(),
        "  ✓ a group where every frame is hidden shows the message".into(),
        "  ✓ a normal group looks the same as before".into(),
        String::new(),
        "Also worth checking:".into(),
        "  1. the same with the other --tb styles (short, line, native)".into(),
        "  2. a group inside another group".into(),
        "  3. a plain error with every frame hidden".into(),
    ]
}

const OTHER_FOUND: &str = "(Other: say which to add, like \"1 and 3\", or describe one)";
const OTHER_NONE: &str = "(Other: describe a case to add)";

#[test]
fn the_card_is_a_card_the_person_answers() {
    assert!(card::is_card_hold("cases_unconfirmed"));
}

#[test]
fn found_cases_read_as_approved_with_add_all_recommended() {
    let log = found(&planned(), "found", 3);
    let v = View::new(&log);
    let c = the_card(&log);
    assert_eq!(pw::lines(&pw::asked(&v, &c)), lines_found());
    assert_eq!(
        picker::card_options(&v, &c),
        json!([
            { "label": "Add all three (Recommended)", "description": "the test checks all five" },
            { "label": "Just the first two", "description": "the test checks the two above" }
        ])
    );
    assert_eq!(picker::choice_of(&v, &c, "Add all three (Recommended)").as_deref(), Some("add_all"));
    assert_eq!(picker::choice_of(&v, &c, "Just the first two").as_deref(), Some("keep_planned"));
    assert_eq!(picker::choice_of(&v, &c, "1 and 3"), None, "words typed are the picker's typed answer");
}

#[test]
fn the_terminal_shows_the_card_below_its_question() {
    let log = found(&planned(), "found", 3);
    let v = View::new(&log);
    let c = the_card(&log);
    let shown = words::card(&v, &c);
    let want = format!(
        "familiar-ai: a question for you · Before I write the test for \"{PART}\", here's what it will check:
\n    ✓ a group where every frame is hidden shows the message
    ✓ a normal group looks the same as before
\n  Also worth checking:
    1. the same with the other --tb styles (short, line, native)
    2. a group inside another group
    3. a plain error with every frame hidden
\n    • Add all three (Recommended)   the test checks all five
    • Just the first two            the test checks the two above
    {OTHER_FOUND}"
    );
    assert_eq!(shown, want);
}

#[test]
fn none_found_and_a_failed_finder_read_as_approved() {
    let log = found(&planned(), "none", 0);
    let v = View::new(&log);
    let c = the_card(&log);
    let lines = pw::lines(&pw::asked(&v, &c));
    assert_eq!(lines[4..], ["".to_string(), "Nothing else seemed worth checking.".to_string()]);
    assert_eq!(
        picker::card_options(&v, &c),
        json!([{ "label": "Looks right (Recommended)", "description": "I'll write the test" }, { "label": "Look again", "description": "one more search" }])
    );
    assert_eq!(picker::choice_of(&v, &c, "Look again").as_deref(), Some("look_again"));
    let shown = picker::terminal(&v, &c);
    assert_eq!(shown[shown.len() - 3..], ["    • Looks right (Recommended)     I'll write the test".to_string(), "    • Look again                    one more search".to_string(), format!("    {OTHER_NONE}")]);

    let log = found(&planned(), "failed", 0);
    let v = View::new(&log);
    let c = the_card(&log);
    assert_eq!(pw::lines(&pw::asked(&v, &c))[5], "I couldn't finish looking for more checks.");
    assert_eq!(picker::card_options(&v, &c), json!([{ "label": "Test these", "description": "I'll write the test" }, { "label": "Look again", "description": "one more search" }]));
}

/// "Keep what fits" (the owner's ruling, 2026-10-25) [draft words]: when the binary kept the
/// finder's first eight cases, the card says how many it left out, on its own line after the found
/// checks, set apart as the card's other sayings are; the agent's question must quote it, and the
/// agent is told it with the card's lines.
#[test]
fn the_card_says_how_many_of_the_finders_cases_were_left_out() {
    let note = "the finder listed 9, the first 8 are kept: 1 left out";
    let eight: Vec<serde_json::Value> = (1..=8).map(|i| json!({ "case_id": format!("a{i}"), "short": format!("found case {i}"), "given": "g", "expect": "e", "why": "w" })).collect();
    let log = raw(&planned(), "cases_found", json!({ "criterion_id": "c1", "outcome": "found", "cases": eight, "why": note, "model": "claude-sonnet-5-5", "tokens": 4000 }));
    let v = View::new(&log);
    let c = the_card(&log);
    let a = pw::asked(&v, &c);
    let mut want = lines_found()[..6].to_vec();
    want.extend((1..=8).map(|i| format!("  {i}. found case {i}")));
    want.extend([String::new(), note.to_string()]);
    assert_eq!(pw::lines(&a), want);

    let shown = picker::terminal(&v, &c);
    let at = shown.iter().position(|l| l == "    8. found case 8").unwrap_or_else(|| panic!("{shown:#?}"));
    assert_eq!(shown[at + 1..at + 3], [String::new(), format!("  {note}")], "{shown:#?}");

    let whole = format!("{}\n{OTHER_FOUND}", want.join("\n"));
    assert_eq!(pw::missing(&a, &whole), Vec::<String>::new());
    assert_eq!(pw::missing(&a, &whole.replace(note, "")), [format!("the line \"{note}\"")]);
    let f = picker::facts(&v, &c);
    assert!(f.contains(&format!("\n    8. found case 8\n\n  {note}\n")), "{f}");
}

#[test]
fn a_planned_case_with_no_short_line_reads_as_its_given() {
    let log = found(&planned_with(false), "found", 3);
    let v = View::new(&log);
    assert_eq!(pw::lines(&pw::asked(&v, &the_card(&log)))[3], "  ✓ a group with visible frames");
}

#[test]
fn the_question_quotes_every_line_and_needs_nothing_else() {
    let log = found(&planned(), "found", 3);
    let v = View::new(&log);
    let c = the_card(&log);
    let a = pw::asked(&v, &c);
    let whole = format!("{}\n{OTHER_FOUND}", lines_found().join("\n"));
    assert_eq!(pw::missing(&a, &whole), Vec::<String>::new(), "no 'you asked', no 'what happened', no sentence of the agent's");
    let without = whole.replace("  2. a group inside another group\n", "");
    assert_eq!(pw::missing(&a, &without), ["the line \"2. a group inside another group\""]);
    let without = whole.replace(OTHER_FOUND, "");
    assert_eq!(pw::missing(&a, &without), [format!("the line \"{OTHER_FOUND}\"")]);
    // the agent is told the lines, word for word, and to leave the choices to the hook
    let f = picker::facts(&v, &c);
    assert!(f.starts_with("familiar-ai: the cases card is open for part 1 of what they asked."), "{f}");
    for l in lines_found().iter().filter(|l| !l.is_empty()) {
        assert!(f.contains(&format!("  {l}\n")), "{l} in {f}");
    }
    assert!(f.contains(&format!("  {OTHER_FOUND}\n")), "{f}");
    assert!(f.contains("Ask the person with AskUserQuestion") && f.contains("do not claim hold_answered yourself"), "{f}");
    assert!(!f.contains("In the agent's words"), "{f}");
}
