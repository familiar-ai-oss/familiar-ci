//! Light is for documentation only (the owner, 2026-10-11): "a flag for light
//! that is only allowed if the anchor line is clearly documentation, a
//! markdown, or has the standard lint attributes of a docstring." The binary
//! reads the anchored lines at the record's tip sha, as it does for line 5,
//! and writes `anchor_is_documentation`; plan line 2 reads it. Everything
//! else is the full path (line 3).
//!
//!   light + README.md lines          → matched true  → line 2 holds
//!   light + a Python function        → matched false → refused, claim it --path full
//!   light + the function's docstring → matched true
//!   light + Rust `///`, JS `/** */`  → matched true
//!   full                             → nothing asked

mod common;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use familiar_ai_rules::{plan, View};
use serde_json::{json, Value};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";
const WHY: &str = "its path is light but its anchor is not documentation: claim it --path full";
const NOTE: &str = "(--path light only when the anchor is documentation: a markdown file or a docstring; the binary checks it; everything else is --path full and also needs line 3)";

const README: &str = "# Greet\n\nUsage: run greet with a name.\n";
const APP_PY: &str = "def greet(name):\n    \"\"\"Return the greeting for a name.\"\"\"\n    return \"hello \" + name\n";
const LIB_RS: &str = "/// Greets a name.\n/// Twice over.\npub fn greet() {}\n";
const APP_JS: &str = "/**\n * Greets a name.\n */\nfunction greet() {}\n";

struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(name: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("familiar-ai-doc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).expect("temp project");
        let p = Project { dir };
        p.git(&["init", "-q", "-b", "main"]);
        for (f, body) in [("README.md", README), ("src/app.py", APP_PY), ("src/lib.rs", LIB_RS), ("src/app.js", APP_JS)] {
            std::fs::write(p.dir.join(f), body).expect("write");
        }
        assert_eq!(p.run(&["setup"], None).0, 0);
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "base"]);
        p
    }

    /// The cases card passed for `c` (plan cases-card, 2026-10-25) [draft names].
    fn cases_kept(&self, c: &str) {
        common::cases_kept_at(&self.dir, c);
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(&self.dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_familiar-ai"));
        cmd.env("FAMILIAR_AI_JUDGE", "off")
            .env("FAMILIAR_AI_INVESTIGATOR", "off")
            .env("FAMILIAR_AI_HOME", std::env::temp_dir().join(format!("familiar-ai-doc-home-{}", std::process::id())))
            .env("PATH", "/usr/bin:/bin")
            .env("CLAUDE_CODE_SESSION_ID", SESSION)
            .env_remove("CLAUDE_PROJECT_DIR")
            .current_dir(&self.dir);
        cmd
    }

    /// The binary, as this project's session; `stdin` when given.
    fn run(&self, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
        let mut cmd = self.command();
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().expect("run familiar-ai");
        child.stdin.take().expect("stdin").write_all(stdin.unwrap_or("").as_bytes()).expect("write stdin");
        let out = child.wait_with_output().expect("wait");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }

    fn claim(&self, args: &[&str]) -> String {
        let mut all = vec!["claim"];
        all.extend_from_slice(args);
        let (code, out, err) = self.run(&all, None);
        assert_eq!(code, 0, "claim {args:?}: {out}{err}");
        out
    }

    fn hook(&self, event: &str, tool: &str, tool_input: Value) -> String {
        let payload = json!({
            "hook_event_name": event,
            "session_id": SESSION,
            "cwd": self.dir.to_string_lossy(),
            "tool_name": tool,
            "tool_input": tool_input,
            "source": "startup",
        });
        let (_, out, err) = self.run(&["hook", event], Some(&payload.to_string()));
        format!("{out}{err}")
    }

    fn log(&self) -> familiar_ai_facts::Log {
        let record = familiar_ai_facts::Record::open_waiting(&self.dir.join(".familiar/record"), std::time::Duration::from_secs(5)).expect("record");
        record.read_all().expect("read")
    }

    /// The binary's documentation checks, as (file_path, matched).
    fn doc_checks(&self) -> Vec<(String, bool)> {
        self.log()
            .iter()
            .filter(|f| f.kind == "anchor_is_documentation")
            .map(|f| (f.fields["file_path"].as_str().unwrap_or("").to_string(), f.fields["matched"] == json!(true)))
            .collect()
    }

    /// The intent, a criterion c1 on `path`, and its anchor on `file` lines `a..=b`.
    fn anchored(&self, path: &str, file: &str, symbol: &str, a: u64, b: u64) -> String {
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", path]);
        let places = json!([{ "file_path": file, "symbol": symbol, "line_start": a, "line_end": b }]).to_string();
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", &places])
    }

    /// Lines 6 to 11.
    fn the_rest(&self) {
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_light_criterion_on_a_readme_is_documentation_and_needs_no_line_3() {
    let p = Project::new("readme");
    p.anchored("light", "README.md", "Usage", 1, 3);
    assert_eq!(p.doc_checks(), vec![("README.md".to_string(), true)], "written after the anchor claim");
    let log = p.log();
    assert!(plan::line_2(&View::new(&log)));
    p.the_rest();
    let log = p.log();
    assert!(plan::planning_confirm_complete(&View::new(&log)), "confirmed with no behavior and no promise");
    assert!(!log.iter().any(|f| f.kind == "behavior" || f.kind == "promise"));
}

#[test]
fn a_light_criterion_on_a_python_function_is_refused_and_taught_the_full_path() {
    let p = Project::new("pyfn");
    let out = p.anchored("light", "src/app.py", "greet", 1, 3);
    assert_eq!(p.doc_checks(), vec![("src/app.py".to_string(), false)]);
    assert!(out.contains(WHY), "the claim's answer names line 2: {out}");
    assert!(out.contains("familiar-ai claim criterion --criterion_id c1") && out.contains("--path full"), "the re-claim: {out}");
    p.the_rest();
    let log = p.log();
    assert!(!plan::line_2(&View::new(&log)), "line 2 is missing");
    assert!(!plan::planning_confirm_complete(&View::new(&log)));
    let refused = p.hook("PreToolUse", "Edit", json!({ "file_path": p.dir.join("src/app.py").to_string_lossy() }));
    assert!(refused.contains("\"permissionDecision\":\"deny\"") && refused.contains(&format!("next: {WHY}")), "{refused}");
    assert!(refused.contains("claim it: familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full"), "{refused}");
    // the cure: claimed again --path full, with line 3
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "an empty name", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    assert!(plan::planning_confirm_complete(&View::new(&p.log())), "full: confirmed");
}

#[test]
fn a_python_docstring_is_documentation() {
    let p = Project::new("pydoc");
    p.anchored("light", "src/app.py", "greeting", 2, 2);
    assert_eq!(p.doc_checks(), vec![("src/app.py".to_string(), true)]);
    assert!(plan::line_2(&View::new(&p.log())));
}

#[test]
fn rust_doc_lines_and_a_jsdoc_block_are_documentation() {
    let p = Project::new("rs");
    p.anchored("light", "src/lib.rs", "Greets", 1, 2);
    assert_eq!(p.doc_checks(), vec![("src/lib.rs".to_string(), true)]);
    let p = Project::new("js");
    p.anchored("light", "src/app.js", "Greets", 1, 3);
    assert_eq!(p.doc_checks(), vec![("src/app.js".to_string(), true)]);
    // the code under the doc is not
    let p = Project::new("rscode");
    p.anchored("light", "src/lib.rs", "greet", 1, 3);
    assert_eq!(p.doc_checks(), vec![("src/lib.rs".to_string(), false)]);
}

#[test]
fn a_full_criterion_asks_nothing_new() {
    let p = Project::new("full");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "an empty name", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    let places = json!([{ "file_path": "src/app.py", "symbol": "greet", "line_start": 1, "line_end": 3 }]).to_string();
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", &places]);
    p.the_rest();
    assert!(p.doc_checks().is_empty(), "no documentation check for a full criterion");
    assert!(plan::planning_confirm_complete(&View::new(&p.log())));
}

#[test]
fn the_session_start_carries_the_owners_line_2_note() {
    let p = Project::new("start");
    let out = p.hook("SessionStart", "", json!({}));
    assert!(out.contains(NOTE), "{out}");
    assert!(!out.contains("a bug fix or a small change is the light path"), "{out}");
}
