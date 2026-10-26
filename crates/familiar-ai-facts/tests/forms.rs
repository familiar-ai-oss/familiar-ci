//! Each form's closed sets, value shapes and optional fields are enforced, and `FORM` lists every kind in order.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "facts_tests";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

// pass_please_pass.test_name is optional, with each of the three outcomes.
#[test]
fn pass_please_pass_test_name_optional() {
    let f = json!({"criterion_id": "c1", "file_path": "tests/test_c1.py", "commit": "K1", "outcome": "failed", "ran_in": "laptop", "with_anchor_reverted": false});
    let log = append_raw(&Vec::new(), "pass_please_pass", fields(f.clone()), AT, TENANT).unwrap();
    assert!(log[0].to_json().get("test_name").is_none());
    for outcome in ["passed", "failed", "did_not_run"] {
        let mut each = fields(f.clone());
        each.insert("outcome".to_string(), json!(outcome));
        let log = append_raw(&Vec::new(), "pass_please_pass", each, AT, TENANT).unwrap();
        assert_eq!(log[0].fields_as::<PassPleasePass>().unwrap().test_name, None, "{outcome}");
    }
}

// A light criterion appends without behavior_id or promise_id.
#[test]
fn criterion_link_fields_optional() {
    let f = json!({"criterion_id": "c1", "text": "a right password opens a session", "path": "light"});
    let log = append_raw(&Vec::new(), "criterion", fields(f), AT, TENANT).unwrap();
    let flat = log[0].to_json();
    assert_eq!(flat["path"], json!("light"));
    assert!(flat.get("behavior_id").is_none());
}

// model_seen carries exactly its two text fields.
#[test]
fn model_seen_is_the_conversation_and_the_model_as_text() {
    let ok = json!({"agent_conversation_id": "s1/a9", "model": "claude-sonnet-5-5"});
    let log = append_raw(&Vec::new(), "model_seen", fields(ok), AT, TENANT).unwrap();
    let typed: ModelSeen = log[0].fields_as().unwrap();
    assert_eq!(typed.model, "claude-sonnet-5-5");
    for bad in [json!({"agent_conversation_id": "s1", "model": 5}), json!({"agent_conversation_id": "s1"}), json!({"agent_conversation_id": "s1", "model": "m", "rung": "sonnet"})] {
        assert!(append_raw(&Vec::new(), "model_seen", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}

// judge_verdict's answer is yes or no; the rest is text.
#[test]
fn judge_verdict_answers_yes_or_no() {
    let ok = json!({"criterion_id": "c1", "model": "claude-sonnet-5-5", "answers_intent": "no", "reason": "the test checks the wrong error"});
    let log = append_raw(&Vec::new(), "judge_verdict", fields(ok.clone()), AT, TENANT).unwrap();
    let typed: JudgeVerdict = log[0].fields_as().unwrap();
    assert_eq!(typed.answers_intent, AnswersIntent::No);
    let again = append(&Vec::new(), &typed, AT, TENANT).unwrap();
    assert_eq!(again, log, "typed and raw give the same bytes");
    for (k, bad) in [("answers_intent", json!("unknown")), ("answers_intent", json!(true)), ("reason", json!(1))] {
        let mut f = fields(ok.clone());
        f.insert(k.to_string(), bad.clone());
        assert!(append_raw(&Vec::new(), "judge_verdict", f, AT, TENANT).is_err(), "{k}={bad}");
    }
}

// judge_unanswered's why is a closed set; the seal is text.
#[test]
fn judge_unanswered_says_why_from_a_closed_set() {
    for (why, typed) in [("error", JudgeUnansweredWhy::Error), ("timeout", JudgeUnansweredWhy::Timeout), ("unparseable", JudgeUnansweredWhy::Unparseable), ("cannot_see", JudgeUnansweredWhy::CannotSee)] {
        let ok = json!({"criterion_id": "c1", "seal": "4f1c2ab", "why": why});
        let log = append_raw(&Vec::new(), "judge_unanswered", fields(ok), AT, TENANT).unwrap();
        let read: JudgeUnanswered = log[0].fields_as().unwrap();
        assert_eq!(read.why, typed);
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    }
    for bad in [json!({"criterion_id": "c1", "seal": "s", "why": "busy"}), json!({"criterion_id": "c1", "seal": 1, "why": "error"}), json!({"criterion_id": "c1", "why": "error"})] {
        assert!(append_raw(&Vec::new(), "judge_unanswered", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}

// investigator_unfinished's why is a closed set; the conversation is text.
#[test]
fn investigator_unfinished_says_why_from_a_closed_set() {
    for (why, typed) in [("budget", InvestigatorUnfinishedWhy::Budget), ("timeout", InvestigatorUnfinishedWhy::Timeout), ("error", InvestigatorUnfinishedWhy::Error), ("no_green", InvestigatorUnfinishedWhy::NoGreen), ("no_seal", InvestigatorUnfinishedWhy::NoSeal), ("unparseable", InvestigatorUnfinishedWhy::Unparseable), ("unverified", InvestigatorUnfinishedWhy::Unverified)] {
        let ok = json!({"criterion_id": "c1", "conversation": "s1/i1", "why": why});
        let log = append_raw(&Vec::new(), "investigator_unfinished", fields(ok), AT, TENANT).unwrap();
        let read: InvestigatorUnfinished = log[0].fields_as().unwrap();
        assert_eq!(read.why, typed);
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    }
    for bad in [json!({"criterion_id": "c1", "conversation": "s1/i1", "why": "crashed"}), json!({"criterion_id": "c1", "conversation": 1, "why": "error"}), json!({"criterion_id": "c1", "why": "budget"})] {
        assert!(append_raw(&Vec::new(), "investigator_unfinished", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}

// fix_location_category's category is a closed set, and `needs` may be left out.
#[test]
fn fix_location_category_is_a_closed_set() {
    for (cat, typed) in [("this_repository", FixLocation::ThisRepository), ("dependency", FixLocation::Dependency), ("environment", FixLocation::Environment), ("request", FixLocation::Request)] {
        let ok = json!({"criterion_id": "c1", "category": cat, "where": "w"});
        let log = append_raw(&Vec::new(), "fix_location_category", fields(ok), AT, TENANT).unwrap();
        let read: FixLocationCategory = log[0].fields_as().unwrap();
        assert_eq!((read.category, read.needs.clone()), (typed, None));
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    }
    let with_needs = json!({"criterion_id": "c1", "category": "dependency", "where": "the library", "needs": "a release"});
    let log = append_raw(&Vec::new(), "fix_location_category", fields(with_needs), AT, TENANT).unwrap();
    assert_eq!(log[0].fields_as::<FixLocationCategory>().unwrap().needs.as_deref(), Some("a release"));
    for bad in [json!({"criterion_id": "c1", "category": "upstream", "where": "w"}), json!({"criterion_id": "c1", "category": "dependency"}), json!({"criterion_id": "c1", "category": "dependency", "where": "w", "needs": 3})] {
        assert!(append_raw(&Vec::new(), "fix_location_category", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}

// person_said_answered's `for` is a whole number and its `how` a closed set; a criterion's `for`
// is an optional whole number and a verdict's `gap` optional text.
#[test]
fn the_persons_words_and_their_answers_have_their_shapes() {
    let log = append_raw(&Vec::new(), "person_said", fields(json!({"text": "fix it"})), AT, TENANT).unwrap();
    let typed: PersonSaid = log[0].fields_as().unwrap();
    assert_eq!(typed.text, "fix it");
    let ok = json!({"for": 12, "how": "question"});
    let log = append_raw(&Vec::new(), "person_said_answered", fields(ok.clone()), AT, TENANT).unwrap();
    let typed: PersonSaidAnswered = log[0].fields_as().unwrap();
    assert_eq!((typed.for_t, typed.how), (12, How::Question));
    assert_eq!(append(&Vec::new(), &typed, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for (k, bad) in [("how", json!("done")), ("for", json!("12")), ("for", json!(-1))] {
        let mut f = fields(ok.clone());
        f.insert(k.to_string(), bad.clone());
        assert!(append_raw(&Vec::new(), "person_said_answered", f, AT, TENANT).is_err(), "{k}={bad}");
    }
    let c = json!({"criterion_id": "c2", "text": "t", "path": "light", "for": 12});
    let log = append_raw(&Vec::new(), "criterion", fields(c), AT, TENANT).unwrap();
    assert_eq!(log[0].fields_as::<Criterion>().unwrap().for_t, Some(12));
    let bad = json!({"criterion_id": "c2", "text": "t", "path": "light", "for": "twelve"});
    assert!(append_raw(&Vec::new(), "criterion", fields(bad), AT, TENANT).is_err());
    let v = json!({"criterion_id": "c1", "model": "m", "answers_intent": "no", "reason": "r", "gap": "the alias in serialization"});
    let log = append_raw(&Vec::new(), "judge_verdict", fields(v), AT, TENANT).unwrap();
    assert_eq!(log[0].fields_as::<JudgeVerdict>().unwrap().gap.as_deref(), Some("the alias in serialization"));
}

#[test]
fn no_form_registers_state_or_status() {
    for row in FORM {
        assert_ne!(row.kind, "state");
        assert_ne!(row.kind, "status");
        for field in row.fields {
            assert_ne!(*field, "state", "{} registers state", row.kind);
            assert_ne!(*field, "status", "{} registers status", row.kind);
        }
    }
}

// New kinds are added at the end, so existing positions in KINDS never change;
// the one exception is familiar_switch, the binary's own and not in the model:
// anchor_is_documentation (the owner, 2026-10-11) sits at 35 in the model's
// order and familiar_switch after it. Definition change (the owner,
// 2026-10-11, light is for documentation only): 36 kinds became 37.
// Definition change (the owner's plan of 2026-10-14, test outcome translation,
// step S2): the six kinds the translation needs come after familiar_switch, so
// the form has 43 kinds.
// Definition change (the owner, 2026-10-15, the seed): record_seeded, the mark
// setup's seed writes last, comes after subagent_ended, so the form has 44 kinds.
// Definition change (the owner, 2026-10-16, S8g): holder_resumed, a resumed lane's session
// started again, comes after record_seeded, so the form has 45 kinds.
// Definition change (the owner, 2026-10-16, S7b): investigator_finding [draft], the binary's
// diagnosis of a test that never went red, comes after holder_resumed, so the form has 46 kinds.
// Definition change (the owner, 2026-10-19, lane a1): reader_chosen [draft], the adapter's choice of
// how a runner is read, comes after investigator_finding, so the form has 47 kinds.
// Definition change (the owner's plan of 2026-10-20, step 3): fix_verdict [draft], the reviewer's
// word on a green fix, comes after reader_chosen, so the form has 48 kinds.
// Definition change (the owner, 2026-10-20, an existing test's lines change only when announced)
// [draft]: existing_test_announced, existing_test_verdict and touching_test_derived come
// after fix_verdict, so the form has 51 kinds.
// Definition change (the owner's plan of 2026-10-21, step 3, places) [draft]: places_listed, the
// binary's, and place_classified, the agent's, come after touching_test_derived, so the form has 53
// kinds.
// Definition change (the owner's plan of 2026-10-21, step 2, a clarifying question on a signal)
// [draft]: reading_alternatives and one_reading, the agent's word on how the person's ask reads,
// come after place_classified, so the form has 55 kinds.
// Definition change (the owner's plan of 2026-10-23, test first then pick the spot; owner approved
// 2026-10-23) [draft]: trace_listed, the binary's, and off_list_reason, the agent's, come after
// one_reading, so the form had 57 kinds.
// Definition change (the owner's plan of 2026-10-25, the cases card; owner approved 2026-10-25, KINDS
// 57 -> 58) [draft]: cases_found, the binary's, comes after off_list_reason, so the form has 58 kinds.
#[test]
fn form_has_exactly_58_kinds() {
    assert_eq!(FORM.len(), 58);
    assert_eq!(KINDS.len(), 58);
    assert_eq!(KINDS[22], "model_seen");
    assert_eq!(KINDS[23], "judge_verdict");
    assert_eq!(KINDS[24], "person_said");
    assert_eq!(KINDS[25], "person_said_answered");
    assert_eq!(KINDS[26], "judge_unanswered");
    assert_eq!(KINDS[27], "investigator_unfinished");
    assert_eq!(KINDS[28], "fix_location_category");
    assert_eq!(KINDS[29], "run_reserved");
    assert_eq!(KINDS[30], "run_started");
    assert_eq!(KINDS[31], "run_ended");
    assert_eq!(KINDS[32], "run_cancel_requested");
    assert_eq!(KINDS[33], "tests_observed");
    assert_eq!(KINDS[34], "ci_report_seen");
    assert_eq!(KINDS[35], "anchor_is_documentation");
    assert_eq!(KINDS.get(36), Some(&"familiar_switch"));
    assert_eq!(KINDS.get(37), Some(&"test_log_translation"));
    assert_eq!(KINDS.get(38), Some(&"dependency_drafted"));
    assert_eq!(KINDS.get(39), Some(&"dependency_unanswered"));
    assert_eq!(KINDS.get(40), Some(&"told"));
    assert_eq!(KINDS.get(41), Some(&"criterion_covered"));
    assert_eq!(KINDS.get(42), Some(&"subagent_ended"));
    assert_eq!(KINDS.get(43), Some(&"record_seeded"));
    assert_eq!(KINDS.get(44), Some(&"holder_resumed"));
    assert_eq!(KINDS.get(45), Some(&"investigator_finding"));
    assert_eq!(KINDS.get(46), Some(&"reader_chosen"));
    assert_eq!(KINDS.get(47), Some(&"fix_verdict"));
    assert_eq!(KINDS.get(48), Some(&"existing_test_announced"));
    assert_eq!(KINDS.get(49), Some(&"existing_test_verdict"));
    assert_eq!(KINDS.get(50), Some(&"touching_test_derived"));
    assert_eq!(KINDS.get(51), Some(&"places_listed"));
    assert_eq!(KINDS.get(52), Some(&"place_classified"));
    assert_eq!(KINDS.get(53), Some(&"reading_alternatives"));
    assert_eq!(KINDS.get(54), Some(&"one_reading"));
    assert_eq!(KINDS.get(55), Some(&"trace_listed"));
    assert_eq!(KINDS.get(56), Some(&"off_list_reason"));
    assert_eq!(KINDS.get(57), Some(&"cases_found"));
    for (row, kind) in FORM.iter().zip(KINDS.iter()) {
        assert_eq!(row.kind, *kind);
    }
    assert_eq!(kind_count(), 58);
}

// commit_seen.existing_tests_changed [draft] is a field of the fact, not of the typed form: a
// list of {file_path, tests, lines}, each line range two whole numbers.
#[test]
fn commit_seen_may_carry_the_standing_change_to_existing_tests() {
    let base = json!({"commit": "k1", "files": ["tests/test_a.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": []});
    let mut ok = base.clone();
    ok["existing_tests_changed"] = json!([{"file_path": "tests/test_a.py", "tests": ["test_x"], "lines": [[3, 4]]}]);
    assert!(append_raw(&Vec::new(), "commit_seen", fields(ok), AT, TENANT).is_ok());
    let mut bad = base;
    bad["existing_tests_changed"] = json!([{"file_path": "tests/test_a.py", "tests": ["test_x"], "lines": ["3"]}]);
    assert!(append_raw(&Vec::new(), "commit_seen", fields(bad), AT, TENANT).is_err());
}

// record_seeded (the owner's name, 2026-10-15) is the mark setup's seed writes last: the commit
// it walked to, as text, and how many plan facts it copied first, a whole number. Both are
// required.
#[test]
fn record_seeded_names_the_commit_walked_to_and_how_many_plan_facts_came_first() {
    let ok = json!({"commit": "b111499", "plan_lines": 445});
    let log = append_raw(&Vec::new(), "record_seeded", fields(ok.clone()), AT, TENANT).unwrap();
    let read: RecordSeeded = log[0].fields_as().unwrap();
    assert_eq!((read.commit.as_str(), read.plan_lines), ("b111499", 445));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for missing in ["commit", "plan_lines"] {
        let mut f = fields(ok.clone());
        f.remove(missing);
        let got = append_raw(&Vec::new(), "record_seeded", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::MissingField { .. })), "{missing} is required: {got:?}");
    }
    for (k, bad) in [("commit", json!(7)), ("plan_lines", json!("445")), ("plan_lines", json!(-1)), ("plan_lines", json!(1.5))] {
        let mut f = fields(ok.clone());
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "record_seeded", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
}

// run_ended may carry the exit code, the signal, the tail of each stream and, per stream
// (stdout, stderr or report), the bytes it carried and the bytes the drain dropped. A run_ended
// without them, as every record before S2 holds, still appends and reads.
#[test]
fn run_ended_may_carry_its_exit_its_signal_its_tails_and_what_was_dropped() {
    let bare = json!({"run_id": "r1", "outcome": "failed", "end": "exited"});
    let log = append_raw(&Vec::new(), "run_ended", fields(bare.clone()), AT, TENANT).unwrap();
    let read: RunEnded = log[0].fields_as().unwrap();
    assert_eq!((read.exit_code, read.signal, read.stdout_tail.as_deref(), read.stderr_tail.as_deref(), read.truncated.as_ref()), (None, None, None, None, None));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
    let full = json!({"run_id": "r1", "outcome": "failed", "end": "signalled", "exit_code": 139, "signal": 11, "stdout_tail": "collected 1 item", "stderr_tail": "Segmentation fault",
        "truncated": [{"stream": "stdout", "total_bytes": 9000000, "dropped_bytes": 611392}, {"stream": "stderr", "total_bytes": 12, "dropped_bytes": 0}, {"stream": "report", "total_bytes": 4096, "dropped_bytes": 0}]});
    let log = append_raw(&Vec::new(), "run_ended", fields(full.clone()), AT, TENANT).unwrap();
    let read: RunEnded = log[0].fields_as().unwrap();
    assert_eq!((read.exit_code, read.signal), (Some(139), Some(11)));
    let rows = read.truncated.clone().unwrap_or_default();
    assert_eq!(rows.iter().map(|r| r.stream).collect::<Vec<_>>(), [Stream::Stdout, Stream::Stderr, Stream::Report]);
    assert_eq!((rows[0].total_bytes, rows[0].dropped_bytes), (9_000_000, 611_392));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for (k, bad) in [
        ("exit_code", json!("1")),
        ("exit_code", json!(1.5)),
        ("signal", json!("SIGSEGV")),
        ("stdout_tail", json!(3)),
        ("stderr_tail", json!(["a"])),
        ("truncated", json!({"stream": "stdout", "total_bytes": 1, "dropped_bytes": 0})),
        ("truncated", json!([{"stream": "both", "total_bytes": 1, "dropped_bytes": 0}])),
        ("truncated", json!([{"stream": "stdout", "total_bytes": -1, "dropped_bytes": 0}])),
        ("truncated", json!([{"stream": "stdout", "total_bytes": 1}])),
    ] {
        let mut f = fields(full.clone());
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "run_ended", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
}

// pass_please_pass may name the run it came from, a field of the fact read from its field map; a
// result without one still appends, and a run_id that is not text is refused. Definition change
// (the owner, 2026-10-14: a sealed test changed without an announcement is tampering): the typed
// PassPleasePass keeps the seven fields the sealed tests build (c24's
// record_legacy_seal_restamp.rs among them) and has no run_id; the run's result is written with
// it through append_raw.
#[test]
fn pass_please_pass_may_name_its_run() {
    let ok = json!({"criterion_id": "c1", "file_path": "tests/test_c1.py", "commit": "K1", "outcome": "failed", "ran_in": "laptop", "run_id": "r1"});
    let log = append_raw(&Vec::new(), "pass_please_pass", fields(ok.clone()), AT, TENANT).unwrap();
    assert_eq!(log[0].fields.get("run_id").and_then(Value::as_str), Some("r1"));
    let sealed = PassPleasePass {
        criterion_id: "c1".into(),
        file_path: "tests/test_c1.py".into(),
        test_name: None,
        commit: "K1".into(),
        outcome: Outcome::Failed,
        ran_in: RanIn::Laptop,
        with_anchor_reverted: None,
    };
    assert_eq!(log[0].fields_as::<PassPleasePass>().unwrap(), sealed, "the typed form reads the rest");
    let mut without = fields(ok.clone());
    without.remove("run_id");
    let log = append_raw(&Vec::new(), "pass_please_pass", without, AT, TENANT).unwrap();
    assert!(log[0].fields.get("run_id").is_none());
    assert_eq!(append(&Vec::new(), &sealed, AT, TENANT).unwrap(), log, "the typed form writes no run_id");
    let mut bad = fields(ok);
    bad.insert("run_id".to_string(), json!(7));
    assert!(matches!(append_raw(&Vec::new(), "pass_please_pass", bad, AT, TENANT), Err(AppendError::BadValue(_))));
}

// test_log_translation's by (regex or binary), label and classified_on (full or cut) are closed
// sets; failing_tests is a list of text; location ({file_path, line}) and missing_name may be
// left out.
#[test]
fn test_log_translation_takes_its_words_from_closed_sets() {
    let ok = json!({"run_id": "r1", "criterion_id": "c1", "commit": "T1", "by": "regex", "label": "tests_failed", "evidence": "FAILED tests/test_c1.py::test_greet - AssertionError", "failing_tests": ["tests/test_c1.py::test_greet"], "classified_on": "full"});
    let labels = [
        "passed", "tests_failed", "collection_error", "missing_target_symbol", "no_tests_collected", "setup_error", "usage_error",
        "environment_error", "runner_crash", "killed", "crashed", "timed_out", "unclassified",
    ];
    for (k, words) in [("label", &labels[..]), ("by", &["regex", "binary"][..]), ("classified_on", &["full", "cut"][..])] {
        for word in words {
            let mut f = fields(ok.clone());
            f.insert(k.to_string(), json!(word));
            let got = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT);
            assert!(got.is_ok(), "{k}={word}: {got:?}");
        }
    }
    let log = append_raw(&Vec::new(), "test_log_translation", fields(ok.clone()), AT, TENANT).unwrap();
    let read: TestLogTranslation = log[0].fields_as().unwrap();
    assert_eq!((read.location.clone(), read.missing_name.clone()), (None, None));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
    let mut placed = fields(ok.clone());
    placed.insert("location".to_string(), json!({"file_path": "src/login.py", "line": 41}));
    placed.insert("missing_name".to_string(), json!("check_password"));
    let log = append_raw(&Vec::new(), "test_log_translation", placed, AT, TENANT).unwrap();
    let read: TestLogTranslation = log[0].fields_as().unwrap();
    assert_eq!(read.location, Some(Location { file_path: "src/login.py".into(), line: 41, function: None, kind: None }));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for (k, bad) in [
        ("label", json!("failed")),
        ("by", json!("agent")),
        ("by", json!("model")),
        ("classified_on", json!("half")),
        ("failing_tests", json!("tests/test_c1.py::test_greet")),
        ("failing_tests", json!([1])),
        ("location", json!({"file_path": "src/login.py"})),
        ("location", json!({"file_path": "src/login.py", "line": -2})),
        ("missing_name", json!(1)),
    ] {
        let mut f = fields(ok.clone());
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
    for missing in ["run_id", "criterion_id", "commit", "by", "label", "evidence", "failing_tests", "classified_on"] {
        let mut f = fields(ok.clone());
        f.remove(missing);
        let got = append_raw(&Vec::new(), "test_log_translation", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::MissingField { .. })), "{missing} is required: {got:?}");
    }
}

// dependency_drafted names the hold_answered it answers by that answer's t, a whole number; its
// place, when it has one, has the shape of a place_chosen place.
#[test]
fn dependency_drafted_answers_an_approval_and_may_name_a_place() {
    let ok = json!({"approval": 19, "criterion_id": "c4", "text": "the hashing helper returns text, not bytes", "test_file": "tests/test_c4.py"});
    let log = append_raw(&Vec::new(), "dependency_drafted", fields(ok.clone()), AT, TENANT).unwrap();
    let read: DependencyDrafted = log[0].fields_as().unwrap();
    assert_eq!((read.approval, read.place.clone()), (19, None));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "absent stays absent");
    let mut placed = fields(ok.clone());
    placed.insert("place".to_string(), json!({"file_path": "src/hashing.py", "symbol": "digest", "line_start": 3, "line_end": 9}));
    let log = append_raw(&Vec::new(), "dependency_drafted", placed, AT, TENANT).unwrap();
    let read: DependencyDrafted = log[0].fields_as().unwrap();
    assert_eq!(read.place, Some(Place { file_path: "src/hashing.py".into(), symbol: "digest".into(), line_start: 3, line_end: 9 }));
    assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes");
    for (k, bad) in [
        ("approval", json!("19")),
        ("approval", json!(-1)),
        ("approval", json!(1.5)),
        ("place", json!({"file_path": "src/hashing.py", "symbol": "digest", "line_start": 3})),
        ("place", json!([{"file_path": "src/hashing.py", "symbol": "digest", "line_start": 3, "line_end": 9}])),
        ("test_file", json!(["tests/test_c4.py"])),
    ] {
        let mut f = fields(ok.clone());
        f.insert(k.to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "dependency_drafted", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{k}={bad} gave {got:?}");
    }
    for missing in ["approval", "criterion_id", "text", "test_file"] {
        let mut f = fields(ok.clone());
        f.remove(missing);
        let got = append_raw(&Vec::new(), "dependency_drafted", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::MissingField { .. })), "{missing} is required: {got:?}");
    }
}

// dependency_unanswered's approval is a whole number and its why text; told's about is text (a
// run id, or a hold_answered's t written as text); criterion_covered and subagent_ended are two
// text fields each. Every field is required.
#[test]
fn the_unanswered_draft_the_telling_the_cover_and_the_subagents_end_have_their_shapes() {
    let ok = [
        ("dependency_unanswered", json!({"approval": 19, "why": "the drafted place is under .venv"})),
        ("told", json!({"about": "19"})),
        ("criterion_covered", json!({"criterion_id": "c3", "by_criterion": "c4"})),
        ("subagent_ended", json!({"agent_id": "a1", "agent_type": "general-purpose"})),
    ];
    for (kind, f) in &ok {
        let log = append_raw(&Vec::new(), kind, fields(f.clone()), AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"));
        assert_eq!(log[0].kind, *kind);
        for key in f.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
            let mut less = fields(f.clone());
            less.remove(&key);
            let got = append_raw(&Vec::new(), kind, less, AT, TENANT);
            assert!(matches!(got, Err(AppendError::MissingField { .. })), "{kind}.{key} is required: {got:?}");
        }
    }
    let read: DependencyUnanswered = append_raw(&Vec::new(), "dependency_unanswered", fields(ok[0].1.clone()), AT, TENANT).unwrap()[0].fields_as().unwrap();
    assert_eq!(read.approval, 19);
    let read: Told = append_raw(&Vec::new(), "told", fields(ok[1].1.clone()), AT, TENANT).unwrap()[0].fields_as().unwrap();
    assert_eq!(read.about, "19");
    let read: CriterionCovered = append_raw(&Vec::new(), "criterion_covered", fields(ok[2].1.clone()), AT, TENANT).unwrap()[0].fields_as().unwrap();
    assert_eq!(read.by_criterion, "c4");
    let read: SubagentEnded = append_raw(&Vec::new(), "subagent_ended", fields(ok[3].1.clone()), AT, TENANT).unwrap()[0].fields_as().unwrap();
    assert_eq!(read.agent_type, "general-purpose");
    for (kind, bad) in [
        ("dependency_unanswered", json!({"approval": "19", "why": "w"})),
        ("dependency_unanswered", json!({"approval": 19, "why": 2})),
        ("told", json!({"about": 19})),
        ("criterion_covered", json!({"criterion_id": "c3", "by_criterion": ["c4"]})),
        ("subagent_ended", json!({"agent_id": 1, "agent_type": "Explore"})),
    ] {
        let got = append_raw(&Vec::new(), kind, fields(bad.clone()), AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{kind} {bad} gave {got:?}");
    }
}

#[test]
fn form_optional_fields_are_registered() {
    for row in FORM {
        for opt in row.optional {
            assert!(
                row.fields.contains(opt),
                "{}: optional '{}' not in fields",
                row.kind,
                opt
            );
        }
    }
}

// ci_report_seen.outcome has its own closed set, which pass_please_pass and run_ended do not accept.
#[test]
fn ci_report_seen_outcome_is_its_own_closed_set() {
    let seen = |outcome: &str| {
        fields(json!({"sha": "K1", "run_id": "4242", "finished_at": "2026-10-10T12:00:00Z", "outcome": outcome, "report": ["hosted CI-proven at K1, run 4242"]}))
    };
    for word in ["proven", "not_proven", "could_not", "cancelled", "timed_out"] {
        let log = append_raw(&Vec::new(), "ci_report_seen", seen(word), AT, TENANT);
        assert!(log.is_ok(), "{word}: {log:?}");
    }
    for word in ["passed", "failed", "did_not_run", "success"] {
        let got = append_raw(&Vec::new(), "ci_report_seen", seen(word), AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{word} gave {got:?}");
    }
    let mut empty = seen("cancelled");
    empty.insert("report".to_string(), json!([]));
    assert!(append_raw(&Vec::new(), "ci_report_seen", empty, AT, TENANT).is_ok(), "the report may be empty");
    let p = json!({"criterion_id": "c1", "file_path": "tests/test_c1.py", "commit": "K1", "outcome": "proven", "ran_in": "pipeline", "with_anchor_reverted": false});
    let got = append_raw(&Vec::new(), "pass_please_pass", fields(p), AT, TENANT);
    assert!(matches!(got, Err(AppendError::BadValue(_))), "pass_please_pass refuses proven: {got:?}");
    let r = json!({"run_id": "r1", "outcome": "proven", "end": "exited"});
    assert!(matches!(append_raw(&Vec::new(), "run_ended", fields(r), AT, TENANT), Err(AppendError::BadValue(_))), "run_ended refuses proven");
    for missing in ["sha", "run_id", "finished_at", "outcome", "report"] {
        let mut f = seen("proven");
        f.remove(missing);
        assert!(matches!(append_raw(&Vec::new(), "ci_report_seen", f, AT, TENANT), Err(AppendError::MissingField { .. })), "{missing} is required");
    }
    let typed = CiReportSeen { sha: "K1".into(), run_id: "4242".into(), finished_at: "2026-10-10T12:00:00Z".into(), outcome: "proven".into(), report: vec![] };
    let log = append(&Vec::new(), &typed, AT, TENANT).expect("typed");
    assert_eq!(log[0].kind, "ci_report_seen");
}

// familiar_switch has one required field, `to`, which is off or on.
#[test]
fn familiar_switch_is_to_off_or_on() {
    for word in ["off", "on"] {
        let log = append_raw(&Vec::new(), "familiar_switch", fields(json!({"to": word})), AT, TENANT);
        assert!(log.is_ok(), "{word}: {log:?}");
    }
    for word in ["idle", "OFF", "true", ""] {
        let got = append_raw(&Vec::new(), "familiar_switch", fields(json!({"to": word})), AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{word} gave {got:?}");
    }
    let got = append_raw(&Vec::new(), "familiar_switch", fields(json!({"to": true})), AT, TENANT);
    assert!(matches!(got, Err(AppendError::BadValue(_))), "a bool gave {got:?}");
    let got = append_raw(&Vec::new(), "familiar_switch", Map::new(), AT, TENANT);
    assert!(matches!(got, Err(AppendError::MissingField { .. })), "to is required: {got:?}");
    let got = append_raw(&Vec::new(), "familiar_switch", fields(json!({"to": "off", "by": "agent"})), AT, TENANT);
    assert!(matches!(got, Err(AppendError::UnregisteredKey { .. })), "only `to`: {got:?}");
    let typed = FamiliarSwitch { to: "off".into() };
    let log = append(&Vec::new(), &typed, AT, TENANT).expect("typed");
    assert_eq!(log[0].kind, "familiar_switch");
    assert_eq!(log[0].to_json()["to"], json!("off"));
}

// intent_submitted's `for` is an optional whole number, and the typed form keeps it.
#[test]
fn intent_submitted_keeps_and_checks_its_for() {
    let ok = json!({"text": "greet with hello", "sha": "base", "for": 12});
    let log = append_raw(&Vec::new(), "intent_submitted", fields(ok.clone()), AT, TENANT).unwrap();
    let typed: IntentSubmitted = log[0].fields_as().unwrap();
    assert_eq!(append(&Vec::new(), &typed, AT, TENANT).unwrap(), log, "the typed form keeps for");
    let without = json!({"text": "greet with hello", "sha": "base"});
    let log = append_raw(&Vec::new(), "intent_submitted", fields(without), AT, TENANT).unwrap();
    assert!(log[0].fields_as::<IntentSubmitted>().is_ok(), "a fact with no for still parses");
    assert!(!log[0].fields.contains_key("for"), "absent stays absent");
    for bad in [json!("not-a-position"), json!(-1), json!({"unexpected": true}), json!(1.5)] {
        let mut f = fields(ok.clone());
        f.insert("for".to_string(), bad.clone());
        let got = append_raw(&Vec::new(), "intent_submitted", f, AT, TENANT);
        assert!(matches!(got, Err(AppendError::BadValue(_))), "for={bad} gave {got:?}");
    }
}

// holder_resumed (the owner's draft name, 2026-10-16, S8g): a lane's headless session started
// again with the id of a holder that ended; its agent_id is text, required.
#[test]
fn the_holder_resumed_has_its_shape() {
    let f = json!({"agent_id": "9b2c4d6e-1f3a-4b5c-8d7e-0a1b2c3d4e5f"});
    let log = append_raw(&Vec::new(), "holder_resumed", fields(f), AT, TENANT).unwrap_or_else(|e| panic!("{e}"));
    let read: HolderResumed = log[0].fields_as().unwrap();
    assert_eq!(read.agent_id, "9b2c4d6e-1f3a-4b5c-8d7e-0a1b2c3d4e5f");
    assert!(matches!(append_raw(&Vec::new(), "holder_resumed", Map::new(), AT, TENANT), Err(AppendError::MissingField { .. })));
    assert!(matches!(append_raw(&Vec::new(), "holder_resumed", fields(json!({"agent_id": 1})), AT, TENANT), Err(AppendError::BadValue(_))));
}

// investigator_finding [draft] (the owner, 2026-10-16, S7b): each category carries its own
// fields and no other's: wrong_test its guidance, requirement_missed what was missed and the
// scope, behavior_exists neither. A quote is the test's or the code's lines at a commit, or the
// person's words by the turn they were said.
#[test]
fn investigator_finding_carries_its_categorys_own_fields() {
    let base = |category: &str| {
        json!({
            "criterion_id": "c1", "category": category, "model": "claude-opus-4-6",
            "runs": [{"run_id": "r1", "commit": "K2", "label": "passed"}],
            "reason": "why",
            "quotes": [
                {"role": "code", "file_path": "src/app.sh", "line_start": 2, "line_end": 3, "commit": "K1", "text": "echo hello"},
                {"role": "person", "said": 0, "text": "greet"}
            ]
        })
    };
    let with = |category: &str, extra: &[(&str, &str)]| {
        let mut v = base(category);
        for (k, x) in extra {
            v[*k] = json!(x);
        }
        v
    };
    for ok in [
        with("wrong_test", &[("guidance", "check hello")]),
        with("behavior_exists", &[]),
        with("requirement_missed", &[("missed", "the empty name"), ("scope", "greet only")]),
    ] {
        let log = append_raw(&Vec::new(), "investigator_finding", fields(ok.clone()), AT, TENANT).unwrap_or_else(|e| panic!("{ok}: {e}"));
        let read: InvestigatorFinding = log[0].fields_as().unwrap();
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes: {ok}");
    }
    let quotes = |q: Value| {
        let mut v = with("behavior_exists", &[]);
        v["quotes"] = q;
        v
    };
    let mut no_model = with("behavior_exists", &[]);
    no_model.as_object_mut().unwrap().remove("model");
    for bad in [
        with("wrong_test", &[]),
        with("requirement_missed", &[("missed", "x")]),
        with("requirement_missed", &[("scope", "x")]),
        with("behavior_exists", &[("guidance", "x")]),
        with("wrong_test", &[("guidance", "x"), ("missed", "y")]),
        with("false_green", &[]),
        quotes(json!([{"role": "test", "file_path": "t.sh", "line_start": 1, "line_end": 1, "text": "x"}])),
        quotes(json!([{"role": "person", "text": "x"}])),
        quotes(json!([{"role": "judge", "said": 0, "text": "x"}])),
        no_model,
    ] {
        assert!(append_raw(&Vec::new(), "investigator_finding", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}

// fix_verdict [draft] (the owner's plan of 2026-10-20, step 3): the reviewer's word on a green
// fix at a commit: covers yes, no or not_judged. A "no" carries its gap and the quotes the binary
// read again, each a file, a line and its text; a not_judged says why (error, timeout,
// unparseable or unverified) and what happened, in a few words.
#[test]
fn fix_verdict_is_yes_no_or_not_judged_with_its_quotes_and_why() {
    let no = json!({
        "criterion_id": "c1", "commit": "K3", "model": "claude-opus-4-6", "covers": "no",
        "reason": "the docstring puts the file first", "gap": "the precedence the docstring documents",
        "quotes": [{"file_path": "src/config.py", "line": 12, "text": "Values are read from the file first."}]
    });
    let yes = json!({"criterion_id": "c1", "commit": "K3", "model": "claude-opus-4-6", "covers": "yes", "reason": "it does what was asked"});
    let unverified = json!({
        "criterion_id": "c1", "commit": "K3", "model": "claude-opus-4-6", "covers": "not_judged", "reason": "r", "gap": "g",
        "quotes": [{"file_path": "src/x.py", "line": 3, "text": "nope"}], "why": "unverified", "detail": "quote 1: src/x.py line 3 does not read as quoted"
    });
    let timeout = json!({"criterion_id": "c1", "commit": "K3", "model": "opus", "covers": "not_judged", "reason": "", "why": "timeout", "detail": "stopped after 300 s"});
    for ok in [no.clone(), yes.clone(), unverified, timeout] {
        let log = append_raw(&Vec::new(), "fix_verdict", fields(ok.clone()), AT, TENANT).unwrap_or_else(|e| panic!("{ok}: {e}"));
        let read: FixVerdict = log[0].fields_as().unwrap();
        assert_eq!(append(&Vec::new(), &read, AT, TENANT).unwrap(), log, "typed and raw give the same bytes: {ok}");
    }
    let read: FixVerdict = append_raw(&Vec::new(), "fix_verdict", fields(no.clone()), AT, TENANT).unwrap()[0].fields_as().unwrap();
    assert_eq!(read.covers, FixCovers::No);
    assert_eq!(read.quotes, vec![FixQuote { file_path: "src/config.py".into(), line: 12, text: "Values are read from the file first.".into() }]);
    assert!(append_raw(&Vec::new(), "fix_verdict", fields(yes), AT, TENANT).unwrap()[0].to_json().get("quotes").is_none(), "no quotes, no field");
    let with = |k: &str, x: Value| {
        let mut v = no.clone();
        v[k] = x;
        v
    };
    let mut no_commit = no.clone();
    no_commit.as_object_mut().unwrap().remove("commit");
    for bad in [
        with("covers", json!("maybe")),
        with("covers", json!(false)),
        with("why", json!("bored")),
        with("quotes", json!([{"file": "src/x.py", "line": 3, "text": "x"}])),
        with("quotes", json!([{"file_path": "src/x.py", "line": "3", "text": "x"}])),
        with("rung", json!("opus")),
        no_commit,
    ] {
        assert!(append_raw(&Vec::new(), "fix_verdict", fields(bad.clone()), AT, TENANT).is_err(), "{bad}");
    }
}
