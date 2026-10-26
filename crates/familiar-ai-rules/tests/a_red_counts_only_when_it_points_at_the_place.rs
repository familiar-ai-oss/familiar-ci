//! The regex's translation is what the rules read (plan §6.1, §6.2, §6.5, lane S6): a run's
//! family comes from its `test_log_translation` through `run_id`; a seal needs a valid red, one
//! that points at the plan's place or is an assertion; a result without `run_id` is legacy only
//! before the record's first translation (the grandfather rule), and a pipeline result without
//! one is legacy until S9d; attempts are counted per outcome.

mod common;

use common::{raw, sealed, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::outcome::{self, Family, Route, Stage};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, plan, View};
use serde_json::{json, Value};

/// c1 planned, started and its test committed alone: the tests-first commit, not yet run.
fn tests_first() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    step(&log, "test_written", None)
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run of c1's command at the latest commit as the binary writes it: reserved, ended,
/// translated as `label` (with `extra` fields: location, missing_name), then the results of the
/// own and the touching test naming the run, then the stamp. The touching test passed.
fn ran(log: &Log, word: &str, label: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

fn last_translation(log: &Log) -> Fact {
    log.iter().rev().find(|f| f.kind == "test_log_translation").cloned().expect("a translation")
}

fn family_of(label: &str, extra: Value) -> Family {
    let log = ran(&tests_first(), if label == "passed" { "passed" } else { "failed" }, label, extra);
    let v = View::new(&log);
    let t = last_translation(&log);
    outcome::family(&v, t.fields["run_id"].as_str().unwrap_or(""))
}

const PLACE_FN: &str = "def check_password(user, pw):";

#[test]
fn the_family_of_each_label_and_the_place_rule_table() {
    let none = json!({});
    let at = |file: &str, line: u64, function: Option<&str>| {
        let mut l = json!({"file_path": file, "line": line});
        if let Some(f) = function {
            l["function"] = json!(f);
        }
        json!({"location": l})
    };
    let missing = |name: &str, file: Option<&str>| match file {
        Some(f) => json!({"missing_name": name, "location": {"file_path": f, "line": 1}}),
        None => json!({"missing_name": name}),
    };
    let rows: Vec<(&str, Value, Family, &str)> = vec![
        ("passed", none.clone(), Family::Green, "exit 0, no match"),
        ("tests_failed", none.clone(), Family::ValidRed, "legacy (no counts): a red with no location reads as an assertion"),
        ("tests_failed", at(TEST, 7, Some("def test_session():")), Family::ValidRed, "legacy (no counts): a red in the planned test reads as an assertion"),
        ("tests_failed", at("src/login.py", 45, Some(PLACE_FN)), Family::ValidRed, "an exception inside the place's function"),
        ("tests_failed", at("src/login.py", 45, Some("def hash_password(pw):")), Family::TestBroken, "the place's file, another function"),
        ("tests_failed", at("src/login.py", 45, None), Family::TestBroken, "the place's file, no function read"),
        ("tests_failed", at("src/db.py", 3, Some("def connect():")), Family::TestBroken, "an exception outside the place"),
        ("missing_target_symbol", missing("check_password", Some(TEST)), Family::ValidRed, "the place's symbol, missing in the planned test"),
        ("missing_target_symbol", missing("login.check_password", Some(TEST)), Family::ValidRed, "compared on the last path segment"),
        ("missing_target_symbol", missing("login::check_password", Some(TEST)), Family::ValidRed, "a Rust path too"),
        ("missing_target_symbol", missing("os.check_password", Some(TEST)), Family::Unknown, "the module the output names is not the place's file: the name matches by chance, so the run can't be read"),
        ("missing_target_symbol", missing("pkg.login.check_password", Some(TEST)), Family::Unknown, "a module path that only shares its last part with the place's file (pkg.login vs src/login.py): a chance match, can't be read"),
        ("missing_target_symbol", missing("src.login.check_password", Some(TEST)), Family::ValidRed, "the module path is the place's file (src/login.py)"),
        ("missing_target_symbol", missing("other.nope", Some(TEST)), Family::TestBroken, "another name, qualified with another module: plainly not the place's, the test is broken"),
        ("missing_target_symbol", missing("Nosuch", Some(TEST)), Family::TestBroken, "another name"),
        ("missing_target_symbol", missing("check_password", Some("tests/conftest.py")), Family::TestBroken, "not a planned test file"),
        ("missing_target_symbol", missing("check_password", None), Family::Unknown, "no location extracted"),
        ("collection_error", none.clone(), Family::TestBroken, ""),
        ("no_tests_collected", none.clone(), Family::TestBroken, ""),
        ("setup_error", none.clone(), Family::TestBroken, ""),
        ("usage_error", none.clone(), Family::CommandBroken, "exit 127 on a local path, a syntax error"),
        ("environment_error", none.clone(), Family::NotTheAgents, "exit 127 on a bare word"),
        ("crashed", none.clone(), Family::Crashed, "132, 134, 135, 136, 139"),
        ("killed", none.clone(), Family::Transient, "130, 137, 143"),
        ("runner_crash", none.clone(), Family::Transient, ""),
        ("timed_out", none.clone(), Family::TooSlow, ""),
        ("unclassified", none.clone(), Family::Unknown, "a nonzero exit with no match, any other 128+N"),
    ];
    for (label, extra, want, why) in rows {
        assert_eq!(family_of(label, extra.clone()), want, "{label} {extra}: {why}");
    }
}

/// Lane H6 (plan of record 2026-10-19, the family table, D1, D2, D4, D7): a translation that
/// carries `counts` is the binary's new reading, and a red is honest only on positive evidence:
/// where it points and its kind. Anything else is can't tell (a card). The table above is the
/// legacy reading, kept for a translation without `counts`.
#[test]
fn a_new_style_reading_is_an_honest_red_only_where_it_points_and_by_its_kind() {
    let red = |location: Option<Value>| match location {
        Some(l) => json!({"location": l, "counts": {"failed": 1}}),
        None => json!({"counts": {"failed": 1}}),
    };
    let at = |file: &str, function: Option<&str>, kind: Option<&str>| {
        let mut l = json!({"file_path": file, "line": 7});
        if let Some(f) = function {
            l["function"] = json!(f);
        }
        if let Some(k) = kind {
            l["kind"] = json!(k);
        }
        Some(l)
    };
    let rows: Vec<(&str, Value, Family, &str)> = vec![
        ("tests_failed", red(None), Family::Unknown, "no location: can't tell (§6.2)"),
        ("tests_failed", red(at(TEST, Some("def test_session():"), Some("assertion"))), Family::ValidRed, "an assertion in a planned test file"),
        ("tests_failed", red(at(TEST, Some("def test_session():"), Some("exception"))), Family::Unknown, "an exception in a planned test file: can't tell (D1)"),
        ("tests_failed", red(at(TEST, None, None)), Family::Unknown, "a planned test file, no kind: no evidence of an assertion"),
        ("tests_failed", red(at("src/login.py", Some(PLACE_FN), Some("exception"))), Family::ValidRed, "inside the place's function"),
        ("tests_failed", red(at("src/login.py", Some(PLACE_FN), Some("assertion"))), Family::ValidRed, "inside the place's function, any kind"),
        ("tests_failed", red(at("src/login.py", Some("def hash_password(pw):"), Some("exception"))), Family::Unknown, "the place's file, another function: can't tell, the person confirms (the owner, 2026-10-19)"),
        ("tests_failed", red(at("src/db.py", Some("def connect():"), Some("exception"))), Family::Unknown, "an exception in the project outside the place: can't tell, the person confirms (the owner, 2026-10-19)"),
        ("tests_failed", red(at("src/db.py", Some("def connect():"), Some("assertion"))), Family::Unknown, "an assertion elsewhere: can't tell (D2)"),
        ("tests_failed", red(at("src/db.py", Some("def connect():"), None)), Family::Unknown, "elsewhere, no kind"),
        ("missing_target_symbol", json!({"missing_name": "check_password", "location": {"file_path": TEST, "line": 1, "kind": "compile"}, "counts": {}}), Family::ValidRed, "as today: the place's symbol, missing in the planned test"),
        ("missing_target_symbol", json!({"missing_name": "check_password", "counts": {}}), Family::Unknown, "as today: no location"),
        ("passed", json!({"counts": {"passed": 2}}), Family::Green, "a pass"),
        ("passed", json!({"counts": {"passed": 1, "deselected": 3}}), Family::Green, "deselected stays a pass (D4)"),
        ("passed", json!({"counts": {"passed": 1, "skipped": 1}}), Family::Unknown, "a skip beside the pass: can't tell (D4)"),
        ("passed", json!({"counts": {}}), Family::Green, "nothing counted, no seal run to judge against"),
        ("collection_error", json!({"counts": {}}), Family::TestBroken, "as today"),
        ("unclassified", json!({"counts": {}}), Family::Unknown, "as today"),
    ];
    for (label, extra, want, why) in rows {
        assert_eq!(family_of(label, extra.clone()), want, "{label} {extra}: {why}");
    }
}

#[test]
fn a_new_style_red_with_no_location_or_an_exception_in_the_test_seals_nothing() {
    let sealed_by = |extra: Value| {
        let log = ran(&tests_first(), "failed", "tests_failed", extra);
        criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some()
    };
    assert!(sealed_by(json!({"location": {"file_path": TEST, "line": 7, "kind": "assertion"}, "counts": {"failed": 1}})), "an assertion in the planned test seals");
    assert!(!sealed_by(json!({"counts": {"failed": 1}})), "a red with no location is no seal");
    assert!(!sealed_by(json!({"location": {"file_path": TEST, "line": 7, "kind": "exception"}, "counts": {"failed": 1}})), "D1: can't tell");
    assert!(!sealed_by(json!({"location": {"file_path": "src/db.py", "line": 3, "function": "def connect():", "kind": "assertion"}, "counts": {"failed": 1}})), "D2: can't tell");
    let log = ran(&tests_first(), "failed", "tests_failed", json!({"counts": {"failed": 1}}));
    assert!(whys(&log).contains(&"run_unclassified".to_string()), "the person's card: {:?}", whys(&log));
}

#[test]
fn points_at_place_reads_the_function_not_only_the_lines() {
    let log = ran(&tests_first(), "failed", "tests_failed", json!({"location": {"file_path": "src/login.py", "line": 90, "function": PLACE_FN}}));
    let v = View::new(&log);
    let a = plan::anchor_of(&v, "c1").expect("the anchor");
    assert!(outcome::points_at_place(&last_translation(&log), a, &v), "line 90 is past the claimed 40–62, but inside check_password");
    let log = ran(&tests_first(), "failed", "tests_failed", json!({"location": {"file_path": "src/login.py", "line": 50, "function": "def check_passwords_later():"}}));
    let v = View::new(&log);
    let a = plan::anchor_of(&v, "c1").expect("the anchor");
    assert!(!outcome::points_at_place(&last_translation(&log), a, &v), "inside the claimed lines, but another function: a name is matched whole");
}

#[test]
fn each_family_routes_by_its_stage() {
    use Stage::{AfterFix, BeforeSeal};
    for (f, before, after) in [
        (Family::Green, Route::Refuse, Route::Green),
        (Family::ValidRed, Route::Seal, Route::Attempt),
        (Family::TestBroken, Route::Refuse, Route::Attempt),
        (Family::CommandBroken, Route::Command, Route::Command),
        (Family::NotTheAgents, Route::Card, Route::Card),
        (Family::Crashed, Route::Card, Route::Attempt),
        (Family::Transient, Route::Retry, Route::Retry),
        (Family::TooSlow, Route::Refuse, Route::Attempt),
        (Family::Unknown, Route::Card, Route::Card),
    ] {
        assert_eq!((outcome::route(f, BeforeSeal), outcome::route(f, AfterFix)), (before, after), "{f:?}");
    }
}

#[test]
fn a_seal_needs_a_valid_red_never_a_bare_failed() {
    let sealed_by = |label: &str, extra: Value| {
        let log = ran(&tests_first(), "failed", label, extra);
        criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some()
    };
    assert!(sealed_by("tests_failed", json!({})), "an assertion seals");
    assert!(sealed_by("missing_target_symbol", json!({"missing_name": "check_password", "location": {"file_path": TEST, "line": 1}})));
    for label in ["unclassified", "collection_error", "environment_error", "usage_error", "crashed", "killed", "timed_out"] {
        assert!(!sealed_by(label, json!({})), "{label} ran and did not pass, and is no seal");
    }
    assert!(!sealed_by("missing_target_symbol", json!({"missing_name": "Nosuch", "location": {"file_path": TEST, "line": 1}})));
    assert!(!sealed_by("tests_failed", json!({"location": {"file_path": "src/db.py", "line": 3, "function": "def connect():"}})));
}

#[test]
fn the_grandfather_rule_reads_a_result_without_run_id_as_legacy_only_before_the_first_translation() {
    // an e68d414-shaped record: no translation anywhere, the seal stands on the word
    let log = sealed();
    let v = View::new(&log);
    assert!(criterion::sealed_at(&v, "c1", criterion::Place::Laptop).is_some(), "legacy: read as it always was");
    let r = log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == TEST).expect("the result");
    assert_eq!(outcome::result_family(&v, "c1", r), Family::ValidRed);
    // a new record: a translation came first, so a laptop result without run_id is Unknown
    let log = tests_first();
    let log = raw(&log, "test_log_translation", json!({"run_id": "r-old", "criterion_id": "c1", "commit": "x", "by": "regex", "label": "passed", "evidence": "", "failing_tests": [], "classified_on": "full"}));
    let id = head(&log);
    let log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": TEST, "commit": id, "outcome": "failed", "ran_in": "laptop", "with_anchor_reverted": false}));
    let log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": TOUCHING, "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false}));
    let log = step(&log, "fingerprint_stamped", None);
    let v = View::new(&log);
    let r = log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == TEST).expect("the result");
    assert_eq!(outcome::result_family(&v, "c1", r), Family::Unknown, "never failed");
    assert!(criterion::sealed_at(&v, "c1", criterion::Place::Laptop).is_none());
    // S9d: a pipeline result without run_id after the first translation is Unknown, as the laptop's
    let log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": TEST, "commit": id, "outcome": "failed", "ran_in": "pipeline", "with_anchor_reverted": false}));
    let v = View::new(&log);
    let p = log.last().expect("the pipeline result");
    assert_eq!(outcome::result_family(&v, "c1", p), Family::Unknown);
}

#[test]
fn attempts_are_counted_per_outcome_and_try_again_resets_its_own() {
    let log = ran(&tests_first(), "failed", "tests_failed", json!({}));
    assert_eq!(whys(&log), Vec::<String>::new(), "sealed");
    let slow = |log: &Log| ran(&step(log, "code", None), "failed", "timed_out", json!({}));
    let red = |log: &Log| ran(&step(log, "code", None), "failed", "tests_failed", json!({}));
    let log = slow(&log);
    let log = slow(&log);
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "timed_out"), 2);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "tests_failed"), 0, "not counted across outcomes");
    assert!(!whys(&log).contains(&"two_failures".to_string()), "two_failures reads tests_failed only: {:?}", whys(&log));
    // S7: two of one family are its own card, which pauses the part until the person answers
    assert!(whys(&log).contains(&"two_timed_out".to_string()), "{:?}", whys(&log));
    // the person's try_again on one outcome's hold resets that outcome only
    let log = raw(&log, "hold_answered", json!({"criterion_id": "c1", "hold": "two_timed_out", "choice": "try_again", "typed": ""}));
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "timed_out"), 0);
    let log = red(&log);
    assert_eq!(lifecycle::attempts_of(&View::new(&log), "c1", "tests_failed"), 1);
    let log = red(&log);
    let v = View::new(&log);
    assert_eq!((lifecycle::attempts_of(&v, "c1", "tests_failed"), lifecycle::attempts(&v, "c1")), (2, 2));
    assert!(whys(&log).contains(&"two_failures".to_string()));
    assert_eq!(lifecycle::attempts_of(&v, "c1", "timed_out"), 0, "not counted across outcomes");
}

#[test]
fn a_tests_first_run_that_passes_is_still_a_try_and_an_unclassified_one_is_no_red_run() {
    let log = ran(&tests_first(), "passed", "passed", json!({}));
    let v = View::new(&log);
    assert_eq!(lifecycle::tries_not_red(&v, "c1"), 1);
    assert_eq!(whys(&log), ["tests_not_red"]);
    let log = ran(&tests_first(), "failed", "unclassified", json!({}));
    let v = View::new(&log);
    assert!(lifecycle::red_runs(&v, "c1").is_empty(), "a red run is a valid red");
}

#[test]
fn green_reads_the_family_of_the_pass_and_an_unclassified_exit_0_is_not_green() {
    let sealed = ran(&tests_first(), "failed", "tests_failed", json!({}));
    let log = ran(&step(&sealed, "code", None), "passed", "passed", json!({}));
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "green");
    // exit 0 that the table would not call a pass (pytest's --help) is no green
    let log = ran(&step(&sealed, "code", None), "passed", "unclassified", json!({}));
    assert_ne!(lifecycle::state(&View::new(&log), "c1"), "green");
}
