//! `familiar-ai verify` on a history of several parts: each part is proven at
//! its own commits, walked from its own intent's sha, under the command in
//! force at its own last commit; a later lane's stack line never changes an
//! earlier part's command, and the merged head is not where parts are proven.

mod common;

use std::time::Duration;

use common::Project;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, PLAN_FILE};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(20);
const A_FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";

/// `src/app.sh` and `lib/b.sh` with the record ignored, committed as `base`, no setup.
fn project(name: &str) -> Project {
    Project::builder(name).no_setup().file(".gitignore", ".familiar/record\n").file("lib/b.sh", "echo b\n").create()
}

impl Project {
    /// The `base` commit, the root of the history.
    fn base(&self) -> String {
        self.git(&["rev-list", "--max-parents=0", "HEAD"])
    }

    /// The plan's claims, written as the binary writes the plan file.
    fn plan(&self, claims: &[(&str, Value)]) {
        let claims: Vec<Claim> = claims
            .iter()
            .map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).unwrap_or_else(|e| panic!("{k}: {e}")))
            .collect();
        let path = self.path(FAMILIAR_FOLDER).join(PLAN_FILE);
        familiar_ai_plan::write(&path, &claims).expect("plan");
    }

    /// verify here, its exit code and what it printed.
    fn verify(&self, wait: Duration) -> (i32, String) {
        let out = familiar_ai::run(["verify"], "", &around(&self.repo(), wait));
        (out.exit_code, format!("{}{}", out.stdout_text, out.stderr_text))
    }

    /// A part on the branch checked out: its plan and tests alone, then its
    /// fix; the fix commit's id.
    fn part(&self, name: &str, plan: &[(&str, Value)], tests: &[(&str, &str)], fix: (&str, &str)) -> String {
        self.plan(plan);
        for (path, text) in tests {
            self.write(path, text);
        }
        self.land(&format!("{name}: tests first"));
        self.write(fix.0, fix.1);
        self.land(&format!("{name}: fix"))
    }

    /// Merge `branch` onto main with `--no-ff`, its plan the union given.
    fn merge(&self, branch: &str, union: &[(&str, Value)]) -> String {
        // the plans conflict when both lanes wrote one; the union resolves it
        let _ = std::process::Command::new("git")
            .args(["merge", "-q", "--no-ff", "--no-commit", branch])
            .current_dir(self.repo())
            .output()
            .expect("git merge");
        self.plan(union);
        self.land(&format!("merge {branch}"))
    }
}

fn around(dir: &std::path::Path, wait: Duration) -> Surroundings {
    Surroundings {
        now: 1.0,
        project_dir: Some(dir.to_path_buf()),
        cwd: Some(dir.to_path_buf()),
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_secs(1),
        test_run_wait: wait,
        run_inline: Duration::ZERO,
        runner: None,
        judge_dir: None,
        judge_wait: Duration::ZERO,
        investigator: None,
        conversation: None,
        bench: false,
    }
}

/// One part's plan: its intent at `sha`, its part on one file's lines, its
/// test and its own `test_stack_detected` at the repository root.
fn part_plan(sha: &str, c: &str, text: &str, anchor: (&str, &str, u64, u64), test: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("intent_submitted", json!({"text": format!("ask of {c}"), "sha": sha})),
        ("criterion", json!({"criterion_id": c, "text": text, "path": "light"})),
        ("place_chosen", json!({"criterion_id": c, "places": [{"file_path": anchor.0, "symbol": anchor.1, "line_start": anchor.2, "line_end": anchor.3}]})),
        ("test_planned", json!({"criterion_id": c, "file_path": test, "will": "create"})),
        ("test_stack_detected", json!({"codebase_id": format!("fx-{c}"), "project_root": "", "test_stack": "sh", "test_command": format!("sh {test}")})),
        ("criterion_head", json!({"criterion_id": c})),
        ("criterion_assigned", json!({"criterion_id": c, "agent_conversation_id": format!("s-{c}")})),
        ("fix_location_category", json!({"criterion_id": c, "category": "this_repository", "where": ""})),
        ("test_written", json!({"criterion_id": c, "file_path": test, "test_name": ""})),
    ]
}

fn plan_a(sha: &str) -> Vec<(&'static str, Value)> {
    part_plan(sha, "c1", "greet says hello", ("src/app.sh", "greet", 1, 3), "tests/test_c1.sh")
}

fn plan_b(sha: &str) -> Vec<(&'static str, Value)> {
    part_plan(sha, "c2", "b says bee", ("lib/b.sh", "b", 1, 1), "tests/test_c2.sh")
}

fn union(a: &[(&'static str, Value)], b: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    a.iter().chain(b.iter()).cloned().collect()
}

/// The one line of the part named `text`.
fn line_of<'a>(out: &'a str, text: &str) -> &'a str {
    out.lines().find(|l| l.contains(&format!("\"{text}\""))).unwrap_or_else(|| panic!("no line for {text}: {out}"))
}

#[test]
fn two_lanes_merged_are_each_proven_at_their_own_fix_under_their_own_command_exit_0() {
    let r = project("two-lanes");
    let base = r.base();
    let (a, b) = (plan_a(&base), plan_b(&base));
    r.git(&["checkout", "-q", "-b", "lane-a", &base]);
    let a_fix = r.part("a", &a, &[("tests/test_c1.sh", "grep -q hello src/app.sh\n")], ("src/app.sh", A_FIXED));
    r.git(&["checkout", "-q", "-b", "lane-b", &base]);
    let b_fix = r.part("b", &b, &[("tests/test_c2.sh", "grep -q bee lib/b.sh\n")], ("lib/b.sh", "echo bee\n"));
    r.git(&["checkout", "-q", "main"]);
    let m1 = r.merge("lane-a", &a);
    let head = r.merge("lane-b", &union(&a, &b));
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 0, "{out}");
    assert_eq!(out.lines().count(), 2, "one line per part: {out}");
    let (la, lb) = (line_of(&out, "greet says hello"), line_of(&out, "b says bee"));
    assert!(la.contains(" · proven · ") && la.ends_with("ran: sh tests/test_c1.sh"), "A under its own command: {out}");
    assert!(lb.contains(" · proven · ") && lb.ends_with("ran: sh tests/test_c2.sh"), "B under its own command: {out}");
    // each at its own commits: A's last commit is its fix or the merge that
    // brought it, never B's merge; B's is its fix or B's merge
    assert!(la.contains(&format!("green at {}", &a_fix[..7])) || la.contains(&format!("green at {}", &m1[..7])), "A at its own commit: {out}");
    assert!(!la.contains(&head[..7]), "A is not proven at the merged head: {out}");
    assert!(lb.contains(&format!("green at {}", &b_fix[..7])) || lb.contains(&format!("green at {}", &head[..7])), "B at its own commit: {out}");
}

#[test]
fn a_sealed_test_a_later_part_changes_is_tampering_for_its_part_and_the_later_part_is_proven_exit_1() {
    let r = project("shared-test");
    let base = r.base();
    let a = plan_a(&base);
    r.part("a", &a, &[("tests/test_c1.sh", "grep -q hello src/app.sh\n")], ("src/app.sh", A_FIXED));
    // B starts from main after A landed, and its fix also adds a check to A's
    // sealed test, unannounced, with code
    let b = plan_b(&r.head());
    r.write("tests/test_c1.sh", "grep -q hello src/app.sh\ngrep -q greet src/app.sh\n");
    r.git(&["stash", "push", "-q", "--", "tests/test_c1.sh"]);
    r.part("b", &union(&a, &b), &[("tests/test_c2.sh", "grep -q bee lib/b.sh\n")], ("lib/b.sh", "echo bee\n"));
    r.git(&["stash", "pop", "-q"]);
    r.write("lib/b.sh", "echo bee\n# and A's test grows\n");
    r.land("b: A's sealed test grows, with code");
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 1, "{out}");
    let (la, lb) = (line_of(&out, "greet says hello"), line_of(&out, "b says bee"));
    assert!(la.contains("not proven: its sealed tests were changed after the tests-first commit"), "A's sealed file changed is tampering: {out}");
    assert!(lb.contains(" · proven · ") && lb.ends_with("ran: sh tests/test_c2.sh"), "B is proven by its own test: {out}");
}

#[test]
fn an_earlier_ask_is_walked_from_its_own_intents_sha_and_proven_under_its_own_command_exit_0() {
    let r = project("two-asks");
    let base = r.base();
    let a = plan_a(&base);
    let a_fix = r.part("a", &a, &[("tests/test_c1.sh", "grep -q hello src/app.sh\n")], ("src/app.sh", A_FIXED));
    // the second ask's intent is at A's fix: A's tests-first commit is before it
    let b = plan_b(&a_fix);
    let b_fix = r.part("b", &union(&a, &b), &[("tests/test_c2.sh", "grep -q bee lib/b.sh\n")], ("lib/b.sh", "echo bee\n"));
    let (exit, out) = r.verify(WAIT);
    assert_eq!(exit, 0, "{out}");
    let (la, lb) = (line_of(&out, "greet says hello"), line_of(&out, "b says bee"));
    assert!(la.contains(" · proven · ") && la.contains(&format!("green at {}", &a_fix[..7])) && la.ends_with("ran: sh tests/test_c1.sh"), "A from its own sha, at its own fix, its own command: {out}");
    assert!(lb.contains(" · proven · ") && lb.contains(&format!("green at {}", &b_fix[..7])) && lb.ends_with("ran: sh tests/test_c2.sh"), "{out}");
}
