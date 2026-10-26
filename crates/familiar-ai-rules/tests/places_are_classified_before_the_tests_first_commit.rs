//! Places: where else the behavior lives, listed before the fix (the owner's plan of 2026-10-21,
//! step 3) [draft names]. The binary lists the sites that name what the planned place names
//! (`places_listed`); the agent classifies each (`place_classified`): `applies`, which adds a
//! contract case of kind `place` that needs its own test like any case, or `not_affected` with a
//! one-line reason. While a listed site of a part in its turn is unclassified, the part holds
//! `places_unclassified`, the agent's hold, never a card: its tests-first commit is refused until
//! every site is classified. A listing with no site holds nothing.

mod common;

use common::{raw, step, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::contract_unmet;
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, places, plan, terminal, View};
use serde_json::{json, Value};

const PRESENT: &str = "tests/test_c1.py::test_present";
const ABSENT: &str = "tests/test_c1.py::test_absent";
const TWIN: &str = "tests/test_c1.py::test_twin";

fn planned() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let cases = json!([
        {"case_id": "n1", "kind": "normal", "given": "the trigger present", "expect": "the new result"},
        {"case_id": "o1", "kind": "opposite", "given": "the trigger absent", "expect": "the result as before"}
    ]);
    raw(&log, "promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a user name and a password", "gives_back": "a session", "refuses": "a wrong password", "cases": cases}))
}

/// The binary's listing of c1's places: two sites.
fn listed(log: &Log) -> Log {
    raw(
        log,
        "places_listed",
        json!({"criterion_id": "c1", "sites": [
            {"file": "src/login.py", "line": 80, "def": "def check_token(token):", "names": ["check_password"]},
            {"file": "src/views.py", "line": 12, "def": "def login(request):", "names": ["check_password"]}
        ]}),
    )
}

fn classified(log: &Log, file: &str, line: u64, verdict: &str, reason: &str) -> Log {
    raw(log, "place_classified", json!({"criterion_id": "c1", "file": file, "line": line, "verdict": verdict, "reason": reason}))
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::holds(&View::new(log), "c1").iter().map(|h| h.why.to_string()).collect()
}

fn tests_commit(log: &Log) -> Decision {
    decide(&View::new(log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code: false })
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run at the latest commit read by Familiar's reporter with its per-test lists, the touching
/// test passed, the stamp.
fn ran(log: &Log, word: &str, label: &str, failing: &[&str], ran_tests: &[&str]) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let counts = json!({"failed": failing.len(), "passed": ran_tests.len() - failing.len(), "skipped": 0});
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "reporter", "label": label, "evidence": "", "classified_on": "full", "counts": counts, "failing_tests": failing, "ran_tests": ran_tests, "skipped_tests": []});
    if word == "failed" {
        t["location"] = json!({"file_path": TEST, "line": 7, "kind": "assertion"});
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

fn sealed(log: &Log, failing: &[&str]) -> Log {
    let log = step(log, "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    ran(&log, "failed", "tests_failed", failing, &[PRESENT, ABSENT, TWIN])
}

fn judged(log: &Log, cases: &[(&str, &str, bool)]) -> Log {
    let cases: Vec<Value> = cases.iter().map(|(id, test, ok)| json!({"case_id": id, "test": test, "ok": ok})).collect();
    raw(log, "judge_verdict", json!({"criterion_id": "c1", "model": "claude-opus-4-6", "answers_intent": "yes", "reason": "r", "cases": cases}))
}

#[test]
fn a_listed_site_left_unclassified_holds_the_tests_first_commit() {
    let log = listed(&step(&planned(), "start_check", None));
    assert!(whys(&log).contains(&places::HOLD.to_string()), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "held");
    let Decision::Refuse(r) = tests_commit(&log) else { panic!("the tests-first commit is refused") };
    assert_eq!(r.hold.as_deref(), Some(places::HOLD));
    // the refusal names each site left and the claim that clears it; never a card
    let words = terminal::refusal(&View::new(&log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code: false }).unwrap_or_default();
    assert!(words.contains("held: places_unclassified"), "{words}");
    assert!(words.contains("  unclassified: src/login.py:80 (in def check_token(token):) names check_password"), "{words}");
    assert!(words.contains("  unclassified: src/views.py:12 (in def login(request):) names check_password"), "{words}");
    assert!(words.contains("(place_classified)"), "{words}");
    assert!(familiar_ai_rules::criterion::answers_of(places::HOLD).is_none(), "the agent's hold, not a card");
    // no commit is offered while it holds
    let v = View::new(&log);
    assert!(!familiar_ai_rules::moves::criterion_admissible(&v).iter().any(|m| m.label.as_deref() == Some("tests_only")));

    // one classified, one left: still held, naming only the one left
    let log = classified(&log, "src/login.py", 80, "applies", "a token is checked the same way");
    assert!(whys(&log).contains(&places::HOLD.to_string()));
    let words = terminal::refusal(&View::new(&log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code: false }).unwrap_or_default();
    assert!(!words.contains("src/login.py:80") && words.contains("src/views.py:12"), "{words}");
    // not_affected with a blank reason classifies nothing
    let log = classified(&log, "src/views.py", 12, "not_affected", "  ");
    assert!(whys(&log).contains(&places::HOLD.to_string()), "a blank reason is no reason");
    let log = classified(&log, "src/views.py", 12, "not_affected", "it only calls check_password and passes its answer on");
    assert!(!whys(&log).contains(&places::HOLD.to_string()), "{:?}", whys(&log));
    assert!(tests_commit(&log).allowed());
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "testing");
    step(&log, "tests_only", None);
}

#[test]
fn a_listing_with_no_site_holds_nothing() {
    let log = raw(&step(&planned(), "start_check", None), "places_listed", json!({"criterion_id": "c1", "sites": []}));
    assert!(!whys(&log).contains(&places::HOLD.to_string()));
    assert!(tests_commit(&log).allowed());
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "testing");
}

#[test]
fn a_classification_before_the_listing_or_of_a_site_not_listed_counts_for_nothing() {
    let log = step(&planned(), "start_check", None);
    let log = classified(&log, "src/login.py", 80, "applies", "early");
    let log = listed(&log);
    let log = classified(&log, "src/views.py", 12, "not_affected", "it only passes the answer on");
    let log = classified(&log, "src/other.py", 3, "not_affected", "never listed");
    let left: Vec<String> = places::unclassified(&View::new(&log), "c1").iter().map(|s| format!("{}:{}", s.file, s.line)).collect();
    assert_eq!(left, vec!["src/login.py:80".to_string()]);
}

#[test]
fn an_applies_place_is_a_contract_case_of_kind_place_that_needs_its_test() {
    let log = listed(&planned());
    let log = classified(&log, "src/login.py", 80, "applies", "a token is checked the same way");
    let log = classified(&log, "src/views.py", 12, "not_affected", "it only passes the answer on");
    let cases = plan::cases_of(&View::new(&log), "c1");
    let place: Vec<&plan::Case> = cases.iter().filter(|k| k.kind == places::KIND).collect();
    assert_eq!(place.len(), 1, "{cases:?}");
    assert_eq!(place[0].case_id, "place1");
    assert_eq!(place[0].given, "the same call through src/login.py:80 (def check_token(token):)");
    assert_eq!(place[0].expect, "a token is checked the same way");
    assert_eq!(cases.len(), 3, "the promise's two and the place's");

    // the judge named no test for it: green stays away
    let log = sealed(&log, &[PRESENT, TWIN]);
    let none = judged(&log, &[("n1", "test_present", true), ("o1", "test_absent", true), ("place1", "none", false)]);
    assert_eq!(contract_unmet(&View::new(&none), "c1").as_deref(), Some("case place1 (place): no test exercises it, as the judge read the tests"));
    // a place's test is confirmed by the runner as a normal case's: it fails at the seal run
    let passed_there = judged(&sealed(&listed_and_classified(), &[PRESENT]), &[("n1", "test_present", true), ("o1", "test_absent", true), ("place1", "test_twin", true)]);
    assert_eq!(
        contract_unmet(&View::new(&passed_there), "c1").as_deref(),
        Some("case place1 (place): the judge named test_twin, but it did not fail at the seal run; a normal case's test fails before the fix")
    );
    let named = judged(&log, &[("n1", "test_present", true), ("o1", "test_absent", true), ("place1", "test_twin", true)]);
    assert_eq!(contract_unmet(&View::new(&named), "c1"), None);
    let green = ran(&step(&named, "code", None), "passed", "passed", &[], &[PRESENT, ABSENT, TWIN]);
    assert_eq!(lifecycle::state(&View::new(&green), "c1"), "green");
}

fn listed_and_classified() -> Log {
    let log = listed(&planned());
    let log = classified(&log, "src/login.py", 80, "applies", "a token is checked the same way");
    classified(&log, "src/views.py", 12, "not_affected", "it only passes the answer on")
}
