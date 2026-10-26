//! Proves each read refusal names the offending line, and each refused kind is refused on write.

mod common;

use common::one_of_each;
use familiar_ai_plan::{from_bytes, Claim, LineError, PlanError};

fn read_err(text: &str) -> LineError {
    match from_bytes(text.as_bytes()) {
        Err(PlanError::Line(e)) => e,
        other => panic!("expected a line error, got {other:?}"),
    }
}

#[test]
fn refuse_a_line_that_is_not_a_json_object() {
    let good = one_of_each()[0].line();
    let text = format!("{good}\nnot json\n");
    assert_eq!(read_err(&text), LineError::NotAnObject { line: 2 });
    let text = format!("[1,2]\n{good}\n");
    assert_eq!(read_err(&text), LineError::NotAnObject { line: 1 });
}

#[test]
fn refuse_a_blank_line() {
    let good = one_of_each()[0].line();
    let text = format!("{good}\n\n{good}\n");
    assert_eq!(read_err(&text), LineError::NotAnObject { line: 2 });
}

#[test]
fn refuse_an_unknown_kind() {
    let text = r#"{"kind":"wish_granted","text":"x"}"#.to_string() + "\n";
    assert_eq!(
        read_err(&text),
        LineError::UnknownKind {
            line: 1,
            kind: "wish_granted".to_string()
        }
    );
}

// The person's answers stay out of the plan: a hold_answered committed in the plan would be read
// by verify as the person's, a release by hand passing a part (review of lane s2-facts, S2).
// Definition change (the owner, 2026-10-14): no kind mirrored into the plan may let CI accept a
// person's approval, so the test outcome translation's kinds stay out, dependency_drafted and
// criterion_covered among them. Definition change (the owner, 2026-10-25: "Mirror them"): the
// answer on the cases card, one of its own choices, is the one answer the plan carries
// (the_cases_card_is_mirrored.rs); a hold_answered on no hold, as here, is refused as before.
#[test]
fn refuse_a_kind_not_in_the_plan_list_on_read() {
    for kind in [
        "pass_please_pass",
        "commit_seen",
        "hold_answered",
        "pipeline_run_seen",
        "criterion_transfer",
        "test_log_translation",
        "dependency_drafted",
        "dependency_unanswered",
        "told",
        "criterion_covered",
        "subagent_ended",
    ] {
        let text = format!("{{\"kind\":\"{kind}\"}}\n");
        assert_eq!(
            read_err(&text),
            LineError::RefusedKind {
                line: 1,
                kind: kind.to_string()
            },
            "{kind}"
        );
    }
}

#[test]
fn refuse_an_unregistered_key() {
    let good = one_of_each()[1].line(); // behavior
    let text = format!(
        "{good}\n{{\"kind\":\"behavior\",\"behavior_id\":\"b1\",\"text\":\"x\",\"bogus\":1}}\n"
    );
    assert_eq!(
        read_err(&text),
        LineError::UnregisteredKey {
            line: 2,
            kind: "behavior".to_string(),
            key: "bogus".to_string()
        }
    );
}

#[test]
fn refuse_the_stamped_keys_t_at_tenant() {
    for key in ["t", "at", "tenant"] {
        let text = format!(
            "{{\"kind\":\"intent_submitted\",\"text\":\"x\",\"sha\":\"s\",\"{key}\":\"y\"}}\n"
        );
        match read_err(&text) {
            LineError::UnregisteredKey { line, key: k, .. } => {
                assert_eq!((line, k), (1, key.to_string()), "{key}");
            }
            other => panic!("expected UnregisteredKey for '{key}', got {other:?}"),
        }
    }
}

#[test]
fn refuse_a_missing_required_field() {
    let good = one_of_each()[0].line();
    let text = format!("{good}\n{{\"kind\":\"intent_submitted\",\"sha\":\"s\"}}\n");
    assert_eq!(
        read_err(&text),
        LineError::MissingField {
            line: 2,
            kind: "intent_submitted".to_string(),
            field: "text".to_string(),
        }
    );
}

#[test]
fn refuse_a_value_outside_a_closed_set() {
    let good = one_of_each()[0].line();
    let text = format!(
        "{good}\n{{\"kind\":\"criterion\",\"criterion_id\":\"c1\",\"text\":\"t\",\"path\":\"sideways\"}}\n"
    );
    match read_err(&text) {
        LineError::BadValue { line, message } => {
            assert_eq!(line, 2);
            assert!(message.contains("path"), "{message}");
        }
        other => panic!("expected BadValue, got {other:?}"),
    }
}

// The refused kinds, on write.

#[test]
fn refuse_a_kind_not_in_the_plan_list_on_write() {
    for kind in [
        "pass_please_pass",
        "commit_seen",
        "hold_answered",
        "pipeline_run_seen",
        "criterion_transfer",
        "test_log_translation",
        "dependency_drafted",
        "dependency_unanswered",
        "told",
        "criterion_covered",
        "subagent_ended",
    ] {
        match Claim::new(kind, serde_json::Map::new()) {
            Err(PlanError::RefusedKind(k)) => assert_eq!(k, kind),
            other => panic!("expected RefusedKind for {kind}, got {other:?}"),
        }
    }
}

#[test]
fn a_result_or_its_output_can_never_be_written_into_a_plan() {
    use serde_json::json;
    let result = json!({"criterion_id": "c1", "file_path": "t.py", "commit": "T1", "outcome": "failed", "ran_in": "laptop"});
    assert!(
        familiar_ai_plan::Claim::new("pass_please_pass", result.as_object().cloned().unwrap())
            .is_err()
    );
    // No plan kind accepts a key that could carry output or a stamp.
    for kind in familiar_ai_plan::PLAN_KINDS {
        for key in [
            "output", "stdout", "stderr", "state", "status", "t", "at", "tenant",
        ] {
            let fields = json!({ key: "x" }).as_object().cloned().unwrap();
            assert!(
                familiar_ai_plan::Claim::new(kind, fields).is_err(),
                "{kind} accepted {key}"
            );
        }
    }
}

#[test]
fn a_blank_line_in_the_middle_is_refused_with_its_line_number() {
    let text = "{\"kind\":\"criterion_head\",\"criterion_id\":\"c1\"}\n\n{\"kind\":\"criterion_head\",\"criterion_id\":\"c2\"}\n";
    let err = familiar_ai_plan::from_bytes(text.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("line 2"), "{err}");
    // The same two claims without the blank line read as two claims.
    let ok = text.replace("\n\n", "\n");
    assert_eq!(
        familiar_ai_plan::from_bytes(ok.as_bytes()).unwrap().len(),
        2
    );
    // A file with no newline after its last line still reads.
    assert_eq!(
        familiar_ai_plan::from_bytes(ok.trim_end().as_bytes())
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a_plan_of_ten_thousand_claims_reads_back_quickly() {
    let line = "{\"kind\":\"criterion_head\",\"criterion_id\":\"c1\"}\n";
    let text = line.repeat(10_000);
    let started = std::time::Instant::now();
    assert_eq!(
        familiar_ai_plan::from_bytes(text.as_bytes()).unwrap().len(),
        10_000
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn keys_are_written_in_the_forms_order_whatever_order_they_came_in() {
    use serde_json::json;
    let a = json!({"will": "create", "file_path": "tests/t.py", "criterion_id": "c1"});
    let claim =
        familiar_ai_plan::Claim::new("test_planned", a.as_object().cloned().unwrap()).unwrap();
    assert_eq!(
        claim.line(),
        r#"{"kind":"test_planned","criterion_id":"c1","file_path":"tests/t.py","will":"create"}"#
    );
}
