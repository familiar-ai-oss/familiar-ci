//! `familiar-ai setup`: make `.familiar`, ignore its laptop-only files, mark the
//! project as set up outside the repository, and offer the status line. Setup
//! is always its own commit, so it refuses up front when the index or its files
//! hold other changes. It is safe to re-run.

use std::path::Path;

use crate::Outcome;
use crate::door::project::{find_project, plain, start_dir};
use crate::door::kept::{RUNS_FOLDER, TRANSCRIPTS_FOLDER};
use crate::door::verify_report::REPORT_FILE;
use crate::door::{FAMILIAR_FOLDER, INVESTIGATOR_LOG, JUDGE_LOG, PLAN_FILE, RECORD_FILE};
use crate::input::Surroundings;

/// Told at SessionStart when the project has no `.familiar` folder; other
/// events stay silent.
pub const NOT_SET_UP: &str = "familiar-ai: installed, and this project is not set up yet, so nothing is checked here · next: `familiar-ai setup` (or /familiar-ai:setup) starts it";

/// The lines setup adds to the end of the project's CLAUDE.md. A plugin cannot ship a CLAUDE.md,
/// and CLAUDE.md is what an agent re-reads after compaction, so the binary writes it.
pub const CLAUDE_MD_LINES: &str = "## familiar-ai

This project is governed by the familiar-ai plugin: proof that the work
is done, each requirement by a test that failed first, was not changed,
and passes. The rulebook arrives in your context at the session's start
and again after compaction and resume, as lines beginning
`familiar-ai:`; they are the binary's facts. Follow them. When a hook
refuses, the `familiar-ai:plan` skill says what to do.
`familiar-ai status` is the only picture of where the work stands; the
record is a database file, never opened by hand. The switch
(`familiar-ai on` / `off`) and the question cards are the person's.
";

/// The heading that marks the lines as already there.
const CLAUDE_MD_HEADING: &str = "## familiar-ai";

/// Exit 0 with a line per step. Exit 1 with one line on stderr when a step
/// fails; the earlier lines stay on stdout.
pub fn run_setup(around: &Surroundings) -> Outcome {
    let start = start_dir(around);
    let root = find_project(&start).unwrap_or(start);
    let mut lines = Vec::new();
    // Every line begins with `familiar-ai:`.
    let out = |lines: &[String]| lines.iter().map(|l| format!("{}{l}\n", crate::teach::words::LEAD)).collect::<String>();
    match set_up(&root, around, &mut lines) {
        Ok(()) => plain(0, out(&lines), String::new()),
        Err(message) => plain(1, out(&lines), format!("familiar-ai: {message}\n")),
    }
}

/// The record's and plan's paths relative to the repository top, `/`-separated.
pub(crate) fn paths_in_repository(root: &Path, worktree: &Path) -> Result<(String, String), String> {
    let same = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (root_c, worktree_c) = (same(root), same(worktree));
    let rel = root_c.strip_prefix(&worktree_c).map_err(|_| {
        format!("{} is not inside the repository at {}", root.display(), worktree.display())
    })?;
    let mut parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.push(FAMILIAR_FOLDER.to_string());
    let folder = parts.join("/");
    Ok((format!("{folder}/{RECORD_FILE}"), format!("{folder}/{PLAN_FILE}")))
}

/// Run the steps in order. An `Err` is the line saying what failed.
fn set_up(root: &Path, around: &Surroundings, lines: &mut Vec<String>) -> Result<(), String> {
    let worktree = match familiar_ai_git::worktree_root(root) {
        Ok(Some(w)) => w,
        _ => return Err(format!("{} is not inside a git repository; nothing was made", root.display())),
    };
    may_commit(root, &worktree)?;
    make_folder(root, lines)?;
    restore_plan_start_record(root, around, lines)?;
    mark_set_up(around, root, lines)?;
    keep_record_ignored(root, &worktree, lines)?;
    // Offer the status line when none is set.
    lines.push(crate::door::statusline::offer(root)?);
    lines.push(add_claude_md_lines(root)?);
    commit_setup(root, &worktree, lines)
}

pub const SETUP_MESSAGE: &str = "familiar-ai setup";

/// The files setup edits, at the project's root: what its commit holds.
pub const SETUP_FILES: [&str; 3] = [".gitignore", ".claude/settings.json", "CLAUDE.md"];

fn ignore_lines() -> Vec<String> {
    [RECORD_FILE, JUDGE_LOG, INVESTIGATOR_LOG, RUNS_FOLDER, TRANSCRIPTS_FOLDER, REPORT_FILE].iter().map(|n| format!("{FAMILIAR_FOLDER}/{n}")).collect()
}

/// `name` (relative to the root) as git names it from the repository's top.
fn in_repository(root: &Path, worktree: &Path, name: &str) -> Result<String, String> {
    let (record_rel, _) = paths_in_repository(root, worktree)?;
    let prefix = record_rel.strip_suffix(&format!("{FAMILIAR_FOLDER}/{RECORD_FILE}")).unwrap_or_default();
    Ok(format!("{prefix}{name}"))
}

/// The file as HEAD holds it (`None`: no commit yet, or not there).
fn at_head(worktree: &Path, rel: &str) -> Option<Vec<u8>> {
    let head = familiar_ai_git::head_commit(worktree).ok().flatten()?;
    familiar_ai_git::file_at(worktree, &head, rel).ok()
}

/// True when the only change from HEAD is setup's own: its ignore lines, its
/// status line or its CLAUDE.md lines.
fn only_setups(name: &str, head: Option<&[u8]>, work: Option<&[u8]>) -> bool {
    if head == work {
        return true;
    }
    let text = |b: Option<&[u8]>| b.map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    if name == "CLAUDE.md" {
        let (head, work) = (text(head), text(work));
        return work.strip_suffix(CLAUDE_MD_LINES).is_some_and(|before| before.trim_end_matches('\n') == head.trim_end_matches('\n'));
    }
    if name == ".gitignore" {
        let ours = ignore_lines();
        let rest = |t: String| t.lines().filter(|l| !ours.iter().any(|o| o == l.trim())).map(str::to_string).collect::<Vec<_>>();
        return rest(text(head)) == rest(text(work));
    }
    let rest = |t: String| -> Option<serde_json::Value> {
        let mut v: serde_json::Value = if t.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(&t).ok()? };
        let ours = v.pointer("/statusLine/command").and_then(serde_json::Value::as_str) == Some(crate::door::statusline::COMMAND);
        if let (true, Some(o)) = (ours, v.as_object_mut()) {
            o.remove("statusLine");
        }
        Some(v)
    };
    matches!((rest(text(head)), rest(text(work))), (Some(a), Some(b)) if a == b)
}

/// Refuse before making anything if setup's commit would carry other changes.
fn may_commit(root: &Path, worktree: &Path) -> Result<(), String> {
    use crate::door::git_program::{git, git_code};
    for who in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
        git(root, &["var", who], &[], "").map_err(|_| {
            "git does not know who you are (user.name and user.email), and setup is its own commit: set them, then run `familiar-ai setup` again; nothing was made".to_string()
        })?;
    }
    match git_code(root, &["diff", "--cached", "--quiet"]) {
        Some(0) => {}
        Some(1) => {
            return Err("the index holds staged changes, and setup is its own commit that never rides with work: commit or unstage them, then run `familiar-ai setup` again; nothing was made".to_string())
        }
        _ => return Err("git cannot say whether the index holds staged changes; nothing was made".to_string()),
    }
    for name in SETUP_FILES {
        let rel = in_repository(root, worktree, name)?;
        let work = std::fs::read(root.join(name)).ok();
        if !only_setups(name, at_head(worktree, &rel).as_deref(), work.as_deref()) {
            return Err(format!(
                "{name} has uncommitted changes of your own, and setup is its own commit that never rides with work: commit or set them aside, then run `familiar-ai setup` again; nothing was made"
            ));
        }
    }
    Ok(())
}

/// Commit the files in `SETUP_FILES` that differ from HEAD; nothing to commit
/// means already set up.
fn commit_setup(root: &Path, worktree: &Path, lines: &mut Vec<String>) -> Result<(), String> {
    use crate::door::git_program::{git, git_line};
    let mut paths: Vec<&str> = Vec::new();
    for name in SETUP_FILES {
        let rel = in_repository(root, worktree, name)?;
        let head = at_head(worktree, &rel);
        if std::fs::read(root.join(name)).ok() == head {
            continue;
        }
        if head.is_none() && familiar_ai_git::is_ignored(worktree, &rel).unwrap_or(false) {
            lines.push(format!("{name} is ignored by git, so it stays out of the setup commit"));
            continue;
        }
        paths.push(name);
    }
    if paths.is_empty() {
        lines.push("already set up: setup has nothing to commit".to_string());
        return Ok(());
    }
    let mut add = vec!["add", "--"];
    add.extend(&paths);
    git(root, &add, &[], "").map_err(|e| format!("the edits are made but not committed: {e}"))?;
    let mut commit = vec!["commit", "--only", "-q", "-m", SETUP_MESSAGE, "--"];
    commit.extend(&paths);
    git(root, &commit, &[], "").map_err(|e| format!("the edits are made but not committed: {e}"))?;
    let id = git_line(root, &["rev-parse", "--short=7", "HEAD"]).unwrap_or_default();
    lines.push(format!("committed {} as one commit \"{}\" ({id})", paths.join(" and "), SETUP_MESSAGE));
    Ok(())
}

/// Make `.familiar` unless a folder or a file is already there.
fn make_folder(root: &Path, lines: &mut Vec<String>) -> Result<(), String> {
    let folder = root.join(FAMILIAR_FOLDER);
    if folder.is_dir() {
        lines.push(format!("{FAMILIAR_FOLDER} is already at {}", root.display()));
    } else if folder.exists() {
        return Err(format!("{} is in the way: it is not a folder", folder.display()));
    } else {
        std::fs::create_dir(&folder).map_err(|e| format!("cannot make {}: {e}", folder.display()))?;
        lines.push(format!("made {FAMILIAR_FOLDER} at {}", root.display()));
    }
    Ok(())
}

/// Bring the committed plan back from git if it is missing, and start the
/// record if it is missing or empty: from the plan committed at HEAD and the
/// history, as `familiar-ai verify` writes it in a fresh clone, so the plan
/// file it mirrors never shrinks; empty when that plan is.
fn restore_plan_start_record(root: &Path, around: &Surroundings, lines: &mut Vec<String>) -> Result<(), String> {
    let folder = root.join(FAMILIAR_FOLDER);
    let plan = folder.join(PLAN_FILE);
    let committed = crate::door::set_up::plan_at_head(root);
    if !plan.exists() {
        if let Some(bytes) = &committed {
            std::fs::write(&plan, bytes).map_err(|e| format!("cannot write {}: {e}", plan.display()))?;
            lines.push(format!("restored {FAMILIAR_FOLDER}/{PLAN_FILE} from git"));
        }
    }
    let record_path = folder.join(RECORD_FILE);
    if !plan.is_file() {
        return Ok(());
    }
    let missing = !record_path.exists();
    let record = familiar_ai_facts::Record::open(&record_path).map_err(|e| format!("cannot start {}: {e}", record_path.display()))?;
    let empty = record.is_empty().map_err(|e| format!("cannot read {}: {e}", record_path.display()))?;
    drop(record);
    let has_claims = committed.as_deref().and_then(|b| familiar_ai_plan::from_bytes(b).ok()).is_some_and(|c| !c.is_empty());
    if !empty || !has_claims {
        if missing {
            lines.push(format!("{FAMILIAR_FOLDER}/{RECORD_FILE} starts empty"));
        }
        return Ok(());
    }
    let cannot = |e: String| format!("cannot start {} from the committed plan: {e}", record_path.display());
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten().unwrap_or_else(|| root.to_path_buf());
    let head = familiar_ai_git::head_commit(&worktree).ok().flatten().unwrap_or_default();
    let plan_only = crate::door::verify::plan_facts(&worktree, root, &head, around.now).map_err(cannot)?;
    let parts = crate::door::tidy::counted(familiar_ai_rules::action::crits(&familiar_ai_rules::View::new(&plan_only)).len(), "part");
    let plan_lines = plan_only.len();
    let start = format!("{FAMILIAR_FOLDER}/{RECORD_FILE} starts from the committed plan: {parts}");
    let (checked, line) = match crate::door::verify::recheck(root, around, false) {
        Ok(checked) => {
            let walked = checked.log.iter().filter(|f| f.kind == "commit_seen").count();
            let counted = crate::door::tidy::counted;
            let line = format!("{start}, {} rechecked, {}", counted(walked, "commit"), counted(checked.runs.len(), "run"));
            (checked, line)
        }
        Err(why) => {
            let line = format!("{start}; the history could not be rechecked ({})", crate::teach::verify::why_could_not(&why));
            (crate::door::verify::Rechecked { log: plan_only, runs: Vec::new(), stopped: None, head, parts: Vec::new(), unverified: Vec::new(), notes: Vec::new() }, line)
        }
    };
    write_seed(&checked, plan_lines, root, around).map_err(cannot)?;
    lines.push(line);
    Ok(())
}

/// Setup's seed, never `verify`'s (the owner, 2026-10-15): verify's facts, then each of its runs
/// but the teeth again as this checkout's laptop evidence (its result per test file, then, after a
/// commit's results, its stamp with the walk's fingerprint), then `record_seeded` naming the head
/// and how many plan facts came first, all in one write, so a seed that does not finish leaves
/// the record empty and no mark. A record that took facts meanwhile gets verify's own, as before.
fn write_seed(checked: &crate::door::verify::Rechecked, plan_lines: usize, root: &Path, around: &Surroundings) -> Result<(), String> {
    use serde_json::{json, Value};
    let path = root.join(FAMILIAR_FOLDER).join(RECORD_FILE);
    let record = familiar_ai_facts::Record::open_waiting(&path, around.record_wait).map_err(|e| e.to_string())?;
    if !record.is_empty().map_err(|e| e.to_string())? {
        drop(record);
        return crate::door::verify::write_record(checked, root, around);
    }
    // verify's runs are read by their ends and translations; their reservations are the
    // pipeline's, never a run owed in this checkout (S9d)
    let reserved_by_pipeline = |f: &familiar_ai_facts::Fact| f.kind == "run_reserved" && f.fields.get("by").and_then(Value::as_str) == Some("pipeline");
    let mut facts: Vec<(&str, serde_json::Map<String, Value>, f64)> = checked.log.iter().filter(|f| !reserved_by_pipeline(f)).map(|f| (f.kind, f.fields.clone(), f.at)).collect();
    let pipeline = |f: &&familiar_ai_facts::Fact| f.fields.get("ran_in").and_then(Value::as_str) == Some("pipeline");
    let teeth = |f: &familiar_ai_facts::Fact| f.fields.get("with_anchor_reverted") == Some(&json!(true));
    for f in checked.log.iter().filter(pipeline) {
        if (f.kind == "pass_please_pass" && !teeth(f)) || f.kind == "fingerprint_stamped" {
            let mut twin = f.fields.clone();
            twin.insert("ran_in".to_string(), json!("laptop"));
            facts.push((f.kind, twin, around.now));
        }
    }
    let mark = json!({ "commit": checked.head, "plan_lines": plan_lines });
    facts.push(("record_seeded", mark.as_object().cloned().unwrap_or_default(), around.now));
    record.append_all(&facts, crate::door::TENANT).map(|_| ()).map_err(|e| e.to_string())
}

/// Write the marker that survives the folder's removal, even before the plan
/// was ever committed.
fn mark_set_up(around: &Surroundings, root: &Path, lines: &mut Vec<String>) -> Result<(), String> {
    match crate::door::marker::write(around.data_dir.as_deref(), root, around.now)? {
        crate::door::marker::Marked::Written(p) => lines.push(format!("marked this project as set up at {}", p.display())),
        crate::door::marker::Marked::Already(p) => lines.push(format!("this project is already marked as set up at {}", p.display())),
        crate::door::marker::Marked::NoHome => lines.push(
            "no home folder is known (HOME, XDG_DATA_HOME or FAMILIAR_AI_HOME), so this project is not marked as set up on this machine".to_string(),
        ),
    }
    Ok(())
}

/// Add a `.gitignore` line for each laptop-only path git does not already
/// ignore. Refuse if the plan, which must be committed, is ignored.
fn keep_record_ignored(root: &Path, worktree: &Path, lines: &mut Vec<String>) -> Result<(), String> {
    let (record_rel, plan_rel) = paths_in_repository(root, worktree)?;
    let ignored = |rel: &str| familiar_ai_git::is_ignored(worktree, rel).map_err(|e| e.to_string());
    let folder_rel = record_rel.strip_suffix(RECORD_FILE).unwrap_or_default().to_string();
    for name in [RECORD_FILE, JUDGE_LOG, INVESTIGATOR_LOG, RUNS_FOLDER, TRANSCRIPTS_FOLDER, REPORT_FILE] {
        let rel = format!("{folder_rel}{name}");
        let line = format!("{FAMILIAR_FOLDER}/{name}");
        if ignored(&rel)? {
            lines.push(format!("{line} is already ignored by git"));
            continue;
        }
        let gitignore = root.join(".gitignore");
        let mut text = match std::fs::read_to_string(&gitignore) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("cannot read {}: {e}", gitignore.display())),
        };
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&line);
        text.push('\n');
        std::fs::write(&gitignore, text).map_err(|e| format!("cannot write {}: {e}", gitignore.display()))?;
        if !ignored(&rel)? {
            return Err(format!("{line} was added to {} but git still does not ignore it", gitignore.display()));
        }
        lines.push(format!("added {line} to {}", gitignore.display()));
    }
    if ignored(&plan_rel)? {
        return Err(format!(
            "{FAMILIAR_FOLDER}/{PLAN_FILE} is ignored by git, but the plan is committed with the work: take it out of the ignore rules"
        ));
    }
    if ignored(&format!("{folder_rel}{}", crate::door::ADAPTER_FILE))? {
        return Err(format!(
            "{FAMILIAR_FOLDER}/{} is ignored by git, but the adapter is committed with the work like the plan: take it out of the ignore rules",
            crate::door::ADAPTER_FILE
        ));
    }
    Ok(())
}

/// Append `CLAUDE_MD_LINES` to the end of the project's CLAUDE.md after one blank line, or start
/// the file with them; nothing above is read or changed. A file that has the heading is left as it is.
fn add_claude_md_lines(root: &Path) -> Result<String, String> {
    let path = root.join("CLAUDE.md");
    let mut bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    if String::from_utf8_lossy(&bytes).lines().any(|l| l.trim_end() == CLAUDE_MD_HEADING) {
        return Ok("CLAUDE.md already carries the familiar-ai lines".to_string());
    }
    if !bytes.is_empty() {
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(CLAUDE_MD_LINES.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(format!("added the familiar-ai lines to {}", path.display()))
}
