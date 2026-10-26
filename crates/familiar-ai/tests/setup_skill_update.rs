//! The setup skill pins one version, the crate's and the plugin's; when the
//! installed binary answers with another, its first step offers the update
//! with the person's consent.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The value of the first `version = "..."` line in a TOML section, or of the
/// first such line anywhere when `section` is `None`.
fn toml_version(text: &str, section: Option<&str>) -> Option<String> {
    let mut in_section = section.is_none();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = section.is_none_or(|s| line == s);
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// The crate's version as `crates/familiar-ai/Cargo.toml` gives it, following
/// `version.workspace = true` to the workspace's `Cargo.toml`.
fn crate_version() -> String {
    let manifest = read("crates/familiar-ai/Cargo.toml");
    if manifest.lines().any(|l| l.trim().replace(' ', "") == "version.workspace=true") {
        toml_version(&read("Cargo.toml"), Some("[workspace.package]"))
            .expect("the workspace Cargo.toml has a [workspace.package] version")
    } else {
        toml_version(&manifest, Some("[package]")).expect("crates/familiar-ai/Cargo.toml has a version")
    }
}

fn plugin_version() -> String {
    let manifest = read("plugin/.claude-plugin/plugin.json");
    let value: serde_json::Value = serde_json::from_str(&manifest).expect("plugin.json is JSON");
    value["version"].as_str().expect("plugin.json has a version").to_string()
}

const PIN: &str = "The version this plugin is built for is `";

fn skill() -> String {
    read("plugin/skills/setup/SKILL.md")
}

/// Every word in the text that looks like a version: starts with a digit and
/// has at least two dots, such as `0.1.0-dev`.
fn versions_in(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .map(|w| w.trim_end_matches(['.', '-']))
        .filter(|w| w.starts_with(|c: char| c.is_ascii_digit()) && w.matches('.').count() >= 2)
        .map(str::to_string)
        .collect()
}

#[test]
fn the_setup_skill_pins_one_version_equal_to_the_crate_and_the_plugin() {
    let skill = skill();
    let start = skill.find(PIN).unwrap_or_else(|| panic!("the setup skill has no line `{PIN}<version>`"));
    let rest = &skill[start + PIN.len()..];
    let pinned = &rest[..rest.find('`').expect("the pinned version is closed by a backquote")];

    assert_eq!(pinned, crate_version(), "the setup skill pins {pinned}, the crate is another version");
    assert_eq!(pinned, plugin_version(), "the setup skill pins {pinned}, plugin.json is another version");
    assert_eq!(pinned, env!("CARGO_PKG_VERSION"));

    let others: Vec<String> = versions_in(&skill).into_iter().filter(|v| v != pinned).collect();
    assert!(others.is_empty(), "the setup skill names versions other than the pinned one: {others:?}");
}

#[test]
fn the_setup_skills_first_step_offers_the_update_with_consent() {
    let skill = skill();
    let steps = skill.find("## Steps").expect("the setup skill has a Steps section");
    let step_one = skill.find("1. **Work out the machine.**").expect("the setup skill has step 1");
    let first = &skill[steps..step_one];

    assert!(
        first.contains("is installed; this plugin is built for"),
        "the first step does not tell the person the installed version is another one: {first}"
    );
    let lower = first.to_lowercase();
    assert!(
        lower.contains("only after they agree") || lower.contains("consent"),
        "the first step does not ask before the update: {first}"
    );
    assert!(first.contains("go straight to step 8"), "the first step no longer skips to step 8 when the version matches: {first}");
}

/// Step 8 of the skill, up to step 9.
fn step_8(skill: &str) -> String {
    let start = skill.find("8. **Set the project up.**").expect("the setup skill has step 8");
    let end = skill.find("9. **Done.**").expect("the setup skill has step 9");
    skill[start..end].to_string()
}

#[test]
fn step_8_says_the_dry_run_line_that_the_adapter_is_committed_and_offers_sharing_once() {
    // plan A (lane a-surf): the dry run's line, the adapter beside the plan, and A-6's one-time opt-in
    let s = step_8(&skill());
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    for want in [
        "the dry run for <runner> started (run d-…); its choice of how the runner is read goes in .familiar/adapter",
        "`.familiar/adapter` is committed with the work, like the plan",
        "familiar-ai share on",
        "familiar-ai share off",
        "in a terminal of their own",
        "Familiar's own probe output, never the person's code",
        "Ask the person once",
        ".familiar/runs/gaps",
    ] {
        assert!(flat.contains(want), "step 8 names {want:?}:\n{s}");
    }
    assert!(flat.contains("never run `familiar-ai share on` yourself"), "the agent never gives the yes: {s}");
}
