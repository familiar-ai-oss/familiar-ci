//! Unsigned answers flagged (the owner, 2026-10-17) on a verified approval
//! (2026-10-18, "The local record is advisory; CI is the wall"): a cleared
//! hold reads "answered, verified <t>" only when its `hold_answered` names a
//! `.familiar/approvals/<t>.json` committed at HEAD whose `.sig` verifies, by
//! the check CI uses, against the allowed signers available locally
//! (`FAMILIAR_AI_ALLOWED_SIGNERS` in the person's environment, never a file in
//! the tree). Anything else reads "answered, not verified": a forged
//! signature, a wrong signer, a wrong repository, no signature, an answer
//! picked through AskUserQuestion or `--unsigned`, and every answer while no
//! signers are configured. `status`, `status --json` (each criterion's
//! `answers`) and the status line show it.

mod common;

use common::Project;
use serde_json::Value;
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey};

const NAMESPACE: &str = "familiar-ai-approval";
const SIGNERS: &str = "FAMILIAR_AI_ALLOWED_SIGNERS";
const PASSPHRASE: &str = "correct horse battery staple";
const NOT_VERIFIED: &str = "answered, not verified";

fn key() -> PrivateKey {
    PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("key")
}

fn allowed(k: &PrivateKey) -> String {
    format!("person@example.com {}\n", k.public_key().to_openssh().expect("public"))
}

fn sign(k: &PrivateKey, bytes: &[u8]) -> String {
    k.sign(NAMESPACE, HashAlg::Sha512, bytes).expect("sign").to_pem(LineEnding::LF).expect("pem")
}

/// A set-up project with c1 planned, run from the person's terminal; the
/// person's key `k` encrypted in its folder; `signers` in its environment.
fn laptop(name: &str, k: &PrivateKey, signers: Option<&str>) -> (Project, std::path::PathBuf) {
    let b = Project::builder(name).no_session().env("FAMILIAR_AI_BENCH", "0");
    let p = match signers {
        Some(s) => b.env(SIGNERS, s).create(),
        None => b.env_remove(SIGNERS).create(),
    };
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", "s1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    let file = p.dir.join("familiar_ai_approve");
    k.encrypt(&mut OsRng, PASSPHRASE).expect("encrypt").write_openssh_file(&file, LineEnding::LF).expect("key file");
    (p, file)
}

const ANSWER: [&str; 6] = ["--criterion_id", "c1", "--hold", "two_failures", "--choice", "release"];

/// `familiar-ai approve` with the passphrase on stdin; returns the approval's t.
fn approve(p: &Project, file: &std::path::Path) -> String {
    let mut args: Vec<String> = vec!["approve".into()];
    args.extend(ANSWER.iter().map(|s| s.to_string()));
    args.extend(["--key".to_string(), file.display().to_string()]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = p.run_as(None, &args, &format!("{PASSPHRASE}\n"));
    assert_eq!(code, 0, "{out}{err}");
    let answers = p.criterion_json("c1")["answers"].clone();
    answers[0]["approval"].as_str().unwrap_or_else(|| panic!("an approval named: {answers}")).to_string()
}

fn approvals(p: &Project, t: &str, ext: &str) -> std::path::PathBuf {
    p.path(&format!(".familiar/approvals/{t}.{ext}"))
}

/// c1's one answer in `status --json`.
fn the_answer(p: &Project) -> Value {
    let answers = p.criterion_json("c1")["answers"].clone();
    assert_eq!(answers.as_array().map(Vec::len), Some(1), "{answers}");
    answers[0].clone()
}

fn reads_not_verified(p: &Project, why: &str) {
    let a = the_answer(p);
    assert_eq!(a["verified"], Value::Bool(false), "{why}: {a}");
    assert_eq!(a["words"], Value::from(NOT_VERIFIED), "{why}: {a}");
    let status = p.status();
    assert!(status.contains(NOT_VERIFIED), "{why}: {status}");
    let (_, line, _) = p.run_as(None, &["statusline"], "");
    assert!(line.contains("1 answered, not verified"), "{why}: {line}");
}

#[test]
fn a_committed_approval_signed_by_a_local_signer_reads_verified() {
    let k = key();
    let (p, file) = laptop("flag-verified", &k, Some(&allowed(&k)));
    let t = approve(&p, &file);
    p.land("the person's approval");
    let a = the_answer(&p);
    assert_eq!((a["hold"].clone(), a["choice"].clone()), (Value::from("two_failures"), Value::from("release")), "{a}");
    assert_eq!(a["verified"], Value::Bool(true), "{a}");
    assert_eq!(a["words"], Value::from(format!("answered, verified {t}")), "{a}");
    let status = p.status();
    assert!(!status.contains(NOT_VERIFIED), "{status}");
    let (_, line, _) = p.run_as(None, &["statusline"], "");
    assert!(!line.contains("not verified"), "{line}");
}

#[test]
fn a_forged_signature_reads_not_verified() {
    let k = key();
    let (p, file) = laptop("flag-forged", &k, Some(&allowed(&k)));
    let t = approve(&p, &file);
    let sig = std::fs::read_to_string(approvals(&p, &t, "sig")).expect("sig");
    // the same signature's armour with its body's bytes changed
    let lines: Vec<&str> = sig.lines().collect();
    let mut forged: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mid = forged.len() / 2;
    forged[mid] = forged[mid].chars().rev().collect();
    std::fs::write(approvals(&p, &t, "sig"), forged.join("\n") + "\n").expect("forge");
    p.land("a forged approval");
    reads_not_verified(&p, "forged");
}

#[test]
fn a_signer_not_allowed_locally_reads_not_verified() {
    let (k, other) = (key(), key());
    let (p, file) = laptop("flag-wrong-signer", &k, Some(&allowed(&other)));
    approve(&p, &file);
    p.land("the approval");
    reads_not_verified(&p, "wrong signer");
}

#[test]
fn an_approval_signed_for_another_repository_reads_not_verified() {
    let k = key();
    let (p, file) = laptop("flag-wrong-repo", &k, Some(&allowed(&k)));
    let t = approve(&p, &file);
    let body = std::fs::read_to_string(approvals(&p, &t, "json")).expect("json");
    let mut v: Value = serde_json::from_str(&body).expect("payload");
    v["repo"] = Value::from("0000000000000000000000000000000000000000");
    let mut moved = serde_json::to_vec(&v).expect("json");
    moved.push(b'\n');
    std::fs::write(approvals(&p, &t, "json"), &moved).expect("write");
    std::fs::write(approvals(&p, &t, "sig"), sign(&k, &moved)).expect("sig");
    p.land("an approval for another repository");
    reads_not_verified(&p, "wrong repo");
}

#[test]
fn an_approval_with_no_signature_reads_not_verified() {
    let k = key();
    let (p, file) = laptop("flag-no-sig", &k, Some(&allowed(&k)));
    let t = approve(&p, &file);
    std::fs::remove_file(approvals(&p, &t, "sig")).expect("remove");
    p.land("an approval with no signature");
    reads_not_verified(&p, "no signature");
}

#[test]
fn an_unsigned_answer_reads_not_verified() {
    let k = key();
    let (p, _) = laptop("flag-unsigned", &k, Some(&allowed(&k)));
    let mut args = vec!["approve"];
    args.extend(ANSWER);
    args.push("--unsigned");
    let (code, out, err) = p.run_as(None, &args, "");
    assert_eq!(code, 0, "{out}{err}");
    p.land("an unsigned answer");
    let a = the_answer(&p);
    assert_eq!(a["approval"], Value::Null, "{a}");
    reads_not_verified(&p, "unsigned");
}

#[test]
fn an_answer_picked_through_ask_user_question_reads_not_verified() {
    let k = key();
    let (p, _) = laptop("flag-picked", &k, Some(&allowed(&k)));
    // the picker records the pick as this claim, with no approval
    let mut args = vec!["hold_answered"];
    args.extend(ANSWER);
    args.extend(["--typed", ""]);
    p.claim(&args);
    reads_not_verified(&p, "picked");
}

#[test]
fn with_no_local_signers_nothing_reads_verified() {
    let k = key();
    let (p, file) = laptop("flag-no-signers", &k, None);
    approve(&p, &file);
    p.land("the person's approval");
    reads_not_verified(&p, "no signers");
}
