//! The allowed signers are read as `ssh-keygen -Y verify` reads them, and no
//! approval file is passed over unreported (lane s9dw's review, findings 4
//! and 5): a `cert-authority` line, a key outside its `valid-after` /
//! `valid-before` window, or one whose `namespaces=` leaves out
//! `familiar-ai-approval`, signs no approval; a file in the approvals folder
//! that names no criterion is "approval not verified" for every part, since
//! it could be anyone's.

mod common;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey};

const NAMESPACE: &str = "familiar-ai-approval";
const SIGNERS: &str = "FAMILIAR_AI_ALLOWED_SIGNERS";

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

fn keys(line: &str) -> usize {
    familiar_ai::door::approval::allowed_keys(line).len()
}

#[test]
fn the_allowed_signers_options_are_read_as_ssh_keygen_reads_them() {
    let k = key();
    let pk = k.public_key().to_openssh().expect("public");
    assert_eq!(keys(&format!("person@example.com {pk}")), 1, "a plain line");
    assert_eq!(keys(&format!("person@example.com namespaces=\"git,familiar-ai-approval\" {pk}")), 1, "its namespace listed");
    assert_eq!(keys(&format!("person@example.com namespaces=\"familiar-*\" {pk}")), 1, "a pattern that matches");
    assert_eq!(keys(&format!("person@example.com valid-after=20000101,valid-before=29991231 {pk}")), 1, "within its window");
    assert_eq!(keys(&format!("person@example.com namespaces=\"git\" {pk}")), 0, "only git's");
    assert_eq!(keys(&format!("person@example.com cert-authority {pk}")), 0, "a certificate authority signs certificates, not approvals");
    assert_eq!(keys(&format!("person@example.com cert-authority,namespaces=\"familiar-ai-approval\" {pk}")), 0, "still a certificate authority");
    assert_eq!(keys(&format!("person@example.com valid-before=20000101 {pk}")), 0, "expired");
    assert_eq!(keys(&format!("person@example.com valid-after=29990101Z {pk}")), 0, "not yet valid");
    assert_eq!(keys(&format!("person@example.com valid-before=garbage {pk}")), 0, "an option it cannot read fails closed");
}

#[test]
fn an_expired_signer_signs_no_approval() {
    let k = key();
    let pk = k.public_key().to_openssh().expect("public");
    let p = project("signers-expired", Some(&format!("person@example.com valid-before=20000101 {pk}\n")));
    let fix = p.never_red();
    let body = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
}

#[test]
fn a_file_in_the_approvals_naming_no_criterion_is_reported_against_every_part() {
    let k = key();
    let p = project("junk-no-criterion", Some(&allowed(&k)));
    let fix = p.never_red();
    let body = payload(&p.base(), "c1", "two_tries_not_red", "release", &fix, 7);
    p.approval(7, &body, Some(&sign(&k, &body)));
    assert!(p.verify().1.contains(RELEASED), "the signed answer alone");
    p.approval(8, b"{\"nothing\": 1}\n", None);
    let (code, out) = p.verify();
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(NOT_VERIFIED), "{out}");
    assert!(out.contains("approvals/8.json"), "the file is named: {out}");
}
