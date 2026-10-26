//! CI's half of S9d (plan §6.5, §6.9, §7.1 "On CI"; lane S9d-ci), through `verify` in a fresh
//! clone:
//!
//! - every verify run is reserved, ended and translated, and each result names its run;
//! - c_dep: c1's run crashed at its tests-first commit T (k1), the person answered
//!   `a_dependency` (a signed approval, the only witness CI reads), c3 was planned, red at k2 and
//!   green at k3 (G), where the laptop stamped c1 again; CI moves c1's tests-first to that
//!   mirrored stamp's commit when its own runs show the crash at T and c3 green at k3, c1's tests
//!   there are its proof, and nothing from T to it touched c1's place;
//! - the fences: a mirrored `place_chosen {hold, choice}` without a witness is a plain re-anchor;
//!   a pipeline stamp in the plan is not the pipeline's own; CI does not run a command the agent
//!   changed after the person's `fix_what_broke_it`.

mod common;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey};

const NAMESPACE: &str = "familiar-ai-approval";
const SIGNERS: &str = "FAMILIAR_AI_ALLOWED_SIGNERS";

/// The greeting, wrong until c1's fix.
const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";
const APP_FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
/// The helper the greeting needs, wrong until c3's fix.
const LIB: &str = "helper() { echo no; }\n";
const LIB_FIXED: &str = "helper() { echo ok; }\n";
/// c1's test crashes (exit 139, a segfault's code) while the helper it needs is missing.
const C1_TEST: &str = "[ \"$(. ./src/lib.sh; helper)\" = ok ] || exit 139\nsh src/app.sh | grep -qx hello\n";
const C3_TEST: &str = ". ./src/lib.sh\nhelper | grep -qx ok\n";

fn key() -> PrivateKey {
    PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("key")
}

fn allowed(k: &PrivateKey) -> String {
    format!("person@example.com {}\n", k.public_key().to_openssh().expect("public"))
}

fn sign(k: &PrivateKey, bytes: &[u8]) -> String {
    k.sign(NAMESPACE, HashAlg::Sha512, bytes).expect("sign").to_pem(LineEnding::LF).expect("pem")
}

fn payload(repo: &str, c: &str, hold: &str, choice: &str, commit: &str, t: u64) -> Vec<u8> {
    let v = json!({"repo": repo, "criterion_id": c, "hold": hold, "choice": choice, "run_id": "", "commit": commit, "t": t});
    let mut b = serde_json::to_vec(&v).expect("json");
    b.push(b'\n');
    b
}

fn project(name: &str, k: &PrivateKey) -> Project {
    Project::builder(name).no_setup().file(".gitignore", ".familiar/record\n.familiar/verify-report.json\n").env(SIGNERS, allowed(k)).create()
}

fn claims(lines: &[(&str, Value)]) -> Vec<Claim> {
    lines.iter().map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).expect("claim")).collect()
}

fn part(c: &str, text: &str, place: &str, symbol: &str, line_end: u64) -> Vec<(&'static str, Value)> {
    let test = format!("tests/test_{c}.sh");
    vec![
        ("criterion", json!({"criterion_id": c, "text": text, "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
        ("place_chosen", json!({"criterion_id": c, "places": [{"file_path": place, "symbol": symbol, "line_start": 1, "line_end": line_end}]})),
        ("test_planned", json!({"criterion_id": c, "file_path": test, "will": "create", "test_command": format!("sh {test}")})),
        ("criterion_head", json!({"criterion_id": c})),
        ("criterion_assigned", json!({"criterion_id": c, "agent_conversation_id": "s1"})),
        ("fix_location_category", json!({"criterion_id": c, "category": "this_repository", "where": ""})),
        ("test_written", json!({"criterion_id": c, "file_path": test, "test_name": ""})),
    ]
}

impl Project {
    fn base(&self) -> String {
        self.git(&["rev-list", "--max-parents=0", "HEAD"])
    }

    fn write_plan(&self, lines: &[(&str, Value)]) {
        familiar_ai_plan::write(&self.path(FAMILIAR_FOLDER).join(PLAN_FILE), &claims(lines)).expect("plan");
    }

    /// The fingerprint the binary observes of `c`'s test at `commit`.
    fn hash(&self, c: &str, commit: &str) -> String {
        let asked = vec![(c.to_string(), vec![format!("tests/test_{c}.sh")])];
        let seen = familiar_ai_git::observe_tests(&self.repo(), commit, &asked).expect("observe");
        seen[0].1.clone().expect("a fingerprint")
    }

    fn approve(&self, k: &PrivateKey, c: &str, hold: &str, choice: &str, commit: &str, t: u64) {
        let body = payload(&self.base(), c, hold, choice, commit, t);
        let dir = self.path(FAMILIAR_FOLDER).join("approvals");
        std::fs::create_dir_all(&dir).expect("approvals");
        std::fs::write(dir.join(format!("{t}.json")), &body).expect("json");
        std::fs::write(dir.join(format!("{t}.sig")), sign(k, &body)).expect("sig");
    }

    /// `verify` in a fresh clone: its exit code, what it said, and the record it wrote.
    fn verify_in_clone(&self, name: &str) -> (i32, String, Vec<familiar_ai_facts::Fact>) {
        let clone = self.dir.join(format!("clone-{name}"));
        let _ = std::fs::remove_dir_all(&clone);
        self.git_in(&self.dir, &["clone", "-q", self.repo().to_str().expect("path"), clone.to_str().expect("path")]);
        let (code, out, err) = self.run_in(&clone, &["verify"], "");
        let facts = common::project::facts_at(&clone);
        (code, format!("{out}{err}"), facts)
    }
}

/// The c_dep history: the helper and greeting at S, c1's test alone (k1, where it crashes),
/// `between` (code on c1's place before G, when given), c3's test alone (k2), c3's fix (k3, G),
/// c1's fix (k4), then the plan with the laptop's stamps and the person's signed approval of
/// `a_dependency` at k1. Returns (k1, k3).
fn c_dep(p: &Project, k: &PrivateKey, between: Option<&str>) -> (String, String) {
    p.write("src/app.sh", APP);
    p.write("src/lib.sh", LIB);
    let s = p.land("the greeting and its helper");
    p.write("tests/test_c1.sh", C1_TEST);
    let k1 = p.land("c1: tests first");
    if let Some(app) = between {
        p.write("src/app.sh", app);
        p.land("c1's place, before the dependency");
    }
    p.write("tests/test_c3.sh", C3_TEST);
    let k2 = p.land("c3: tests first");
    p.write("src/lib.sh", LIB_FIXED);
    let k3 = p.land("c3: the helper");
    p.write("src/app.sh", APP_FIXED);
    let k4 = p.land("c1: the greeting");
    let (h1, h3) = (p.hash("c1", &k4), p.hash("c3", &k4));
    let stamp = |c: &str, id: &str, h: &str| ("fingerprint_stamped", json!({"criterion_id": c, "commit": id, "proof_not_tampered": h, "ran_in": "laptop"}));
    let mut lines = vec![
        ("intent_submitted", json!({"text": "greet with hello", "sha": s})),
        ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
        ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
        ("test_stack_detected", json!({"codebase_id": "fx", "project_root": ".", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"})),
    ];
    lines.extend(part("c1", "greet says hello", "src/app.sh", "greet", 3));
    lines.extend(part("c3", "the helper says ok", "src/lib.sh", "helper", 1));
    lines.extend([stamp("c3", &k2, &h3), stamp("c3", &k3, &h3), stamp("c1", &k3, &h1), stamp("c1", &k4, &h1)]);
    p.write_plan(&lines);
    p.approve(k, "c1", "crashed", "a_dependency", &k1, 7);
    p.land("the plan and the person's approval");
    (k1, k3)
}

#[test]
fn a_fresh_clone_verify_of_a_c_dep_history_proves_the_part_at_the_mirrored_stamps_commit() {
    let k = key();
    let p = project("ci-c-dep", &k);
    let (_, k3) = c_dep(&p, &k, None);
    let (code, out, facts) = p.verify_in_clone("c-dep");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&format!("\"greet says hello\" · proven · red at {}", &k3[..7])), "{k3} {out}");
    assert!(out.contains("\"the helper says ok\" · proven"), "{out}");
    // every pipeline result names its run, reserved by the pipeline and translated
    let of = |kind: &'static str| facts.iter().filter(move |f| f.kind == kind);
    let results: Vec<_> = of("pass_please_pass").filter(|f| f.fields.get("ran_in") == Some(&json!("pipeline"))).collect();
    assert!(!results.is_empty(), "{out}");
    for r in &results {
        let rid = r.fields.get("run_id").and_then(Value::as_str).unwrap_or_else(|| panic!("a pipeline result without its run: {:?}", r.fields));
        assert!(of("run_reserved").any(|f| f.fields.get("run_id") == Some(&json!(rid)) && f.fields.get("by") == Some(&json!("pipeline"))), "{rid}");
        assert!(of("run_ended").any(|f| f.fields.get("run_id") == Some(&json!(rid))), "{rid}");
        assert!(of("test_log_translation").any(|f| f.fields.get("run_id") == Some(&json!(rid))), "{rid}");
    }
    let crashed = of("test_log_translation").any(|f| f.criterion_id() == Some("c1") && f.fields.get("label") == Some(&json!("crashed")));
    assert!(crashed, "CI's own run at T crashed, read through its translation");
}

#[test]
fn a_c_dep_history_whose_stamp_fails_condition_3_is_unproven() {
    let k = key();
    let p = project("ci-c-dep-touched", &k);
    // c1's place changed between T and the stamp: the red there could hold c1's own fix
    let (k1, k3) = c_dep(&p, &k, Some("#!/bin/sh\ngreet() { echo  hi; }\ngreet\n"));
    let (code, out, _) = p.verify_in_clone("c-dep-touched");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"greet says hello\" · not proven"), "{out}");
    assert!(!out.contains(&format!("red at {}", &k3[..7])), "{out}");
    assert!(out.contains(&k1[..7]), "read from T, where it crashed: {out}");
}

/// c1 with no proof of its own, its plan holding `extra` after its lines.
fn plain_part(p: &Project, extra: &[(&str, Value)]) -> String {
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    let k1 = p.land("c1: tests first");
    let mut lines = vec![
        ("intent_submitted", json!({"text": "greet with hello", "sha": p.base()})),
        ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
        ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
        ("test_stack_detected", json!({"codebase_id": "fx", "project_root": ".", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"})),
    ];
    lines.extend(part("c1", "greet says hello", "src/app.sh", "greet", 3));
    lines.extend(extra.iter().cloned());
    p.write_plan(&lines);
    p.land("the plan");
    k1
}

fn kept_place() -> (&'static str, Value) {
    ("place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3}], "hold": "anchor_untouched", "choice": "keep_going"}))
}

#[test]
fn a_mirrored_keep_going_without_a_witness_is_read_as_a_plain_re_anchor() {
    let k = key();
    let p = project("ci-kept-place", &k);
    plain_part(&p, &[kept_place()]);
    let (_, out, facts) = p.verify_in_clone("kept");
    let places: Vec<_> = facts.iter().filter(|f| f.kind == "place_chosen").collect();
    assert_eq!(places.len(), 2, "{out}");
    assert!(places.iter().all(|f| !f.fields.contains_key("hold") && !f.fields.contains_key("choice")), "no witness, no keep_going: {:?}", places.last().map(|f| &f.fields));

    let q = project("ci-kept-place-witnessed", &k);
    let k1 = plain_part(&q, &[kept_place()]);
    q.approve(&k, "c1", "anchor_untouched", "keep_going", &k1, 9);
    q.land("the person's approval");
    let (_, out, facts) = q.verify_in_clone("kept-witnessed");
    let last = facts.iter().rfind(|f| f.kind == "place_chosen").expect("the kept place");
    assert_eq!(last.fields.get("choice"), Some(&json!("keep_going")), "a witness vouches for it: {out}");
}

#[test]
fn a_pipeline_stamp_in_the_plan_is_not_the_pipelines_own() {
    let k = key();
    let p = project("ci-forged-stamp", &k);
    let head_stamp = ("fingerprint_stamped", json!({"criterion_id": "c1", "commit": "0000000", "proof_not_tampered": "forged", "ran_in": "pipeline"}));
    plain_part(&p, &[head_stamp]);
    let (_, out, facts) = p.verify_in_clone("forged-stamp");
    assert!(!facts.iter().any(|f| f.kind == "fingerprint_stamped" && f.fields.get("proof_not_tampered") == Some(&json!("forged"))), "{out}");
}

#[test]
fn ci_does_not_run_a_command_the_agent_changed_after_fix_what_broke_it() {
    let k = key();
    let p = project("ci-command-locked", &k);
    plain_part(&p, &[]);
    // the person's answer on command_broke at the plan's commit: fix what broke it, the command kept
    let at = p.head();
    p.approve(&k, "c1", "command_broke", "fix_what_broke_it", &at, 11);
    p.land("the person's approval");
    let (_, out, _) = p.verify_in_clone("command-kept");
    assert!(!out.contains("the command changed"), "the command the person saw is the one run: {out}");
    // the agent changes c1's command after the answer
    let plan = std::fs::read_to_string(p.path(FAMILIAR_FOLDER).join(PLAN_FILE)).expect("plan");
    std::fs::write(p.path(FAMILIAR_FOLDER).join(PLAN_FILE), plan.replace("\"sh tests/test_c1.sh\"", "\"sh tests/test_c1.sh || true\"")).expect("plan");
    p.land("the agent's command");
    let (code, out, _) = p.verify_in_clone("command-changed");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"greet says hello\" · not proven: approval not verified (the command changed after your fix_what_broke_it at"), "{out}");
}
