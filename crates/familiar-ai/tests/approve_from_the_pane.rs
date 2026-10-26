//! The pane's approval (plan §6.9 B, B-6; lane S9e): Familiar's pane answers a
//! card by running `familiar-ai approve` with the card's id, hold and choice
//! (or the person's typed words) and the passphrase on stdin. What approve
//! records is the answer the AskUserQuestion path records, plus the approval.
//! With no key, the pane says to run the setup step and offers an unsigned
//! answer, recorded with no approval (CI reads "approval not verified").

mod common;

use common::Project;
use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::dependency::restring;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{plan, View};
use serde_json::{json, Value};
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, LineEnding, PrivateKey, SshSig};

const NAMESPACE: &str = "familiar-ai-approval";
const PASSPHRASE: &str = "correct horse battery staple";
const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// A set-up project with c1 planned, and the person's encrypted key beside it.
fn laptop(name: &str) -> (Project, PrivateKey, std::path::PathBuf) {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    let k = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("key");
    let file = p.dir.join("familiar_ai_approve");
    k.encrypt(&mut OsRng, PASSPHRASE).expect("encrypt").write_openssh_file(&file, LineEnding::LF).expect("key file");
    (p, k, file)
}

fn args(rest: &[&str], key: &std::path::Path) -> Vec<String> {
    let mut a: Vec<String> = ["approve", "--criterion_id", "c1", "--hold", "two_failures"].iter().map(|s| s.to_string()).collect();
    a.extend(rest.iter().map(|s| s.to_string()));
    a.extend(["--key".to_string(), key.display().to_string()]);
    a
}

fn approvals(p: &Project) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(p.path(".familiar/approvals"))
        .map(|d| d.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn the_persons_typed_words_are_recorded_and_signed() {
    let (p, k, file) = laptop("pane-typed");
    let a = args(&["--choice", "typed", "--typed", "keep the old greeting"], &file);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(None, &a, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 0, "{out}{err}");
    let answered = p.facts_of("hold_answered");
    assert_eq!(answered.len(), 1, "{answered:?}");
    assert_eq!((answered[0]["choice"].clone(), answered[0]["typed"].clone()), (json!("typed"), json!("keep the old greeting")));
    let t = answered[0]["approval"].as_str().expect("an approval").to_string();
    let body = std::fs::read(p.path(&format!(".familiar/approvals/{t}.json"))).expect("json");
    let v: Value = serde_json::from_slice(&body).expect("payload");
    assert_eq!(v["typed"], json!("keep the old greeting"));
    let sig = SshSig::from_pem(std::fs::read(p.path(&format!(".familiar/approvals/{t}.sig"))).expect("sig")).expect("pem");
    k.public_key().verify(NAMESPACE, &body, &sig).expect("the typed words are signed");
}

#[test]
fn the_panes_blank_session_variable_is_the_persons_own_process() {
    // the pane cannot unset a variable for its child, only set one: it sets CLAUDE_CODE_SESSION_ID empty
    let (p, _, file) = laptop("pane-env");
    let a = args(&["--choice", "release"], &file);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(Some(""), &a, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(approvals(&p).len(), 2);
}

#[test]
fn with_no_key_it_says_to_run_the_setup_step_and_writes_nothing() {
    let (p, _, _) = laptop("pane-no-key");
    let missing = p.dir.join("no_such_key");
    let a = args(&["--choice", "release"], &missing);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(None, &a, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 1, "{err}");
    assert_eq!(
        err,
        format!(
            "familiar-ai: not approved · no approval key at {}: make one in the setup step (/familiar-ai:setup), or answer unsigned (recorded, but CI reads it as approval not verified)\n",
            missing.display()
        )
    );
    assert!(approvals(&p).is_empty());
    assert!(p.facts_of("hold_answered").is_empty());
}

#[test]
fn a_wrong_passphrase_says_it_did_not_unlock_the_key_and_writes_nothing() {
    let (p, _, file) = laptop("pane-wrong");
    let a = args(&["--choice", "release"], &file);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(None, &a, "hunter2\n");
    assert_eq!(code, 1, "{err}");
    assert_eq!(err, format!("familiar-ai: not approved · the passphrase does not open the key at {}\n", file.display()));
    assert!(approvals(&p).is_empty());
    assert!(p.facts_of("hold_answered").is_empty());
}

#[test]
fn unsigned_records_the_answer_with_no_approval_and_needs_no_key() {
    let (p, _, _) = laptop("pane-unsigned");
    let missing = p.dir.join("no_such_key");
    let a = args(&["--choice", "release", "--unsigned"], &missing);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(None, &a, "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("answered unsigned: CI reads it as approval not verified"), "{out}");
    let answered = p.facts_of("hold_answered");
    assert_eq!(answered.len(), 1, "{answered:?}");
    assert_eq!(answered[0]["choice"], json!("release"));
    assert!(answered[0].get("approval").is_none(), "{answered:?}");
    assert!(approvals(&p).is_empty());
}

#[test]
fn unsigned_is_refused_from_an_agents_session_too() {
    let (p, _, file) = laptop("pane-unsigned-agent");
    let a = args(&["--choice", "release", "--unsigned"], &file);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(Some(SESSION), &a, "");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("familiar-ai approve is the person's"), "{err}");
    assert!(p.facts_of("hold_answered").is_empty());
}

// ---- the same answer as the picker's: a miscoped card's covered_by_dependency ----

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TOUCHING: &str = "tests/test_login_old.py";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn test_of(c: &str) -> String {
    format!("tests/test_{c}.py")
}

fn place_of(c: &str) -> String {
    if c == "c1" { "src/login.py".to_string() } else { format!("src/dep_{c}.py") }
}

fn hash_of(c: &str) -> String {
    if c == "c1" { "S1".to_string() } else { format!("H{c}") }
}

fn commit(log: &Log, id: &str, files: &[String], tests: bool, code: bool, overlap: &[&str]) -> Log {
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    for c in plan::criteria(&View::new(&log)) {
        let seen = log.iter().any(|f| f.kind == "tests_observed" && f.criterion_id() == Some(c.as_str()));
        if seen || files.contains(&test_of(&c)) {
            log = raw(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash_of(&c) }));
        }
    }
    log
}

fn tests_commit(log: &Log, id: &str, c: &str) -> Log {
    let log = commit(log, id, &[test_of(c)], true, false, &[]);
    raw(&log, "test_written", json!({ "criterion_id": c, "file_path": test_of(c), "test_name": "t1" }))
}

fn code_commit(log: &Log, id: &str, c: &str) -> Log {
    commit(log, id, &[place_of(c)], false, true, &[c])
}

fn run(log: &Log, c: &str, id: &str, label: &str) -> Log {
    let word = if label == "passed" { "passed" } else { "failed" };
    let rid = format!("r-{c}-{id}-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": c, "criterion_rev": "R", "commit": id, "command": "python -m pytest", "resources": ["tests"], "wait": "60", "by": "binary" }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": word, "end": "exited" }));
    log = raw(&log, "test_log_translation", json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" }));
    log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": test_of(c), "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    if c == "c1" {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": TOUCHING, "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    }
    raw(&log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": hash_of(c), "ran_in": "laptop" }))
}

fn start_check(log: &Log, c: &str, lines: &str) -> Log {
    let (symbol, from, to) = if c == "c1" { ("check_password".to_string(), 40, 62) } else { (format!("fn_{c}"), 1, 9) };
    raw(log, "anchor_sha_matched", json!({ "criterion_id": c, "file_path": place_of(c), "symbol": symbol, "line_start": from, "line_end": to, "sha": "base", "lines_hash": lines, "matched": true, "checked_at": "start" }))
}

/// c1 crashed; the person's a_dependency drafted c3, red then green; c1 passed after: miscoped.
fn miscoped() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = start_check(&log, "c1", "L0");
    let log = tests_commit(&log, "k1", "c1");
    let log = run(&log, "c1", "k1", "crashed");
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    let t2 = log.last().map(|f| f.t).unwrap_or(0);
    let mut log = raw(&log, "dependency_drafted", json!({ "approval": t2, "criterion_id": "c3", "text": "c3: what c1 needs", "test_file": test_of("c3") }));
    log = raw(&log, "criterion", json!({ "criterion_id": "c3", "text": "c3: what c1 needs", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = raw(&log, "place_chosen", json!({ "criterion_id": "c3", "places": [{ "file_path": place_of("c3"), "symbol": "fn_c3", "line_start": 1, "line_end": 9 }] }));
    log = raw(&log, "anchor_sha_matched", json!({ "criterion_id": "c3", "file_path": place_of("c3"), "symbol": "fn_c3", "line_start": 1, "line_end": 9, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c3", "file_path": test_of("c3"), "will": "create" }));
    for (kind, fields) in restring(&View::new(&log), "c3", "c1") {
        log = raw(&log, kind, Value::Object(fields));
    }
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c3", "agent_conversation_id": "a1" }));
    log = raw(&log, "fix_location_category", json!({ "criterion_id": "c3", "category": "this_repository", "where": "" }));
    let log = start_check(&log, "c3", "L0");
    let log = tests_commit(&log, "k2", "c3");
    let log = run(&log, "c3", "k2", "tests_failed");
    let log = code_commit(&log, "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    run(&log, "c1", "k3", "passed")
}

#[test]
fn approving_covered_by_dependency_closes_the_part_as_the_picker_does() {
    let log = miscoped();
    let follow = familiar_ai::door::approval::follow_ups(&log, "c1", "miscoped", "covered_by_dependency");
    assert_eq!(follow, vec![("criterion_covered".to_string(), json!({"criterion_id": "c1", "by_criterion": "c3"}))]);
    assert!(familiar_ai::door::approval::follow_ups(&log, "c1", "miscoped", "re_plan").is_empty());
    assert!(familiar_ai::door::approval::follow_ups(&log, "c1", "two_failures", "release").is_empty());
}
