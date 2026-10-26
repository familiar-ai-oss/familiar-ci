//! verify in a fresh clone accepts a fix commit that touches a test announced
//! before it, rechecking red at the red seal's tests-first commit and green at
//! the head; with the announcement taken out of the plan it exits 1.

mod common;
mod way_back;
use way_back::*;

#[test]
fn an_announced_edit_after_green_is_proven_in_a_fresh_clone_exit_0() {
    let p = Sealed::way_back("verify");
    p.commit_plan();
    let (code, out) = p.verify_in_clone("announced");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"greet says hello\" · proven · red at "), "{out}");
    assert!(out.contains(" · green at ") && out.contains("fails with the change taken back out (exit 1) · ran: sh tests/test_c1.sh"), "{out}");
}

#[test]
fn the_same_edit_with_the_announcement_taken_out_of_the_plan_is_not_proven_exit_1() {
    let p = Sealed::way_back("verify-unannounced");
    let plan = p.repo().join(".familiar/plan");
    let text = std::fs::read_to_string(&plan).expect("plan");
    assert!(text.contains("\"kind\":\"test_change_announced\""), "{text}");
    let kept: String = text.lines().filter(|l| !l.contains("\"kind\":\"test_change_announced\"")).map(|l| format!("{l}\n")).collect();
    std::fs::write(&plan, kept).expect("plan");
    p.commit_plan();
    let (code, out) = p.verify_in_clone("unannounced");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("its sealed tests were changed after the tests-first commit"), "{out}");
}
