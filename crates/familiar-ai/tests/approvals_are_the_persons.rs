//! The witness's files and CI's wiring are out of the agent's reach (plan
//! §6.9, B-5): `familiar-ai approve` is refused from an agent's shell as `on`
//! and `off` are, whatever the command does to its environment, and the
//! agent's edits and shell writes of `.familiar/approvals/` and `.github/` are
//! refused. A person's own terminal passes through no hook.

mod common;

use common::Project;
use serde_json::{json, Value};

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

fn reason(v: &Value) -> String {
    assert!(refused(v), "{v}");
    v["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason").to_string()
}

const APPROVE: &str = "familiar-ai: not approved · familiar-ai approve is the person's: the person signs by running familiar-ai approve in a terminal of their own (Familiar's pane shows the command)";

#[test]
fn an_agents_approve_is_refused_whatever_its_environment() {
    let p = Project::new("approve-shell");
    for cmd in [
        "familiar-ai approve --criterion_id c1 --hold anchor_moved --choice keep_going",
        "echo hunter2 | familiar-ai approve --criterion_id c1 --hold two_failures --choice release",
        "env -u CLAUDE_CODE_SESSION_ID familiar-ai approve --criterion_id c1 --hold two_failures --choice release < pass.txt",
        "cd /tmp && env -i PATH=$PATH ./target/debug/familiar-ai approve --criterion_id c1 --hold two_failures --choice release",
        "sh -c 'familiar-ai approve --criterion_id c1 --hold two_failures --choice release'",
    ] {
        assert_eq!(reason(&p.bash("PreToolUse", cmd)), APPROVE, "{cmd}");
    }
    assert!(!refused(&p.bash("PreToolUse", "grep -n 'familiar-ai approve' docs/decisions.md")));
    // switched off by the person, approve stays the person's
    assert_eq!(p.run_as(None, &["off"], "").0, 0);
    assert_eq!(reason(&p.bash("PreToolUse", "familiar-ai approve --criterion_id c1 --hold two_failures --choice release")), APPROVE);
}

#[test]
fn the_agents_edits_of_the_approvals_and_of_github_are_refused() {
    let p = Project::new("approve-edits");
    let root = p.repo();
    for rel in [".familiar/approvals/7.json", ".familiar/approvals/7.sig", ".github/workflows/familiar-verify.yml", ".github/CODEOWNERS"] {
        let path = root.join(rel).display().to_string();
        let write = p.tool("PreToolUse", "Write", json!({"file_path": path, "content": "x"}));
        assert!(refused(&write), "Write {rel}: {write}");
        let edit = p.tool("PreToolUse", "Edit", json!({"file_path": path, "old_string": "a", "new_string": "b"}));
        assert!(refused(&edit), "Edit {rel}: {edit}");
    }
    let github = reason(&p.tool("PreToolUse", "Write", json!({"file_path": root.join(".github/CODEOWNERS").display().to_string(), "content": "x"})));
    assert_eq!(github, "familiar-ai: .github/CODEOWNERS is the person's: CI's configuration decides which approvals it trusts, so the agent never edits .github/");
    for cmd in [
        "echo '{}' > .familiar/approvals/7.json",
        "cp /tmp/forged.sig .familiar/approvals/7.sig",
        "echo 'on: pull_request_target' >> .github/workflows/familiar-verify.yml",
        "tee .github/CODEOWNERS < /dev/null",
    ] {
        assert!(refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
    // reading them is fine
    for cmd in ["cat .github/workflows/familiar-verify.yml", "ls .familiar/approvals"] {
        assert!(!refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}
