//! A linked worktree follows main's switch until it is switched itself, and
//! each surface says whose switch it is: the status line, the session's
//! start, `status` and `status --json`, and `familiar-ai on|off` there.
//! See docs/decisions.md#the-off-switch

mod common;

use std::path::{Path, PathBuf};

use common::Project;
use familiar_ai_facts::Record;
use serde_json::{json, Value};

const SESSION: &str = "6a7b8c9d-0e1f-4a2b-8c3d-4e5f6a7b8c9d";
const TAIL: &str = "Every hook allows everything and says nothing. The person turns it on with the button or by running familiar-ai on in a terminal of their own.";
const AS_MAIN: &str = "Familiar is turned off for this worktree, as main is.";
const MAIN_ON: &str = "Familiar is turned off for this worktree; main is on.";

/// main set up and committed; the binary runs from the person's terminal.
fn project(name: &str) -> Project {
    Project::builder(name).no_session().create()
}

impl Project {
    /// A new worktree beside the repository on a new branch named `name`,
    /// set up there as a lane is first: its record starts empty.
    fn worktree(&self, name: &str) -> PathBuf {
        let at = self.dir.join(name);
        self.git(&["worktree", "add", "-q", "-b", name, &at.to_string_lossy()]);
        let (code, out, err) = self.run_in(&at, &["setup"], "");
        assert_eq!(code, 0, "setup in the worktree: {out}{err}");
        at
    }

    /// `familiar-ai hook <event>` at `at`, the payload's `cwd` there.
    fn hook_at(&self, at: &Path, event: &str, fields: Value) -> (i32, String, String) {
        let mut payload = json!({ "hook_event_name": event, "session_id": SESSION, "cwd": at.to_string_lossy(), "source": "startup" });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        self.run_in(at, &["hook", event], &payload.to_string())
    }

    /// The agent's edit of the code at `at`.
    fn edit_at(&self, at: &Path) -> (i32, String, String) {
        self.hook_at(at, "PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": at.join("src/app.sh").to_string_lossy() } }))
    }

    fn start_at(&self, at: &Path) -> Value {
        let (code, out, err) = self.hook_at(at, "SessionStart", json!({}));
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(out.trim()).expect("json")
    }

    fn json_at(&self, at: &Path) -> Value {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).expect("status json")
    }

    fn bar_at(&self, at: &Path) -> String {
        self.run_in(at, &["statusline"], "").1
    }

    fn first_status_line_at(&self, at: &Path) -> String {
        self.run_in(at, &["status"], "").1.lines().next().unwrap_or("").to_string()
    }

    fn facts_at(&self, at: &Path) -> u64 {
        Record::open(&at.join(".familiar/record")).expect("record").len().expect("len")
    }
}

fn switch_of(v: &Value) -> Value {
    json!({ "off": v["off"], "switch_from": v["switch_from"], "main_off": v["main_off"] })
}

fn says_nothing(out: &(i32, String, String)) {
    let (code, stdout, stderr) = out;
    assert_eq!(*code, 0, "{stdout}{stderr}");
    assert_eq!(stderr, "");
    assert_eq!(stdout.trim(), "{}", "said something: {stdout}");
}

fn refused_with_the_plan(out: &(i32, String, String)) {
    assert!(out.1.contains("familiar-ai: the plan comes first"), "the edit is governed: {}{}", out.1, out.2);
    assert!(out.1.contains("\"permissionDecision\":\"deny\""), "{}", out.1);
}

#[test]
fn main_off_a_new_worktree_follows_it_and_says_so() {
    let p = project("follow");
    let repo = p.repo();
    assert_eq!(p.run(&["off"], "").0, 0);
    let wt = p.worktree("lane");

    // the worktree's own record says nothing of a switch: main's decides
    let _ = p.json_at(&wt);
    let before = p.facts_at(&wt);
    says_nothing(&p.edit_at(&wt));
    says_nothing(&p.hook_at(&wt, "Stop", json!({})));
    assert_eq!(p.facts_at(&wt), before, "a hook wrote while off");

    let start = p.start_at(&wt);
    assert_eq!(start["hookSpecificOutput"]["additionalContext"], format!("{AS_MAIN} {TAIL}"), "{start}");
    assert_eq!(switch_of(&p.json_at(&wt)), json!({ "off": true, "switch_from": "main", "main_off": true }));
    assert_eq!(p.bar_at(&wt), "lane | Familiar | OFF · as main\n");
    assert_eq!(p.first_status_line_at(&wt), AS_MAIN);

    // main itself is as today
    assert_eq!(switch_of(&p.json_at(&repo)), json!({ "off": true, "switch_from": "here", "main_off": null }));
    assert_eq!(p.bar_at(&repo), "repo | Familiar | OFF\n");
    assert_eq!(p.first_status_line_at(&repo), "Familiar is turned off for this project.");
    let start = p.start_at(&repo);
    assert_eq!(start["hookSpecificOutput"]["additionalContext"], format!("Familiar is turned off for this project. {TAIL}"));
}

#[test]
fn on_in_the_worktree_switches_only_the_worktree_and_each_combination_is_worded() {
    let p = project("own");
    let repo = p.repo();
    assert_eq!(p.run(&["off"], "").0, 0);
    let wt = p.worktree("lane");

    let (code, out, err) = p.run_in(&wt, &["on"], "");
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "Familiar is turned on for this worktree. Main stays off.\n");
    assert_eq!(p.json_at(&repo)["off"], json!(true), "main stays off");
    says_nothing(&p.edit_at(&repo));
    refused_with_the_plan(&p.edit_at(&wt));
    assert_eq!(p.bar_at(&wt), "lane | Familiar | ON · here · main OFF | idle\n");
    assert_eq!(switch_of(&p.json_at(&wt)), json!({ "off": false, "switch_from": "here", "main_off": true }));
    assert_eq!(p.first_status_line_at(&wt), "Familiar is on here; main is off.");

    // off here too, as main is
    let (_, out, _) = p.run_in(&wt, &["off"], "");
    assert_eq!(out, "Familiar is turned off for this worktree. Main stays off.\n");
    assert_eq!(p.start_at(&wt)["hookSpecificOutput"]["additionalContext"], format!("{AS_MAIN} {TAIL}"));
    assert_eq!(p.bar_at(&wt), "lane | Familiar | OFF · here\n");
    assert_eq!(switch_of(&p.json_at(&wt)), json!({ "off": true, "switch_from": "here", "main_off": true }));

    // main turned on: the worktree's own switch still decides for it
    let (_, out, _) = p.run(&["on"], "");
    assert_eq!(out, "Familiar is turned on for this project.\n", "main's words are as today");
    says_nothing(&p.edit_at(&wt));
    assert_eq!(p.start_at(&wt)["hookSpecificOutput"]["additionalContext"], format!("{MAIN_ON} {TAIL}"));
    assert_eq!(p.bar_at(&wt), "lane | Familiar | OFF · here · main ON\n");
    assert_eq!(p.first_status_line_at(&wt), MAIN_ON);
    let (_, out, _) = p.run_in(&wt, &["on"], "");
    assert_eq!(out, "Familiar is turned on for this worktree. Main stays on.\n");
    assert_eq!(p.bar_at(&wt), "lane | Familiar | ON · here | idle\n");
    let (_, out, _) = p.run_in(&wt, &["off"], "");
    assert_eq!(out, "Familiar is turned off for this worktree. Main stays on.\n");
}

#[test]
fn main_turned_on_a_second_worktree_with_no_switch_is_on_as_main() {
    let p = project("second");
    assert_eq!(p.run(&["off"], "").0, 0);
    let first = p.worktree("lane");
    says_nothing(&p.edit_at(&first));
    assert_eq!(p.run(&["on"], "").0, 0);
    let second = p.worktree("lane2");
    refused_with_the_plan(&p.edit_at(&second));
    refused_with_the_plan(&p.edit_at(&first));
    assert_eq!(p.bar_at(&second), "lane2 | Familiar | ON · as main | idle\n");
    assert_eq!(switch_of(&p.json_at(&second)), json!({ "off": false, "switch_from": "main", "main_off": false }));
    let (_, text, _) = p.run_in(&second, &["status"], "");
    assert!(!text.contains("turned off") && !text.contains("main is"), "{text}");
}

#[test]
fn main_with_no_familiar_folder_leaves_the_worktree_on_as_today() {
    let p = Project::builder("unset").no_session().no_setup().create();
    let wt = p.worktree("lane");
    assert!(!p.path(".familiar").exists(), "main is not set up");
    refused_with_the_plan(&p.edit_at(&wt));
    assert_eq!(p.bar_at(&wt), "lane | Familiar | ON | idle\n");
    assert_eq!(switch_of(&p.json_at(&wt)), json!({ "off": false, "switch_from": "here", "main_off": null }));
}
