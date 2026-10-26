//! `.familiar/adapter` is familiar-ai's own, as the plan is (plan A-1, lane a-surf): the
//! binary writes it from the dry run's choice and it is committed with the work. The agent's
//! edits and shell writes of it are refused, a write whose path the text does not spell fails
//! closed. Setup refusing an ignore rule that would hide it waits for the owner's word (the
//! ignored test below).

mod common;

use common::Project;
use serde_json::{json, Value};

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

fn reason(v: &Value) -> String {
    v["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The adapter's own refusal, beside the plan's.
const OWN: &str = "familiar-ai: .familiar/adapter is written by familiar-ai itself from the dry run's choice of how each test runner is read, never by hand; it is committed with the work like the plan";

#[test]
fn an_edit_of_the_adapter_is_refused_naming_whose_it_is() {
    let p = Project::new("adapter-edit");
    let adapter = p.path(".familiar/adapter").display().to_string();
    for (tool, input) in [
        ("Edit", json!({"file_path": adapter, "old_string": "a", "new_string": "b"})),
        ("Write", json!({"file_path": adapter, "content": "{}"})),
        ("Write", json!({"file_path": ".familiar/adapter", "content": "{}"})),
    ] {
        let out = p.tool("PreToolUse", tool, input.clone());
        assert!(refused(&out), "{tool} {input}: {out}");
        assert!(reason(&out).starts_with(OWN), "{tool}: {}", reason(&out));
    }
}

#[test]
fn a_shell_write_of_the_adapter_is_refused_and_reading_it_is_not() {
    let p = Project::new("adapter-shell");
    for cmd in [
        "cp /tmp/mine .familiar/adapter",
        "tee .familiar/adapter < /tmp/mine",
        "sed -i 's/pytest:tables/pytest:reporter/' .familiar/adapter",
    ] {
        let out = p.bash("PreToolUse", cmd);
        assert!(refused(&out), "{cmd}: {out}");
        assert!(reason(&out).starts_with(OWN), "{cmd}: {}", reason(&out));
    }
    // a move into the folder is the folder's own refusal, which comes first
    let out = p.bash("PreToolUse", "mv /tmp/mine ./.familiar/adapter");
    assert!(refused(&out), "{out}");
    // a path the text does not spell, or an interpreter, fails closed when the text names it
    for cmd in [
        "cp /tmp/mine $PWD/.familiar/adapter",
        "D=.familiar/adapter; cp /tmp/mine $D",
        "python3 -c \"open('.familiar/adapter','w').write('{}')\"",
    ] {
        let out = p.bash("PreToolUse", cmd);
        assert!(refused(&out), "{cmd}: {out}");
        assert!(reason(&out).contains(".familiar/adapter"), "{cmd}: {}", reason(&out));
    }
    for cmd in ["cat .familiar/adapter", "git add .familiar/plan .familiar/adapter", "git diff .familiar/adapter", "python3 -m pytest tests/test_adapter.py"] {
        let out = p.bash("PreToolUse", cmd);
        assert!(!refused(&out) || !reason(&out).contains("adapter"), "{cmd}: {out}");
    }
}

// Held for the owner (lane a-surf, 2026-10-19): the sealed `setup_starts_a_project.rs`
// (`setup_appends_to_an_existing_gitignore_and_leaves_one_that_already_covers_the_record`) pins
// `.familiar/*` + `!.familiar/plan` as a setup that succeeds untouched, and that rule hides the
// adapter. Refusing it changes a sealed test's definition, which is the owner's word.
#[test]
fn setup_refuses_an_ignore_rule_that_would_hide_the_adapter() {
    for ignore in [".familiar/adapter\n", ".familiar/*\n!.familiar/plan\n"] {
        let p = Project::builder("adapter-ignored").no_setup().file(".gitignore", ignore).create();
        let (code, _out, err) = p.run(&["setup"], "");
        assert_eq!(code, 1, "{ignore:?}: {err}");
        assert_eq!(
            err,
            "familiar-ai: .familiar/adapter is ignored by git, but the adapter is committed with the work like the plan: take it out of the ignore rules\n",
            "{ignore:?}"
        );
    }
    let p = Project::builder("adapter-not-ignored").no_setup().file(".gitignore", "build/\n").create();
    let (code, _out, err) = p.run(&["setup"], "");
    assert_eq!(code, 0, "{err}");
}
