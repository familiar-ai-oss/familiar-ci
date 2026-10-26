//! `familiar-ai status --json` is the mod's only source: it carries one
//! number for what needs the person, and the CI words whenever a part is
//! green or beyond, so no surface derives either.
//!
//! ```text
//!   counts.needs_you   every open card and everything waiting on the person
//!   ci                 the CI line at HEAD; "CI pending" when a part is green
//!                      or beyond and there is no HEAD to read; null otherwise
//! ```

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "3d7c2a91-5b0e-4f6a-8c1d-2e9f0a7b6c54";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").create()
}

impl Project {
    /// The intent and c1 planned through its last line.
    fn planned(&self, with_stack: bool) {
        self.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
        let path = self.path("src/a.sh").to_string_lossy().into_owned();
        let out = self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
        let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
        self.claim(&["intent_submitted", "--for", &t, "--text", "a says hello"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "a says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/a.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        if with_stack {
            self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        }
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    }
}

fn state_name(j: &Value) -> &str {
    j["criteria"][0]["state_name"].as_str().unwrap_or("")
}

/// A card open: `counts.needs_you` is a number and counts it, the same as
/// the cards listed and as `waiting_on_you`.
#[test]
fn needs_you_is_one_number_that_counts_the_open_cards() {
    let p = project("card");
    p.planned(false);
    let j = p.status_json();
    let cards = j["cards"].as_array().expect("cards").len();
    assert_eq!(cards, 1, "{j}");
    assert!(j["counts"]["needs_you"].is_u64(), "{j}");
    assert_eq!(j["counts"]["needs_you"], cards, "{j}");
    assert_eq!(j["waiting_on_you"], j["counts"]["needs_you"], "{j}");
}

/// c1 green: with a HEAD the CI words are the CI line there; with no HEAD to
/// read they are "CI pending"; before any part is green, and with no HEAD,
/// there are none.
#[test]
fn a_green_part_carries_ci_words_with_or_without_a_head() {
    let p = project("green");
    assert!(p.status_json()["ci"].is_string(), "a HEAD: the CI line");
    p.planned(true);
    p.write("tests/test_c1.sh", "sh src/a.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    assert_eq!(state_name(&p.status_json()), "sealed");

    // no HEAD to read while sealed: no CI words
    std::fs::rename(p.repo().join(".git"), p.dir.join("git-away")).expect("move .git");
    let j = p.status_json();
    assert_eq!(state_name(&j), "sealed", "{j}");
    assert!(j["ci"].is_null(), "nothing green, no HEAD: {j}");
    std::fs::rename(p.dir.join("git-away"), p.repo().join(".git")).expect("put .git back");

    p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: the fix");
    let head7 = p.git(&["rev-parse", "--short=7", "HEAD"]);
    let j = p.status_json();
    assert_eq!(state_name(&j), "green", "{j}");
    assert_eq!(j["ci"], format!("CI: no run for {head7}"), "{j}");

    // green with no HEAD to read: the binary says CI is pending
    std::fs::rename(p.repo().join(".git"), p.dir.join("git-away")).expect("move .git");
    let j = p.status_json();
    assert_eq!(state_name(&j), "green", "{j}");
    assert_eq!(j["ci"], "CI pending", "{j}");
}
