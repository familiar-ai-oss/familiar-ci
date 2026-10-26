//! Detect a project that was set up and then lost its `.familiar` folder, plan
//! file or record, and refuse every hook call until `familiar-ai setup` runs
//! again. See docs/decisions.md#a-removed-folder-refuses-until-setup.

use std::path::Path;

use crate::decide::decision::Decision;
use crate::door::{marker, setup, FAMILIAR_FOLDER, PLAN_FILE, RECORD_FILE};
use crate::input::hook_input::{HookEvent, HookInput};

/// The one line every hook call is told or refused with.
pub const REMOVED: &str = "familiar-ai: this project was set up, and its .familiar folder, plan file or record was removed · next: `familiar-ai setup` (or /familiar-ai:setup) restores the folder, brings the plan back from git and starts the record from it; checking resumes then";

/// True when the project was set up but its folder, plan file or record is
/// missing. The marker counts only when the whole folder is gone, since an
/// empty folder is a fresh setup.
pub fn plan_removed(root: &Path, data_dir: Option<&Path>) -> bool {
    let folder = root.join(FAMILIAR_FOLDER);
    let plan = folder.join(PLAN_FILE);
    let folder_there = folder.is_dir();
    if folder_there && plan.is_file() && folder.join(RECORD_FILE).is_file() {
        return false;
    }
    let plan_on_disk = std::fs::metadata(&plan).is_ok_and(|m| m.is_file() && m.len() > 0);
    if plan_on_disk || plan_at_head(root).is_some() {
        return true;
    }
    !folder_there && marker::present(data_dir, root)
}

/// The plan's bytes at HEAD, if it is tracked and git can be read.
pub fn plan_at_head(root: &Path) -> Option<Vec<u8>> {
    at_head(root, crate::PLAN_FILE)
}

/// The bytes at HEAD of a file of the folder (`plan`, `adapter`), if it is tracked and git can be read.
pub fn at_head(root: &Path, name: &str) -> Option<Vec<u8>> {
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten()?;
    let (_, plan_rel) = setup::paths_in_repository(root, &worktree).ok()?;
    let rel = format!("{}{name}", plan_rel.strip_suffix(crate::PLAN_FILE).unwrap_or_default());
    let head = familiar_ai_git::head_commit(&worktree).ok().flatten()?;
    familiar_ai_git::file_at(&worktree, &head, &rel).ok()
}

/// True for the exact `familiar-ai setup` call, the one let through.
pub fn is_setup_call(input: &HookInput) -> bool {
    input.event == HookEvent::PreToolUse
        && input.file_path().is_none()
        && input.command().map(str::trim) == Some("familiar-ai setup")
}

/// While the folder is missing, let setup through, refuse other tool calls and
/// stops, and tell the rest. A prompt or a finished tool call is told rather
/// than blocked, since blocking a prompt would lock the person out.
pub fn decide(input: &HookInput) -> Decision {
    if is_setup_call(input) {
        return Decision::allow();
    }
    match input.event {
        HookEvent::PreToolUse | HookEvent::Stop | HookEvent::SubagentStop => Decision::Refuse { reason: REMOVED.to_string() },
        HookEvent::SessionStart
        | HookEvent::UserPromptSubmit
        | HookEvent::PostToolUse
        | HookEvent::PostToolUseFailure
        | HookEvent::SubagentStart
        | HookEvent::PreModelSwitch
        | HookEvent::PostModelSwitch => {
            Decision::Allow { told: Some(REMOVED.to_string()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(json: &str, event: HookEvent) -> HookInput {
        HookInput::parse(json, event).expect("parse")
    }

    #[test]
    fn the_setup_call_alone_goes_ahead_a_tool_call_is_refused_a_stop_blocked_the_rest_told() {
        let setup = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"familiar-ai setup"}}"#, HookEvent::PreToolUse);
        assert_eq!(decide(&setup), Decision::allow());
        for cmd in ["familiar-ai setup; rm -rf x", "familiar-ai setup && ls", "sudo familiar-ai setup", "ls"] {
            let i = input(&format!(r#"{{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#), HookEvent::PreToolUse);
            assert_eq!(decide(&i), Decision::Refuse { reason: REMOVED.to_string() }, "{cmd}");
        }
        let edit = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"/w/a.py"}}"#, HookEvent::PreToolUse);
        assert_eq!(decide(&edit), Decision::Refuse { reason: REMOVED.to_string() });
        for (name, ev) in [("Stop", HookEvent::Stop), ("SubagentStop", HookEvent::SubagentStop)] {
            let i = input(&format!(r#"{{"hook_event_name":"{name}"}}"#), ev);
            assert_eq!(decide(&i), Decision::Refuse { reason: REMOVED.to_string() }, "{name}");
        }
        for (name, ev) in [("SessionStart", HookEvent::SessionStart), ("UserPromptSubmit", HookEvent::UserPromptSubmit), ("PostToolUse", HookEvent::PostToolUse)] {
            let i = input(&format!(r#"{{"hook_event_name":"{name}"}}"#), ev);
            assert_eq!(decide(&i), Decision::Allow { told: Some(REMOVED.to_string()) }, "{name}");
        }
        assert_eq!(REMOVED.lines().count(), 1);
        assert!(REMOVED.contains("familiar-ai setup") && REMOVED.contains("removed"));
    }

    #[test]
    fn outside_any_repository_nothing_is_removed() {
        assert!(!plan_removed(&std::env::temp_dir(), None));
        assert!(plan_at_head(&std::env::temp_dir()).is_none());
    }

    #[test]
    fn the_marker_makes_a_missing_folder_a_removal_but_not_an_empty_one() {
        let root = std::env::temp_dir().join(format!("familiar-ai-marked-{}", std::process::id()));
        let data = std::env::temp_dir().join(format!("familiar-ai-marked-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&root).expect("root");
        assert!(!plan_removed(&root, Some(&data)), "no marker, no folder: never set up");
        marker::write(Some(&data), &root, 1.0).expect("marker");
        assert!(plan_removed(&root, Some(&data)), "marked, the folder gone: a removal");
        assert!(!plan_removed(&root, None), "no home known: the marker is not read, nothing refuses");
        let folder = root.join(FAMILIAR_FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        assert!(!plan_removed(&root, Some(&data)), "marked, the folder there and empty: a fresh setup");
        std::fs::write(folder.join(PLAN_FILE), "").expect("plan");
        std::fs::write(folder.join(RECORD_FILE), "").expect("record");
        assert!(!plan_removed(&root, Some(&data)), "all three present");
        std::fs::remove_dir_all(&folder).expect("rm -rf");
        assert!(plan_removed(&root, Some(&data)), "removed whole again");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_non_empty_plan_on_disk_with_the_record_missing_is_a_removal_an_empty_one_is_not() {
        let root = std::env::temp_dir().join(format!("familiar-ai-removed-{}", std::process::id()));
        let folder = root.join(FAMILIAR_FOLDER);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&folder).expect("folder");
        assert!(!plan_removed(&root, None), "fresh setup: folder alone");
        std::fs::write(folder.join(PLAN_FILE), "").expect("plan");
        assert!(!plan_removed(&root, None), "an empty plan says nothing was claimed");
        std::fs::write(folder.join(PLAN_FILE), "{\"kind\":\"intent_submitted\"}\n").expect("plan");
        assert!(plan_removed(&root, None), "claims on disk, no record");
        std::fs::write(folder.join(RECORD_FILE), "").expect("record");
        assert!(!plan_removed(&root, None), "all three present");
        std::fs::remove_file(folder.join(PLAN_FILE)).expect("rm plan");
        assert!(!plan_removed(&root, None), "no plan anywhere: never set up");
        let _ = std::fs::remove_dir_all(&root);
    }
}
