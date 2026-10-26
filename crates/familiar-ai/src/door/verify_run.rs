//! Run `verify`'s test command. The head runs in the clone's own work tree;
//! any other commit, and the teeth run, run in a scratch checkout of that
//! commit with the project's untracked setup linked in. The repository is
//! never changed and nothing is installed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use familiar_ai_git::Anchor;
use familiar_ai_run::{RunOutcome, RunnerDetails, Verdict};

use crate::door::git_program::short;

pub enum At<'a> {
    /// The working folder as it is, the head of a clean clone.
    Here,
    /// A checkout of this commit, in a scratch folder.
    Commit(&'a str),
    /// A checkout of the head, in a scratch folder, with these anchors' lines put back.
    PutBack(&'a str, &'a [Anchor]),
}

/// One run's outcome and, when it did not run at all, why; with its end and translation as the
/// laptop's are written (plan §6.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ran {
    pub passed: bool,
    pub exit: Option<i32>,
    pub not_run: Option<String>,
    /// The `run_ended` fields beyond the move: its word (`outcome`), end, exit and tails.
    pub end: serde_json::Map<String, serde_json::Value>,
    /// The `test_log_translation` fields beyond the move; none when it did not run.
    pub translation: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The repository, the project inside it, and the time a run may take.
pub struct Place<'a> {
    pub worktree: &'a Path,
    pub root: &'a Path,
    pub head: &'a str,
    pub wait: Duration,
}

/// Run `command` at `at`; a copy that cannot be written is a run that did
/// not happen.
pub fn run_at(p: &Place, command: &str, at: &At) -> Ran {
    run_at_read(p, command, at, None, &[])
}

/// [`run_at`] under `reader` (CI's own dry run's choice, plan A): its env ops and flags in the
/// run, its result file read first; `plan_files` are the part's test files the reading names.
pub fn run_at_read(p: &Place, command: &str, at: &At, reader: Option<&str>, plan_files: &[String]) -> Ran {
    match at {
        At::Here => {
            let runner = familiar_ai_run::details_for(command);
            let reports = report_folder();
            let read = run_read(p.root, command, false, p.wait, runner, runner.map_or(&[][..], |r| r.env), reader, &reports, None);
            let _ = std::fs::remove_dir_all(&reports);
            ran(p, command, p.head, &read, reader, plan_files)
        }
        At::Commit(commit) => in_copy(p, command, commit, &[], reader, plan_files),
        At::PutBack(head, anchors) => in_copy(p, command, head, anchors, reader, plan_files),
    }
}

/// Run `command` in `dir` as it is (the working tree, nothing copied or committed) under
/// `reader`, with the runner's env and then `extra`, as Familiar's own runs are read: the reader's
/// env ops and flags, its result file read before its report folder is removed. `familiar-ai try`
/// [draft] runs this way.
pub fn run_here_read(dir: &Path, command: &str, wait: Duration, reader: &str, extra: &[(&str, &str)]) -> Watched {
    let runner = familiar_ai_run::details_for(command);
    let mut env: Vec<(&str, &str)> = runner.map_or(&[][..], |r| r.env).to_vec();
    env.extend_from_slice(extra);
    let reports = report_folder();
    let read = run_read(dir, command, false, wait, runner, &env, Some(reader), &reports, None);
    let _ = std::fs::remove_dir_all(&reports);
    read
}

/// A run's own report folder, outside every copy.
fn report_folder() -> PathBuf {
    let n = COPIES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("familiar-ai-report-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Run `command` in `dir` with `env` under `reader` when it can read the command (its env ops
/// after `env`, its flags in the runner's segment, its result file read through its converter
/// from `reports`), else as before (a shell script through its own trace). In its own cargo
/// target when `own_target`.
#[allow(clippy::too_many_arguments)]
fn run_read(dir: &Path, command: &str, own_target: bool, wait: Duration, runner: Option<&RunnerDetails>, env: &[(&str, &str)], reader: Option<&str>, reports: &Path, on_start: Option<&mut dyn FnMut(i32) -> bool>) -> Watched {
    let target = |c: &str| if own_target { in_own_target(c) } else { c.to_string() };
    // a shell script keeps its trace whatever reader comes with it (review finding 3a)
    let traced = runner.is_some_and(RunnerDetails::traced);
    let applied = reader.filter(|id| *id != "none" && !traced).and_then(|id| {
        let kit = crate::door::readers::kit_dir().ok()?;
        let a = crate::door::readers::apply(id, command, &kit, reports).ok()?;
        std::fs::create_dir_all(reports).ok()?;
        Some((id, a))
    });
    let started = std::time::SystemTime::now();
    let Some((id, a)) = applied else {
        let (out, trace) = familiar_ai_run::run_test_command_traced(&target(command), dir, wait, on_start, env, runner);
        let report = runner.map(|r| read_reports_since(dir, r.reports, Some(started))).unwrap_or_default() + &trace;
        return Watched { out, report, reported: None };
    };
    let mut all: Vec<familiar_ai_run::EnvOp> = env.iter().map(|(k, v)| familiar_ai_run::EnvOp::Set(k, v)).collect();
    all.extend(a.ops.iter().map(crate::door::readers::Op::as_env));
    let before = crate::door::readers::reports_before(id, dir);
    let out = familiar_ai_run::run_test_command_ops(&target(&a.command), dir, wait, on_start, &all);
    let report = runner.map(|r| read_reports_since(dir, r.reports, Some(started))).unwrap_or_default();
    let exit = match out.end {
        familiar_ai_run::End::Exited(c) => Some(c),
        _ => None,
    };
    let left = crate::door::readers::Left { stdout: (out.stdout_dropped_bytes == 0).then_some(out.stdout.as_str()), run_dir: dir, report_dir: reports, started, exit, before: &before };
    let reported = crate::door::readers::result_file(id, &left);
    Watched { out, report, reported }
}

/// What a watched run produced: its outcome, and the text of its runner's
/// report files (empty when it has none).
#[derive(Debug)]
pub struct Watched {
    pub out: RunOutcome,
    pub report: String,
    /// The text of Familiar's own reporter's result file, when the run had a reporter and the
    /// file was written.
    pub reported: Option<String>,
}

/// Run `command` in a copy of `commit` with `runner`'s env, telling `on_start`
/// the process group as soon as it exists; `false` from `on_start` stops the
/// run. The runner's report files are read before the copy is removed, and a
/// shell script's trace (H-2) after them.
pub fn run_commit_watched(p: &Place, command: &str, commit: &str, runner: Option<&RunnerDetails>, on_start: &mut dyn FnMut(i32) -> bool) -> Result<Watched, String> {
    run_commit_read(p, command, commit, runner, None, on_start)
}

/// [`run_commit_watched`] with `reader` (a [`crate::door::readers::READERS`] id) applied: its env
/// changes after the runner's and its flags in the runner's segment of Familiar's own command,
/// its report folder a temp folder outside the copy; the run's per-test result file (the kit's,
/// or its stdout or report files through the reader's converter, reports older than the run
/// ignored) read before the copy and the folder are removed. A reader that cannot read this
/// command, or a kit that cannot be written, leaves the run to the tables.
pub fn run_commit_read(p: &Place, command: &str, commit: &str, runner: Option<&RunnerDetails>, reader: Option<&str>, on_start: &mut dyn FnMut(i32) -> bool) -> Result<Watched, String> {
    let n = COPIES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("familiar-ai-run-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let reports = report_folder();
    let env = runner.map_or(&[][..], |r| r.env);
    let out = write_copy(p, &dir, commit, &[]).map(|run_dir| {
        let imports = own_imports(p.worktree, &dir);
        let env = with_imports(env, &imports);
        // a shell script is read from its own trace (H5); a reader's env, flags and kit (plan A)
        // apply to the runners that have one, never to sh
        run_read(&run_dir, command, true, p.wait, runner, &env, reader, &reports, Some(on_start))
    });
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&reports);
    out
}

/// Call `f` with a copy of `commit` (named as a run's copy, so `copy_relative` strips it): the
/// project's folder in it and the copy's own `PYTHONPATH`, if any. The copy is removed after.
pub fn with_copy<T>(p: &Place, commit: &str, f: impl FnOnce(&Path, &Option<String>) -> T) -> Result<T, String> {
    let n = COPIES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("familiar-ai-run-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = write_copy(p, &dir, commit, &[]).map(|run_dir| {
        let imports = own_imports(p.worktree, &dir);
        f(&run_dir, &imports)
    });
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Run `command` in `run_dir` (a copy's) as a run of the copy runs it: in its own cargo target,
/// with `runner`'s env, the copy's `imports`, then `ops`.
pub fn run_in_copy(run_dir: &Path, command: &str, wait: Duration, runner: Option<&RunnerDetails>, imports: &Option<String>, ops: &[crate::door::readers::Op], on_start: &mut dyn FnMut(i32) -> bool) -> RunOutcome {
    let env = with_imports(runner.map_or(&[][..], |r| r.env), imports);
    let mut all: Vec<familiar_ai_run::EnvOp> = env.iter().map(|(k, v)| familiar_ai_run::EnvOp::Set(k, v)).collect();
    all.extend(ops.iter().map(crate::door::readers::Op::as_env));
    familiar_ai_run::run_test_command_ops(&in_own_target(command), run_dir, wait, Some(on_start), &all)
}

/// The files under `dir` that `glob` matches (`*` and `?` within a name, `**` any depth of
/// folders), relative to it, in path order.
pub fn matching(dir: &Path, glob: &str) -> Vec<String> {
    let segments: Vec<&str> = glob.split('/').filter(|s| !s.is_empty()).collect();
    let mut found = Vec::new();
    expand(dir, String::new(), &segments, &mut found);
    found.sort();
    found.dedup();
    found
}

/// The text of the files under `dir` that `globs` match (`*` and `?` within a
/// name, `**` any depth of folders), each glob's files in path order, each file
/// once, after the line `[familiar: report <path>]`.
pub fn read_reports(dir: &Path, globs: &[&str]) -> String {
    read_reports_since(dir, globs, None)
}

/// [`read_reports`], leaving out a file last written before `since`: a report the run did not
/// write (one committed with the project, or left by an earlier run) is never read as the run's.
pub fn read_reports_since(dir: &Path, globs: &[&str], since: Option<std::time::SystemTime>) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut text = String::new();
    for g in globs {
        let segments: Vec<&str> = g.split('/').filter(|s| !s.is_empty()).collect();
        let mut found = Vec::new();
        expand(dir, String::new(), &segments, &mut found);
        found.sort();
        found.dedup();
        for rel in found {
            if seen.contains(&rel) {
                continue;
            }
            let stale = since.is_some_and(|s| std::fs::metadata(dir.join(&rel)).and_then(|m| m.modified()).map_or(true, |m| m < s));
            if stale {
                continue;
            }
            let Ok(bytes) = std::fs::read(dir.join(&rel)) else { continue };
            text.push_str(&format!("[familiar: report {rel}]\n{}", String::from_utf8_lossy(&bytes)));
            if !text.ends_with('\n') {
                text.push('\n');
            }
            seen.push(rel);
        }
    }
    text
}

fn expand(dir: &Path, rel: String, segments: &[&str], found: &mut Vec<String>) {
    let join = |name: &str| if rel.is_empty() { name.to_string() } else { format!("{rel}/{name}") };
    let Some((seg, rest)) = segments.split_first() else {
        if !rel.is_empty() && dir.join(&rel).is_file() {
            found.push(rel);
        }
        return;
    };
    if !seg.contains(['*', '?']) {
        return expand(dir, join(seg), rest, found);
    }
    let Ok(entries) = std::fs::read_dir(dir.join(&rel)) else { return };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
        if *seg == "**" {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                expand(dir, join(&name), segments, found);
            }
        } else if wildcard(seg, &name) {
            expand(dir, join(&name), rest, found);
        }
    }
    if *seg == "**" {
        expand(dir, rel, rest, found);
    }
}

/// Whether `name` matches `pattern`, where `*` is any run of characters and `?` one.
fn wildcard(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|c| *c == '*')
}

fn ran(p: &Place, command: &str, commit: &str, read: &Watched, reader: Option<&str>, plan_files: &[String]) -> Ran {
    let (out, report) = (&read.out, read.report.as_str());
    let wait = p.wait;
    let exit = match out.end {
        familiar_ai_run::End::Exited(code) => Some(code),
        _ => None,
    };
    let not_run = out.did_not_run().map(|why| match out.end {
        familiar_ai_run::End::TimedOut => format!("{why} ({} s)", wait.as_secs()),
        _ => why,
    });
    let (end, translation) = crate::door::runner::pipeline_reading_with(p.root, command, commit, out, report, reader, read.reported.as_deref(), plan_files);
    Ran { passed: out.verdict == Verdict::Passed, exit, not_run, end, translation }
}

/// A run that did not happen: it ends `did_not_run`, untranslated.
pub fn not_run(why: String) -> Ran {
    let mut end = serde_json::Map::new();
    end.insert("outcome".into(), "did_not_run".into());
    end.insert("end".into(), "not_started".into());
    Ran { passed: false, exit: None, not_run: Some(why), end, translation: None }
}

static COPIES: AtomicUsize = AtomicUsize::new(0);

fn in_copy(p: &Place, command: &str, commit: &str, anchors: &[Anchor], reader: Option<&str>, plan_files: &[String]) -> Ran {
    let n = COPIES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("familiar-ai-verify-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let reports = report_folder();
    let runner = familiar_ai_run::details_for(command);
    let out = write_copy(p, &dir, commit, anchors).map(|run_dir| {
        let imports = own_imports(p.worktree, &dir);
        let env = with_imports(runner.map_or(&[][..], |r| r.env), &imports);
        let read = run_read(&run_dir, command, true, p.wait, runner, &env, reader, &reports, None);
        ran(p, command, commit, &read, reader, plan_files)
    });
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&reports);
    out.unwrap_or_else(not_run)
}

/// Write the copy and return the project's folder inside it, where the command runs.
fn write_copy(p: &Place, dir: &Path, commit: &str, anchors: &[Anchor]) -> Result<PathBuf, String> {
    let cannot = |what: &str, e: &dyn std::fmt::Display| format!("the copy of {} could not be {what}: {e}", short(commit));
    familiar_ai_git::scratch_at(p.worktree, dir, commit).map_err(|e| cannot("written", &e))?;
    for a in anchors {
        put_back(p.worktree, dir, a).map_err(|e| cannot("given the anchor's lines back", &e))?;
    }
    link_setup(p, dir).map_err(|e| cannot("linked to the project's setup", &e))?;
    let rel = p.root.strip_prefix(p.worktree).unwrap_or(Path::new(""));
    Ok(dir.join(rel))
}

/// Put one anchor's lines back in the copy as they were at its sha; an anchor
/// of lines 0..0 puts back the whole file.
fn put_back(worktree: &Path, dir: &Path, a: &Anchor) -> Result<(), String> {
    if a.line_start == 0 && a.line_end == 0 {
        return familiar_ai_git::file_put_back(worktree, dir, &a.sha, &a.file_path).map_err(|e| e.to_string());
    }
    let earlier = familiar_ai_git::file_at(worktree, &a.sha, &a.file_path).map_err(|e| e.to_string())?;
    let file = dir.join(&a.file_path);
    let tip = std::fs::read(&file).map_err(|e| format!("{}: {e}", a.file_path))?;
    let hunks = familiar_ai_git::line_hunks(&earlier, &tip);
    let rebuilt = familiar_ai_git::lines_put_back(&earlier, &tip, &hunks, &a.file_path, a.line_start, a.line_end)
        .map_err(|e| e.to_string())?;
    std::fs::write(&file, rebuilt).map_err(|e| format!("{}: {e}", a.file_path))
}

/// The command with `CARGO_TARGET_DIR` unset, so the copy builds in its own
/// `target`, which is removed with the copy.
fn in_own_target(command: &str) -> String {
    format!("unset CARGO_TARGET_DIR\n{command}")
}

/// Link the working folder's untracked top-level entries, the project's own
/// setup, into the copy. `target` is never linked: a copy building there bakes
/// its own soon-gone path into the project's test binaries.
fn link_setup(p: &Place, dir: &Path) -> Result<(), String> {
    let tracked = familiar_ai_git::top_level_names(p.worktree, p.head).map_err(|e| e.to_string())?;
    let entries = std::fs::read_dir(p.worktree).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name == ".git" || name == "target" || tracked.iter().any(|t| t == name) || dir.join(name).exists() {
            continue;
        }
        std::os::unix::fs::symlink(entry.path(), dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}

/// The `PYTHONPATH` a run in the copy at `dir` gets, so its Python imports the
/// copy and not the working tree: each entry under the working tree becomes
/// its twin in the copy (the same path under `dir`), and the twins of the
/// linked venvs' editable pointers go first, ahead of the `.pth` lines and the
/// finders that would read the tree. A path with no twin in the copy (an
/// untracked folder) is left as it is. `None` leaves the variable alone. The
/// venvs are only read.
fn own_imports(worktree: &Path, dir: &Path) -> Option<String> {
    let trees: Vec<PathBuf> = std::iter::once(worktree.to_path_buf()).chain(std::fs::canonicalize(worktree)).collect();
    let twin = |path: &Path| {
        let rel = trees.iter().find_map(|t| path.strip_prefix(t).ok())?;
        Some(dir.join(rel)).filter(|t| t.exists())
    };
    let mut first: Vec<PathBuf> = Vec::new();
    for pointer in editable_pointers(worktree, dir) {
        if let Some(t) = twin(&pointer) {
            if !first.contains(&t) {
                first.push(t);
            }
        }
    }
    let mut changed = !first.is_empty();
    let given = std::env::var_os("PYTHONPATH");
    let entries: Vec<PathBuf> = given
        .iter()
        .flat_map(std::env::split_paths)
        .map(|e| match twin(&e) {
            Some(t) => {
                changed = true;
                t
            }
            None => e,
        })
        .collect();
    if !changed {
        return None;
    }
    let all = std::env::join_paths(first.into_iter().chain(entries)).ok()?;
    all.into_string().ok()
}

/// The folders a linked venv's editable installs read from the working tree:
/// each absolute path line of a `site-packages/*.pth`, and the folder holding
/// each `MAPPING` value of a setuptools `__editable___*_finder.py`. A venv is a
/// top-level entry linked into the copy at `dir` that holds `pyvenv.cfg`.
fn editable_pointers(worktree: &Path, dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(worktree).into_iter().flatten().flatten() {
        let venv = entry.path();
        if !dir.join(entry.file_name()).is_symlink() || !venv.join("pyvenv.cfg").is_file() {
            continue;
        }
        for lib in std::fs::read_dir(venv.join("lib")).into_iter().flatten().flatten() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(lib.path().join("site-packages")).into_iter().flatten().flatten().map(|f| f.path()).collect();
            files.sort();
            for file in files {
                let Some(name) = file.file_name().and_then(|n| n.to_str()) else { continue };
                let Ok(text) = std::fs::read_to_string(&file) else { continue };
                if name.ends_with(".pth") {
                    found.extend(text.lines().map(str::trim).filter(|l| l.starts_with('/')).map(PathBuf::from));
                } else if name.starts_with("__editable___") && name.ends_with("_finder.py") {
                    found.extend(mapping_values(&text).iter().filter_map(|v| Path::new(v).parent().map(Path::to_path_buf)));
                }
            }
        }
    }
    found
}

/// The values of the dict literal `MAPPING` in a setuptools editable finder:
/// every second quoted string between its braces.
fn mapping_values(text: &str) -> Vec<String> {
    let Some(start) = text.find("\nMAPPING").map(|i| i + 1).or_else(|| text.starts_with("MAPPING").then_some(0)) else { return Vec::new() };
    let rest = &text[start..];
    let Some(body) = rest.find('{').and_then(|open| rest[open..].find('}').map(|close| &rest[open + 1..open + close])) else { return Vec::new() };
    let mut strings = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\'' && c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(d) = chars.next() {
            match d {
                '\\' => s.extend(chars.next()),
                d if d == c => break,
                d => s.push(d),
            }
        }
        strings.push(s);
    }
    strings.into_iter().skip(1).step_by(2).collect()
}

/// `env` and, when there is one, the copy's own `PYTHONPATH` after it.
fn with_imports<'a>(env: &[(&'a str, &'a str)], imports: &'a Option<String>) -> Vec<(&'a str, &'a str)> {
    let mut all = env.to_vec();
    if let Some(path) = imports {
        all.push(("PYTHONPATH", path));
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("familiar-ai-reports-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn put(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(dir));
        let _ = std::fs::write(path, text);
    }

    #[test]
    fn the_report_files_a_glob_matches_are_read_in_order_each_after_its_name() {
        let dir = scratch("read");
        put(&dir, "target/surefire-reports/b.Test.txt", "Tests run: 1, Failures: 1\n");
        put(&dir, "target/surefire-reports/a.Test.xml", "<testsuite failures=\"1\"/>");
        put(&dir, "target/surefire-reports/notes.log", "not a report\n");
        put(&dir, "target/other/c.txt", "elsewhere\n");
        let maven = &["target/surefire-reports/*.txt", "target/surefire-reports/*.xml"];
        assert_eq!(
            read_reports(&dir, maven),
            "[familiar: report target/surefire-reports/b.Test.txt]\nTests run: 1, Failures: 1\n\
             [familiar: report target/surefire-reports/a.Test.xml]\n<testsuite failures=\"1\"/>\n"
        );
        put(&dir, "build/test-results/test/TEST-a.xml", "<a/>\n");
        put(&dir, "build/test-results/test/deep/TEST-b.xml", "<b/>\n");
        put(&dir, "build/test-results/TEST-top.xml", "<top/>\n");
        put(&dir, "build/test-results/test/binary/output.bin", "x");
        assert_eq!(
            read_reports(&dir, &["build/test-results/**/*.xml"]),
            "[familiar: report build/test-results/TEST-top.xml]\n<top/>\n\
             [familiar: report build/test-results/test/TEST-a.xml]\n<a/>\n\
             [familiar: report build/test-results/test/deep/TEST-b.xml]\n<b/>\n",
            "** is any depth, the top included"
        );
        assert_eq!(read_reports(&dir, &[]), "", "no globs, no report");
        assert_eq!(read_reports(&dir.join("gone"), maven), "", "nothing there, no report");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_report_written_before_the_run_is_not_the_runs() {
        // an all-pass surefire file committed with the project, or left by an earlier run
        let dir = scratch("stale");
        put(&dir, "target/surefire-reports/TEST-Old.xml", "<testsuite tests=\"3\" failures=\"0\"/>");
        std::thread::sleep(Duration::from_millis(20));
        let started = std::time::SystemTime::now();
        std::thread::sleep(Duration::from_millis(20));
        put(&dir, "target/surefire-reports/TEST-New.xml", "<testsuite tests=\"1\" failures=\"1\"/>");
        let read = read_reports_since(&dir, &["target/surefire-reports/*.xml"], Some(started));
        assert!(read.contains("TEST-New.xml") && !read.contains("TEST-Old.xml"), "{read}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn repo(name: &str, files: &[(&str, &str)]) -> (PathBuf, String) {
        let dir = scratch(name);
        let git = |args: &[&str]| {
            let _ = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .status();
        };
        git(&["init", "-q"]);
        for (rel, text) in files {
            put(&dir, rel, text);
            let _ = Command::new("chmod").arg("+x").arg(dir.join(rel)).status();
        }
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "files"]);
        let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap_or_default();
        (dir, head)
    }

    #[test]
    fn a_run_returns_its_runners_report_read_before_the_copy_is_removed() {
        let script = "#!/bin/sh\nmkdir -p target/surefire-reports\necho 'Tests run: 2, Failures: 1' > target/surefire-reports/x.ATest.txt\nexit 1\n";
        let (dir, head) = repo("watched", &[("mvnw", script)]);
        let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
        let command = "./mvnw -q test";
        let ran = run_commit_watched(&p, command, &head, familiar_ai_run::details_for(command), &mut |_| true).expect("ran");
        assert_eq!(ran.out.end, familiar_ai_run::End::Exited(1));
        assert_eq!(ran.report, "[familiar: report target/surefire-reports/x.ATest.txt]\nTests run: 2, Failures: 1\n");
        assert!(!dir.join("target/surefire-reports").exists(), "the report was written in the copy, not the worktree");
        let none = run_commit_watched(&p, command, &head, None, &mut |_| true).expect("ran");
        assert_eq!(none.report, "", "no row, no report read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_copy_links_the_untracked_top_level_entries_and_never_a_tracked_one() {
        let (dir, first) = repo("link", &[("src/app.sh", "echo a\n")]);
        put(&dir, "later.txt", "tracked at the head, not at the first commit\n");
        let _ = Command::new("git").args(["add", "later.txt"]).current_dir(&dir).status();
        let _ = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "later"])
            .current_dir(&dir)
            .status();
        let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap_or_default();
        assert_ne!(head, first, "the second commit landed");
        put(&dir, ".venv/pyvenv.cfg", "home = /usr/bin\n");
        put(&dir, "build/out.txt", "built\n");
        put(&dir, "target/debug/x", "built\n");
        let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
        let copy = scratch("link-copy");
        let _ = std::fs::remove_dir(&copy);
        write_copy(&p, &copy, &first, &[]).expect("the copy");
        for name in [".venv", "build"] {
            let link = std::fs::read_link(copy.join(name));
            assert_eq!(link.ok(), Some(dir.join(name)), "{name}: untracked, linked to the working tree's");
        }
        for name in ["src", ".git"] {
            assert!(!copy.join(name).is_symlink(), "{name}: the copy's own");
        }
        assert!(!copy.join("later.txt").exists(), "tracked at the head: never linked, though the first commit lacks it");
        assert!(!copy.join("target").exists(), "target is never linked");
        let _ = std::fs::remove_dir_all(&copy);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_gets_its_runners_env() {
        let (dir, head) = repo("env", &[("ctest", "#!/bin/sh\necho \"on_failure=${CTEST_OUTPUT_ON_FAILURE-unset}\"\n")]);
        let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(30) };
        let ran = run_commit_watched(&p, "./ctest", &head, familiar_ai_run::details_for("./ctest"), &mut |_| true).expect("ran");
        assert_eq!(ran.out.stdout.trim(), "on_failure=1");
        let plain = run_commit_watched(&p, "./ctest", &head, None, &mut |_| true).expect("ran");
        assert_eq!(plain.out.stdout.trim(), "on_failure=unset");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

