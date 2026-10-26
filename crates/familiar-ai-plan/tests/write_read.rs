//! Proves every plan kind round-trips through bytes and a file with stable bytes, one claim per
//! line, and that no claim can carry a test's output or a state.

mod common;

use common::{fields, one_of_each, unique_tmp_dir};
use familiar_ai_plan::{from_bytes, read, to_bytes, write, Claim, PlanError};
use serde_json::{json, Map, Value};

#[test]
fn round_trip_every_allowed_kind() {
    let claims = one_of_each();
    let back = from_bytes(&to_bytes(&claims)).expect("read after write");
    assert_eq!(claims, back);
}

#[test]
fn bytes_are_stable() {
    let claims = one_of_each();
    assert_eq!(to_bytes(&claims), to_bytes(&claims));
    // Field insertion order does not matter; the form's field order does.
    let mut shuffled = Map::new();
    shuffled.insert("sha".to_string(), json!("abc123"));
    shuffled.insert("text".to_string(), json!("ship login"));
    let claim = Claim::new("intent_submitted", shuffled).expect("valid");
    assert_eq!(
        claim.line(),
        r#"{"kind":"intent_submitted","text":"ship login","sha":"abc123"}"#
    );
}

#[test]
fn one_claim_is_one_line_with_trailing_newline() {
    let claims = one_of_each();
    let text = String::from_utf8(to_bytes(&claims)).expect("utf8");
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines.len(), claims.len() + 1, "newline after the last line");
    assert_eq!(lines.last(), Some(&""));
    for (claim, line) in claims.iter().zip(lines.iter()) {
        assert!(!line.is_empty(), "no blank lines");
        assert_eq!(&claim.line(), line);
    }
}

#[test]
fn empty_file_is_an_empty_plan_and_a_missing_file_is_distinct() {
    assert_eq!(from_bytes(b"").expect("empty"), Vec::<Claim>::new());
    let missing = unique_tmp_dir().join(".familiar").join("plan.jsonl");
    match read(&missing) {
        Err(PlanError::NotFound(p)) => assert_eq!(p, missing),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn path_write_creates_parents_and_leaves_no_temp_file() {
    let dir = unique_tmp_dir();
    let path = dir.join(".familiar").join("nested").join("plan.jsonl");
    let claims = one_of_each();
    write(&path, &claims).expect("write");
    assert_eq!(read(&path).expect("read back"), claims);
    let entries: Vec<_> = std::fs::read_dir(path.parent().expect("parent"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(
        entries,
        vec![std::ffi::OsString::from("plan.jsonl")],
        "no temp file left"
    );
    std::fs::remove_dir_all(&dir).expect("clean up");
}

// No claim can carry a test's output or a state: those keys are refused on write.

#[test]
fn no_claim_ever_contains_output_state_or_status() {
    for key in ["output", "state", "status"] {
        let bad = fields(&[("criterion_id", json!("c1")), ("integration_point", json!("src/app.py:main")), (key, json!("x"))]);
        assert!(
            Claim::new("new_work_no_spot", bad).is_err(),
            "the key '{key}' must be refused"
        );
    }
    let claims = one_of_each();
    let text = String::from_utf8(to_bytes(&claims)).expect("utf8");
    for line in text.lines() {
        let obj: Map<String, Value> = serde_json::from_str(line).expect("a claim per line");
        for key in ["output", "state", "status", "t", "at", "tenant"] {
            assert!(!obj.contains_key(key), "line carries '{key}': {line}");
        }
    }
}
