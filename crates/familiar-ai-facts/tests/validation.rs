//! The append refuses an unknown kind, an unregistered or stamped key, a missing field, a word outside a closed
//! set, a value of the wrong shape and a timestamp that is not a finite number; optional fields may be absent.

mod common;

use common::{sample_for, AT, TENANT};
use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

fn object(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn commit_seen() -> Value {
    json!({"commit": "T1", "files": ["tests/test_c1.py"], "touches_tests": true, "touches_code": false,
           "anchor_overlap": [], "tests_hash": "S1"})
}

#[test]
fn raw_append_refuses_unknown_kind() {
    let err = append_raw(&Vec::new(), "no_such_kind", Map::new(), AT, TENANT).unwrap_err();
    assert_eq!(err, AppendError::UnknownKind("no_such_kind".to_string()));
}

#[test]
fn raw_append_refuses_unregistered_key() {
    let mut fields = sample_for("criterion");
    fields.insert("bogus".to_string(), json!("x"));
    let err = append_raw(&Vec::new(), "criterion", fields, AT, TENANT).unwrap_err();
    assert_eq!(
        err,
        AppendError::UnregisteredKey {
            kind: "criterion".to_string(),
            key: "bogus".to_string()
        }
    );
}

#[test]
fn raw_append_refuses_stamped_keys() {
    for stamped in ["t", "at", "kind", "tenant"] {
        let mut fields = sample_for("criterion");
        fields.insert(stamped.to_string(), json!("x"));
        let err = append_raw(&Vec::new(), "criterion", fields, AT, TENANT).unwrap_err();
        assert_eq!(
            err,
            AppendError::StampedKey(stamped.to_string()),
            "stamped key {stamped}"
        );
    }
}

#[test]
fn raw_append_refuses_missing_required_field() {
    let mut fields = sample_for("promise");
    fields.remove("accepts");
    let err = append_raw(&Vec::new(), "promise", fields, AT, TENANT).unwrap_err();
    assert_eq!(
        err,
        AppendError::MissingField {
            kind: "promise".to_string(),
            field: "accepts".to_string()
        }
    );
}

#[test]
fn raw_append_refuses_value_outside_closed_set() {
    // One per closed set, plus a non-text value.
    let cases: &[(&str, &str, Value)] = &[
        ("criterion", "path", json!("wide")),
        ("pass_please_pass", "outcome", json!("error")),
        ("pass_please_pass", "ran_in", json!("cloud")),
        ("anchor_sha_matched", "checked_at", json!("middle")),
        ("test_planned", "will", json!("delete")),
        ("test_planned", "will", json!(3)),
        // the test outcome translation's own sets (S2); `by` is not the shared `agent`
        ("test_log_translation", "by", json!("agent")),
        ("test_log_translation", "label", json!("failed")),
        ("test_log_translation", "classified_on", json!("half")),
    ];
    for (kind, field, value) in cases {
        let mut fields = sample_for(kind);
        fields.insert(field.to_string(), value.clone());
        let err = append_raw(&Vec::new(), kind, fields, AT, TENANT).unwrap_err();
        match err {
            AppendError::BadValue(m) => assert!(m.contains(field), "{m}"),
            other => panic!("{kind}.{field}: expected BadValue, got {other:?}"),
        }
    }
}

#[test]
fn a_value_of_the_wrong_shape_is_refused() {
    let cases = [
        (
            "criterion_assigned",
            json!({"criterion_id": 5, "agent_conversation_id": "a1"}),
        ),
        ("commit_seen", {
            let mut v = commit_seen();
            v["touches_tests"] = json!("yes");
            v
        }),
        ("commit_seen", {
            let mut v = commit_seen();
            v["files"] = json!("tests/test_c1.py");
            v
        }),
        (
            "place_chosen",
            json!({"criterion_id": "c1", "places": [{"file_path": "a", "symbol": "s", "line_start": "40", "line_end": 62}]}),
        ),
        (
            "place_chosen",
            json!({"criterion_id": "c1", "places": [{"file_path": "a", "symbol": "s", "line_start": -1, "line_end": 62}]}),
        ),
    ];
    for (kind, fields) in cases {
        let got = append_raw(&Vec::new(), kind, object(fields.clone()), 0.0, "_default_");
        assert!(
            matches!(got, Err(AppendError::BadValue(_))),
            "{kind} {fields} gave {got:?}"
        );
    }
}

#[test]
fn with_anchor_reverted_and_tests_hash_may_be_absent() {
    let ppp = json!({"criterion_id": "c1", "file_path": "tests/test_c1.py", "commit": "T1", "outcome": "failed", "ran_in": "laptop"});
    let log = append_raw(&Vec::new(), "pass_please_pass", object(ppp), 0.0, "_default_").unwrap();
    let typed: PassPleasePass = log[0].fields_as().unwrap();
    assert_eq!(typed.with_anchor_reverted, None);
    assert_eq!(typed.test_name, None);

    let mut cs = commit_seen();
    cs.as_object_mut().unwrap().remove("tests_hash");
    let log = append_raw(&log, "commit_seen", object(cs), 0.0, "_default_").unwrap();
    assert_eq!(log[1].fields_as::<CommitSeen>().unwrap().tests_hash, None);
    assert!(log[1].fields_as::<PassPleasePass>().is_err());
}

#[test]
fn outcome_has_exactly_three_words() {
    let ppp = |word: &str| object(json!({"criterion_id": "c1", "file_path": "t", "commit": "T1", "outcome": word, "ran_in": "laptop"}));
    for word in ["passed", "failed", "did_not_run"] {
        let log = append_raw(&Vec::new(), "pass_please_pass", ppp(word), 0.0, "_default_");
        assert!(log.is_ok(), "{word}: {log:?}");
    }
    for word in ["error", "skipped", "crashed", "timeout", "Passed", "didnotrun", ""] {
        assert!(append_raw(&Vec::new(), "pass_please_pass", ppp(word), 0.0, "_default_").is_err(), "{word}");
    }
}

// A timestamp that is not a finite number is refused.
#[test]
fn a_non_finite_at_is_refused() {
    for at in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let got = append_raw(&Vec::new(), "criterion_head", sample_for("criterion_head"), at, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(ref m)) if m.contains("at")), "{at} gave {got:?}");
    }
}
