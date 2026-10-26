//! A translation may carry the kind of its location and the counts its output gave (lane h1):
//! `location.kind` is assertion, exception, compile or signal; `counts` has passed, failed,
//! skipped and deselected, whole numbers, and no other key. Both are optional, so a record
//! written before them reads unchanged.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "facts_tests";

fn base() -> Map<String, Value> {
    json!({"run_id": "r1", "criterion_id": "c1", "commit": "T1", "by": "regex", "label": "tests_failed", "evidence": "FAILED tests/test_c1.py::test_greet", "failing_tests": ["tests/test_c1.py::test_greet"], "classified_on": "full"})
        .as_object()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_translation_round_trips_with_a_kind_and_counts() {
    for (word, kind) in [
        ("assertion", LocationKind::Assertion),
        ("exception", LocationKind::Exception),
        ("compile", LocationKind::Compile),
        ("signal", LocationKind::Signal),
    ] {
        let mut f = base();
        f.insert("location".into(), json!({"file_path": "src/login.py", "line": 41, "kind": word}));
        f.insert("counts".into(), json!({"passed": 3, "failed": 1, "skipped": 0, "deselected": 2}));
        let log = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT).unwrap();
        let read: TestLogTranslation = log[0].fields_as().unwrap();
        assert_eq!(read.location, Some(Location { file_path: "src/login.py".into(), line: 41, function: None, kind: Some(kind) }));
        assert_eq!(read.counts, Some(Counts { passed: Some(3), failed: Some(1), skipped: Some(0), deselected: Some(2) }));
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes: {word}");
    }
    let mut f = base();
    f.insert("counts".into(), json!({}));
    let log = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT).unwrap();
    assert_eq!(log[0].fields_as::<TestLogTranslation>().unwrap().counts, Some(Counts::default()), "{{}} is the marker: counted, none found");
    let log = append_raw(&Vec::new(), "test_log_translation", base(), AT, TENANT).unwrap();
    let read: TestLogTranslation = log[0].fields_as().unwrap();
    assert_eq!((read.counts.clone(), read.location.clone()), (None, None), "an old translation reads unchanged");
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
}

#[test]
fn a_bad_kind_or_an_unknown_count_key_is_refused() {
    for (k, bad) in [
        ("location", json!({"file_path": "src/login.py", "line": 41, "kind": "panic"})),
        ("location", json!({"file_path": "src/login.py", "line": 41, "kind": 3})),
        ("counts", json!({"errors": 1})),
        ("counts", json!({"passed": 1, "xfailed": 1})),
        ("counts", json!({"passed": -1})),
        ("counts", json!({"passed": "3"})),
        ("counts", json!(3)),
    ] {
        let mut f = base();
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
}
