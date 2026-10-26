//! After a pick (plan §6.7): the holder's line is the pick's `AFTER_PICK` row read on the log
//! after it, `{next part}` in its three cases; a pick with no row keeps today's line.

use familiar_ai::teach::{picker, words};
use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).expect("appended")
}

/// The holder's line for the pick `(hold, choice)` on c1.
fn told(log: &Log, hold: &str, choice: &str) -> String {
    let after = raw(log, "hold_answered", json!({"criterion_id": "c1", "hold": hold, "choice": choice, "typed": ""}));
    picker::told_after(&View::new(log), &View::new(&after), "c1", hold, choice, &words::choice(choice))
}

fn q(log: &Log, c: &str) -> String {
    words::quoted(&View::new(log), c)
}

#[test]
fn a_release_on_one_string_names_the_part_it_starts() {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let line = told(&log, "two_failures", "release");
    assert!(line.ends_with(&format!("{} is released. Next: {} starts.", q(&log, "c1"), q(&log, "c2"))), "{line}");
}

#[test]
fn a_release_beside_another_string_says_what_is_still_in_play() {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let log = raw(&log, "criterion_head", json!({"criterion_id": "c2"}));
    let line = told(&log, "two_failures", "release");
    assert!(line.ends_with(&format!("Next: still in play: {} (testing).", q(&log, "c2"))), "{line}");
}

#[test]
fn a_release_of_the_last_part_says_the_plan_is_complete() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let line = told(&log, "crashed", "release");
    assert!(line.ends_with("Next: no part is left; the plan is complete."), "{line}");
}

#[test]
fn try_again_says_two_more_tries() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let line = told(&log, "two_test_broken", "try_again");
    assert!(line.contains(&format!("Try again on {}: ", q(&log, "c1"))) && line.ends_with("Two more tries before this returns to the person."), "{line}");
}

#[test]
fn try_again_on_two_tries_not_red_says_the_tests_passed() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let line = told(&log, "two_tries_not_red", "try_again");
    assert!(line.contains("its tests passed on the code as it is, before any fix; write a test that fails there"), "{line}");
    assert!(!line.contains("a failure"), "{line}");
}

#[test]
fn its_the_red_and_its_the_pass_say_what_the_persons_word_did() {
    // H-4 (the owner, 2026-10-19): the person confirmed what Familiar couldn't read
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let line = told(&log, "run_unclassified", "its_the_red");
    assert!(line.contains(&format!("{} is sealed on the run the person read as the red", q(&log, "c1"))) && line.contains("the test stays as it is"), "{line}");
    let line = told(&log, "run_unclassified", "its_the_pass");
    assert!(line.contains(&format!("{} is green on the run the person read as the pass", q(&log, "c1"))), "{line}");
}

#[test]
fn a_pick_with_no_row_keeps_todays_line() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    for (hold, choice) in [("anchor_moved", "keep_going"), ("two_failures", "take_over")] {
        let line = told(&log, hold, choice);
        assert_eq!(line, picker::told(&View::new(&log), "c1", &words::choice(choice)), "{hold} {choice}");
        assert!(line.ends_with("nothing is left to claim for it."), "{line}");
    }
}
