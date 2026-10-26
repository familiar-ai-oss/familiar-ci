//! An edit of a sealed test is refused naming the criterion whose test the
//! file is, whatever criterion the edit was made for: the refusal carries
//! `criterion_id`, so the claim that announces the change names the right one.

mod common;
use common::*;
use familiar_ai_rules::hooks::decide;
use familiar_ai_rules::{Decision, Event, Refusal, View};

#[test]
fn an_edit_of_c1s_sealed_test_made_for_c2_is_refused_naming_c1() {
    let log = sealed();
    let v = View::new(&log);
    let edit = Event::FileEdit { criterion_id: Some("c2".into()), file_path: TEST.into() };
    match decide(&v, &edit) {
        Decision::Refuse(Refusal { hold, criterion_id, .. }) => {
            assert_eq!(hold.as_deref(), Some("proof_tampered"));
            assert_eq!(criterion_id.as_deref(), Some("c1"));
        }
        other => panic!("expected proof_tampered naming c1, got {other:?}"),
    }
}
