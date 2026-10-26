//! `familiar-ai launch_lane`: start a lane, a headless `claude -p` in a
//! worktree of its own, launched the way the investigator is (our plugin, the
//! conversation id the binary gives it), so its prompt, the brief, is never
//! taken for the person's words; the person's words of the current ask are
//! copied into its record instead. One lane runs per worktree: its process
//! group is kept beside its log and read the way a run's is. See
//! docs/decisions.md#the-persons-words-travel-with-a-lane

use std::path::{Path, PathBuf};

use familiar_ai_facts::{Fact, Log, PersonSaid};
use serde_json::Value;

use crate::door::kept::RUNS_FOLDER;
use crate::door::project::{open_project, open_record, plain};
use crate::door::{cannot, procgroup, FAMILIAR_FOLDER, TENANT};
use crate::input::{Launching, Surroundings};
use crate::teach::words::LEAD;
use crate::Outcome;

/// Where a lane keeps its own files, inside its `.familiar/runs`, which git ignores.
const LANE_FOLDER: &str = "lane";
/// `claude`'s stream-json output.
const LOG: &str = "claude.jsonl";
const STDERR: &str = "claude.stderr";
/// The session id the lane was launched with, which `--resume` continues.
const SESSION: &str = "session";
/// The lane's process group: its number and its leader's start time.
const PROCESS: &str = "process";

/// The tools a lane may use without asking, since nobody is there to answer.
const ALLOWED_TOOLS: &str = "Read,Edit,Write,Glob,Grep,Bash";

/// What the command line asked for.
#[derive(Debug, Clone)]
pub struct Asked {
    pub brief: PathBuf,
    pub worktree: Option<PathBuf>,
    pub resume: bool,
    pub plugin_dir: Option<PathBuf>,
    pub model: Option<String>,
    pub budget: Option<String>,
}

fn refused(line: String) -> Outcome {
    plain(1, String::new(), format!("{LEAD}{line}\n"))
}

/// The lane's session file under the project root `root`, which `--resume` continues.
pub(crate) fn session_file(root: &Path) -> PathBuf {
    root.join(FAMILIAR_FOLDER).join(RUNS_FOLDER).join(LANE_FOLDER).join(SESSION)
}

/// Launch or resume the lane; every refusal is exit 1 and makes nothing.
pub fn run_launch(asked: &Asked, around: &Surroundings, launching: &Launching) -> Outcome {
    match launch(asked, around, launching) {
        Ok(out) | Err(out) => out,
    }
}

fn launch(asked: &Asked, around: &Surroundings, launching: &Launching) -> Result<Outcome, Outcome> {
    // The launching checkout's record is read once, for the person's words, and
    // let go before anything else: the lane's seed can take minutes, and the
    // person's own session there must not wait on it.
    let (root, log) = {
        let (root, record) = open_project(around)?;
        let log = if asked.resume { Vec::new() } else { record.read_all().map_err(|e| refused(cannot("read its record", &e)))? };
        (root, log)
    };
    let here = crate::door::project::start_dir(around);
    let brief = here.join(&asked.brief);
    let typed = asked.brief.display();
    if !std::fs::metadata(&brief).is_ok_and(|m| m.is_file()) {
        return Err(refused(format!("the brief {typed} cannot be read · next: write the lane's prompt to a file and pass it with --brief")));
    }
    let Some(plugin) = asked.plugin_dir.clone().or_else(|| launching.plugin_root.clone()) else {
        return Err(refused(format!("the plugin's folder is not known, and a lane without it runs ungoverned · next: familiar-ai launch_lane --brief {typed} --plugin-dir <the plugin's folder>")));
    };
    let Some(claude) = launching.claude.clone() else {
        return Err(refused("no claude was found on PATH, so no lane is started".to_string()));
    };
    let (top, rel) = repository_top(&root).map_err(refused)?;
    let folder = match &asked.worktree {
        Some(w) => here.join(w),
        None => top.with_file_name(format!("{}-{}", file_name(&top), stem(&brief))),
    };
    let shown = folder.display().to_string();
    let lane_root = folder.join(&rel);
    let kept = lane_root.join(FAMILIAR_FOLDER).join(RUNS_FOLDER).join(LANE_FOLDER);
    if running(&kept) {
        return Err(refused(format!("a lane is already running in {shown}; one session runs per worktree · next: wait for it to end, or launch the lane in another folder with --worktree")));
    }
    let resume = format!("familiar-ai launch_lane --brief {} --worktree {shown} --resume", brief.display());
    let (session, first) = if asked.resume {
        let Some(s) = std::fs::read_to_string(kept.join(SESSION)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else {
            return Err(refused(format!("no lane was launched in {shown}, so there is nothing to resume · next: familiar-ai launch_lane --brief {} --worktree {shown}", brief.display())));
        };
        let line = format!("{LEAD}the lane in {shown} resumes its conversation {s}");
        (s, line)
    } else {
        // The words are known before anything is made: a stale prompt makes nothing.
        let words = current_words(&log, around.session_id.as_deref()).map_err(refused)?;
        let branch = format!("lane-{}", file_name(&folder).strip_prefix(&format!("{}-", file_name(&top))).unwrap_or(&file_name(&folder)));
        if !folder.exists() {
            let head = familiar_ai_git::head_commit(&top).ok().flatten().unwrap_or_else(|| "HEAD".to_string());
            crate::door::git_program::git(&top, &["worktree", "add", "-q", "-b", &branch, &shown, &head], &[], "").map_err(refused)?;
        }
        let branch = crate::door::git_program::git_line(&folder, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or(branch);
        let carried = seed(&words, &lane_root, around)?;
        let s = crate::door::investigator_run::minted(&shown, "lane", around.now, carried);
        std::fs::create_dir_all(&kept).and_then(|()| std::fs::write(kept.join(SESSION), &s)).map_err(|e| refused(format!("{}: {e}", kept.display())))?;
        let prompts = if carried == 1 { "1 prompt".to_string() } else { format!("{carried} prompts") };
        (s, format!("{LEAD}a lane runs in {shown} on the branch {branch}, carrying {prompts} of the person's"))
    };
    let mut args = crate::door::investigator_run::claude_args(
        asked.model.as_deref(),
        &plugin,
        &["stream-json", "--verbose"],
        asked.budget.as_deref(),
        if asked.resume { ["--resume", &session] } else { ["--session-id", &session] },
    );
    args.extend(["--permission-mode", "acceptEdits", "--allowedTools", ALLOWED_TOOLS].map(str::to_string));
    let log = kept.join(LOG);
    start(&claude, &args, &session, launching, &lane_root, &brief, &kept).map_err(|e| refused(format!("the lane's claude could not start: {e}")))?;
    Ok(plain(0, format!("{first}\n  its log: {}\n  resume it: {resume}\n", log.display()), String::new()))
}

/// The repository's top folder as the project names it, and the project's
/// folder inside it.
fn repository_top(root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let worktree = match familiar_ai_git::worktree_root(root) {
        Ok(Some(w)) => w,
        _ => return Err(format!("{} is not inside a git repository, so no lane is started", root.display())),
    };
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let rel = same(root).strip_prefix(same(&worktree)).map(Path::to_path_buf).unwrap_or_default();
    let mut top = root.to_path_buf();
    for _ in rel.components() {
        top.pop();
    }
    Ok((top, rel))
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `greet.brief.md` → `greet`.
fn stem(brief: &Path) -> String {
    file_name(brief).split('.').next().unwrap_or("lane").to_string()
}

/// Whether the lane's recorded process group still has a live member.
fn running(kept: &Path) -> bool {
    let text = std::fs::read_to_string(kept.join(PROCESS)).unwrap_or_default();
    let mut parts = text.split_whitespace();
    match (parts.next().and_then(|g| g.parse::<i32>().ok()), parts.next()) {
        (Some(pgid), Some(start)) => procgroup::alive(pgid, start),
        _ => false,
    }
}

fn text(f: &Fact) -> &str {
    f.fields.get("text").and_then(Value::as_str).unwrap_or_default()
}

/// The person's words the current ask names: the prompts the run's intent and
/// its criteria name with `for`, else the latest prompt typed in `session`,
/// the session launching the lane. A prompt of another session, or of none
/// recorded, is never carried on its own: `Err` is the refusal naming the
/// latest one. See docs/decisions.md#the-persons-words-travel-with-a-lane
pub fn current_words<'a>(log: &'a Log, session: Option<&str>) -> Result<Vec<&'a Fact>, String> {
    use crate::teach::person;
    let said = person::said(log);
    if person::run_intent(log).and_then(|i| i.fields.get("for")).is_some() {
        let named = person::words(log);
        let words: Vec<&Fact> = said.iter().copied().filter(|f| named.iter().any(|w| w == text(f))).collect();
        if !words.is_empty() {
            return Ok(words);
        }
    }
    let here = |f: &&Fact| session.is_some() && f.fields.get("agent_conversation_id").and_then(Value::as_str) == session;
    if let Some(f) = said.iter().copied().rev().find(here) {
        return Ok(vec![f]);
    }
    let next = "next: type the ask in this session, or claim the intent naming it first";
    Err(match said.last() {
        Some(f) => format!(
            "no lane is started: the person's latest words, \"{}\" (t {t}), were typed {}, not in this session, and no intent names them · {next}: familiar-ai claim intent_submitted --for {t} --text <the ask>",
            person::one_line(text(f)),
            crate::door::person::day(f.at),
            t = f.t
        ),
        None => format!("no lane is started: the record holds no words of the person's for it · {next}"),
    })
}

/// Set the lane's project up when its record is new and copy the person's
/// words into it, each once with the session it was typed in; returns how many it carries.
fn seed(words: &[&Fact], lane_root: &Path, around: &Surroundings) -> Result<usize, Outcome> {
    if !lane_root.join(FAMILIAR_FOLDER).join(crate::door::RECORD_FILE).is_file() {
        let there = Surroundings { cwd: Some(lane_root.to_path_buf()), project_dir: None, ..around.clone() };
        let set = crate::door::setup::run_setup(&there);
        if set.exit_code != 0 {
            return Err(plain(1, set.stdout_text, set.stderr_text));
        }
    }
    let record = match open_record(lane_root, around.record_wait) {
        Ok(Some(r)) => r,
        Ok(None) => return Err(refused(format!("{} has no .familiar folder after setup", lane_root.display()))),
        Err(e) => return Err(refused(cannot("read the lane's record", &e))),
    };
    let had = record.read_all().map_err(|e| refused(cannot("read the lane's record", &e)))?;
    let had: Vec<&str> = crate::teach::person::said(&had).iter().filter_map(|f| f.fields.get("text").and_then(Value::as_str)).collect();
    for w in words.iter().filter(|w| !had.contains(&text(w))) {
        let said = PersonSaid {
            text: text(w).to_string(),
            agent_conversation_id: w.fields.get("agent_conversation_id").and_then(Value::as_str).map(str::to_string),
        };
        record.append(&said, around.now, TENANT).map_err(|e| refused(cannot("write the lane's record", &e)))?;
    }
    Ok(words.len())
}

/// Start the lane's `claude -p` detached, the brief on its input, and keep its group.
fn start(claude: &Path, args: &[String], session: &str, launching: &Launching, dir: &Path, brief: &Path, kept: &Path) -> Result<(), String> {
    // The governing binary first on PATH, so the lane's hooks and its shell run this one.
    let mut path = std::ffi::OsString::new();
    if let Some(bin) = launching.exe.as_deref().and_then(Path::parent) {
        path.push(bin);
        path.push(":");
    }
    path.push(launching.path.clone().unwrap_or_default());
    let path = path.to_string_lossy().into_owned();
    let set = [("FAMILIAR_AI_CONVERSATION_ID", session), ("PATH", path.as_str())];
    std::fs::create_dir_all(kept).map_err(|e| e.to_string())?;
    let pid = crate::door::child::detached(claude, args, dir, &set, &["CLAUDE_CODE_SESSION_ID"], brief, [&kept.join(LOG), &kept.join(STDERR)])?;
    let pgid = i32::try_from(pid).map_err(|e| e.to_string())?;
    let start = procgroup::start_of(pgid).unwrap_or_default();
    std::fs::write(kept.join(PROCESS), format!("{pgid} {start}\n")).map_err(|e| e.to_string())
}
