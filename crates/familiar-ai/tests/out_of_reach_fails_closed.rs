//! The witness's files and CI's wiring stay out of the agent's reach when the
//! command's text does not spell the path plainly (lane s9dw's review,
//! findings 1-3): a shell write the hook cannot resolve fails closed when the
//! text names `.github` or `.familiar/approvals`; `.github` is the
//! repository's, so a project nested in a subfolder cannot reach it through
//! `..`; and `approve` run through a variable or `eval` is refused as the
//! plain form is.

mod common;

use common::Project;
use serde_json::{json, Value};

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

const APPROVE: &str = "familiar-ai: not approved · familiar-ai approve is the person's: the person signs by running familiar-ai approve in a terminal of their own (Familiar's pane shows the command)";

#[test]
fn a_shell_write_the_hook_cannot_resolve_fails_closed_for_github_and_the_approvals() {
    let p = Project::new("reach-unresolved");
    for cmd in [
        "cp evil.yml $PWD/.github/workflows/ci.yml",
        "cp evil.yml \"${PWD}/.github/workflows/ci.yml\"",
        "cp forged.sig ~/repo/.familiar/approvals/7.sig",
        "D=.github/workflows/ci.yml; cp evil.yml $D",
        "python3 -c \"open('.github/workflows/x.yml','w').write('x')\"",
        "python3 -c \"import os; os.rename('evil.yml', '.github/workflows/ci.yml')\"",
        "python3 -c \"import shutil; shutil.copy('f.sig', '.familiar/approvals/7.sig')\"",
        "patch .github/workflows/ci.yml < drop-verify.diff",
    ] {
        assert!(refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
    // reading them, or writing elsewhere, stays allowed
    for cmd in ["cat $PWD/.github/workflows/ci.yml", "cp $PWD/.github/workflows/ci.yml /tmp/ci.yml", "cp a.txt $PWD/notes.txt", "python3 -c \"print(1)\""] {
        assert!(!refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}

#[test]
fn github_is_the_repositorys_even_from_a_project_nested_in_a_subfolder() {
    let p = Project::new("reach-nested");
    let app = p.repo().join("app");
    std::fs::create_dir_all(app.join(".familiar")).expect("nested .familiar");
    let at_app = |fields: Value| {
        let mut payload = p.payload("PreToolUse", fields);
        payload["cwd"] = json!(app.to_string_lossy());
        p.hook_with("PreToolUse", &payload)
    };
    let ci = p.repo().join(".github/workflows/ci.yml").display().to_string();
    let via_dots = app.join("../.github/workflows/ci.yml").display().to_string();
    for path in [&ci, &via_dots] {
        let write = at_app(json!({"tool_name": "Write", "tool_input": {"file_path": path, "content": "x"}}));
        assert!(refused(&write), "Write {path}: {write}");
    }
    for cmd in ["echo 'on: push' > ../.github/workflows/ci.yml", &format!("tee {ci} < /dev/null")] {
        let shell = at_app(json!({"tool_name": "Bash", "tool_input": {"command": cmd}}));
        assert!(refused(&shell), "{cmd}: {shell}");
    }
}

#[test]
fn approve_through_a_variable_or_eval_is_refused_as_the_plain_form_is() {
    let p = Project::new("reach-approve");
    for cmd in [
        "f=familiar-ai; $f approve --criterion_id c1 --hold two_failures --choice release",
        "eval \"familiar-ai approve --criterion_id c1 --hold two_failures --choice release\"",
        "eval familiar-ai approve --criterion_id c1 --hold two_failures --choice release",
        "a=approve; familiar-ai $a --criterion_id c1 --hold two_failures --choice release",
        "`which familiar-ai` approve --criterion_id c1 --hold two_failures --choice release",
    ] {
        let out = p.bash("PreToolUse", cmd);
        assert!(refused(&out), "{cmd}: {out}");
        assert_eq!(out["hookSpecificOutput"]["permissionDecisionReason"], APPROVE, "{cmd}");
    }
    for cmd in ["git commit -m 'approve: the signed answer'", "grep -rn approve crates/", "f=ls; $f -la"] {
        assert!(!refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}
