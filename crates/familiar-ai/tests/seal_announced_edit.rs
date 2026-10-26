//! After an announced edit lands and passes, the hook and `status` say the
//! criterion is green under the new stamp with its teeth owed on the pull
//! request; an edit that comes out red is a new seal.

mod common;
mod way_back;
use way_back::*;

const AGAIN: &str = "familiar-ai: \\\"greet says hello\\\" · green again under the new stamp · teeth on the pull request";

#[test]
fn the_edit_passing_is_green_again_under_the_new_stamp_with_teeth_owed() {
    let p = Sealed::green("owed");
    p.announce();
    let told = p.edit_test(REWORDED);
    assert!(told.contains(AGAIN), "{told}");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status_json());
    assert!(p.status().contains("\"greet says hello\" · green again under the new stamp · teeth on the pull request"), "{}", p.status());
}

#[test]
fn the_edit_coming_out_red_is_a_new_seal_in_todays_words() {
    let p = Sealed::green("owed-red");
    p.announce();
    let told = p.edit_test(STRICTER);
    assert!(told.contains("they fail, as they should before the fix · stamped"), "{told}");
    assert!(!told.contains("green again"), "{told}");
    assert_eq!(p.state_name("c1"), "sealed", "{}", p.status_json());
    let stamps = p.facts_of("fingerprint_stamped");
    assert_eq!(stamps.last().map(|s| s["commit"].clone()), Some(p.head().into()), "the edit's commit is the new seal: {stamps:?}");
}
