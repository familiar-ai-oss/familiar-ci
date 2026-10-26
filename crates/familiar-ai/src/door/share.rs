//! The person's one-time yes to sharing gap recordings (plan A-6, 2026-10-19 [draft]).
//!
//! A gap recording is Familiar's own probe output, never the person's code: a dry run
//! leaves it in the project's `.familiar/runs/gaps/<run_id>/<probe>/`, which git ignores.
//! After the person's yes the sender posts the recordings that wait there; the endpoint is
//! the owner's to name, and until it is named the sender does nothing and they stay queued.
//! The yes is the machine's, not a project's: one file in the data folder beside the
//! project markers, written only from the person's own terminal (an agent's shell is
//! refused, as it is for `on` and `off`).

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::door::project::plain;
use crate::door::{marker, FAMILIAR_FOLDER};
use crate::input::Surroundings;
use crate::Outcome;

/// Where recordings are sent: the owner names it (plan A-6, as §6.9 A). `None` until then.
pub const ENDPOINT: Option<&str> = None;

/// What `share on` says.
pub const YES_WORDS: &str = "Gap recordings will be shared. A recording is Familiar's own probe output, never your code. Until an endpoint is named they wait in each project's .familiar/runs/gaps. familiar-ai share off takes this back.\n";
/// What `share off` says.
pub const NO_WORDS: &str = "Gap recordings will not be shared. They stay in each project's .familiar/runs/gaps. familiar-ai share on gives the yes again.\n";
/// Why a choice cannot be kept when no data folder is known.
const NO_FOLDER: &str = "no data folder is known to keep the choice in (set FAMILIAR_AI_HOME, XDG_DATA_HOME or HOME)";

/// The file inside a recording that marks it as sent.
pub const SENT: &str = "sent";
/// The files of a recording that are sent, beside everything under its `report/`: the fixture layout, nothing else.
const FILES: [&str; 5] = ["cmd", "exit", "stdout", "stderr", "version"];
/// The most of one file that is sent.
const MAX_FILE: usize = 256 * 1024;
/// The most files of a recording's `report/` that are sent, and how deep they may lie.
const MAX_REPORT_FILES: usize = 64;
const MAX_REPORT_DEPTH: usize = 3;

/// What the person has said about sharing, for this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Never asked, or nothing clear is kept: nothing is shared.
    Unasked,
    /// A yes.
    On,
    /// A no.
    Off,
}

/// The choice kept in the data folder. No folder, no file, or anything but a plain
/// `share: on` or `share: off` first line is [`Choice::Unasked`]: only a clear yes shares.
pub fn choice(data_dir: Option<&Path>) -> Choice {
    let Some(text) = data_dir.and_then(|d| std::fs::read_to_string(marker::share_path(d)).ok()) else {
        return Choice::Unasked;
    };
    match text.lines().next() {
        Some("share: on") => Choice::On,
        Some("share: off") => Choice::Off,
        _ => Choice::Unasked,
    }
}

/// Keep the choice in the data folder: `share: on` or `share: off`, then the time, one line each.
pub fn write(data_dir: Option<&Path>, on: bool, now: f64) -> Result<PathBuf, String> {
    let data_dir = data_dir.ok_or_else(|| NO_FOLDER.to_string())?;
    let path = marker::share_path(data_dir);
    std::fs::create_dir_all(data_dir).map_err(|e| format!("cannot make {}: {e}", data_dir.display()))?;
    let text = format!("share: {}\nat: {}\n", if on { "on" } else { "off" }, seconds(now));
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

fn seconds(now: f64) -> u64 {
    if now.is_finite() && now >= 0.0 {
        now as u64
    } else {
        0
    }
}

/// Why an agent may not give or take back the yes: from its own shell, or by a command
/// the hook reads before it runs.
pub fn refusal(to: &str) -> String {
    format!("familiar-ai: not changed · familiar-ai share {to} is the person's choice, not the agent's: the person makes it by running familiar-ai share {to} in a terminal of their own")
}

/// `familiar-ai share on` or `off`: the person's choice, kept for the machine; refused
/// from an agent's session. It needs no project.
pub fn run_share(on: bool, around: &Surroundings) -> Outcome {
    let to = if on { "on" } else { "off" };
    if around.session_id.is_some() {
        return plain(1, String::new(), format!("{}\n", refusal(to)));
    }
    match write(around.data_dir.as_deref(), on, around.now) {
        Ok(_) => plain(0, if on { YES_WORDS } else { NO_WORDS }.to_string(), String::new()),
        Err(e) => plain(1, String::new(), format!("familiar-ai: not kept · {e}\n")),
    }
}

/// Where a project's dry runs leave gap recordings, inside its git-ignored `.familiar/runs`.
pub fn gaps_dir(root: &Path) -> PathBuf {
    root.join(FAMILIAR_FOLDER).join("runs").join("gaps")
}

/// A recording waiting to be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    pub run_id: String,
    pub probe: String,
    pub dir: PathBuf,
}

/// Every recording under `.familiar/runs/gaps/<run_id>/<probe>/` that is not marked sent,
/// by run and then probe.
pub fn queued(root: &Path) -> Vec<Recording> {
    let mut out = Vec::new();
    for run in sorted_dirs(&gaps_dir(root)) {
        let run_id = name_of(&run);
        for probe in sorted_dirs(&run) {
            if !probe.join(SENT).exists() {
                out.push(Recording { run_id: run_id.clone(), probe: name_of(&probe), dir: probe });
            }
        }
    }
    out
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The folders inside `dir`, sorted; a link is not one.
fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
        .collect();
    dirs.sort();
    dirs
}

/// A plain file's text, cut at [`MAX_FILE`]; a link, a folder or a file that cannot be read is nothing.
fn read_capped(path: &Path) -> Option<String> {
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(MAX_FILE as u64 + 1).read_to_end(&mut bytes).ok()?;
    let cut = bytes.len() > MAX_FILE;
    bytes.truncate(MAX_FILE);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if cut {
        text.push_str("\n[cut: the rest of the file is not sent]\n");
    }
    Some(text)
}

/// The plain files under a recording's `report/`, as `(path inside report/, path)`, sorted, links skipped.
fn report_files(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let name = name_of(&path);
        let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            if depth > 0 {
                report_files(&path, &rel, depth - 1, out);
            }
        } else if meta.is_file() && out.len() < MAX_REPORT_FILES {
            out.push((rel, path));
        }
    }
}

/// What is posted for one recording: its run, its probe and its own files as text.
fn payload(r: &Recording) -> String {
    let mut files = serde_json::Map::new();
    for name in FILES {
        if let Some(text) = read_capped(&r.dir.join(name)) {
            files.insert(name.to_string(), text.into());
        }
    }
    let mut report = Vec::new();
    report_files(&r.dir.join("report"), "report", MAX_REPORT_DEPTH, &mut report);
    for (rel, path) in report {
        if let Some(text) = read_capped(&path) {
            files.insert(rel, text.into());
        }
    }
    serde_json::json!({ "run_id": r.run_id, "probe": r.probe, "files": files }).to_string()
}

/// What a call of the sender did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sent {
    /// No yes is kept (never asked, or a no): nothing leaves the machine.
    NotShared,
    /// A yes, but no endpoint is named yet: the recordings stay queued.
    NoEndpoint,
    /// The queue was worked: how many went, the first failure (the call stops there, so
    /// the next one goes on from it), and how many still wait.
    Queue { sent: usize, failed: Option<String>, waiting: usize },
}

/// The sender: with the person's yes and an endpoint, post each queued recording's own
/// files with `post(endpoint, payload)` and mark it sent; stop at the first failure. With
/// no yes, or no endpoint, it does nothing at all.
pub fn send_queued(root: &Path, choice: Choice, endpoint: Option<&str>, now: f64, post: &mut dyn FnMut(&str, &str) -> Result<(), String>) -> Sent {
    if choice != Choice::On {
        return Sent::NotShared;
    }
    let Some(endpoint) = endpoint else {
        return Sent::NoEndpoint;
    };
    let queue = queued(root);
    let mut sent = 0;
    let mut failed = None;
    for r in &queue {
        let marked = format!("sent: {}\nto: {endpoint}\n", seconds(now));
        // posted first, marked after: a recording whose mark cannot be written is sent again, never lost
        if let Err(e) = post(endpoint, &payload(r)) {
            failed = Some(e);
            break;
        }
        if let Err(e) = std::fs::write(r.dir.join(SENT), marked) {
            failed = Some(format!("cannot mark {}/{} sent: {e}", r.run_id, r.probe));
            break;
        }
        sent += 1;
    }
    Sent::Queue { sent, failed, waiting: queue.len() - sent }
}
