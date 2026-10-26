//! A project that was set up and had its folder removed refuses every hook
//! call until `familiar-ai setup` restores it; a project with no plan in git at
//! HEAD stays not set up.

mod common;

use std::process::Command;

use common::{planned_to_line_7, run_binary, stdout, Scratch};

/// The plan committed at HEAD with the folder removed refuses every
/// hook call until `familiar-ai setup` restores it.
#[test]
fn a_removed_folder_with_the_plan_committed_refuses_until_setup_restores_it() {
    let p = planned_to_line_7("removed");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(&p.dir)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "the plan so far"]);
    let plan_committed = p.read(".familiar/plan");
    assert!(plan_committed.contains("\"kind\":\"test_planned\""), "{plan_committed}");
    let hook = |event: &str, rest: &str| {
        let input = format!("{{\"hook_event_name\":\"{event}\",\"cwd\":\"{}\"{rest}}}", p.dir.display());
        let output = run_binary(&["hook", event], &input);
        assert_eq!(common::exit_code(&output), 0, "{event}: {}", common::stderr(&output));
        serde_json::from_str::<serde_json::Value>(&stdout(&output)).expect("json")
    };
    let edit = format!(",\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}/pydantic/json_schema.py\"}}", p.dir.display());

    // the agent removes the folder: an edit is denied, a stop is blocked, one plain line each
    std::fs::remove_dir_all(p.dir.join(".familiar")).expect("rm -rf .familiar");
    let out = hook("PreToolUse", &edit);
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(reason.contains("was removed") && reason.contains("familiar-ai setup"), "{reason}");
    assert_eq!(reason.lines().count(), 1);
    let out = hook("Stop", "");
    assert_eq!(out["decision"], "block", "{out}");
    assert_eq!(out["reason"].as_str(), Some(reason));
    let out = hook("SessionStart", "");
    assert_eq!(out["hookSpecificOutput"]["additionalContext"].as_str(), Some(reason), "told: {out}");
    assert!(!out.to_string().contains("not set up"), "{out}");
    assert!(!p.dir.join(".familiar").exists(), "nothing is created");
    // the plan file alone removed: the same
    git(&["checkout", "--", ".familiar/plan"]);
    std::fs::remove_file(p.dir.join(".familiar/plan")).expect("rm the plan");
    assert_eq!(hook("PreToolUse", &edit)["hookSpecificOutput"]["permissionDecision"], "deny");
    std::fs::remove_dir_all(p.dir.join(".familiar")).expect("rm -rf .familiar");

    // the exact setup call goes through, restores the folder and the plan, and
    // starts the record from the committed plan
    let out = hook("PreToolUse", ",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"familiar-ai setup\"}");
    assert_eq!(out, serde_json::json!({}), "the setup call is let through");
    let out = hook("PreToolUse", ",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"familiar-ai setup && rm -rf x\"}");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "only the exact call");
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("made .familiar at "), "{out}");
    assert!(
        out.contains("familiar-ai: restored .familiar/plan from git\nfamiliar-ai: .familiar/record starts from the committed plan: 1 part, 1 commit rechecked, 0 runs\n"),
        "{out}"
    );
    assert_eq!(p.read(".familiar/plan"), plan_committed);
    assert!(p.dir.join(".familiar/record").is_file(), "the record is started");
    // the plan file carries no finding and no answer of the cases card, so the seeded record owes
    // it again: passed as before (plan cases-card, 2026-10-25) [draft names]
    common::cases_kept_at(&p.dir, "c1");
    let (code, out, _) = p.run(&["status"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("familiar-ai: the plan is still being written\nfamiliar-ai: \"schema names the model\""), "the committed plan is in the record: {out}");

    // an edit is decided by the seeded record, the next missing line taught
    let out = hook("PreToolUse", &edit);
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(reason.starts_with("familiar-ai: the plan comes first"), "{reason}");
    assert!(!reason.contains("claim intent_submitted") && reason.contains("claim it: familiar-ai claim test_stack_detected"), "the intent is in the record: {reason}");
    assert!(!reason.contains("was removed"), "{reason}");

    // the record alone removed, the plan non-empty on disk: a removal too; setup
    // starts the record from the committed plan
    let (code, _, err) = p.run(&["claim", "intent_submitted", "--for", "0", "--text", "again"]);
    assert_eq!(code, 0, "{err}");
    assert!(!p.read(".familiar/plan").is_empty());
    std::fs::remove_file(p.dir.join(".familiar/record")).expect("rm the record");
    let out = hook("PreToolUse", &edit);
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").contains("was removed"), "{out}");
    assert!(!p.dir.join(".familiar/record").exists(), "the refusal creates nothing");
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("familiar-ai: .familiar/record starts from the committed plan: 1 part, 1 commit rechecked, 0 runs\n"), "{out}");
    assert!(!out.contains("restored"), "the plan was on disk: {out}");
    // the plan file carries no finding and no answer of the cases card, so the seeded record owes
    // it again: passed as before (plan cases-card, 2026-10-25) [draft names]
    common::cases_kept_at(&p.dir, "c1");
    let out = hook("PreToolUse", &edit);
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").starts_with("familiar-ai: the plan comes first"), "work continues: {out}");

    // a project with no plan in git at HEAD stays not set up: allow, create nothing
    let q = Scratch::new("never-set-up");
    q.git_init();
    std::fs::write(q.dir.join("a.txt"), "x\n").expect("write");
    let input = format!("{{\"hook_event_name\":\"PreToolUse\",\"cwd\":\"{}\",\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}/a.txt\"}}}}", q.dir.display(), q.dir.display());
    let output = run_binary(&["hook", "PreToolUse"], &input);
    assert_eq!(stdout(&output), "{}\n");
    assert!(!q.dir.join(".familiar").exists());
}
