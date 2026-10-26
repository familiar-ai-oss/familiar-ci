//! An edit or commit of prose, of the agent's support files, or of comment
//! lines alone is passive: it goes through and opens no run. One changed line
//! of code, a string literal included, is not passive.

mod common;

use common::Project;
use serde_json::{json, Value};

const RUST: &str = "// greets the person\nfn greet() -> &'static str {\n    \"hi\"\n}\n";

const SESSION: &str = "7d2f9e3a-1c73-4ea0-8c69-3b4c5d6e7f80";

fn project(name: &str) -> Project {
    Project::builder(name)
        .session(SESSION)
        .no_app()
        .file("src/app.rs", RUST)
        .file("src/tool.py", "# a helper\ndef tool():\n    return 'hi'\n")
        .file("README.md", "# app\n\nIt greets.\n")
        .file("notes/plan.py", "x = 1\n")
        .file(".gitattributes", "notes/** linguist-documentation\n")
        .create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }

    /// The hook before a Write of `file`.
    fn write_tool(&self, file: &str, content: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Write", "tool_input": { "file_path": path, "content": content } }))
    }

    /// The hook around a shell command, with empty output after it.
    fn shell(&self, event: &str, command: &str) -> Value {
        let mut input = json!({ "tool_name": "Bash", "tool_input": { "command": command } });
        if event == "PostToolUse" {
            input["tool_response"] = json!({ "stdout": "", "stderr": "" });
        }
        self.hook(event, input)
    }

    fn statusline(&self) -> String {
        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "workspace": { "current_dir": self.repo().to_string_lossy(), "project_dir": self.repo().to_string_lossy() } });
        let (code, out, err) = self.run(&["statusline"], &input.to_string());
        assert_eq!((code, err.as_str()), (0, ""));
        // the status line shows the ask and its parts; CI is not shown here
        assert!(!out.contains("CI"), "{out}");
        out
    }

    fn plan_file(&self) -> String {
        self.read(".familiar/plan")
    }
}

fn allowed(out: &Value) -> bool {
    out.get("decision").is_none() && out["hookSpecificOutput"]["permissionDecision"].is_null()
}

#[test]
fn prose_support_files_and_comments_go_through_and_open_no_run() {
    let p = project("prose");
    p.hook("UserPromptSubmit", json!({ "prompt": "tidy the docs and the comments" }));
    for out in [
        p.edit("README.md", "It greets.", "It greets the person by name."),
        p.write_tool("CLAUDE.md", "# notes for the agent\n"),
        p.write_tool(".claude/skills/greet/SKILL.md", "---\nname: greet\n---\n"),
        p.write_tool(".claude/settings.json", "{}\n"),
        p.write_tool("docs/diagram.svg", "<svg/>\n"),
        p.write_tool("LICENSE", "MIT\n"),
        p.edit("src/app.rs", "// greets the person", "// greets the person, politely"),
        p.edit("src/tool.py", "# a helper", "# a helper for the greeting"),
        p.edit("notes/plan.py", "x = 1", "x = 2"),
        p.shell("PreToolUse", "cat > docs/usage.md << 'EOF'\nrun it\nEOF"),
    ] {
        assert!(allowed(&out), "passive: {out}");
    }
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
    assert!(!p.plan_file().contains("intent_submitted"), "{}", p.plan_file());
}

#[test]
fn a_changed_line_of_code_is_not_passive_and_is_named() {
    let p = project("code");
    p.hook("UserPromptSubmit", json!({ "prompt": "say hello" }));
    let out = p.edit("src/app.rs", "\"hi\"", "\"hello\"");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "a string literal is code: {out}");
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: the plan comes first · line 1: the intent · src/app.rs is code\n  line 3 is code: \"hello\"\n"), "{reason}");
    // a comment and a line of code together: not passive
    let out = p.edit("src/app.rs", "// greets the person\nfn greet()", "// greets\nfn greet_the_person()");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
}

#[test]
fn a_commit_of_prose_and_comments_goes_and_stays_idle() {
    let p = project("commit");
    p.hook("UserPromptSubmit", json!({ "prompt": "fix the typo in the readme" }));
    p.write("README.md", "# app\n\nIt greets, politely.\n");
    p.write("src/app.rs", &RUST.replace("// greets the person", "// greets the person, politely"));
    assert!(allowed(&p.shell("PreToolUse", "git commit -am 'docs: politely'")));
    p.git(&["commit", "-qam", "docs: politely"]);
    let out = p.shell("PostToolUse", "git commit -am 'docs: politely'");
    assert!(allowed(&out), "a commit of prose and comments goes: {out}");
    assert!(allowed(&p.hook("Stop", json!({})))); 
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
}
