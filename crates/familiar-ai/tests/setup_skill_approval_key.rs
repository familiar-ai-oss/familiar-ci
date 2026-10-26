//! The setup skill offers the approval key as an optional step (plan §6.9,
//! S9d): the person runs the two lines in a terminal of their own, the agent
//! never runs ssh-keygen, the key never goes into ssh-agent and is never made
//! without a passphrase; it says what the key buys and what happens without
//! it. docs/decisions.md says why the passphrase is the presence.

use std::path::{Path, PathBuf};

fn read(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The section of the skill under the heading, up to the next `## `.
fn section(text: &str, heading: &str) -> String {
    let start = text.find(heading).unwrap_or_else(|| panic!("no {heading:?} in the skill"));
    let rest = &text[start + heading.len()..];
    rest[..rest.find("\n## ").unwrap_or(rest.len())].to_string()
}

#[test]
fn the_setup_skill_offers_the_approval_key_with_its_two_lines() {
    let s = section(&read("plugin/skills/setup/SKILL.md"), "## Optional: the approval key");
    for want in [
        "ssh-keygen -t ed25519 -f ~/.ssh/familiar_ai_approve",
        "FAMILIAR_AI_ALLOWED_SIGNERS",
        "~/.ssh/familiar_ai_approve.pub",
        "in a terminal of their own",
        "never run `ssh-keygen` yourself",
        "never through `!`",
        "never with `-N`",
        "never added to `ssh-agent`",
        "CI honors their card answers",
        "approval not verified",
        "Ask the person whether they want it",
    ] {
        assert!(s.contains(want), "the section names {want:?}:\n{s}");
    }
}

#[test]
fn the_decisions_say_why_the_passphrase_is_the_presence() {
    let d = read("docs/decisions.md");
    let s = section(&d, "### The passphrase is the presence");
    for want in ["familiar-ai approve", "stdin", "ssh-agent", "FAMILIAR_AI_ALLOWED_SIGNERS", "approval not verified", "familiar-ai-approval"] {
        assert!(s.contains(want), "{want:?}:\n{s}");
    }
}
