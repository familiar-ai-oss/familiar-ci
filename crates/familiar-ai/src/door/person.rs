//! Capture the person's words: each prompt typed in the main conversation of
//! their own session becomes `person_said`. A prompt writes no intent; the agent
//! claims the intent at its first edit of code.

use std::path::Path;

use familiar_ai_facts::{Log, PersonSaid, Record};

use crate::door::{cannot, TENANT};
use crate::input::hook_input::HookInput;

/// The frames Claude Code puts at the start of a message it relays from an
/// agent (another session, a subagent's hand-back), which arrives through
/// UserPromptSubmit like a typed prompt; the one list of them.
pub const RELAYED_AGENT_FRAMES: [&str; 3] = ["Another Claude session sent a message:", "<agent-message from=", "[Subagent hand-back]"];

/// Whether `prompt` begins with a frame of a message relayed from an agent.
pub fn relayed_from_an_agent(prompt: &str) -> bool {
    let start = prompt.trim_start();
    RELAYED_AGENT_FRAMES.iter().any(|f| start.starts_with(f))
}

/// Whether this hook call is the person typing in a session they run; a
/// session the binary launched is given its prompt, never typed into, and a
/// message relayed from an agent is never the person's.
pub fn is_the_persons(input: &HookInput) -> bool {
    input.is_the_person_speaking()
        && !input.prompt_text().is_some_and(relayed_from_an_agent)
        && !input.in_subagent()
        && input.conversation.is_none()
        && !input.by_binary
}

/// Write the person's words. `Err` only when the record cannot be written.
pub(crate) fn heard(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    let Some(prompt) = input.prompt_text().filter(|p| is_the_persons(input) && !p.trim().is_empty()) else {
        return Ok(());
    };
    let write = |e: familiar_ai_facts::RecordError| cannot("write its record", &e);
    let said = PersonSaid { text: prompt.to_string(), agent_conversation_id: input.session_conversation_id() };
    log.push(record.append(&said, now, TENANT).map_err(write)?);
    Ok(())
}

/// `at` broken down in the local zone, else in UTC; `None` only when neither can be read.
fn broken_down(at: f64) -> Option<libc::tm> {
    // Only the minute or the day is shown, so truncating the fraction is fine.
    #[allow(clippy::cast_possible_truncation)]
    let secs = at.floor() as libc::time_t;
    // SAFETY: an all-zero `tm` is a valid value; localtime_r and gmtime_r read
    // `secs` and write only into `tm`, both on this stack
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if !libc::localtime_r(&secs, &mut tm).is_null() || !libc::gmtime_r(&secs, &mut tm).is_null() {
            return Some(tm);
        }
    }
    None
}

/// A fact's time as `h:mm` in the local zone, or UTC when the zone cannot be read.
pub fn clock(at: f64) -> String {
    if let Some(tm) = broken_down(at) {
        return format!("{}:{:02}", tm.tm_hour, tm.tm_min);
    }
    #[allow(clippy::cast_possible_truncation)]
    let day = (at.floor() as i64).rem_euclid(86_400);
    format!("{}:{:02}", day / 3600, (day % 3600) / 60)
}

/// A fact's day as `YYYY-MM-DD` in the local zone, or UTC when the zone cannot be read.
pub fn day(at: f64) -> String {
    match broken_down(at) {
        Some(tm) => format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday),
        None => "an unknown day".to_string(),
    }
}

/// The files the agent is about to change or has just committed; while no run
/// is open, an act of code asks for an intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// The files, in the order named.
    pub files: Vec<Touched>,
    /// A commit that already landed, so it can only be reported, not refused.
    pub landed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touched {
    /// Relative to the project.
    pub path: String,
    /// A passive file opens no run.
    pub passive: bool,
    /// The first changed line of code and its number, when the change is known.
    pub code: Option<(u64, String)>,
}

impl Act {
    /// The first file that is not passive: what asks for the intent.
    pub fn code(&self) -> Option<&Touched> {
        self.files.iter().find(|t| !t.passive)
    }
}

fn touched(root: &Path, path: String, change: Option<(String, String)>) -> Touched {
    let pair = change.as_ref().map(|(o, n)| (o.as_str(), n.as_str()));
    Touched {
        passive: crate::door::commits::passive(root, &path, pair),
        code: pair.and_then(|(o, n)| crate::door::commits::first_code_line(&path, o, n)),
        path,
    }
}

/// The file after an Edit or MultiEdit of `before`, or a Write's content.
fn after(input: &serde_json::Value, before: &str) -> Option<String> {
    use serde_json::Value;
    if let Some(content) = input.get("content").and_then(Value::as_str) {
        return Some(content.to_string());
    }
    let one = |text: &str, e: &Value| -> Option<String> {
        let (old, new) = (e.get("old_string")?.as_str()?, e.get("new_string")?.as_str()?);
        // The edit will fail when `old` is not found, so its change is unknown.
        if old.is_empty() || !text.contains(old) {
            return None;
        }
        Some(if e.get("replace_all").and_then(Value::as_bool) == Some(true) { text.replace(old, new) } else { text.replacen(old, new, 1) })
    };
    match input.get("edits").and_then(Value::as_array) {
        Some(edits) => edits.iter().try_fold(before.to_string(), |t, e| one(&t, e)),
        None => one(before, input),
    }
}

/// `path` made absolute and without `.` or `..`, its deepest existing folder
/// resolved through any links, so a file not written yet is still placed.
fn canonical(path: &Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let mut clean = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    let mut at = clean.as_path();
    loop {
        if let Ok(real) = std::fs::canonicalize(at) {
            return rest.iter().rev().fold(real, |p, r| p.join(r));
        }
        match (at.file_name(), at.parent()) {
            (Some(name), Some(up)) => {
                rest.push(name.to_os_string());
                at = up;
            }
            _ => return clean,
        }
    }
}

/// Whether the file a tool names lies outside the project's folder, after
/// canonicalising both: such a file is never this project's code, so a change
/// to it is passive. A relative path is read from `cwd`.
pub fn outside_the_project(root: &Path, cwd: &Path, file_path: &str) -> bool {
    let named = Path::new(file_path);
    let full = if named.is_absolute() { named.to_path_buf() } else { cwd.join(named) };
    !canonical(&full).starts_with(canonical(root))
}

pub fn act(input: &HookInput, root: &Path) -> Option<Act> {
    use crate::door::shell_write::{inside_project, written_paths, Written, A_FILE};
    use crate::input::hook_input::HookEvent;
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    let files: Vec<Touched> = match input.event {
        HookEvent::PreToolUse => match (input.file_path(), input.command()) {
            (Some(f), _) if outside_the_project(root, cwd, f) => Vec::new(),
            (Some(f), _) => {
                let rel = crate::decide::decision::relative_to(root, f);
                let before = std::fs::read_to_string(root.join(&rel)).unwrap_or_default();
                let change = input.tool_input.as_ref().and_then(|i| after(i, &before)).map(|a| (before, a));
                vec![touched(root, rel, change)]
            }
            (None, Some(c)) => written_paths(c)
                .into_iter()
                .filter_map(|w| match w {
                    Written::Unknown => Some(A_FILE.to_string()),
                    Written::Path(p) => inside_project(root, cwd, &p),
                })
                .map(|p| touched(root, p, None))
                .collect(),
            _ => Vec::new(),
        },
        HookEvent::PostToolUse if input.command().is_some_and(crate::decide::decision::mentions_git_commit) => {
            let head = familiar_ai_git::head_commit(root).ok().flatten()?;
            let parent = format!("{head}^");
            let text = |c: &str, p: &str| familiar_ai_git::file_at(root, c, p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            let files: Vec<Touched> = familiar_ai_git::changed(root, &head)
                .ok()?
                .into_iter()
                .map(|f| {
                    let change = (text(&parent, &f.path), text(&head, &f.path));
                    touched(root, f.path, Some(change))
                })
                .collect();
            return (!files.is_empty()).then_some(Act { files, landed: true });
        }
        _ => Vec::new(),
    };
    (!files.is_empty()).then_some(Act { files, landed: false })
}
