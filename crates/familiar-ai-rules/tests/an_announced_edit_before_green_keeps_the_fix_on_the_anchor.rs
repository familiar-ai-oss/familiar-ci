//! An announcement before green keeps the fix already on the anchor: the
//! test re-committed red is a new seal, and code on the anchor counts from the
//! first seal of the epoch, so a last change outside the anchor still greens.
//! Left unannounced, the red re-commit is a new seal as today, and the fix
//! before it does not carry. See
//! docs/decisions.md#the-way-back

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::{lifecycle, View};

/// Sealed red; the fix on the anchor, the test still failing (try 1);
/// announced or not; test v2 committed alone, red; code outside the anchor,
/// the test passes.
fn walk(announced: bool) -> Log {
    let log = step(&sealed(), "code", None);
    let mut log = run(&log, "failed");
    if announced {
        log = step(&log, "test_change_announced", None);
    }
    let log = step(&log, "changed_test", None);
    let log = run(&log, "failed");
    let log = step(&log, "code_outside_anchor", None);
    run(&log, "passed")
}

#[test]
fn announced_before_green_the_fix_on_the_anchor_still_counts() {
    let log = walk(true);
    let v = View::new(&log);
    let whys = lifecycle::whys(&v, "c1");
    assert!(!whys.contains(&"anchor_untouched".to_string()), "{whys:?}");
    assert_eq!(lifecycle::state(&v, "c1"), "green", "{whys:?}");
}

#[test]
fn unannounced_the_recommit_is_a_new_seal_and_the_fix_does_not_carry() {
    let log = walk(false);
    let v = View::new(&log);
    let whys = lifecycle::whys(&v, "c1");
    assert!(whys.contains(&"anchor_untouched".to_string()), "{whys:?}");
    assert_ne!(lifecycle::state(&v, "c1"), "green", "{whys:?}");
}
