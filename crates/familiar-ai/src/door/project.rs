//! Find a project and open its record, shared by `claim`, `status`, `setup` and `hook`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai_facts::{Record, RecordError};

use crate::decide::hook_output;
use crate::Outcome;
use crate::door::{FAMILIAR_FOLDER, RECORD_FILE};
use crate::input::Surroundings;

/// The nearest directory holding `.familiar`, searching up to the repository root
/// (the first directory holding `.git`) and no further.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start.to_path_buf());
    while let Some(d) = dir {
        if d.join(FAMILIAR_FOLDER).is_dir() {
            return Some(d);
        }
        if d.join(".git").exists() {
            return None;
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

/// The project a hook call belongs to: the session's project folder, unless
/// the working folder is in another git worktree nested under it (a subagent's
/// at `.claude/worktrees/agent-…`). That worktree is its own project: the
/// nearest `.familiar` within it, else its top, which has no record and so is
/// ungoverned; a parent checkout's `.familiar` is never its.
pub fn hook_project(project_dir: Option<&Path>, cwd: Option<&Path>) -> Option<PathBuf> {
    let (Some(dir), Some(cwd)) = (project_dir, cwd) else {
        return project_dir.or(cwd).map(Path::to_path_buf);
    };
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let top = |p: &Path| familiar_ai_git::worktree_root(p).ok().flatten().map(|t| same(&t));
    match (top(dir), top(cwd)) {
        (Some(outer), Some(inner)) if outer != inner && inner.starts_with(same(dir)) => Some(find_project(cwd).unwrap_or(inner)),
        _ => Some(dir.to_path_buf()),
    }
}

/// Where `claim`, `status` and `setup` start looking: the project's root
/// as the agent was started in it, else the working directory.
pub fn start_dir(around: &Surroundings) -> PathBuf {
    around
        .project_dir
        .clone()
        .or_else(|| around.cwd.clone())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// An outcome for a plain command, which prints no hook JSON.
pub fn plain(exit_code: i32, stdout_text: String, stderr_text: String) -> Outcome {
    Outcome {
        exit_code,
        stdout_text,
        stderr_text,
    }
}

fn no_project(where_: &Path) -> Outcome {
    plain(
        1,
        String::new(),
        format!(
            "familiar-ai: no {FAMILIAR_FOLDER} folder in {} or above it in this repository: Familiar is not set up here (run `familiar-ai setup` at the project's root, or /familiar-ai:setup)\n",
            where_.display()
        ),
    )
}

/// The project for `claim` and `status`, and its record.
pub fn open_project(around: &Surroundings) -> Result<(PathBuf, Record), Outcome> {
    let start = start_dir(around);
    let Some(root) = find_project(&start) else {
        return Err(no_project(&start));
    };
    match open_record(&root, around.record_wait) {
        Ok(Some(record)) => Ok((root, record)),
        Ok(None) => Err(no_project(&start)),
        Err(RecordError::Busy) => Err(plain(1, String::new(), format!("familiar-ai: {}\n", hook_output::BUSY_MESSAGE))),
        Err(e) => Err(plain(1, String::new(), format!("familiar-ai: {}\n", crate::door::cannot("read its record", &e)))),
    }
}

/// Open the project's record, waiting its turn; `Ok(None)` without creating
/// anything when the project has no `.familiar` folder.
pub fn open_record(root: &Path, wait: Duration) -> Result<Option<Record>, RecordError> {
    let folder = root.join(FAMILIAR_FOLDER);
    if !folder.is_dir() {
        return Ok(None);
    }
    Record::open_waiting(&folder.join(RECORD_FILE), wait).map(Some)
}

/// The same project folder in the checkout whose working directory is
/// `other_top`: `root`'s path inside its own working directory, joined there.
pub fn same_project_in(root: &Path, other_top: &Path) -> Option<PathBuf> {
    let top = familiar_ai_git::worktree_root(root).ok().flatten()?;
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let rel = same(root).strip_prefix(same(&top)).ok()?.to_path_buf();
    Some(other_top.join(rel))
}

/// The project in main's working directory when `root` is in a linked
/// worktree; `None` in main itself or outside a repository.
pub fn main_project(root: &Path) -> Option<PathBuf> {
    let main = familiar_ai_git::main_worktree_root(root).ok().flatten()?;
    same_project_in(root, &main)
}

/// Another checkout of this repository with the same project set up, as
/// read: its folder's name, its short name (the folder's name less main's
/// and a dash, `hook-words` for `familiar-ai-oss-hook-words`), its branch,
/// its project root, and its record's facts with its switch, or why they
/// could not be read (`Err(Busy)`: in use past the wait).
pub struct Other {
    pub folder: String,
    pub name: String,
    pub branch: Option<String>,
    pub root: PathBuf,
    pub read: Result<(familiar_ai_facts::Log, crate::door::switch::Switch), RecordError>,
    /// Main's own checkout (`Checkout.is_main`, the `main_repo()` entry).
    pub is_main: bool,
    /// An agent lives there: see [`alive`].
    pub alive: bool,
}

/// Whether `root` is in main's own checkout, not a linked worktree; outside a
/// repository it reads as main, which reads every part as before.
pub fn is_main(root: &Path) -> bool {
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (Some(top), Ok(all)) = (familiar_ai_git::worktree_root(root).ok().flatten(), familiar_ai_git::checkouts(root)) else {
        return true;
    };
    all.iter().find(|c| same(&c.top) == same(&top)).is_none_or(|c| c.is_main)
}

/// Whether an agent lives in the checkout at `root` (the owner, 2026-10-15),
/// read when it is read, no fact and no clock: L1, the lane's session file
/// exists (`--resume` can continue it); L2, its worktree lock names a
/// `claude` process, `claude agent … (pid P start S)`, whose start time,
/// field 22 of `/proc/P/stat`, is S (Assumption 73). Transcripts are not read.
pub fn alive(root: &Path, locked: Option<&str>) -> bool {
    crate::door::lane::session_file(root).is_file() || locked.is_some_and(live_lock)
}

fn live_lock(reason: &str) -> bool {
    let Some((_, named)) = reason.strip_prefix("claude agent ").and_then(|r| r.rsplit_once("(pid ")) else {
        return false;
    };
    let words: Vec<&str> = named.trim_end_matches(')').split_whitespace().collect();
    let [pid, "start", start] = words.as_slice() else {
        return false;
    };
    if !pid.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(19).map(str::to_string)))
        .is_some_and(|s| s == *start)
}

/// The cards of another checkout that reach this one (the owner, 2026-10-15):
/// main's every card; an alive checkout's cards on the parts touched there;
/// none from anything else. `log` is that checkout's record as read.
pub fn relayed_cards(o: &Other, log: &familiar_ai_facts::Log) -> Vec<familiar_ai_rules::terminal::Card> {
    if !o.is_main && !o.alive {
        return Vec::new();
    }
    let v = familiar_ai_rules::View::new(log);
    familiar_ai_rules::terminal::cards(&v)
        .into_iter()
        .filter(|c| o.is_main || familiar_ai_rules::lifecycle::touched_here(&v, &c.criterion_id))
        .collect()
}

/// Every other checkout of the repository that holds `root` whose same
/// project folder is set up, main first. Each record is opened with `wait`
/// and let go at once, so the caller must hold no record of its own meanwhile.
pub fn other_checkouts(root: &Path, wait: Duration) -> Vec<Other> {
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let Some(top) = familiar_ai_git::worktree_root(root).ok().flatten() else {
        return Vec::new();
    };
    let Ok(all) = familiar_ai_git::checkouts(root) else {
        return Vec::new();
    };
    let main_folder = all.first().and_then(|c| c.top.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    all.into_iter()
        .filter(|c| same(&c.top) != same(&top))
        .filter_map(|c| {
            let root = same_project_in(root, &c.top)?;
            let folder = c.top.file_name().map_or_else(|| c.top.display().to_string(), |n| n.to_string_lossy().into_owned());
            let name = folder.strip_prefix(&format!("{main_folder}-")).filter(|n| !n.is_empty()).unwrap_or(&folder).to_string();
            let read = match open_record(&root, wait) {
                Ok(Some(record)) => record.read_all(),
                Ok(None) => return None,
                Err(e) => Err(e),
            }
            .and_then(|log| crate::door::switch::here(&root, &log, wait).map(|s| (log, s)));
            let alive = alive(&root, c.locked.as_deref());
            Some(Other { folder, name, branch: c.branch, root, read, is_main: c.is_main, alive })
        })
        .collect()
}
