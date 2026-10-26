//! An agent's shell command is read by its text before it runs: `claude -p`
//! starts a lane by hand, so it is refused naming `familiar-ai launch_lane`;
//! `familiar-ai on`, `off` and `claim hold_answered` are the person's, so
//! they are refused whatever the command does to its environment. A person's
//! own terminal passes through no hook and switches as before.

mod common;

use common::Project;
use serde_json::{json, Value};

const LANE: &str = "familiar-ai: a lane is started with familiar-ai launch_lane, so the person's words travel with it and one session runs per worktree · next: familiar-ai launch_lane --brief <the brief's file>";
const CARD: &str = "familiar-ai: not claimed yet · this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick";

fn switch(to: &str) -> String {
    format!("familiar-ai: not switched · familiar-ai {to} is the person's switch, not the agent's: the person turns it {to} with the button or by running familiar-ai {to} in a terminal of their own")
}

fn reason(v: &Value) -> String {
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{v}");
    v["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason").to_string()
}

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

#[test]
fn an_agents_claude_print_is_refused_naming_launch_lane() {
    let p = Project::new("print");
    for cmd in [
        "claude -p \"build the greeting\"",
        "cd ../familiar-ai-oss-lane && env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID claude -p \"$(cat brief)\" --model opus",
        "claude --print 'x' --output-format json",
        "nohup /usr/local/bin/claude --model sonnet -p 'x' > lane.log 2>&1 &",
        "sh -c 'claude -p hello'",
    ] {
        assert_eq!(reason(&p.bash("PreToolUse", cmd)), LANE, "{cmd}");
    }
    // the words claude and -p in text, claude without -p, and launch_lane itself go ahead
    for cmd in ["echo 'claude -p is refused'", "git log --grep 'claude -p'", "claude --version", "familiar-ai launch_lane --brief lane.brief.md", "ls -p"] {
        assert!(!refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}

#[test]
fn an_agents_switch_is_refused_whatever_its_environment() {
    let p = Project::new("switch");
    for (cmd, to) in [
        ("familiar-ai off", "off"),
        ("env -u CLAUDE_CODE_SESSION_ID familiar-ai off", "off"),
        ("env -u CLAUDE_CODE_SESSION_ID familiar-ai on", "on"),
        ("CLAUDE_CODE_SESSION_ID= ./target/debug/familiar-ai on", "on"),
        ("cd /tmp && env -i PATH=$PATH familiar-ai off", "off"),
    ] {
        assert_eq!(reason(&p.bash("PreToolUse", cmd)), switch(to), "{cmd}");
    }
    assert!(!refused(&p.bash("PreToolUse", "familiar-ai status")));
    assert!(!refused(&p.bash("PreToolUse", "grep -n 'familiar-ai on' docs/reference.md")));

    // switched off by the person, the agent still cannot switch it back on
    assert_eq!(p.run_as(None, &["off"], "").0, 0, "the person's own terminal passes no hook");
    assert_eq!(reason(&p.bash("PreToolUse", "env -u CLAUDE_CODE_SESSION_ID familiar-ai on")), switch("on"));
    assert_eq!(p.run_as(None, &["on"], "").0, 0, "the person's own terminal passes no hook");
}

#[test]
fn an_agents_card_answer_is_refused_whatever_its_environment() {
    let p = Project::new("card");
    for cmd in [
        "familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''",
        "env -u CLAUDE_CODE_SESSION_ID familiar-ai claim hold_answered --criterion_id c1 --hold two_failures --choice release --typed ''",
        "echo '{\"criterion_id\":\"c1\",\"hold\":\"anchor_moved\",\"choice\":\"re_plan\",\"typed\":\"\"}' | env -u CLAUDE_CODE_SESSION_ID familiar-ai claim hold_answered",
        "env -u CLAUDE_CODE_SESSION_ID familiar-ai plan <<'JSON'\n[{\"kind\":\"hold_answered\",\"criterion_id\":\"c1\",\"hold\":\"anchor_moved\",\"choice\":\"keep_going\",\"typed\":\"\"}]\nJSON",
    ] {
        let r = reason(&p.bash("PreToolUse", cmd));
        assert!(r.starts_with(CARD), "{cmd}: {r}");
    }
    assert!(!refused(&p.bash("PreToolUse", "familiar-ai claim intent_submitted --for 0 --text 'greet with hello'")));
    // a session the binary launched is an agent too
    let launched = json!({ "tool_name": "Bash", "tool_input": { "command": "env -u CLAUDE_CODE_SESSION_ID familiar-ai off" } });
    let mut cmd = p.command();
    cmd.env("FAMILIAR_AI_CONVERSATION_ID", common::project::DEFAULT_SESSION).args(["hook", "PreToolUse"]);
    let (code, out, err) = Project::output(cmd, &p.payload("PreToolUse", launched).to_string());
    assert_eq!(code, 0, "{err}");
    assert_eq!(reason(&serde_json::from_str(out.trim()).expect("json")), switch("off"));
}
