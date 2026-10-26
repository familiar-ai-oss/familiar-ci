//! A git worktree made under the project's folder (as Claude Code makes a
//! subagent's, at `.claude/worktrees/agent-…`) is its own project: the project
//! is the nearest `.familiar` within the same git worktree as the working
//! folder. With its own `.familiar` it is governed by its own record; without
//! one it is ungoverned, never the parent checkout's. Seen 2026-10-12: a
//! worker there was told about the main checkout's record, because the hook
//! took the session's project folder (`CLAUDE_PROJECT_DIR`, the parent's).

mod common;

use std::path::{Path, PathBuf};

use common::project::facts_at;
use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "2e9a4f6b-7c13-4d58-9e02-5f6a7b8c9d0e";

/// The project, switched on, with a worktree nested under its folder.
fn nested(name: &str) -> (Project, PathBuf) {
    let p = Project::builder(name).session(SESSION).create();
    let wt = p.repo().join(".claude/worktrees/agent-a1");
    p.git(&["worktree", "add", "-q", "-b", "agent-a1", &wt.to_string_lossy()]);
    (p, wt)
}

impl Project {
    /// From here on the session's project folder is the parent's, as Claude
    /// Code passes it to a subagent's hooks.
    fn session_in_the_parent(&self) {
        self.set_env("CLAUDE_PROJECT_DIR", Some(&self.repo().to_string_lossy()));
    }

    /// A hook call whose working folder is `cwd`.
    fn hook_at(&self, cwd: &Path, event: &str, fields: Value) -> Value {
        let mut payload = self.payload(event, fields);
        payload["cwd"] = json!(cwd.to_string_lossy());
        self.hook_with(event, &payload)
    }

    fn edit_at(&self, cwd: &Path, file: &Path) -> Value {
        self.hook_at(cwd, "PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": file.to_string_lossy(), "old_string": "echo hi", "new_string": "echo hello" } }))
    }
}

fn reason(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

fn said_at(repo: &Path) -> Vec<String> {
    facts_at(repo).into_iter().filter(|f| f.kind == "person_said").filter_map(|f| f.fields.get("text").and_then(Value::as_str).map(str::to_string)).collect()
}

#[test]
fn a_nested_worktree_without_its_own_familiar_is_ungoverned_not_the_parents() {
    let (p, wt) = nested("nested-none");
    assert!(!wt.join(".familiar").exists(), "this worktree has no .familiar of its own");
    p.session_in_the_parent();
    let before = p.facts().len();

    assert_eq!(p.hook_at(&wt, "UserPromptSubmit", json!({ "prompt": "make greet say hello" })), json!({}));
    let out = p.edit_at(&wt, &wt.join("src/app.sh"));
    assert_eq!(out, json!({}), "ungoverned: allowed, nothing told");
    // from a folder inside the worktree too
    std::fs::create_dir_all(wt.join("src/deep")).expect("subfolder");
    assert_eq!(p.edit_at(&wt.join("src/deep"), &wt.join("src/app.sh")), json!({}));
    assert_eq!(p.facts().len(), before, "the parent's record is not written");

    // the parent itself is still governed by its own record
    let out = p.edit_at(&p.repo(), &p.path("src/app.sh"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
}

#[test]
fn a_nested_worktree_with_its_own_familiar_is_governed_by_its_own_record() {
    let (p, wt) = nested("nested-own");
    let (code, out, err) = p.run_in(&wt, &["setup"], "");
    assert_eq!(code, 0, "setup in the worktree: {out}{err}");
    assert!(wt.join(".familiar").is_dir(), "the worktree has its own .familiar");
    p.session_in_the_parent();
    let before = p.facts().len();

    p.hook_at(&wt, "UserPromptSubmit", json!({ "prompt": "make greet say hello in the lane" }));
    assert_eq!(said_at(&wt), ["make greet say hello in the lane"], "the worktree's own record hears it");
    assert_eq!(p.facts().len(), before, "the parent's record is not written");
    assert!(!said_at(&p.repo()).iter().any(|s| s.contains("in the lane")));

    let out = p.edit_at(&wt, &wt.join("src/app.sh"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let r = reason(&out);
    assert!(r.starts_with("familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code\n"), "named relative to the worktree: {r}");
    assert!(r.contains("\"make greet say hello in the lane\""), "the worktree's own prompt: {r}");
}
