//! A test command that worked and fails now is the person's card (plan §6.3): fix_what_broke_it
//! names the good run and the bad one and locks the command, so a new test_stack_detected is
//! refused until a run that is not the command's breakage; a command that never worked is sent
//! back, not carded.

mod common;
mod run_cards;

use run_cards::*;
use serde_json::json;

fn broke(name: &str) -> common::Project {
    let p = sealed(name);
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    commit(&p, "c1: a first fix");
    relabel(&p, "usage_error", json!({}));
    p
}

#[test]
fn a_command_that_broke_after_it_worked_is_the_card() {
    let p = broke("cmd-card");
    assert_eq!(cards(&p), ["command_broke"]);
    let status = p.status();
    assert!(status.contains("the test command worked before and fails now. What now?"), "{status}");
    let card = ask(&p);
    let labels: Vec<String> = card["questions"][0]["options"].as_array().into_iter().flatten().filter_map(|o| o["label"].as_str().map(str::to_string)).collect();
    assert_eq!(labels[0], "have the agent fix what broke the way the tests run, without changing how they are run");
}

#[test]
fn fix_what_broke_it_names_both_runs_once_and_locks_the_command() {
    let p = broke("cmd-fix");
    let runs: Vec<serde_json::Value> = p.facts_of("run_reserved");
    let (good, bad) = (&runs[runs.len() - 2], &runs[runs.len() - 1]);
    let short = |v: &serde_json::Value| v["commit"].as_str().unwrap_or("").chars().take(7).collect::<String>();
    let out = pick(&p, &ask(&p), "have the agent fix what broke the way the tests run, without changing how they are run");
    let cta = format!(
        "`sh tests/test_c1.sh` worked at {} (run {}) and fails since {} (run {}). Fix what your change broke; the command is locked.",
        short(good),
        good["run_id"].as_str().unwrap_or(""),
        short(bad),
        bad["run_id"].as_str().unwrap_or("")
    );
    assert_eq!(context(&out).matches(&cta).count(), 1, "{out}");
    let (code, _, err) = p.try_claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh -x"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("the test command is locked: the person chose to fix what broke it for \"greet says hello\""), "{err}");
    // another stack root is not this part's command: it is not locked (§6.3, "for this criterion")
    p.write("web/test_web.sh", "exit 0\n");
    let (code, _, err) = p.try_claim(&["test_stack_detected", "--project_root", "web", "--test_stack", "sh", "--test_command", "sh web/test_web.sh"]);
    assert_eq!(code, 0, "{err}");
    // the fix runs with the same command, and the lock is gone
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    commit(&p, "c1: the fix");
    let (code, _, err) = p.try_claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    assert_eq!(code, 0, "{err}");
}

#[test]
fn a_command_that_never_worked_is_sent_back_with_its_way_out() {
    let p = planned("cmd-never");
    p.write(TEST_FILE, RED);
    commit(&p, "c1: tests first");
    relabel(&p, "usage_error", json!({}));
    assert!(cards(&p).is_empty(), "no card");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let reason = stop["reason"].as_str().unwrap_or("");
    assert!(reason.contains("the test did not fail for the reason it tests: it is broken, too slow, or its command never ran it · next: "), "{reason}");
}
