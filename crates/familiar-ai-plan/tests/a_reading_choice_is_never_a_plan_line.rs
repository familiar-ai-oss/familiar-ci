//! The adapter's reader_chosen [draft] (lane a1) is the binary's own record of how a runner is
//! read; a plan file never carries it, on read or on write.

use familiar_ai_plan::{from_bytes, Claim, LineError, PlanError, PLAN_KINDS};

#[test]
fn a_reader_chosen_line_is_refused_on_read_and_on_write() {
    assert!(!PLAN_KINDS.contains(&"reader_chosen"));
    let text = "{\"kind\":\"reader_chosen\"}\n";
    match from_bytes(text.as_bytes()) {
        Err(PlanError::Line(e)) => assert_eq!(e, LineError::RefusedKind { line: 1, kind: "reader_chosen".to_string() }),
        other => panic!("expected a refused kind, got {other:?}"),
    }
    match Claim::new("reader_chosen", serde_json::Map::new()) {
        Err(PlanError::RefusedKind(k)) => assert_eq!(k, "reader_chosen"),
        other => panic!("expected RefusedKind, got {other:?}"),
    }
}
