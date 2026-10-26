//! CI trusts a card's answer only when a witness the agent cannot write
//! vouches for it (plan §6.9 path B): the person signs with an encrypted
//! ed25519 key, `familiar-ai approve` reading the passphrase on stdin only, and
//! `verify` checks each `.familiar/approvals/<t>.json` + `.sig` committed at
//! HEAD against the allowed signers in CI's own configuration
//! (`FAMILIAR_AI_ALLOWED_SIGNERS`), never a file in the tree. A part answered
//! with no verified approval is "approval not verified" and not proven.

mod common;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, SshSig};

const NAMESPACE: &str = "familiar-ai-approval";
const SIGNERS: &str = "FAMILIAR_AI_ALLOWED_SIGNERS";
const PASSPHRASE: &str = "correct horse battery staple";
const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

fn key() -> PrivateKey {
    PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("key")
}

/// One allowed_signers line for the key, as `ssh-keygen -Y verify` reads it.
fn allowed(k: &PrivateKey) -> String {
    format!("person@example.com {}\n", k.public_key().to_openssh().expect("public"))
}

fn sign(k: &PrivateKey, bytes: &[u8]) -> String {
    k.sign(NAMESPACE, HashAlg::Sha512, bytes).expect("sign").to_pem(LineEnding::LF).expect("pem")
}

/// The payload `approve` signs, as the bytes of its file.
fn payload(repo: &str, c: &str, hold: &str, choice: &str, commit: &str, t: u64) -> Vec<u8> {
    let v = json!({"repo": repo, "criterion_id": c, "hold": hold, "choice": choice, "run_id": "", "commit": commit, "t": t});
    let mut b = serde_json::to_vec(&v).expect("json");
    b.push(b'\n');
    b
}

/// `src/app.sh`, the record ignored, no setup, with CI's allowed signers when given.
fn project(name: &str, signers: Option<&str>) -> Project {
    let b = Project::builder(name).no_setup().file(".gitignore", ".familiar/record\n.familiar/verify-report.json\n");
    match signers {
        Some(s) => b.env(SIGNERS, s).create(),
        None => b.env_remove(SIGNERS).create(),
    }
}

impl Project {
    fn base(&self) -> String {
        self.git(&["rev-list", "--max-parents=0", "HEAD"])
    }

    fn plan(&self, claims: &[(&str, Value)]) {
        let claims: Vec<Claim> = claims.iter().map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).expect("claim")).collect();
        familiar_ai_plan::write(&self.path(FAMILIAR_FOLDER).join(PLAN_FILE), &claims).expect("plan");
    }

    /// c1, whose test never fails at its tests-first commit: not proven
    /// unless the person released it. Returns the fix's commit.
    fn never_red(&self) -> String {
        let sha = self.base();
        self.plan(&[
            ("intent_submitted", json!({"text": "greet with hello", "sha": sha})),
            ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
            ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
            ("criterion", json!({"criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
            ("place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3}]})),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
            ("test_stack_detected", json!({"codebase_id": "fx", "project_root": "src/", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"})),
            ("criterion_head", json!({"criterion_id": "c1"})),
            ("criterion_assigned", json!({"criterion_id": "c1", "agent_conversation_id": "s1"})),
            ("fix_location_category", json!({"criterion_id": "c1", "category": "this_repository", "where": ""})),
            ("test_written", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": ""})),
        ]);
        self.write("tests/test_c1.sh", "grep -q greet src/app.sh\n");
        self.land("c1: tests first");
        self.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
        self.land("c1: fix")
    }

    /// Commit `<t>.json` and, when given, `<t>.sig` beside the plan.
    fn approval(&self, t: u64, json: &[u8], sig: Option<&str>) {
        let dir = self.path(FAMILIAR_FOLDER).join("approvals");
        std::fs::create_dir_all(&dir).expect("approvals");
        std::fs::write(dir.join(format!("{t}.json")), json).expect("json");
        if let Some(s) = sig {
            std::fs::write(dir.join(format!("{t}.sig")), s).expect("sig");
        }
        self.land("the person's approval");
    }

    fn verify(&self) -> (i32, String) {
        let (code, out, err) = self.run_as(None, &["verify"], "");
        (code, format!("{out}{err}"))
    }
}

const RELEASED: &str = "familiar-ai: \"greet says hello\" · released by you";
const NOT_VERIFIED: &str = "familiar-ai: \"greet says hello\" · not proven: approval not verified";

#[test]
fn a_valid_signature_from_an_allowed_signer_is_the_persons_answer() {
    let k = key();
    let p = project("witness-valid", Some(&allowed(&k)));
    let fix = p.never_red();
    let (code, out) = p.verify();
    assert_eq!(code, 1, "without the answer it is not proven: {out}");
    let body = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    let (code, out) = p.verify();
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(RELEASED), "{out}");
}

#[test]
fn a_signature_for_another_card_is_not_verified() {
    let k = key();
    let p = project("witness-other-card", Some(&allowed(&k)));
    let fix = p.never_red();
    let signed = payload(&p.base(), "c2", "two_tries_not_red", "release", &fix, 7);
    let shown = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &shown, Some(&sign(&k, &signed)));
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
    assert!(!out.contains(RELEASED), "{out}");
}

#[test]
fn a_signer_not_in_cis_allowed_list_is_not_verified() {
    let (k, other) = (key(), key());
    let p = project("witness-not-allowed", Some(&allowed(&other)));
    let fix = p.never_red();
    let body = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
}

#[test]
fn an_allowed_signers_file_in_the_tree_is_not_cis_configuration() {
    let k = key();
    let p = project("witness-in-tree", None);
    let fix = p.never_red();
    p.write(".github/allowed_signers", &allowed(&k));
    p.write(".familiar/allowed_signers", &allowed(&k));
    let body = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
}

#[test]
fn no_witness_is_approval_not_verified_and_fails_closed() {
    let k = key();
    let p = project("witness-none", Some(&allowed(&k)));
    let fix = p.never_red();
    p.approval(7, &payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7), None);
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
}

#[test]
fn an_approval_bound_to_another_repository_or_namespace_is_not_verified() {
    let k = key();
    let p = project("witness-repo", Some(&allowed(&k)));
    let fix = p.never_red();
    let body = payload("0000000000000000000000000000000000000000", "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "another repository: {out}");

    let q = project("witness-namespace", Some(&allowed(&k)));
    let fix = q.never_red();
    let body = payload(&q.base(), "c1", "two_tries_not_red", "release", &fix, 8);
    let sig = k.sign("git", HashAlg::Sha512, &body).expect("sign").to_pem(LineEnding::LF).expect("pem");
    q.approval(8, &body, Some(&sig));
    let (code, out) = q.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "another namespace: {out}");
}

#[test]
fn a_forged_hold_answered_in_the_plan_is_never_the_persons_answer() {
    let k = key();
    let p = project("witness-forged", Some(&allowed(&k)));
    p.never_red();
    let path = p.path(FAMILIAR_FOLDER).join(PLAN_FILE);
    let mut text = std::fs::read_to_string(&path).expect("plan");
    text.push_str("{\"kind\":\"hold_answered\",\"criterion_id\":\"c1\",\"hold\":\"two_tries_not_red\",\"choice\":\"release\",\"typed\":\"\",\"approval\":\"7\"}\n");
    std::fs::write(&path, text).expect("write");
    p.land("a release by hand");
    let (code, out) = p.verify();
    assert_ne!(code, 0, "{out}");
    assert!(!out.contains(RELEASED), "{out}");
}

// ---- familiar-ai approve, on the person's laptop ----

/// A set-up project with c1 planned, and the person's encrypted key in its home.
fn laptop(name: &str) -> (Project, PrivateKey, std::path::PathBuf) {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    let k = key();
    let file = p.dir.join("familiar_ai_approve");
    k.encrypt(&mut OsRng, PASSPHRASE).expect("encrypt").write_openssh_file(&file, LineEnding::LF).expect("key file");
    (p, k, file)
}

const ANSWER: [&str; 7] = ["approve", "--criterion_id", "c1", "--hold", "two_failures", "--choice", "release"];

fn approve_args(key: &std::path::Path) -> Vec<String> {
    let mut a: Vec<String> = ANSWER.iter().map(|s| s.to_string()).collect();
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
fn approve_signs_the_card_with_the_passphrase_from_stdin_and_records_the_answer() {
    let (p, k, file) = laptop("approve-signs");
    let args = approve_args(&file);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(None, &args, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 0, "{out}{err}");
    let names = approvals(&p);
    assert_eq!(names.len(), 2, "{names:?}");
    let t = names[0].strip_suffix(".json").expect("json first");
    assert_eq!(names[1], format!("{t}.sig"));
    let body = std::fs::read(p.path(&format!(".familiar/approvals/{t}.json"))).expect("json");
    let v: Value = serde_json::from_slice(&body).expect("payload");
    assert_eq!(v["repo"], json!(p.git(&["rev-list", "--max-parents=0", "HEAD"])));
    assert_eq!((v["criterion_id"].clone(), v["hold"].clone(), v["choice"].clone(), v["run_id"].clone()), (json!("c1"), json!("two_failures"), json!("release"), json!("")));
    assert_eq!(v["commit"], json!(p.head()));
    assert_eq!(v["t"].to_string(), t);
    let sig = SshSig::from_pem(std::fs::read(p.path(&format!(".familiar/approvals/{t}.sig"))).expect("sig")).expect("pem");
    k.public_key().verify(NAMESPACE, &body, &sig).expect("the person's key signed these bytes");
    let answered = p.facts_of("hold_answered");
    assert_eq!(answered.len(), 1, "{answered:?}");
    assert_eq!(answered[0]["approval"], json!(t));
    assert_eq!(answered[0]["commit"], json!(p.head()));
    assert_eq!((answered[0]["choice"].clone(), answered[0]["run_id"].clone()), (json!("release"), json!("")));
}

#[test]
fn a_wrong_passphrase_signs_nothing_and_records_nothing() {
    let (p, _, file) = laptop("approve-wrong");
    let args = approve_args(&file);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(None, &args, "not the passphrase\n");
    assert_eq!(code, 1, "{err}");
    assert!(err.starts_with("familiar-ai: not approved · "), "{err}");
    assert!(approvals(&p).is_empty());
    assert!(p.facts_of("hold_answered").is_empty());
}

#[test]
fn approve_is_refused_from_an_agents_session() {
    let (p, _, file) = laptop("approve-agent");
    let args = approve_args(&file);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(Some(SESSION), &args, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("familiar-ai approve is the person's"), "{err}");
    assert!(approvals(&p).is_empty());
    assert!(p.facts_of("hold_answered").is_empty());
}

#[test]
fn the_passphrase_is_never_taken_from_the_command_line() {
    let (p, _, file) = laptop("approve-argv");
    let mut args = approve_args(&file);
    args.extend(["--passphrase".to_string(), PASSPHRASE.to_string()]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, _, _) = p.run_as(None, &args, "");
    assert_eq!(code, 2, "no such option: a usage error");
    assert!(approvals(&p).is_empty());
    // nor from an empty stdin with the passphrase in the environment
    p.set_env("FAMILIAR_AI_PASSPHRASE", Some(PASSPHRASE));
    let args = approve_args(&file);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(None, &args, "");
    assert_eq!(code, 1, "{err}");
    assert!(approvals(&p).is_empty());
    assert!(p.facts_of("hold_answered").is_empty());
}

#[test]
fn a_key_with_no_passphrase_is_no_presence_and_is_refused() {
    let (p, k, _) = laptop("approve-bare");
    let bare = p.dir.join("bare_key");
    k.write_openssh_file(&bare, LineEnding::LF).expect("bare key");
    let args = approve_args(&bare);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, _, err) = p.run_as(None, &args, "\n");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("has no passphrase"), "{err}");
    assert!(approvals(&p).is_empty());
}

#[test]
fn what_approve_writes_is_what_verify_accepts() {
    let (p, k, file) = laptop("approve-then-verify");
    // the work: tests first, never red, the fix
    p.write("tests/test_c1.sh", "grep -q greet src/app.sh\n");
    p.land("c1: tests first");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.land("c1: fix");
    let args = approve_args(&file);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(None, &args, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 0, "{out}{err}");
    p.land("the approval");
    // CI: a fresh clone, no record, the allowed signer from its configuration
    let clone = p.dir.join("clone");
    p.git(&["clone", "-q", p.repo().to_str().expect("path"), clone.to_str().expect("path")]);
    p.set_env(SIGNERS, Some(&allowed(&k)));
    let (code, out, err) = p.run_in(&clone, &["verify"], "");
    assert!(out.contains(RELEASED) || out.contains("· proven ·"), "{out}{err}");
    assert!(!out.contains("approval not verified"), "{out}{err}");
    let _ = code;
}

/// H-4: c1 at its tests-first commit, its laptop run `r-1` read can't-tell ("unclassified",
/// deciding line "boom").
fn cant_tell_on_the_laptop() -> familiar_ai_facts::Log {
    cant_tell_with("boom")
}

/// The same, its deciding line `evidence`.
fn cant_tell_with(evidence: &str) -> familiar_ai_facts::Log {
    use familiar_ai_facts::Log;
    use familiar_ai_rules::sample::{apply_move, criterion_fixture};
    use familiar_ai_rules::View;
    let step = |log: &Log, label: &str| -> Log {
        let v = View::new(log);
        let m = familiar_ai_rules::moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label} move"));
        apply_move(log, &m, 0.0, "t").expect("applied")
    };
    let mut log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    for s in ["start_check", "tests_only", "test_written"] {
        log = step(&log, s);
    }
    let log = ran(&log, "r-1", "laptop", "unclassified", evidence);
    step(&log, "fingerprint_stamped")
}

/// CI's acceptance of an its_the_red bound on the laptop to `laptop`'s line, CI's own run
/// reading `ci`.
fn ci_accepts(laptop: &str, ci: &str) -> Option<String> {
    let log = cant_tell_with(laptop);
    let bound = familiar_ai::door::approval::confirm_binding(&log, "c1", "run_unclassified", "its_the_red").expect("offered").expect("bound");
    let p = confirming("its_the_red", &bound.label, &bound.evidence);
    familiar_ai::door::verify::ci_confirms(&ran(&log, "p-c1-1", "pipeline", "unclassified", ci), "c1", &p)
}

#[test]
fn the_deciding_line_is_compared_without_its_timings_and_copy_paths_and_never_without_its_test() {
    // the coordinator (2026-10-19): run-specific noise never refuses an honest confirmation
    let line = |t: &str| format!("FAILED tests/test_c1.py::test_login - KeyError: 'id' in {t}");
    assert_eq!(ci_accepts(&line("0.002s"), &line("0.010s")).as_deref(), Some("p-c1-1"), "a timing differs");
    assert_eq!(ci_accepts("ok (12ms)  at 12:01:02", "ok (7ms) at 12:03:04").as_deref(), Some("p-c1-1"), "a duration, a clock, spaces");
    assert_eq!(
        ci_accepts("/tmp/familiar-ai-run-123-1/tests/test_c1.py:12: AssertionError", "/var/tmp/familiar-ai-verify-9-2/tests/test_c1.py:12: AssertionError").as_deref(),
        Some("p-c1-1"),
        "the scratch copies' paths"
    );
    // the failing test differs: refused
    let other = "FAILED tests/test_c1.py::test_logout - KeyError: 'id' in 0.010s";
    assert_eq!(ci_accepts(&line("0.002s"), other), None, "another failing test");
    assert_eq!(ci_accepts("tests/test_c1.py:12: AssertionError", "tests/test_c1.py:13: AssertionError"), None, "another line of the test");
}

/// A run of c1 at the latest commit, by the binary (`laptop`) or by CI (`pipeline`).
fn ran(log: &familiar_ai_facts::Log, rid: &str, ran_in: &str, label: &str, evidence: &str) -> familiar_ai_facts::Log {
    let raw = |log: &familiar_ai_facts::Log, kind: &str, fields: Value| familiar_ai_facts::append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").expect(kind);
    let id = log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string();
    let by = if ran_in == "pipeline" { "pipeline" } else { "binary" };
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": [], "wait": "0", "by": by}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": "failed", "end": "exited"}));
    log = raw(&log, "test_log_translation", json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": evidence, "failing_tests": [], "classified_on": "full"}));
    for fp in ["tests/test_c1.py", "tests/test_login_old.py"] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": "failed", "ran_in": ran_in, "with_anchor_reverted": false, "run_id": rid}));
    }
    log
}

/// An approval's payload as `approve` signs it for a confirming choice.
fn confirming(choice: &str, label: &str, evidence: &str) -> familiar_ai::door::approval::Payload {
    serde_json::from_value(json!({
        "repo": "r", "criterion_id": "c1", "hold": "run_unclassified", "choice": choice, "typed": "", "run_id": "r-1",
        "commit": "k1", "t": 9, "label": label, "evidence": evidence,
    }))
    .expect("a payload")
}

#[test]
fn its_the_red_binds_the_cards_run_its_label_and_its_deciding_line() {
    // H-4 (the owner, 2026-10-19): the approval binds what the person saw
    use familiar_ai::door::approval::{confirm_binding, Confirmed};
    let log = cant_tell_on_the_laptop();
    let bound = confirm_binding(&log, "c1", "run_unclassified", "its_the_red").expect("offered");
    assert_eq!(bound, Some(Confirmed { run_id: "r-1".into(), label: "unclassified".into(), evidence: "boom".into() }));
    assert!(confirm_binding(&log, "c1", "run_unclassified", "its_the_pass").is_err(), "not offered before the seal");
    assert_eq!(confirm_binding(&log, "c1", "run_unclassified", "try_again"), Ok(None), "another choice binds nothing more");
    // the signed body carries them for a confirming choice only
    let card = familiar_ai::door::approval::Card { criterion_id: "c1".into(), hold: "run_unclassified".into(), choice: "its_the_red".into(), run_id: String::new(), typed: String::new() };
    let body = familiar_ai::door::approval::body("r", &card, "k1", 9, bound.as_ref());
    assert_eq!((body["run_id"].as_str(), body["label"].as_str(), body["evidence"].as_str()), (Some("r-1"), Some("unclassified"), Some("boom")), "{body}");
    let plain = familiar_ai::door::approval::body("r", &familiar_ai::door::approval::Card { choice: "try_again".into(), ..card }, "k1", 9, None);
    assert!(plain.get("label").is_none() && plain.get("evidence").is_none(), "{plain}");
}

#[test]
fn ci_accepts_a_confirmation_only_where_its_own_run_reads_cant_tell_with_the_same_label_and_deciding_line() {
    use familiar_ai::door::verify::ci_confirms;
    let laptop = cant_tell_on_the_laptop();
    // CI's own run at the tests-first commit reads the same: accepted, bound to CI's run
    let same = ran(&laptop, "p-c1-1", "pipeline", "unclassified", "boom");
    assert_eq!(ci_confirms(&same, "c1", &confirming("its_the_red", "unclassified", "boom")).as_deref(), Some("p-c1-1"));
    // another deciding line, another label, or a reading CI can tell: refused
    let other = ran(&laptop, "p-c1-1", "pipeline", "unclassified", "bang");
    assert_eq!(ci_confirms(&other, "c1", &confirming("its_the_red", "unclassified", "boom")), None);
    let told = ran(&laptop, "p-c1-1", "pipeline", "tests_failed", "boom");
    assert_eq!(ci_confirms(&told, "c1", &confirming("its_the_red", "tests_failed", "boom")), None, "CI could tell");
    // a pass is never read at the tests-first commit, and the laptop's own run is not CI's
    assert_eq!(ci_confirms(&same, "c1", &confirming("its_the_pass", "unclassified", "boom")), None);
    assert_eq!(ci_confirms(&laptop, "c1", &confirming("its_the_red", "unclassified", "boom")), None);
}
