//! Sharing gap recordings is the person's one-time yes (plan A-6, 2026-10-19 [draft]):
//! `familiar-ai share on` and `share off` are typed in the person's own terminal and kept
//! in the machine's marker folder, once for the machine and in no project; from an
//! agent's shell, or by a command the hook reads before it runs, they are refused, as
//! `on` and `off` are, whatever the command does to its environment.

mod common;

use common::Project;
use serde_json::Value;

const SESSION: &str = "7a2e9c41-3b5d-4e86-9f10-a4c2d8e6b753";

const YES_WORDS: &str = "Gap recordings will be shared. A recording is Familiar's own probe output, never your code. Until an endpoint is named they wait in each project's .familiar/runs/gaps. familiar-ai share off takes this back.\n";
const NO_WORDS: &str = "Gap recordings will not be shared. They stay in each project's .familiar/runs/gaps. familiar-ai share on gives the yes again.\n";
const NO_HOME: &str = "familiar-ai: not kept · no data folder is known to keep the choice in (set FAMILIAR_AI_HOME, XDG_DATA_HOME or HOME)\n";

/// The binary runs from the person's terminal unless a test names the agent's session.
fn project(name: &str) -> Project {
    Project::builder(name).no_session().no_app().create()
}

fn refusal(to: &str) -> String {
    format!("familiar-ai: not changed · familiar-ai share {to} is the person's choice, not the agent's: the person makes it by running familiar-ai share {to} in a terminal of their own")
}

fn reason(v: &Value) -> String {
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{v}");
    v["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason").to_string()
}

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

/// The machine's choice: a file in the binary's home, beside the project markers.
fn kept(p: &Project) -> Option<String> {
    std::fs::read_to_string(p.home().join("share")).ok()
}

#[test]
fn a_yes_from_the_persons_terminal_is_kept_for_the_machine_and_can_be_taken_back() {
    let p = project("yes");
    assert_eq!(kept(&p), None, "nothing is kept before the person says");

    let (code, stdout, stderr) = p.run(&["share", "on"], "");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, YES_WORDS);
    assert_eq!(stderr, "");
    let text = kept(&p).expect("the choice is in the machine's marker folder");
    assert!(text.starts_with("share: on\nat: "), "{text}");
    // the same yes again is still a yes
    assert_eq!(p.run(&["share", "on"], "").0, 0);
    assert!(kept(&p).expect("kept").starts_with("share: on\n"));

    let (code, stdout, stderr) = p.run(&["share", "off"], "");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, NO_WORDS);
    assert!(kept(&p).expect("kept").starts_with("share: off\nat: "));

    // the choice is the machine's: nothing of it is in the project
    assert!(!p.path(".familiar/share").exists());
    assert!(!p.read(".familiar/plan").contains("share"));
}

#[test]
fn the_choice_needs_no_project() {
    let p = project("anywhere");
    let outside = p.dir.join("not-a-project");
    std::fs::create_dir_all(&outside).expect("a folder outside every repository");
    let (code, stdout, stderr) = p.run_in(&outside, &["share", "on"], "");
    assert_eq!((code, stdout.as_str(), stderr.as_str()), (0, YES_WORDS, ""));
    assert!(kept(&p).expect("kept").starts_with("share: on\n"));
}

#[test]
fn with_no_folder_to_keep_it_in_the_yes_is_refused_not_lost() {
    let p = project("nohome");
    for name in ["FAMILIAR_AI_HOME", "XDG_DATA_HOME", "HOME"] {
        p.set_env(name, None);
    }
    let (code, stdout, stderr) = p.run(&["share", "on"], "");
    assert_eq!((code, stdout.as_str(), stderr.as_str()), (1, "", NO_HOME));
    assert_eq!(kept(&p), None);
}

#[test]
fn an_agent_cannot_give_the_yes_or_take_it_back() {
    let p = project("agent");
    for to in ["on", "off"] {
        let (code, stdout, stderr) = p.run_as(Some(SESSION), &["share", to], "");
        assert_eq!((code, stdout.as_str()), (1, ""), "{to}");
        assert_eq!(stderr, format!("{}\n", refusal(to)), "{to}");
    }
    assert_eq!(kept(&p), None, "an agent's try wrote nothing");

    // a session the binary launched (the investigator) is an agent too
    let mut cmd = p.command_as(None);
    cmd.env("FAMILIAR_AI_CONVERSATION_ID", SESSION).args(["share", "on"]);
    let (code, stdout, stderr) = Project::output(cmd, "");
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert_eq!(stderr, format!("{}\n", refusal("on")));
    assert_eq!(kept(&p), None);

    // the person's yes stands against an agent's try to take it back
    assert_eq!(p.run(&["share", "on"], "").0, 0);
    assert_eq!(p.run_as(Some(SESSION), &["share", "off"], "").0, 1);
    assert!(kept(&p).expect("kept").starts_with("share: on\n"));
}

#[test]
fn an_agents_share_command_is_refused_whatever_its_environment_even_while_off() {
    let p = Project::new("hook");
    for (cmd, to) in [
        ("familiar-ai share on", "on"),
        ("familiar-ai share off", "off"),
        ("env -u CLAUDE_CODE_SESSION_ID familiar-ai share on", "on"),
        ("CLAUDE_CODE_SESSION_ID= ./target/debug/familiar-ai share on", "on"),
        ("cd /tmp && env -i PATH=$PATH familiar-ai share off", "off"),
        ("sh -c 'familiar-ai share on'", "on"),
    ] {
        assert_eq!(reason(&p.bash("PreToolUse", cmd)), refusal(to), "{cmd}");
    }
    // a read, the words in text, and the command with nothing to switch go ahead
    for cmd in ["familiar-ai status", "grep -n 'familiar-ai share on' docs/reference.md", "echo familiar-ai share on", "familiar-ai share"] {
        assert!(!refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }

    // switched off by the person, sharing is still not the agent's choice
    assert_eq!(p.run_as(None, &["off"], "").0, 0, "the person's own terminal passes no hook");
    assert_eq!(reason(&p.bash("PreToolUse", "familiar-ai share on")), refusal("on"));
    assert_eq!(p.run_as(None, &["on"], "").0, 0, "the person's own terminal passes no hook");
}
