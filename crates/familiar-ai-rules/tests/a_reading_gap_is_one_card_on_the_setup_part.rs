//! The gap card (plan A-4, lane a5): what the dry run could not read is one card on the reserved
//! part `setup` [draft], `reading_gap` [draft], answered `keep_going` or `try_again`; it stands
//! while the latest choice carries gaps with no answer after it, it names the runner and the
//! outcomes that will ask, and it never pauses a part.

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{answers_of, PAUSING};
use familiar_ai_rules::reading::SETUP_PART;
use familiar_ai_rules::sample::{criterion_fixture, PROJECT_ROOT, TEST_COMMAND};
use familiar_ai_rules::{holds, lifecycle, moves, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

fn chose(log: &Log, gaps: &[&str]) -> Log {
    raw(log, "reader_chosen", json!({
        "run_id": format!("d-{}", log.len()), "root": PROJECT_ROOT, "runner": "pytest", "reader": "pytest:reporter",
        "command": TEST_COMMAND, "commit": "base", "familiar": "0.1.0",
        "watched": ["pyproject.toml"], "gaps": gaps, "why": "no_choice",
    }))
}

fn answer(log: &Log, choice: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": SETUP_PART, "hold": "reading_gap", "choice": choice, "typed": "" }))
}

fn setup_whys(log: &Log) -> Vec<String> {
    holds::holds_of(&View::new(log), SETUP_PART).into_iter().map(|h| h.why).collect()
}

fn start() -> Log {
    criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture")
}

#[test]
fn the_card_is_the_persons_with_two_choices_and_never_pauses() {
    assert_eq!(answers_of("reading_gap"), Some(&["keep_going", "try_again"][..]));
    assert!(!PAUSING.contains(&"reading_gap"));
}

#[test]
fn gaps_hold_the_setup_part_until_answered_and_a_new_choice_with_gaps_asks_again() {
    let log = chose(&start(), &["P5", "P6"]);
    assert_eq!(setup_whys(&log), ["reading_gap"]);
    let h = holds::holds_of(&View::new(&log), SETUP_PART);
    let clears: Vec<String> = h[0].clears.iter().filter_map(|m| m.get("choice").and_then(Value::as_str).map(str::to_string)).collect();
    assert_eq!(clears, ["keep_going", "try_again"]);
    assert_eq!(setup_whys(&answer(&log, "keep_going")), Vec::<String>::new());
    assert_eq!(setup_whys(&chose(&answer(&log, "keep_going"), &["P5"])), ["reading_gap"]);
    assert_eq!(setup_whys(&chose(&answer(&log, "try_again"), &[])), Vec::<String>::new());
    assert_eq!(setup_whys(&chose(&start(), &[])), Vec::<String>::new(), "no gaps, no card");
    assert_eq!(setup_whys(&start()), Vec::<String>::new(), "no choice, no card");
}

#[test]
fn the_card_names_the_runner_and_the_gaps_and_offers_both_answers() {
    let log = chose(&start(), &["P5", "P6"]);
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.criterion_id == SETUP_PART).expect("the setup card");
    assert_eq!(card.hold, "reading_gap");
    assert_eq!(card.choices.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["keep_going", "try_again"]);
    // definition change (lane a-surf, plan A-4's "which outcomes will ask"): the gaps are the
    // outcomes in words, never the probes' ids
    for w in ["pytest", "a pass beside a skipped test", "no test selected"] {
        assert!(card.question.contains(w), "{w} in {:?}", card.question);
    }
    for id in ["P5", "P6"] {
        assert!(!card.question.contains(id), "{id} in {:?}", card.question);
    }
    let offered: Vec<String> = moves::criterion_admissible(&v)
        .iter()
        .filter(|m| m.kind == "hold_answered" && m.fields.get("criterion_id").and_then(Value::as_str) == Some(SETUP_PART))
        .filter_map(|m| m.fields.get("choice").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert_eq!(offered, vec!["keep_going".to_string(), "try_again".to_string()]);
}

#[test]
fn the_gap_card_changes_no_parts_state_or_moves() {
    let plain = chose(&start(), &[]);
    let gapped = chose(&start(), &["P5"]);
    let (p, g) = (View::new(&plain), View::new(&gapped));
    assert_eq!(lifecycle::state(&p, "c1"), lifecycle::state(&g, "c1"));
    assert!(!lifecycle::paused(&g, "c1"));
    assert!(!lifecycle::paused(&g, SETUP_PART));
    let agent = |v: &View| -> Vec<String> {
        moves::criterion_admissible(v).iter().filter(|m| m.kind != "hold_answered").map(|m| format!("{}{:?}", m.kind, m.label)).collect()
    };
    assert_eq!(agent(&p), agent(&g));
}
