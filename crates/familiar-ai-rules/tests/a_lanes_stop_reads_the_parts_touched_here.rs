//! A lane's stop and its relay read the parts this checkout touched (the owner, 2026-10-15;
//! plan §6.8): a part is touched here when a fact after the latest `record_seeded` names it and
//! is an agent's claim or a laptop result. With no mark every part is touched. Off main the stop
//! reads only the touched parts; main reads every part, as before.

mod seeded;

use familiar_ai_facts::Log;
use familiar_ai_rules::lifecycle::touched_here;
use familiar_ai_rules::{decide, Decision, Event, View};
use seeded::*;
use serde_json::json;

fn touched(log: &Log) -> bool {
    touched_here(&View::new(log), "c1")
}

/// What a stop is told: allowed, or refused naming the hold or the state.
fn stop(log: &Log, main: bool) -> String {
    match decide(log, &Event::Stops { main }, AT) {
        Decision::Allow { .. } => "allowed".to_string(),
        Decision::Refuse(r) => format!("refused: {}", r.hold.as_deref().or(r.state).unwrap_or("?")),
    }
}

/// The agent's claims that touch a part, each naming c1 with the sample world's values.
fn claims() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("criterion", json!({ "criterion_id": "c1", "text": "a right password opens a session", "path": "full", "behavior_id": "b1", "promise_id": "p1" })),
        ("criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "lane" })),
        ("criterion_transfer", json!({ "criterion_id": "c1", "from_conversation": "a1", "to_conversation": "lane" })),
        ("place_chosen", json!({ "criterion_id": "c1", "places": [{ "file_path": CODE, "symbol": "check_password", "line_start": 40, "line_end": 62 }] })),
        ("new_work_spot_chosen", json!({ "criterion_id": "c1", "file_path": CODE, "symbol": "check_password", "beside": "below" })),
        ("new_work_no_spot", json!({ "criterion_id": "c1", "integration_point": "src/login.py:check_password" })),
        ("criterion_head", json!({ "criterion_id": "c1" })),
        ("criterion_strung_on_head", json!({ "criterion_id": "c1", "head": "c2" })),
        ("test_planned", json!({ "criterion_id": "c1", "file_path": TEST, "will": "create" })),
        ("touching_test_declared", json!({ "criterion_id": "c1", "file_path": TOUCHING, "test_name": "t_old" })),
        ("test_change_announced", json!({ "criterion_id": "c1", "reason": "the greeting changed" })),
        ("fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "" })),
    ]
}

#[test]
fn with_no_mark_every_part_is_touched() {
    assert!(touched(&main_green()));
    assert!(touched(&main_midwork()));
    // a record seeded before the mark existed reads as before
    assert!(touched(&seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), false)));
}

#[test]
fn after_the_mark_an_agents_claim_or_a_laptop_result_naming_the_part_touches_it() {
    let seed = seeded(&main_midwork(), ("k1", "failed", "S1"), Some(("k2", "failed", "S1")), true);
    assert!(!touched(&seed), "the seed's own facts, its laptop results and the plan's claims among them, touch nothing");
    for (kind, fields) in claims() {
        assert!(touched(&ap(&seed, kind, fields)), "{kind}");
    }
    assert!(touched(&result(&seed, TEST, "k2", "failed", "laptop", false)), "a laptop result");
    assert!(touched(&lane_code(&seed, "x1", "passed")), "the runner tested it at a lane commit");
}

#[test]
fn the_binarys_facts_the_pipelines_and_the_persons_answer_touch_nothing() {
    let seed = seeded(&main_midwork(), ("k1", "failed", "S1"), Some(("k2", "failed", "S1")), true);
    let untouched = [
        ap(&seed, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "t1" })),
        commit(&seed, "x1", &[CODE], false, true, &["c1"], "S1"),
        stamp(&seed, "k2", "S1", "laptop"),
        result(&seed, TEST, "k2", "failed", "pipeline", false),
        ap(&seed, "hold_answered", json!({ "criterion_id": "c1", "hold": "two_failures", "choice": "keep_going", "typed": "" })),
        ap(&seed, "criterion_assigned", json!({ "criterion_id": "c2", "agent_conversation_id": "lane" })),
    ];
    for log in untouched {
        assert!(!touched(&log), "{} touches c1", log[log.len() - 1].kind);
    }
}

#[test]
fn a_lanes_stop_ignores_an_untouched_inherited_parts_hold_and_mains_reads_every_part() {
    // c1's sealed test rewritten unannounced on main: held on tests_not_red in the seed
    let held = seeded(&main_tampered(), ("k3", "passed", "S1x"), None, true);
    assert_eq!(stop(&held, true), "refused: tests_not_red");
    assert_eq!(stop(&held, false), "allowed");
    // c1 mid-work on main: sealed, not done
    let sealed = seeded(&main_midwork(), ("k1", "failed", "S1"), Some(("k2", "failed", "S1")), true);
    assert_eq!(stop(&sealed, true), "refused: sealed");
    assert_eq!(stop(&sealed, false), "allowed");
}

#[test]
fn a_lane_never_stops_while_a_part_it_claimed_or_broke_is_not_done() {
    let sealed = seeded(&main_midwork(), ("k1", "failed", "S1"), Some(("k2", "failed", "S1")), true);
    let claimed = ap(&sealed, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "lane" }));
    assert_eq!(stop(&claimed, false), "refused: sealed");
    // row D: two lane commits break the inherited part, green on main
    let green = seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), true);
    assert_eq!(stop(&green, false), "allowed");
    let broke = lane_code(&lane_code(&green, "x1", "failed"), "x2", "failed");
    assert_eq!(stop(&broke, false), "refused: two_failures");
    assert_eq!(stop(&broke, true), "refused: two_failures");
}

#[test]
fn with_no_mark_a_stop_off_main_reads_every_part_as_main_does() {
    let main = main_midwork();
    assert_eq!(stop(&main, false), stop(&main, true));
    assert_eq!(stop(&main, false), "refused: sealed");
}
