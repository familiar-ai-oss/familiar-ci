//! A part the binary drafts as a dependency on the person's approval (door/dependency.rs; the
//! fact `dependency_drafted` names it) stands on the finding and card answer of the part it was
//! drafted from (the owner, 2026-10-25: "use its source part's"): it owes no case-finder and
//! opens no card of its own, so the plan's line 3 holds for it. The same lines claimed by hand,
//! with no draft behind them, owe the case-finder as every full-path part does. [draft names]

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{cases, lifecycle, plan, View};
use serde_json::json;
use traced::*;

const DEP_TEXT: &str = "the row writer is read from one place";
const DEP_TEST: &str = "tests/test_writer.py";

/// c1 with a whole contract, past its cases card when `passed` (the case-finder's none and the
/// person's keep_planned), its test written; then the person's a_dependency on its crashed card.
fn approved(passed: bool) -> Log {
    let cases = json!([
        { "case_id": "n1", "kind": "normal", "given": "two rows", "expect": "each row once" },
        { "case_id": "o1", "kind": "opposite", "given": "no rows", "expect": "nothing written" }
    ]);
    let mut log = raw(&planned(), "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing", "cases": cases, "contract_version": 1 }));
    if passed {
        // definition change (the lens panel, approved 2026-10-26): the five lenses, then the synthesis
        log = lenses(&log, "c1");
        log = raw(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "the planned cases cover it", "model": "claude-sonnet-5-5", "tokens": 4000 }));
        log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": cases::CARD, "choice": cases::KEEP, "typed": "" }));
    }
    log = raw(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "" }));
    raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }))
}

/// c2's lines as the binary writes them for a draft (door/dependency.rs `lines`): the path and
/// ids of the part it was drafted from, its planned test, the insert before c1, c1's holder.
fn lines(log: &Log) -> Log {
    let mut log = raw(log, "criterion", json!({ "criterion_id": "c2", "text": DEP_TEXT, "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c2", "file_path": DEP_TEST, "will": "create" }));
    log = raw(&log, "criterion_head", json!({ "criterion_id": "c2" }));
    log = raw(&log, "criterion_strung_on_head", json!({ "criterion_id": "c1", "head": "c2" }));
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c2", "agent_conversation_id": "a1" }));
    raw(&log, "fix_location_category", json!({ "criterion_id": "c2", "category": "this_repository", "where": "" }))
}

/// The binary's draft of c2 for the approval just recorded, then c2's lines.
fn drafted(log: &Log) -> Log {
    let approval = log.last().map(|a| a.t).expect("the approval");
    lines(&raw(log, "dependency_drafted", json!({ "approval": approval, "criterion_id": "c2", "text": DEP_TEXT, "test_file": DEP_TEST })))
}

fn whys(log: &Log, c: &str) -> Vec<String> {
    lifecycle::holds(&View::new(log), c).iter().map(|h| h.why.clone()).collect()
}

#[test]
fn a_drafted_dependency_owes_no_case_finder_and_opens_no_card_of_its_own() {
    let log = drafted(&approved(true));
    let v = View::new(&log);
    assert!(!cases::owed(&v, "c2"), "the finding of the part it was drafted from stands for it");
    assert!(!cases::card_open(&v, "c2"), "and so does the person's answer on that part's card");
    assert!(plan::line_3_of(&v, "c2"), "line 3 holds for the dependency");
    assert!(plan::line_3(&v));
    let whys = whys(&log, "c2");
    assert!(!whys.iter().any(|w| w == cases::OWED || w == cases::CARD), "{whys:?}");
}

#[test]
fn a_drafted_dependency_owes_none_where_the_record_holds_no_finding_of_its_source_part() {
    // a record seeded from the plan file holds no finding or card answer of c1 (lane C's open question on the seed)
    let log = drafted(&approved(false));
    let v = View::new(&log);
    assert!(!cases::owed(&v, "c2"));
    assert!(!cases::card_open(&v, "c2"));
    assert!(plan::line_3_of(&v, "c2"));
}

#[test]
fn the_same_lines_claimed_by_hand_owe_the_case_finder() {
    let log = lines(&approved(true));
    let v = View::new(&log);
    assert!(cases::owed(&v, "c2"), "no draft behind it: a full-path part with a whole contract owes its finder");
    assert!(!plan::line_3_of(&v, "c2"));
    assert!(whys(&log, "c2").iter().any(|w| w == cases::OWED));
}
