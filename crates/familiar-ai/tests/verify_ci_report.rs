//! verify writes `.familiar/verify-report.json` beside the record at exit 0, 1
//! and 2, and appends the job summary when `GITHUB_STEP_SUMMARY` is set; a green
//! run in CI is labelled hosted CI-proven.

mod common;
mod way_back;
use way_back::*;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The CI variables verify reads; cleared so the tests run the same anywhere.
const CI_VARS: [&str; 4] = ["GITHUB_RUN_ID", "GITHUB_SHA", "RUNNER_NAME", "GITHUB_STEP_SUMMARY"];

/// `verify` in a fresh clone of the branch, with `env` set (and every other
/// CI variable cleared): its exit, what it said, and the clone.
fn verify_with(p: &Sealed, name: &str, env: &[(&str, &str)]) -> (i32, String, PathBuf) {
    let clone = p.dir.join(format!("clone-{name}"));
    let _ = std::fs::remove_dir_all(&clone);
    p.git_in(&p.dir, &["clone", "-q", "--branch", "work", p.repo().to_str().expect("path"), clone.to_str().expect("path")]);
    let mut cmd = p.command();
    for v in CI_VARS {
        cmd.env_remove(v);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.current_dir(&clone).arg("verify");
    let (code, out, err) = common::Project::output(cmd, "");
    (code, format!("{out}{err}"), clone)
}

fn report(clone: &Path) -> Value {
    let path = clone.join(".familiar/verify-report.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("the report is JSON")
}

fn head(clone: &Path) -> String {
    let out = Command::new("git").current_dir(clone).args(["rev-parse", "HEAD"]).output().expect("git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn plan_hash(clone: &Path) -> String {
    let out = Command::new("git").current_dir(clone).args(["show", "HEAD:.familiar/plan"]).output().expect("git");
    assert!(out.status.success(), "the plan is committed");
    Sha256::digest(&out.stdout).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn a_proven_run_writes_the_report_beside_the_record_exit_0() {
    let p = Sealed::green("report-0");
    p.commit_plan();
    let (code, out, clone) = verify_with(&p, "proven", &[]);
    assert_eq!(code, 0, "{out}");
    let r = report(&clone);
    assert_eq!(r["run"]["id"], "local", "{r}");
    assert_eq!(r["run"]["runner"], "local", "{r}");
    assert!(r["run"]["at"].is_number(), "the clock, as seconds: {r}");
    assert_eq!(r["sha"], head(&clone).as_str(), "{r}");
    assert_eq!(r["sha"].as_str().map(str::len), Some(40), "{r}");
    assert_eq!(r["plan_hash"], plan_hash(&clone).as_str(), "{r}");
    assert_eq!(r["exit"], 0, "{r}");
    let crits = r["criteria"].as_array().expect("criteria");
    assert_eq!(crits.len(), 1, "{r}");
    assert_eq!(crits[0]["id"], "c1");
    assert_eq!(crits[0]["text"], "greet says hello");
    assert_eq!(crits[0]["outcome"], "proven");
    let evidence = crits[0]["evidence"].as_str().expect("evidence");
    assert!(evidence.starts_with("\"greet says hello\" · proven · red at "), "verify's own line, without the lead: {evidence}");
    assert!(out.contains(&format!("familiar-ai: {evidence}")), "the line as printed: {out}");
    assert!(r.get("could_not").is_none(), "{r}");
}

#[test]
fn in_ci_a_proven_criterion_is_hosted_ci_proven_and_the_summary_is_written() {
    let p = Sealed::green("report-ci");
    p.commit_plan();
    // the head of the branch, as GitHub hands it to the job
    let sha = p.git(&["rev-parse", "HEAD"]).trim().to_string();
    let summary = p.dir.join("step-summary.md");
    std::fs::write(&summary, "earlier step\n").expect("summary");
    let (code, out, clone) = verify_with(
        &p,
        "ci",
        &[("GITHUB_RUN_ID", "4242"), ("GITHUB_SHA", &sha), ("RUNNER_NAME", "r1"), ("GITHUB_STEP_SUMMARY", summary.to_str().expect("path"))],
    );
    assert_eq!(code, 0, "{out}");
    let r = report(&clone);
    assert_eq!(r["run"]["id"], "4242", "{r}");
    assert_eq!(r["run"]["runner"], "r1", "{r}");
    assert_eq!(r["sha"], sha.as_str(), "{r}");
    let label = format!("hosted CI-proven at {}, run 4242", &sha[..7]);
    assert_eq!(r["criteria"][0]["evidence"], label.as_str(), "{r}");
    let md = std::fs::read_to_string(&summary).expect("summary");
    assert!(md.starts_with("earlier step\n"), "appended, never replaced: {md}");
    assert!(md.contains("### familiar-verify: exit 0 (0 proven, 1 not proven, 2 could not prove)"), "{md}");
    assert!(md.contains("\n- \"greet says hello\" · proven · red at "), "{md}");
    assert!(md.contains(&label), "{md}");
}

#[test]
fn a_run_that_is_not_proven_writes_the_report_exit_1() {
    let p = Sealed::way_back("report-1");
    let plan = p.repo().join(".familiar/plan");
    let text = std::fs::read_to_string(&plan).expect("plan");
    assert!(text.contains("\"kind\":\"test_change_announced\""), "{text}");
    let kept: String = text.lines().filter(|l| !l.contains("\"kind\":\"test_change_announced\"")).map(|l| format!("{l}\n")).collect();
    std::fs::write(&plan, kept).expect("plan");
    p.commit_plan();
    let summary = p.dir.join("step-summary.md");
    let (code, out, clone) = verify_with(&p, "unannounced", &[("GITHUB_RUN_ID", "77"), ("GITHUB_STEP_SUMMARY", summary.to_str().expect("path"))]);
    assert_eq!(code, 1, "{out}");
    let r = report(&clone);
    assert_eq!(r["exit"], 1, "{r}");
    assert_eq!(r["criteria"][0]["id"], "c1", "{r}");
    assert_eq!(r["criteria"][0]["outcome"], "not_proven", "{r}");
    let evidence = r["criteria"][0]["evidence"].as_str().expect("evidence");
    assert!(evidence.contains("its sealed tests were changed after the tests-first commit"), "{evidence}");
    assert!(!evidence.starts_with("familiar-ai: "), "{evidence}");
    let md = std::fs::read_to_string(&summary).expect("summary");
    assert!(md.contains("### familiar-verify: exit 1 (0 proven, 1 not proven, 2 could not prove)"), "{md}");
    assert!(!md.contains("hosted CI-proven"), "{md}");
}

#[test]
fn a_run_that_could_not_prove_anything_writes_the_report_exit_2() {
    let p = Sealed::new("report-2");
    let summary = p.dir.join("step-summary.md");
    let (code, out, clone) = verify_with(&p, "nothing", &[("GITHUB_STEP_SUMMARY", summary.to_str().expect("path"))]);
    assert_eq!(code, 2, "{out}");
    let r = report(&clone);
    assert_eq!(r["exit"], 2, "{r}");
    assert_eq!(r["criteria"], serde_json::json!([]), "{r}");
    let could_not = r["could_not"].as_str().expect("could_not");
    assert!(could_not.starts_with("verify could not prove anything: "), "{could_not}");
    assert!(out.contains(&format!("familiar-ai: {could_not}")), "{out}");
    assert_eq!(r["sha"], head(&clone).as_str(), "{r}");
    let md = std::fs::read_to_string(&summary).expect("summary");
    assert!(md.contains("### familiar-verify: exit 2 (0 proven, 1 not proven, 2 could not prove)"), "{md}");
    assert!(md.contains(could_not), "{md}");
}

#[test]
fn outside_any_repository_no_report_and_no_folder_is_made() {
    let p = Sealed::new("report-outside");
    let outside = p.dir.join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    let out = Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
        .env("FAMILIAR_AI_HOME", p.dir.join("home"))
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("GITHUB_STEP_SUMMARY")
        .current_dir(&outside)
        .arg("verify")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    assert!(!outside.join(".familiar").exists(), "no .familiar is made outside a checkout");
}

#[test]
fn a_committed_report_makes_verify_exit_2() {
    let p = Sealed::green("report-tracked");
    p.commit_plan();
    std::fs::write(p.repo().join(".familiar/verify-report.json"), "{\"exit\":0}\n").expect("forged");
    p.git(&["add", "-f", ".familiar/verify-report.json"]);
    p.git(&["commit", "-q", "-m", "a forged report"]);
    let (code, out, _) = verify_with(&p, "tracked", &[]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains(".familiar/verify-report.json"), "{out}");
}

#[test]
fn a_report_that_cannot_be_written_makes_verify_exit_2() {
    let p = Sealed::green("report-unwritable");
    p.commit_plan();
    let clone = p.dir.join("clone-unwritable");
    let _ = std::fs::remove_dir_all(&clone);
    p.git_in(&p.dir, &["clone", "-q", "--branch", "work", p.repo().to_str().expect("path"), clone.to_str().expect("path")]);
    // a folder where the report goes: the write fails
    std::fs::create_dir_all(clone.join(".familiar/verify-report.json")).expect("dir");
    let mut cmd = p.command();
    for v in CI_VARS {
        cmd.env_remove(v);
    }
    cmd.current_dir(&clone).arg("verify");
    let (code, out, err) = common::Project::output(cmd, "");
    assert_eq!(code, 2, "{out}{err}");
    assert!(format!("{out}{err}").contains("verify-report.json"), "{out}{err}");
}
