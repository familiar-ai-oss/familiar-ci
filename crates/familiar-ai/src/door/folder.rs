//! Refuse a Bash command that removes, moves, empties or overwrites the
//! `.familiar` folder. It reads only the command's text, so it turns back the
//! obvious move; the marker outside the repository is what holds once the
//! folder is gone.

use std::path::Path;

use crate::input::hook_input::{HookEvent, HookInput};
use crate::{FAMILIAR_FOLDER, PLAN_FILE, RECORD_FILE};

/// The line a removing command is refused with.
pub const NOT_YOURS: &str = "familiar-ai: Familiar's folder stays as it is: it is the person's to remove, not the agent's";

/// The refusal for a Bash call that removes the folder, when there is one to protect.
pub fn removal_refusal(input: &HookInput, root: &Path) -> Option<String> {
    if input.event != HookEvent::PreToolUse || input.file_path().is_some() {
        return None;
    }
    let command = input.command()?;
    if !root.join(FAMILIAR_FOLDER).is_dir() {
        return None;
    }
    removes_folder(command, || has_uncommitted_content(root)).then(|| NOT_YOURS.to_string())
}

/// True when the command removes, moves, empties or overwrites the folder.
/// `git reset --hard` and `git clean -x` remove it without naming it, so they
/// are refused only when `uncommitted` says there is something to lose.
pub fn removes_folder(command: &str, uncommitted: impl Fn() -> bool) -> bool {
    let words = words_of(command);
    let names = words.iter().any(|w| names_folder(w));
    if names && words.iter().any(|w| matches!(w.as_str(), "rm" | "rmdir" | "unlink" | "mv" | "truncate")) {
        return true;
    }
    if redirects_into(&words) {
        return true;
    }
    let mut i = 0;
    while i < words.len() {
        if words[i] == "git" {
            if let Some((sub, at)) = git_subcommand(&words, i) {
                let rest = &words[at + 1..];
                let next_command = rest.iter().position(|w| is_separator(w)).unwrap_or(rest.len());
                let args = &rest[..next_command];
                match sub {
                    "clean" if names => return true,
                    "clean"
                        if (args.iter().any(|a| a.starts_with('-') && !a.starts_with("--") && a.contains(['x', 'X'])) || args.iter().any(|a| a == "--exclude" || a.starts_with("--exclude=")))
                            && uncommitted() =>
                    {
                        return true;
                    }
                    "checkout" if names && args.iter().any(|a| a == "--") => return true,
                    "restore" if names => return true,
                    "reset" if args.iter().any(|a| a == "--hard") && uncommitted() => return true,
                    _ => {}
                }
                i = at;
            }
        }
        i += 1;
    }
    false
}

/// True when the folder holds a non-empty plan or adapter unlike HEAD's, or a non-empty record.
pub fn has_uncommitted_content(root: &Path) -> bool {
    let folder = root.join(FAMILIAR_FOLDER);
    let non_empty = |name: &str| std::fs::metadata(folder.join(name)).is_ok_and(|m| m.is_file() && m.len() > 0);
    if non_empty(RECORD_FILE) {
        return true;
    }
    // the committed mirrors: each one on disk unlike HEAD's has something to lose
    [PLAN_FILE, crate::door::ADAPTER_FILE].iter().any(|name| {
        non_empty(name) && {
            let on_disk = std::fs::read(folder.join(name)).unwrap_or_default();
            crate::door::set_up::at_head(root, name).as_deref() != Some(on_disk.as_slice())
        }
    })
}

/// Split the command into words, with shell operators as their own words and
/// end quotes stripped.
fn words_of(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    // A line ends a command, so each line break is a word of its own.
    let lines: Vec<&str> = command.lines().collect();
    let raws = lines.iter().enumerate().flat_map(|(n, l)| l.split_whitespace().chain((n + 1 < lines.len()).then_some("\n")));
    for raw in raws {
        if raw == "\n" {
            out.push(raw.to_string());
            continue;
        }
        let mut word = String::new();
        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let two = if i + 1 < chars.len() { Some((c, chars[i + 1])) } else { None };
            if matches!(two, Some(('&', '&')) | Some(('|', '|'))) {
                push_word(&mut out, &mut word);
                out.push(format!("{c}{c}"));
                i += 2;
                continue;
            }
            if matches!(c, ';' | '|' | '(' | ')') || (c == '&' && !word.is_empty() && !word.ends_with('>')) {
                push_word(&mut out, &mut word);
                out.push(c.to_string());
                i += 1;
                continue;
            }
            word.push(c);
            i += 1;
        }
        push_word(&mut out, &mut word);
    }
    out
}

fn push_word(out: &mut Vec<String>, word: &mut String) {
    let trimmed = word.trim_matches(|c| c == '\'' || c == '"');
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    word.clear();
}

pub(crate) fn is_separator(w: &str) -> bool {
    matches!(w, ";" | "|" | "||" | "&&" | "&" | "(" | ")" | "\n")
}

/// True for any path to the folder or into it, absolute, relative or `$PWD`-based.
fn names_folder(word: &str) -> bool {
    let w = word.trim_end_matches('/');
    w == FAMILIAR_FOLDER
        || w.starts_with(&format!("{FAMILIAR_FOLDER}/"))
        || w.ends_with(&format!("/{FAMILIAR_FOLDER}"))
        || w.contains(&format!("/{FAMILIAR_FOLDER}/"))
}

/// True when any redirect, such as `2>` or `&>>`, writes into the folder.
fn redirects_into(words: &[String]) -> bool {
    for (i, w) in words.iter().enumerate() {
        let Some(after) = strip_redirect(w) else {
            continue;
        };
        let target = if after.is_empty() { words.get(i + 1).map(String::as_str) } else { Some(after) };
        if target.is_some_and(names_folder) {
            return true;
        }
    }
    false
}

/// The target after a leading redirect operator, or `None`.
pub(crate) fn strip_redirect(word: &str) -> Option<&str> {
    let rest = word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&');
    let rest = rest.strip_prefix('>')?;
    Some(rest.strip_prefix('>').unwrap_or(rest))
}

/// The git subcommand after `git` and its options, with its index.
fn git_subcommand(words: &[String], at: usize) -> Option<(&str, usize)> {
    let mut i = at + 1;
    while i < words.len() {
        let w = words[i].as_str();
        if is_separator(w) {
            return None;
        }
        if w == "-C" || w == "-c" {
            i += 2;
            continue;
        }
        if w.starts_with('-') {
            i += 1;
            continue;
        }
        return Some((w, i));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn removes(cmd: &str) -> bool {
        removes_folder(cmd, || false)
    }
    fn removes_with_content(cmd: &str) -> bool {
        removes_folder(cmd, || true)
    }

    #[test]
    fn naming_the_folder_with_a_removing_verb_is_refused_in_any_path_form() {
        for cmd in [
            "rm -rf .familiar",
            "rm -rf ./.familiar",
            "rm -rf /work/repo/.familiar",
            "rm -rf \"$PWD/.familiar\"",
            "rm .familiar/plan",
            "rm -f '.familiar/record'",
            "cd /work/repo && rm -rf .familiar",
            "ls; rm -rf .familiar/",
            "sudo rm -rf /work/repo/.familiar",
            "rmdir .familiar",
            "unlink .familiar/record",
            "mv .familiar .familiar.off",
            "mv /work/repo/.familiar /tmp/x",
            "truncate -s 0 .familiar/record",
            "git clean -fd .familiar",
            "git checkout -- .familiar/plan",
            "git -C /work/repo checkout -- .familiar/plan",
            "git restore .familiar/plan",
            "git restore --staged --worktree .familiar",
            "echo > .familiar/record",
            "echo x >.familiar/plan",
            "echo x >> .familiar/record",
            "cmd 2>/work/repo/.familiar/record",
            "cmd &> .familiar/plan",
            "printf '' > .familiar/record",
            "rm .familiar/adapter",
            "echo x > .familiar/adapter",
            "git checkout -- .familiar/adapter",
            "git restore .familiar/adapter",
        ] {
            assert!(removes(cmd), "{cmd}");
        }
    }

    #[test]
    fn reset_hard_and_clean_x_are_refused_only_while_the_folder_has_uncommitted_content() {
        for cmd in ["git reset --hard", "git reset --hard HEAD~1", "git reset --hard abc123", "git -C /w reset --hard", "git clean -fdx", "git clean -xdf", "git clean -fdX", "git clean -f -x", "git clean -f --exclude=target"] {
            assert!(removes_with_content(cmd), "{cmd}");
            assert!(!removes(cmd), "{cmd}: nothing uncommitted, nothing lost");
        }
        for cmd in ["git reset --soft HEAD~1", "git reset HEAD", "git clean -fd", "git clean -n", "git stash"] {
            assert!(!removes_with_content(cmd), "{cmd}");
        }
    }

    #[test]
    fn reading_the_folder_setup_and_other_work_stay_allowed() {
        for cmd in [
            "familiar-ai setup",
            "cat .familiar/plan",
            "ls -la .familiar",
            "ls /work/repo/.familiar/",
            "cat .familiar/record | head",
            "rm -rf build",
            "rm -rf .familiar.off",
            "rm -rf src/familiar",
            "mv a.py b.py",
            "git checkout -- src/app.py",
            "git restore src/app.py",
            "git checkout -b feature",
            "git commit -am 'x'",
            "echo .familiar/plan",
            "grep -r familiar .familiar/plan > out.txt",
            "git clean -fd",
            "pytest -q tests/test_c1.py",
            "git diff .familiar/plan",
        ] {
            assert!(!removes_with_content(cmd), "{cmd}");
        }
    }

    #[test]
    fn the_refusal_needs_a_bash_call_with_the_folder_present() {
        let root = std::env::temp_dir().join(format!("familiar-ai-folder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(FAMILIAR_FOLDER)).expect("folder");
        let input = |json: &str, ev: HookEvent| HookInput::parse(json, ev).expect("parse");
        let rm = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"rm -rf .familiar"}}"#, HookEvent::PreToolUse);
        assert_eq!(removal_refusal(&rm, &root), Some(NOT_YOURS.to_string()));
        assert_eq!(NOT_YOURS.lines().count(), 1);
        let post = input(r#"{"hook_event_name":"PostToolUse","tool_name":"Bash","tool_input":{"command":"rm -rf .familiar"}}"#, HookEvent::PostToolUse);
        assert_eq!(removal_refusal(&post, &root), None, "a finished call cannot be refused");
        let edit = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"/w/.familiar/plan"}}"#, HookEvent::PreToolUse);
        assert_eq!(removal_refusal(&edit, &root), None, "an edit is decision.rs's");
        let cat = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"cat .familiar/plan"}}"#, HookEvent::PreToolUse);
        assert_eq!(removal_refusal(&cat, &root), None);
        // reset --hard: the plan on disk, never committed, is uncommitted content
        let reset = input(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"git reset --hard HEAD"}}"#, HookEvent::PreToolUse);
        assert_eq!(removal_refusal(&reset, &root), None, "nothing in the folder yet");
        std::fs::write(root.join(FAMILIAR_FOLDER).join(PLAN_FILE), "{\"kind\":\"intent_submitted\"}\n").expect("plan");
        assert!(has_uncommitted_content(&root), "a plan with no commit to match");
        assert_eq!(removal_refusal(&reset, &root), Some(NOT_YOURS.to_string()));
        // the adapter on disk, never committed, is uncommitted content too (plan A-1, lane a-surf)
        std::fs::remove_file(root.join(FAMILIAR_FOLDER).join(PLAN_FILE)).expect("rm plan");
        assert!(!has_uncommitted_content(&root), "nothing left");
        std::fs::write(root.join(FAMILIAR_FOLDER).join(crate::door::ADAPTER_FILE), "{\"root\":\"\",\"runner\":\"pytest\"}\n").expect("adapter");
        assert!(has_uncommitted_content(&root), "an adapter with no commit to match");
        assert_eq!(removal_refusal(&reset, &root), Some(NOT_YOURS.to_string()));
        // no folder: nothing to protect
        std::fs::remove_dir_all(root.join(FAMILIAR_FOLDER)).expect("rm");
        assert_eq!(removal_refusal(&rm, &root), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
