//! The agent's first edit or commit of code while no run is open is refused
//! until it claims the intent. The refusal lists the recent prompts with their
//! `t`, so the agent names the one that asked for the work (`--for`).

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "6c1e8d2f-0b62-4d9f-9b58-2a3b4c5d6e7f";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    fn statusline(&self) -> String {
        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "workspace": { "current_dir": self.repo().to_string_lossy(), "project_dir": self.repo().to_string_lossy() } });
        let (code, out, err) = self.run(&["statusline"], &input.to_string());
        assert_eq!((code, err.as_str()), (0, ""));
        // the status line shows the ask and its parts' map; CI is the band's
        assert!(!out.contains("CI"), "{out}");
        out
    }

    fn plan_file(&self) -> String {
        self.read(".familiar/plan")
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal shows beside a prompt's words.
fn t_of(reason: &str, words: &str) -> u64 {
    let line = reason.lines().find(|l| l.contains(&format!("\"{words}\""))).unwrap_or_else(|| panic!("{words:?} is listed: {reason}"));
    let t = line.split_whitespace().find(|w| w.starts_with('t') && w[1..].chars().all(|c| c.is_ascii_digit()) && w.len() > 1).unwrap_or_else(|| panic!("a t beside {words:?}: {line}"));
    t[1..].parse().expect("a number")
}

#[test]
fn the_first_edit_of_code_lists_the_prompts_and_waits_for_the_named_intent() {
    let p = project("first");
    for prompt in ["how does the judge pick its model?", "now make greet say hello", "yes, go ahead"] {
        p.hook("UserPromptSubmit", json!({ "prompt": prompt }));
    }
    let out = p.edit("src/app.sh", "echo hi", "echo hello");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let reason = denied(&out);
    assert!(reason.starts_with("familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code\n"), "{reason}");
    let (a, b, c) = (t_of(&reason, "how does the judge pick its model?"), t_of(&reason, "now make greet say hello"), t_of(&reason, "yes, go ahead"));
    assert!(a < b && b < c, "in the order said: {reason}");
    assert!(reason.contains("\n  name the prompt that asked for this work; --text is your reading of it\n"), "{reason}");
    assert!(reason.contains(&format!("\n  claim it: familiar-ai claim intent_submitted --for {c} --text 'yes, go ahead'")), "{reason}");
    assert!(!p.plan_file().contains("intent_submitted"), "nothing is written for the refusal");

    let (code, out, err) = p.try_claim(&["intent_submitted", "--for", &b.to_string(), "--text", "greet says hello instead of hi"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(p.plan_file().contains(&format!("\"for\":{b}")), "{}", p.plan_file());
    let out = p.edit("src/app.sh", "echo hi", "echo hello");
    let reason = denied(&out);
    assert!(!reason.contains("line 1: the intent"), "the intent is there now: {reason}");
    assert!(reason.starts_with("familiar-ai: the plan comes first · next: write the criterion"), "a run is open, planning: {reason}");
}

#[test]
fn a_commit_of_code_is_judged_once_it_lands() {
    let p = project("landed");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let commit = json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am 'greet says hello'" } });
    let pre = p.hook("PreToolUse", commit.clone());
    assert!(pre["hookSpecificOutput"]["permissionDecision"].is_null(), "what it commits is known once it lands: {pre}");
    std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hello; }\ngreet\n").expect("write");
    p.git(&["commit", "-qam", "greet says hello"]);
    let mut post = commit;
    post["tool_response"] = json!({ "stdout": "", "stderr": "" });
    let post = p.hook("PostToolUse", post);
    assert_eq!(post["decision"], "block", "{post}");
    let reason = post["reason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code\n"), "{reason}");
}

#[test]
fn the_old_doors_intent_with_no_criterion_opens_nothing() {
    let p = project("old-door");
    p.hook("UserPromptSubmit", json!({ "prompt": "can you assess where we are?" }));
    // an intent as an older binary wrote it: the prompt as its text, no `for`
    let head = p.head();
    let record = familiar_ai_facts::Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("record");
    let old = json!({ "text": "can you assess where we are?", "sha": head });
    record.append_raw("intent_submitted", old.as_object().cloned().unwrap_or_default(), 1.0, familiar_ai::TENANT).expect("the old door's intent");
    drop(record);
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
    assert!(p.hook("Stop", json!({})).get("decision").is_none(), "a stop goes");
    let reason = denied(&p.edit("src/app.sh", "echo hi", "echo hello"));
    assert!(reason.starts_with("familiar-ai: the plan comes first · line 1: the intent"), "{reason}");
}
