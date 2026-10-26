//! Every kind in `KINDS` round-trips through append, flat JSON and its typed form; typed and raw
//! appends agree, append never changes the log it is given, `t` is the position, and the readers
//! find by kind.

mod common;

use common::{obj, sample_for, samples, AT, TENANT};
use familiar_ai_facts::*;
use serde_json::json;

/// Read a fact back as its kind's type, append that, and get the same fact.
fn typed_round_trip<T: FactFields + serde::de::DeserializeOwned>(fact: &Fact) -> Log {
    let typed: T = fact.fields_as().unwrap_or_else(|e| panic!("{}: {e}", fact.kind));
    append(&Vec::new(), &typed, fact.at, &fact.tenant).unwrap()
}

/// The typed round trip for a kind; panics for a kind with no type here.
fn typed_again(fact: &Fact) -> Log {
    match fact.kind {
        "intent_submitted" => typed_round_trip::<IntentSubmitted>(fact),
        "behavior" => typed_round_trip::<Behavior>(fact),
        "promise" => typed_round_trip::<Promise>(fact),
        "criterion" => typed_round_trip::<Criterion>(fact),
        "place_chosen" => typed_round_trip::<PlaceChosen>(fact),
        "new_work_spot_chosen" => typed_round_trip::<NewWorkSpotChosen>(fact),
        "new_work_no_spot" => typed_round_trip::<NewWorkNoSpot>(fact),
        "criterion_head" => typed_round_trip::<CriterionHead>(fact),
        "criterion_strung_on_head" => typed_round_trip::<CriterionStrungOnHead>(fact),
        "test_stack_detected" => typed_round_trip::<TestStackDetected>(fact),
        "test_written" => typed_round_trip::<TestWritten>(fact),
        "pass_please_pass" => typed_round_trip::<PassPleasePass>(fact),
        "fingerprint_stamped" => typed_round_trip::<FingerprintStamped>(fact),
        "anchor_sha_matched" => typed_round_trip::<AnchorShaMatched>(fact),
        "test_planned" => typed_round_trip::<TestPlanned>(fact),
        "touching_test_declared" => typed_round_trip::<TouchingTestDeclared>(fact),
        "criterion_assigned" => typed_round_trip::<CriterionAssigned>(fact),
        "criterion_transfer" => typed_round_trip::<CriterionTransfer>(fact),
        "commit_seen" => typed_round_trip::<CommitSeen>(fact),
        "hold_answered" => typed_round_trip::<HoldAnswered>(fact),
        "pipeline_run_seen" => typed_round_trip::<PipelineRunSeen>(fact),
        "test_change_announced" => typed_round_trip::<TestChangeAnnounced>(fact),
        "model_seen" => typed_round_trip::<ModelSeen>(fact),
        "judge_verdict" => typed_round_trip::<JudgeVerdict>(fact),
        "person_said" => typed_round_trip::<PersonSaid>(fact),
        "person_said_answered" => typed_round_trip::<PersonSaidAnswered>(fact),
        "judge_unanswered" => typed_round_trip::<JudgeUnanswered>(fact),
        "investigator_unfinished" => typed_round_trip::<InvestigatorUnfinished>(fact),
        "fix_location_category" => typed_round_trip::<FixLocationCategory>(fact),
        "run_reserved" => typed_round_trip::<RunReserved>(fact),
        "run_started" => typed_round_trip::<RunStarted>(fact),
        "run_ended" => typed_round_trip::<RunEnded>(fact),
        "run_cancel_requested" => typed_round_trip::<RunCancelRequested>(fact),
        "tests_observed" => typed_round_trip::<TestsObserved>(fact),
        "ci_report_seen" => typed_round_trip::<CiReportSeen>(fact),
        "anchor_is_documentation" => typed_round_trip::<AnchorIsDocumentation>(fact),
        "familiar_switch" => typed_round_trip::<FamiliarSwitch>(fact),
        "test_log_translation" => typed_round_trip::<TestLogTranslation>(fact),
        "dependency_drafted" => typed_round_trip::<DependencyDrafted>(fact),
        "dependency_unanswered" => typed_round_trip::<DependencyUnanswered>(fact),
        "told" => typed_round_trip::<Told>(fact),
        "criterion_covered" => typed_round_trip::<CriterionCovered>(fact),
        "subagent_ended" => typed_round_trip::<SubagentEnded>(fact),
        "record_seeded" => typed_round_trip::<RecordSeeded>(fact),
        "holder_resumed" => typed_round_trip::<HolderResumed>(fact),
        "investigator_finding" => typed_round_trip::<InvestigatorFinding>(fact),
        "reader_chosen" => typed_round_trip::<ReaderChosen>(fact),
        "fix_verdict" => typed_round_trip::<FixVerdict>(fact),
        "existing_test_announced" => typed_round_trip::<ExistingTestAnnounced>(fact),
        "existing_test_verdict" => typed_round_trip::<ExistingTestVerdict>(fact),
        "touching_test_derived" => typed_round_trip::<TouchingTestDerived>(fact),
        "places_listed" => typed_round_trip::<PlacesListed>(fact),
        "place_classified" => typed_round_trip::<PlaceClassified>(fact),
        "reading_alternatives" => typed_round_trip::<ReadingAlternatives>(fact),
        "one_reading" => typed_round_trip::<OneReading>(fact),
        // definition change (owner approved 2026-10-23) [draft]
        "trace_listed" => typed_round_trip::<TraceListed>(fact),
        "off_list_reason" => typed_round_trip::<OffListReason>(fact),
        // definition change (owner approved 2026-10-25) [draft]
        "cases_found" => typed_round_trip::<CasesFound>(fact),
        other => panic!("{other} has no typed round trip in this test"),
    }
}

#[test]
fn every_kind_round_trips_flat_and_typed() {
    let sampled: Vec<&str> = samples().iter().map(|(k, _)| *k).collect();
    assert_eq!(sampled, KINDS.to_vec(), "one sample per registered kind, in KINDS order");
    let mut log: Log = Vec::new();
    for (kind, fields) in samples() {
        log = append_raw(&log, kind, fields.clone(), AT, TENANT).unwrap();
        let fact = log.last().unwrap();
        let flat = fact.to_json();
        assert_eq!(flat["t"], json!(log.len() as u64 - 1));
        assert_eq!(flat["at"], json!(AT));
        assert_eq!(flat["kind"], json!(kind));
        assert_eq!(flat["tenant"], json!(TENANT));
        for (k, v) in &fields {
            assert_eq!(&flat[k], v, "{kind}.{k}");
        }
        assert_eq!(&Fact::from_json(&flat).unwrap(), fact);
        let alone = Fact { t: 0, ..fact.clone() };
        assert_eq!(typed_again(fact), vec![alone], "{kind}: the typed form keeps every field");
    }
}

#[test]
fn typed_append_matches_raw() {
    let typed = Criterion {
        criterion_id: "c2".into(),
        text: "a reset link reaches the person".into(),
        path: Path::Light,
        behavior_id: None,
        promise_id: None,
        for_t: None,
    };
    let l1 = append(&Vec::new(), &typed, AT, TENANT).unwrap();
    let l2 = append_raw(
        &Vec::new(),
        "criterion",
        obj(&[
            ("criterion_id", json!("c2")),
            ("text", json!("a reset link reaches the person")),
            ("path", json!("light")),
        ]),
        AT,
        TENANT,
    )
    .unwrap();
    assert_eq!(l1, l2);
    // Omitted optional fields stay omitted.
    let flat = l1[0].to_json();
    assert!(flat.get("behavior_id").is_none());
    assert!(flat.get("promise_id").is_none());
}

#[test]
fn the_flat_shape_is_the_stamped_keys_beside_the_fields() {
    let log = append_raw(
        &Vec::new(),
        "criterion_head",
        obj(&[("criterion_id", json!("c1"))]),
        7.0,
        "_default_",
    )
    .unwrap();
    assert_eq!(
        log[0].to_json(),
        json!({"t": 0, "at": 7.0, "kind": "criterion_head", "criterion_id": "c1", "tenant": "_default_"})
    );
    assert_eq!(log[0].to_json().as_object().unwrap().len(), 5);
}

#[test]
fn append_does_not_mutate_the_log() {
    let log: Log = Vec::new();
    let log = append_raw(
        &log,
        "criterion_head",
        sample_for("criterion_head"),
        AT,
        TENANT,
    )
    .unwrap();
    let snapshot = log_to_json(&log);
    let longer = append_raw(
        &log,
        "criterion_head",
        sample_for("criterion_head"),
        AT + 1.0,
        TENANT,
    )
    .unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(longer.len(), 2);
    assert_eq!(log_to_json(&log), snapshot);
    // The old facts are unchanged inside the new log.
    assert_eq!(longer[0], log[0]);
}

#[test]
fn t_equals_the_position() {
    let mut log: Log = Vec::new();
    for (kind, fields) in samples() {
        log = append_raw(&log, kind, fields, AT, TENANT).unwrap();
    }
    for (i, fact) in log.iter().enumerate() {
        assert_eq!(fact.t, i as u64);
    }
}

#[test]
fn the_readers_find_by_kind_and_criterion() {
    let mut log: Log = Vec::new();
    log = append_raw(
        &log,
        "criterion_head",
        sample_for("criterion_head"),
        AT,
        TENANT,
    )
    .unwrap();
    log = append_raw(&log, "test_written", sample_for("test_written"), AT, TENANT).unwrap();
    let mut w2 = sample_for("test_written");
    w2.insert("criterion_id".to_string(), json!("c2"));
    log = append_raw(&log, "test_written", w2, AT, TENANT).unwrap();
    let mut w3 = sample_for("test_written");
    w3.insert("file_path".to_string(), json!("tests/test_c1b.py"));
    log = append_raw(&log, "test_written", w3, AT, TENANT).unwrap();

    let last = last_of_kind(&log, "test_written").unwrap();
    assert_eq!(last.t, 3);
    assert!(last_of_kind(&log, "promise").is_none());

    let of_c1 = of_criterion(&log, "test_written", "c1");
    assert_eq!(of_c1.len(), 2);
    assert_eq!(of_c1[0].t, 1);
    assert_eq!(of_c1[1].t, 3);
    let of_c2 = of_criterion(&log, "test_written", "c2");
    assert_eq!(of_c2.len(), 1);
    assert_eq!(of_c2[0].t, 2);
    assert!(of_criterion(&log, "test_written", "none").is_empty());

}
