//! The adapter's facts (lane a1), nothing writes them yet: `reader_chosen` [draft], a translation
//! read by Familiar's own reporter (`by: reporter`, `ran_tests`, `skipped_tests`, `versions`), and
//! a reserved run that may name the commit whose setup it puts back (`setup_from`) and be a
//! `dry_run`.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "facts_tests";

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn chosen() -> Map<String, Value> {
    obj(json!({"run_id": "d1", "root": ".", "runner": "pytest", "reader": "pytest_reporter", "command": "pytest -q", "commit": "K3", "familiar": "0.1.0", "watched": ["pyproject.toml"], "gaps": [], "why": "the plugin loaded"}))
}

#[test]
fn a_reader_chosen_round_trips_with_and_without_its_optional_fields() {
    let log = append_raw(&Vec::new(), "reader_chosen", chosen(), AT, TENANT).unwrap();
    let read: ReaderChosen = log[0].fields_as().unwrap();
    assert_eq!((read.setup.clone(), read.versions.clone(), read.ran_in.clone()), (None, None, None));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
    let mut all = chosen();
    all.insert("setup".into(), json!("pyproject.toml"));
    all.insert("versions".into(), json!("pytest 8.3.2"));
    all.insert("ran_in".into(), json!("pipeline"));
    let log = append_raw(&Vec::new(), "reader_chosen", all, AT, TENANT).unwrap();
    let read: ReaderChosen = log[0].fields_as().unwrap();
    assert_eq!((read.setup.as_deref(), read.versions.as_deref(), read.ran_in.as_deref()), (Some("pyproject.toml"), Some("pytest 8.3.2"), Some("pipeline")));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
}

#[test]
fn a_reader_chosen_requires_its_core_and_checks_its_types() {
    for missing in ["run_id", "root", "runner", "reader", "command", "commit", "familiar", "watched", "gaps", "why"] {
        let mut f = chosen();
        f.remove(missing);
        let got = append_raw(&Vec::new(), "reader_chosen", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::MissingField { .. })), "{missing} is required: {got:?}");
    }
    for (k, bad) in [
        ("watched", json!("pyproject.toml")),
        ("watched", json!([1])),
        ("gaps", json!("no_junit")),
        ("gaps", json!([2])),
        ("command", json!(7)),
        ("versions", json!(["a"])),
        ("setup", json!(1)),
        ("ran_in", json!("cloud")),
    ] {
        let mut f = chosen();
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "reader_chosen", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
}

fn translation() -> Map<String, Value> {
    obj(json!({"run_id": "r1", "criterion_id": "c1", "commit": "T1", "by": "reporter", "label": "tests_failed", "evidence": "FAILED tests/test_c1.py::test_greet", "failing_tests": ["tests/test_c1.py::test_greet"], "classified_on": "full"}))
}

#[test]
fn a_translation_may_be_the_reporters_and_carry_the_tests_that_ran() {
    let log = append_raw(&Vec::new(), "test_log_translation", translation(), AT, TENANT).unwrap();
    let read: TestLogTranslation = log[0].fields_as().unwrap();
    assert_eq!((read.ran_tests.clone(), read.skipped_tests.clone(), read.versions.clone()), (None, None, None));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
    let mut f = translation();
    f.insert("ran_tests".into(), json!(["tests/test_c1.py::test_greet"]));
    f.insert("skipped_tests".into(), json!([]));
    f.insert("versions".into(), json!("pytest 8.3.2"));
    let log = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT).unwrap();
    let read: TestLogTranslation = log[0].fields_as().unwrap();
    assert_eq!((read.ran_tests.clone(), read.skipped_tests.clone(), read.versions.as_deref()), (Some(vec!["tests/test_c1.py::test_greet".to_string()]), Some(vec![]), Some("pytest 8.3.2")));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for w in ["regex", "binary", "reporter"] {
        let mut f = translation();
        f.insert("by".into(), json!(w));
        assert!(append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT).is_ok(), "{w}");
    }
    for (k, bad) in [("by", json!("model")), ("by", json!("agent")), ("ran_tests", json!("a")), ("ran_tests", json!([1])), ("skipped_tests", json!({})), ("versions", json!(3))] {
        let mut f = translation();
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
}

fn reserved() -> Map<String, Value> {
    obj(json!({"run_id": "r1", "criterion_id": "c1", "criterion_rev": "v1", "commit": "K2", "command": "true", "resources": [], "wait": "1800", "by": "dry_run"}))
}

#[test]
fn a_reserved_run_may_name_the_commit_whose_setup_it_puts_back() {
    let log = append_raw(&Vec::new(), "run_reserved", reserved(), AT, TENANT).unwrap();
    assert_eq!(log[0].fields.get("by"), Some(&json!("dry_run")), "by is free text");
    let mut f = reserved();
    f.insert("setup_from".into(), json!("abc1234"));
    let log = append_raw(&Vec::new(), "run_reserved", f, AT, TENANT).unwrap();
    assert_eq!(log[0].fields.get("setup_from"), Some(&json!("abc1234")));
    // a form field only, as pass_please_pass.run_id was: the typed struct does not carry it
    let row = form_row("run_reserved").unwrap();
    assert!(row.fields.contains(&"setup_from") && row.optional.contains(&"setup_from"));
}
