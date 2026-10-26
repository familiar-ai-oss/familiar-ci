//! The seed (the owner, 2026-10-15; plan §6.1, rows A-F of the prototype): setup's seed writes
//! its reruns of each part as this checkout's laptop evidence beside verify's pipeline facts,
//! and a laptop stamp with no laptop result behind it is no try. A seeded part then reads as it
//! does on main: the stamps the seed copied from the plan count nothing, a lane's passing commits
//! on a shared anchor count nothing, and a lane that breaks an inherited part gets two_failures
//! on it as main would.

mod seeded;

use familiar_ai_facts::Log;
use familiar_ai_rules::lifecycle::{attempts, state, tries_not_red, whys};
use familiar_ai_rules::moves::owed;
use familiar_ai_rules::{terminal, View};
use seeded::*;

fn cards(log: &Log) -> Vec<String> {
    terminal::cards(&View::new(log)).iter().map(|c| format!("{}:{}", c.criterion_id, c.hold)).collect()
}

fn owed_commits(log: &Log) -> Vec<String> {
    owed(&View::new(log), "c1").into_iter().map(|w| w.commit).collect()
}

fn tries(log: &Log) -> usize {
    tries_not_red(&View::new(log), "c1")
}

fn attempts_of(log: &Log) -> usize {
    attempts(&View::new(log), "c1")
}

fn holds(log: &Log) -> Vec<String> {
    whys(&View::new(log), "c1")
}

/// Row A: a part green on main, seeded the way setup seeded before 2026-10-15 (the plan's
/// laptop stamps and verify's pipeline facts, no laptop result): the copied stamps are no tries,
/// so no two_tries_not_red and no card.
#[test]
fn a_part_green_on_main_seeded_without_its_laptop_runs_counts_no_try() {
    let main = main_green();
    assert_eq!(state(&View::new(&main), "c1"), "green");
    assert_eq!((tries(&main), attempts_of(&main), cards(&main)), (0, 1, Vec::<String>::new()));
    let seed = seeded(&main, ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), false);
    assert_eq!(tries(&seed), 0, "{:?}", holds(&seed));
    assert!(!holds(&seed).iter().any(|h| h == "two_tries_not_red"), "{:?}", holds(&seed));
    assert_eq!(cards(&seed), Vec::<String>::new());
}

/// Row B: the seed with its runs as laptop evidence reads proven, as main reads it, with nothing
/// counted, nothing held and nothing owed.
#[test]
fn the_seed_with_its_runs_as_laptop_evidence_reads_proven_and_owes_nothing() {
    let seed = seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), true);
    assert_eq!(state(&View::new(&seed), "c1"), "proven");
    assert_eq!((tries(&seed), attempts_of(&seed)), (0, 0));
    assert_eq!(holds(&seed), Vec::<String>::new());
    assert_eq!(cards(&seed), Vec::<String>::new());
    assert_eq!(owed_commits(&seed), Vec::<String>::new());
}

/// Row C: two lane commits on the anchor the inherited part shares, its test passing there,
/// count nothing.
#[test]
fn two_lane_commits_on_a_shared_anchor_that_pass_its_test_count_nothing() {
    let seed = seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), true);
    let lane = lane_code(&lane_code(&seed, "x1", "passed"), "x2", "passed");
    assert_eq!(state(&View::new(&lane), "c1"), "proven");
    assert_eq!((tries(&lane), attempts_of(&lane)), (0, 0));
    assert_eq!(cards(&lane), Vec::<String>::new());
}

/// Row D: two lane commits that break the inherited part give two_failures on it, as the same
/// two commits give on main.
#[test]
fn two_lane_commits_that_break_an_inherited_part_give_two_failures_as_on_main() {
    let seed = seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), true);
    let lane = lane_code(&lane_code(&seed, "x1", "failed"), "x2", "failed");
    assert_eq!(attempts_of(&lane), 2);
    assert!(holds(&lane).iter().any(|h| h == "two_failures"), "{:?}", holds(&lane));
    assert_eq!(cards(&lane), ["c1:two_failures"]);
    let main = lane_code(&lane_code(&main_green(), "x1", "failed"), "x2", "failed");
    assert!(holds(&main).iter().any(|h| h == "two_failures"), "{:?}", holds(&main));
    assert_eq!(cards(&main), ["c1:two_failures"]);
}

/// Row E: a part mid-work on main (sealed, one failed fix) arrives sealed with its one attempt
/// and no card.
#[test]
fn a_part_mid_work_on_main_arrives_sealed_with_its_one_attempt() {
    let main = main_midwork();
    assert_eq!((state(&View::new(&main), "c1"), attempts_of(&main)), ("sealed", 1));
    let seed = seeded(&main, ("k1", "failed", "S1"), Some(("k2", "failed", "S1")), true);
    assert_eq!(state(&View::new(&seed), "c1"), "sealed");
    assert_eq!((tries(&seed), attempts_of(&seed)), (0, 1));
    assert_eq!(cards(&seed), Vec::<String>::new());
}

/// Row F: a part whose sealed test main rewrote unannounced (5f051de's shape) arrives held on
/// tests_not_red with main's one try, and no card: verify's tests-first commit is the rewrite.
#[test]
fn a_tampered_part_arrives_held_on_tests_not_red_with_one_try_and_no_card() {
    let main = main_tampered();
    assert_eq!(tries(&main), 1);
    assert_eq!(holds(&main), ["tests_not_red", "proof_tampered"]);
    let seed = seeded(&main, ("k3", "passed", "S1x"), None, true);
    assert_eq!(tries(&seed), 1, "{:?}", holds(&seed));
    assert_eq!(holds(&seed), ["tests_not_red"]);
    assert_eq!(cards(&seed), Vec::<String>::new());
}

/// On every prefix of rows C, D and F from the committed plan on (the seed's walk, verify's
/// facts, the twins, the mark and the lane's commits), no hold is dropped without the person's
/// answer. The plan's own planning holds clear by its claims before that, as on main.
#[test]
fn no_hold_is_dropped_without_an_answer_on_any_prefix_of_the_seeded_rows() {
    let green = seeded(&main_green(), ("k1", "failed", "S1"), Some(("k3", "passed", "S1")), true);
    let rows = [
        ("C", lane_code(&lane_code(&green, "x1", "passed"), "x2", "passed")),
        ("D", lane_code(&lane_code(&green, "x1", "failed"), "x2", "failed")),
        ("F", seeded(&main_tampered(), ("k3", "passed", "S1x"), None, true)),
    ];
    for (row, log) in rows {
        assert!(!log.iter().any(|f| f.kind == "hold_answered"), "row {row} holds no answer");
        let plan_lines = log.iter().find(|f| f.kind == "record_seeded").and_then(|f| f.fields.get("plan_lines")).and_then(serde_json::Value::as_u64).expect("the mark") as usize;
        let v = View::new(&log);
        let mut before: Vec<String> = whys(&v.prefix(plan_lines), "c1");
        for end in plan_lines + 1..=log.len() {
            let now = whys(&v.prefix(end), "c1");
            let dropped: Vec<&String> = before.iter().filter(|h| !now.contains(h)).collect();
            assert!(dropped.is_empty(), "row {row}: {dropped:?} dropped at {} ({}), holds now {now:?}", end - 1, log[end - 1].kind);
            before = now;
        }
    }
}
