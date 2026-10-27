//! Proves `claims_from_log` drops the stamps and the refused kinds, keeps log order, and turns
//! every typed plan form into a claim.

mod common;

use common::fields;
use familiar_ai_facts::{
    append, append_raw, AnchorShaMatched, Behavior, CasesFound, CheckedAt, Criterion, CriterionAssigned,
    CriterionCovered, CriterionHead, CriterionStrungOnHead, DependencyDrafted, FingerprintStamped, FixLocation, FixLocationCategory,
    FoundCase, FoundQuestion, HoldAnswered, IntentSubmitted, LensFound, LensReleased, Log, QuestionAnswered, NewWorkNoSpot, NewWorkSpotChosen, Place, PlaceChosen, Promise, RanIn, TestChangeAnnounced,
    TestPlanned, TestStackDetected, TestWritten, TouchingTestDeclared, Will,
};
use familiar_ai_plan::{claims_from_log, from_bytes, to_bytes, PLAN_KINDS};
use serde_json::json;

#[test]
fn claims_from_log_drops_stamps_and_refused_kinds_keeps_order() {
    let at = 1_700_000_000.0;
    let tenant = "t-1";
    let mut log: Log = Vec::new();
    log = append(
        &log,
        &IntentSubmitted {
            text: "x".into(),
            sha: "s".into(),
            for_t: None,
        },
        at,
        tenant,
    )
    .unwrap();
    // A refused kind between two kept ones.
    log = append_raw(
        &log,
        "pass_please_pass",
        fields(&[
            ("criterion_id", json!("c1")),
            ("file_path", json!("tests/test_c1.py")),
            ("commit", json!("T1")),
            ("outcome", json!("failed")),
            ("ran_in", json!("laptop")),
        ]),
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &CriterionHead {
            criterion_id: "c1".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &NewWorkNoSpot {
            criterion_id: "c3".into(),
            integration_point: "src/app.py:main".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    assert_eq!(log.len(), 4);
    let claims = claims_from_log(&log);
    assert_eq!(
        claims.iter().map(|c| c.kind()).collect::<Vec<_>>(),
        vec!["intent_submitted", "criterion_head", "new_work_no_spot"]
    );
    for claim in &claims {
        for stamped in ["t", "at", "tenant"] {
            assert!(!claim.fields().contains_key(stamped), "{stamped}");
        }
    }
    assert_eq!(from_bytes(&to_bytes(&claims)).expect("read"), claims);
}

// The typed forms all convert through the log into claims.

#[test]
fn claims_from_log_of_every_typed_form() {
    let at = 0.0;
    let tenant = "t";
    let mut log: Log = Vec::new();
    log = append(
        &log,
        &Behavior {
            behavior_id: "b1".into(),
            text: "x".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &Promise {
            promise_id: "p1".into(),
            behavior_id: "b1".into(),
            accepts: "a".into(),
            gives_back: "g".into(),
            refuses: "r".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &Criterion {
            criterion_id: "c1".into(),
            text: "x".into(),
            path: familiar_ai_facts::Path::Full,
            behavior_id: Some("b1".into()),
            promise_id: Some("p1".into()),
            for_t: None,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &PlaceChosen {
            criterion_id: "c1".into(),
            places: vec![Place {
                file_path: "src/login.py".into(),
                symbol: "check_password".into(),
                line_start: 40,
                line_end: 62,
            }],
            hold: None,
            choice: None,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &NewWorkSpotChosen {
            criterion_id: "c1".into(),
            file_path: "src/mail.py".into(),
            symbol: "reset_link".into(),
            beside: "send_mail".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &CriterionHead {
            criterion_id: "c1".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &CriterionStrungOnHead {
            criterion_id: "c2".into(),
            head: "c1".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &TestStackDetected {
            codebase_id: "cb1".into(),
            project_root: "src/".into(),
            test_stack: "python_pytest".into(),
            test_command: "pytest".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &TestPlanned {
            criterion_id: "c1".into(),
            file_path: "tests/test_c1.py".into(),
            will: Will::Create,
            test_command: None,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &TouchingTestDeclared {
            criterion_id: "c1".into(),
            file_path: "tests/test_login_old.py".into(),
            test_name: "t_old".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &TestWritten {
            criterion_id: "c1".into(),
            file_path: "tests/test_c1.py".into(),
            test_name: "t1".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &AnchorShaMatched {
            criterion_id: "c1".into(),
            file_path: "src/login.py".into(),
            symbol: "check_password".into(),
            line_start: 40,
            line_end: 62,
            sha: "abc".into(),
            lines_hash: "L0".into(),
            matched: true,
            checked_at: CheckedAt::Planning,
            tip_matched: None,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &FingerprintStamped {
            criterion_id: "c1".into(),
            commit: "T1".into(),
            proof_not_tampered: "S1".into(),
            ran_in: RanIn::Laptop,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &CriterionAssigned {
            criterion_id: "c1".into(),
            agent_conversation_id: "a1".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &TestChangeAnnounced {
            criterion_id: "c1".into(),
            reason: "spec moved".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &NewWorkNoSpot {
            criterion_id: "c3".into(),
            integration_point: "src/app.py:main".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &FixLocationCategory {
            criterion_id: "c1".into(),
            category: FixLocation::Dependency,
            where_: "the library the anchor calls".into(),
            needs: None,
        },
        at,
        tenant,
    )
    .unwrap();
    // the case-finder's finding and the person's answer on the cases card are claims of the plan.
    // Definition change (the owner, 2026-10-25: "Mirror them") [draft names]: a record rebuilt from
    // the plan carries them, so it never asks again
    log = append(
        &log,
        &CasesFound {
            criterion_id: "c1".into(),
            outcome: "found".into(),
            cases: vec![FoundCase {
                case_id: "a1".into(),
                short: "a locked account is refused".into(),
                given: "a locked account and the right password".into(),
                expect: "refused".into(),
                why: "the code checks the lock first".into(),
            }],
            why: String::new(),
            model: "sonnet".into(),
            tokens: 1234,
            questions: vec![FoundQuestion {
                question_id: "q1".into(),
                text: "Is a locked account refused even with the right password?".into(),
                options: vec!["yes".into(), "no".into()],
                why: "the person's words name no lock".into(),
            }],
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &HoldAnswered {
            criterion_id: "c1".into(),
            hold: "cases_unconfirmed".into(),
            choice: "add_all".into(),
            typed: String::new(),
        },
        at,
        tenant,
    )
    .unwrap();
    // the lens panel (the owner's plan of 2026-10-26, approved). Definition change: a lens's answer
    // and the person's pick on the synthesis's question are claims of the plan, as the synthesis's
    // finding is; which Agent call got which lens is the binary's bookkeeping and stays out
    log = append(
        &log,
        &LensReleased {
            criterion_id: "c1".into(),
            lens: "ambiguity".into(),
            tool_use_id: "toolu_01".into(),
            agent_id: None,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &LensFound {
            criterion_id: "c1".into(),
            lens: "ambiguity".into(),
            outcome: "none".into(),
            findings: Vec::new(),
            model: "opus".into(),
            tokens: 900,
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &QuestionAnswered {
            criterion_id: "c1".into(),
            question_id: "q1".into(),
            choice: "yes".into(),
            typed: String::new(),
        },
        at,
        tenant,
    )
    .unwrap();
    // the person's answer on another card, the dependency drafted for it, and a criterion it
    // covers: none is a claim kind. Definition change (the owner, 2026-10-14): no kind mirrored
    // into the plan may let CI accept a person's approval; verify would read a hand-written one as
    // the person's
    log = append(
        &log,
        &HoldAnswered {
            criterion_id: "c1".into(),
            hold: "crashed".into(),
            choice: "a_dependency".into(),
            typed: String::new(),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &DependencyDrafted {
            approval: (log.len() - 1) as u64,
            criterion_id: "c4".into(),
            text: "the hashing helper returns text, not bytes".into(),
            test_file: "tests/test_c4.py".into(),
            place: Some(Place {
                file_path: "src/hashing.py".into(),
                symbol: "digest".into(),
                line_start: 3,
                line_end: 9,
            }),
        },
        at,
        tenant,
    )
    .unwrap();
    log = append(
        &log,
        &CriterionCovered {
            criterion_id: "c1".into(),
            by_criterion: "c4".into(),
        },
        at,
        tenant,
    )
    .unwrap();
    let claims = claims_from_log(&log);
    assert_eq!(claims.len(), log.len() - 4, "every fact here but the lens's release, the other card's answer, its draft and the covered criterion is a claim kind");
    for kind in ["dependency_drafted", "criterion_covered", "lens_released"] {
        assert!(claims.iter().all(|c| c.kind() != kind), "the plan carries no {kind}");
    }
    let holds: Vec<&str> = claims.iter().filter(|c| c.kind() == "hold_answered").filter_map(|c| c.fields().get("hold").and_then(|v| v.as_str())).collect();
    assert_eq!(holds, ["cases_unconfirmed"], "the plan carries the cases card's answer and no other card's");
    let covered: std::collections::BTreeSet<_> = claims.iter().map(|c| c.kind()).collect();
    let missing: Vec<_> = PLAN_KINDS
        .iter()
        .filter(|k| !covered.contains(**k))
        .copied()
        .collect();
    assert_eq!(
        missing,
        vec!["intent_submitted"],
        "covered by the other test"
    );
    let back = from_bytes(&to_bytes(&claims)).expect("round trip");
    assert_eq!(claims, back);
}

// The mirror writes the seed's plan, then the claims made here (the owner, 2026-10-15; plan §6.8):
// with a record_seeded in the log, its first plan_lines facts are the committed plan, written as
// they are, and after the mark the claims made here; the seed's walk, verify's pipeline facts and
// the twins between them (stamps among them, a plan kind) never become lines. Without a mark the
// plan carries every claim kind in log order, as before.
#[test]
fn the_mirror_writes_the_seeds_plan_then_the_claims_made_here() {
    let (at, tenant) = (1_700_000_000.0, "_default_");
    let committed = common::one_of_each();
    let mut log: Log = Vec::new();
    for c in &committed {
        log = append_raw(&log, c.kind(), c.fields().clone(), at, tenant).unwrap();
    }
    let plan_lines = log.len();
    let fact = |log: &Log, kind: &str, v: serde_json::Value| append_raw(log, kind, v.as_object().cloned().unwrap_or_default(), at, tenant).unwrap();
    log = fact(&log, "commit_seen", json!({ "commit": "K1", "files": ["tests/test_c1.py"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    log = fact(&log, "pipeline_run_seen", json!({ "head_commit": "K1" }));
    for place in ["pipeline", "laptop"] {
        log = fact(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.py", "commit": "K1", "outcome": "failed", "ran_in": place, "with_anchor_reverted": false }));
        log = fact(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": "K1", "proof_not_tampered": "S1", "ran_in": place }));
    }
    let unmarked = log.clone();
    log = fact(&log, "record_seeded", json!({ "commit": "K1", "plan_lines": plan_lines }));
    assert_eq!(String::from_utf8_lossy(&to_bytes(&claims_from_log(&log))), String::from_utf8_lossy(&to_bytes(&committed)), "the committed plan, byte for byte");

    let claimed = append(&log, &CriterionAssigned { criterion_id: "c1".into(), agent_conversation_id: "lane".into() }, at, tenant).unwrap();
    let now = to_bytes(&claims_from_log(&claimed));
    let line = format!("{}\n", json!({ "kind": "criterion_assigned", "criterion_id": "c1", "agent_conversation_id": "lane" }));
    assert_eq!(String::from_utf8_lossy(&now), format!("{}{line}", String::from_utf8_lossy(&to_bytes(&committed))), "the committed plan, then the one claim made here");

    let before: Vec<&str> = claims_from_log(&unmarked).iter().map(|c| c.kind()).collect();
    assert_eq!(before.len(), committed.len() + 1, "with no mark the laptop's stamp is a line, the pipeline's never (S9d): {before:?}");
}

/// Plan §6.9's fence (lane S9d-ci): the pipeline's stamps are CI's own, written in CI's record;
/// the plan mirrors the laptop's alone, so a pipeline stamp never reaches the plan file.
#[test]
fn claims_from_log_keeps_the_laptops_stamps_and_never_the_pipelines() {
    let mut log: Log = Vec::new();
    for ran_in in ["laptop", "pipeline"] {
        log = append_raw(
            &log,
            "fingerprint_stamped",
            fields(&[("criterion_id", json!("c1")), ("commit", json!("K")), ("proof_not_tampered", json!("H")), ("ran_in", json!(ran_in))]),
            1.0,
            "t",
        )
        .unwrap();
    }
    let kept: Vec<String> = claims_from_log(&log).iter().map(|c| c.fields().get("ran_in").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect();
    assert_eq!(kept, ["laptop"]);
}
