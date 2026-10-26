//! Run tests without holding the record: the door appends `run_reserved`, a
//! detached `familiar-ai run-child` runs the command in its own process group and
//! appends `run_started` and `run_ended`, then reserves the next owed run. Each
//! append is conditional on the run still being live. See
//! docs/decisions.md#runs

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use familiar_ai_facts::{AppendIf, Location, Log, Record, RunCancelRequested, RunEnded, RunReserved, RunStarted, Stream, TestLogTranslation, Truncated};
use familiar_ai_run::{End, RunOutcome};
use sha2::{Digest, Sha256};

use crate::Outcome;
use crate::door::project::{find_project, open_project, open_record, plain, start_dir};
use crate::door::runs::{self, eligibility, Eligibility};
use crate::door::git_program::short;
use crate::door::kept::Streams;
use crate::door::verify_run::{Place, Watched};
use crate::door::{procgroup, FAMILIAR_FOLDER, TENANT};
use crate::input::Surroundings;

/// The detached child's own error log in the kept runs' folder, written only where git ignores it.
pub const RUNNER_LOG: &str = "runner.log";

fn log_file(root: &Path) -> Option<std::fs::File> {
    use crate::door::kept::{ignored, RUNS_FOLDER};
    if !ignored(root, RUNS_FOLDER) {
        return None;
    }
    let dir = root.join(FAMILIAR_FOLDER).join(RUNS_FOLDER);
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::OpenOptions::new().create(true).append(true).open(dir.join(RUNNER_LOG)).ok()
}

pub struct Work {
    pub criterion_id: String,
    pub criterion_rev: String,
    pub commit: String,
    pub command: String,
}

/// The run id, derived from the work, its command and the attempt so racing reservers ask for
/// the same id and the append predicate lets only one through; the part's own run and its
/// touching run at one commit have two commands, so two ids (the owner's plan of 2026-10-21,
/// step 0).
pub fn run_id(w: &Work, attempt: usize) -> String {
    let h = Sha256::digest(format!("{}|{}|{}|{}|{attempt}", w.criterion_id, w.criterion_rev, w.commit, w.command).as_bytes());
    format!("r-{}", h.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>())
}

/// Reserve the run for `w` if it is still free at the moment of the append;
/// `Some(id)` when this call reserved it.
pub fn reserve(record: &Record, log: &mut Log, w: &Work, at: f64, around: &Surroundings) -> Result<Option<String>, String> {
    let Eligibility::Free { attempt, after } = eligibility(log, &w.criterion_id, &w.criterion_rev, &w.commit, &w.command, at) else {
        return Ok(None);
    };
    let id = run_id(w, attempt);
    let fact = RunReserved {
        run_id: id.clone(),
        criterion_id: w.criterion_id.clone(),
        criterion_rev: w.criterion_rev.clone(),
        commit: w.commit.clone(),
        command: w.command.clone(),
        resources: vec![runs::RESOURCE.to_string()],
        wait: format!("{}", around.test_run_wait.as_secs()),
        by: "binary".to_string(),
        after: after.map(|a| format!("{a:.0}")),
    };
    let (c, rev, commit, command) = (w.criterion_id.clone(), w.criterion_rev.clone(), w.commit.clone(), w.command.clone());
    let got = record
        .append_if(&fact, at, TENANT, move |l| matches!(eligibility(l, &c, &rev, &commit, &command, at), Eligibility::Free { .. }))
        .map_err(|e| crate::door::cannot("write its record", &e))?;
    match got {
        AppendIf::Appended(f) => {
            log.push(f);
            Ok(Some(id))
        }
        AppendIf::Refused => Ok(None),
    }
}

/// Reserve the dry run owed (plan A, [`crate::door::dry_run::owed`]) when no run is live at the
/// moment of the append: part-less (`criterion_id` empty), holding the tests, at the latest
/// commit seen (empty before one: the child reads the head). `Some(id)` when this call reserved it.
pub fn reserve_dry_run(record: &Record, log: &mut Log, at: f64, around: &Surroundings) -> Result<Option<String>, String> {
    let Some(command) = crate::door::dry_run::owed(log) else { return Ok(None) };
    if runs::any_live(log, at) {
        return Ok(None);
    }
    let commit = log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let earlier = runs::runs(log).iter().filter(|r| r.dry()).count();
    let h = Sha256::digest(format!("dry_run|{command}|{commit}|{earlier}").as_bytes());
    let id = format!("d-{}", h.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>());
    let fact = RunReserved {
        run_id: id.clone(),
        criterion_id: String::new(),
        criterion_rev: String::new(),
        commit,
        command: command.clone(),
        resources: vec![runs::RESOURCE.to_string()],
        wait: format!("{}", around.test_run_wait.as_secs()),
        by: runs::DRY_RUN.to_string(),
        after: None,
    };
    let got = record
        .append_if(&fact, at, TENANT, move |l| crate::door::dry_run::owed(l).as_deref() == Some(command.as_str()) && !runs::any_live(l, at))
        .map_err(|e| crate::door::cannot("write its record", &e))?;
    match got {
        AppendIf::Appended(f) => {
            log.push(f);
            Ok(Some(id))
        }
        AppendIf::Refused => Ok(None),
    }
}

/// The dry run's whole job in the run's child (plan A): the probes under each reader, then its
/// end and its choice in one open of the record, while it is still live.
fn execute_dry_run(root: &Path, run: &runs::Run, log: &Log, place: &Place, around: &Surroundings, clock: &dyn Fn() -> f64) -> Result<(), String> {
    let id = run.run_id.as_str();
    let commit = if run.commit.is_empty() { place.head.to_string() } else { run.commit.clone() };
    let plan_files = crate::teach::example::planned_files(log);
    let mut first = true;
    let mut on_start = |pgid: i32| -> bool {
        let now = clock();
        let rid = id.to_string();
        if first {
            first = false;
            let fact = RunStarted { run_id: id.to_string(), pgid: pgid.to_string(), pgroup_start: procgroup::start_of(pgid).unwrap_or_else(|| "0".to_string()) };
            return with(root, around.record_wait, |r| r.append_if(&fact, now, TENANT, move |l| runs::find(l, &rid).is_some_and(|x| x.can_start(now))).map_err(|e| e.to_string()))
                .is_ok_and(|a| matches!(a, AppendIf::Appended(_)));
        }
        // each later probe goes on while the run is live and not cancelled
        with(root, around.record_wait, |r| r.read_all().map_err(|e| e.to_string())).is_ok_and(|l| runs::find(&l, &rid).is_some_and(|x| x.live() && !x.cancelled()))
    };
    let chosen = crate::door::dry_run::dry_run(place, id, &run.command, &commit, &plan_files, &mut on_start);
    let rid = id.to_string();
    let (fact, chosen) = match chosen {
        Ok(c) => (RunEnded { run_id: rid.clone(), outcome: "passed".into(), end: "exited".into(), ..Default::default() }, Some(c)),
        Err(why) => {
            note(root, &format!("run {id}: {why}"));
            if why.contains("cancelled") {
                return Ok(()); // whoever cancelled it writes its end
            }
            (RunEnded { run_id: rid.clone(), outcome: "did_not_run".into(), end: "not_started".into(), ..Default::default() }, None)
        }
    };
    let at = clock();
    with(root, around.record_wait, |r| {
        let appended = r.append_if(&fact, at, TENANT, move |l| runs::find(l, &rid).is_some_and(|x| x.live())).map_err(|e| crate::door::cannot("write its record", &e))?;
        if let (AppendIf::Appended(_), Some(c)) = (appended, chosen) {
            r.append(&c, at, TENANT).map_err(|e| crate::door::cannot("write its record", &e))?;
            // the choice's committed mirror, at once (plan A)
            if let Ok(l) = r.read_all() {
                if let Err(e) = crate::door::project_adapter(root, &l) {
                    note(root, &format!("run {id}: {e}"));
                }
            }
        }
        Ok(())
    })?;
    chain(root, around, clock())
}

/// Open the record for one step of `f`, then close it.
fn with<T>(root: &Path, wait: Duration, f: impl FnOnce(&Record) -> Result<T, String>) -> Result<T, String> {
    match open_record(root, wait) {
        Ok(Some(r)) => f(&r),
        Ok(None) => Err("the .familiar folder is gone".to_string()),
        Err(e) => Err(crate::door::cannot("open its record", &e)),
    }
}

/// Start the reserved runs as detached children, or in this process when there
/// is no program to launch; the children are returned so a caller may reap them.
pub fn spawn(root: &Path, around: &Surroundings, ids: &[String]) -> Vec<std::process::Child> {
    let mut children = Vec::new();
    for id in ids {
        match &around.runner {
            Some(exe) => {
                let log = log_file(root);
                let (out, err) = match log.and_then(|f| f.try_clone().ok().map(|g| (g, f))) {
                    Some((g, f)) => (Stdio::from(g), Stdio::from(f)),
                    None => (Stdio::null(), Stdio::null()),
                };
                let spawned = Command::new(exe).args(["run-child", "--run_id", id]).current_dir(root).stdin(Stdio::null()).stdout(out).stderr(err).process_group(0).spawn();
                if let Ok(c) = spawned {
                    children.push(c);
                }
            }
            None => {
                let quiet = Surroundings { run_inline: Duration::ZERO, ..around.clone() };
                if let Err(e) = execute(root, id, &quiet) {
                    note(root, &format!("run {id}: {e}"));
                }
            }
        }
    }
    children
}

/// Drop the children that have exited.
pub fn reap(children: &mut Vec<std::process::Child>) {
    children.retain_mut(|c| c.try_wait().map(|s| s.is_none()).unwrap_or(false));
}

/// Wait at most `slice`, returning sooner once all the given children have exited.
pub fn nap(children: &mut Vec<std::process::Child>, slice: Duration) {
    let had = !children.is_empty();
    let until = Instant::now() + slice;
    loop {
        reap(children);
        if (had && children.is_empty()) || Instant::now() >= until {
            return;
        }
        std::thread::sleep(Duration::from_millis(10).min(slice));
    }
}

fn note(root: &Path, line: &str) {
    use std::io::Write;
    if let Some(mut f) = log_file(root) {
        let _ = writeln!(f, "{line}");
    }
}

fn end_name(e: End) -> &'static str {
    match e {
        End::Exited(_) => "exited",
        End::Signalled(_) => "signalled",
        End::TimedOut => "timed_out",
        End::EmptyCommand => "empty_command",
        End::NoDirectory => "no_directory",
        End::NotStarted => "not_started",
        End::Cancelled => "cancelled",
    }
}

/// The run's end, with the evidence of a run that ran (plan §3): its exit code
/// or signal, each stream cleaned and cut, and a `truncated` row for each
/// stream that was cut or drained. An end with no run (`None`), or a run that
/// never started (not started, empty command, no directory), carries none.
fn ended_fact(id: &str, outcome: &str, end: &str, ran: Option<&RunOutcome>) -> RunEnded {
    let mut fact = RunEnded { run_id: id.to_string(), outcome: outcome.to_string(), end: end.to_string(), ..Default::default() };
    let Some(out) = ran.filter(|o| matches!(o.end, End::Exited(_) | End::Signalled(_) | End::TimedOut)) else { return fact };
    use crate::door::kept::{clean, cut};
    let mut truncated = Vec::new();
    let mut tail = |stream: Stream, text: &str, total_bytes: u64, dropped_bytes: u64| {
        let (kept, dropped) = cut(&clean(text));
        if dropped > 0 || dropped_bytes > 0 {
            truncated.push(Truncated { stream, total_bytes, dropped_bytes });
        }
        kept
    };
    fact.stdout_tail = Some(tail(Stream::Stdout, &out.stdout, out.stdout_total_bytes, out.stdout_dropped_bytes));
    fact.stderr_tail = Some(tail(Stream::Stderr, &out.stderr, out.stderr_total_bytes, out.stderr_dropped_bytes));
    match out.end {
        End::Exited(code) => fact.exit_code = Some(i64::from(code)),
        End::Signalled(n) => fact.signal = Some(i64::from(n)),
        _ => {}
    }
    fact.truncated = (!truncated.is_empty()).then_some(truncated);
    fact
}

/// The run's end and, when it was appended, the run's translation, in one open of the record.
fn ended(root: &Path, around: &Surroundings, fact: RunEnded, translation: Option<TestLogTranslation>, at: f64) -> Result<bool, String> {
    let rid = fact.run_id.clone();
    with(root, around.record_wait, |r| {
        let appended = r
            .append_if(&fact, at, TENANT, move |l| runs::find(l, &rid).is_some_and(|x| x.live()))
            .map(|a| matches!(a, AppendIf::Appended(_)))
            .map_err(|e| crate::door::cannot("write its record", &e))?;
        if let Some(t) = translation.filter(|_| appended) {
            translate(r, &t, at)?;
        }
        Ok(appended)
    })
}

/// Append `t` unless its run already has a translation: one per run, whoever writes first.
pub fn translate(record: &Record, t: &TestLogTranslation, at: f64) -> Result<Option<familiar_ai_facts::Fact>, String> {
    let rid = t.run_id.clone();
    let got = record
        .append_if(t, at, TENANT, move |l| !l.iter().any(|f| f.kind == "test_log_translation" && f.fields.get("run_id").and_then(|v| v.as_str()) == Some(rid.as_str())))
        .map_err(|e| crate::door::cannot("write its record", &e))?;
    Ok(match got {
        AppendIf::Appended(f) => Some(f),
        AppendIf::Refused => None,
    })
}

/// The translation of `run` from its output (plan §1.1, §4): `on` is `full` (the whole cleaned
/// capture, in the run's child) or `cut` (the stored tails, in the backstop). The location is kept
/// only when it is a file git tracks at the run's commit, stored relative to the project.
fn translation_of(root: &Path, run: &runs::Run, end: End, s: &Streams, on: &str) -> Option<TestLogTranslation> {
    translated(root, &run.run_id, &run.criterion_id, &run.commit, &run.command, end, s, on)
}

/// The run's reading by Familiar's own reporter (plan A): its result file `reported`, when it
/// reads ([`crate::door::reported::read`]), is the translation `by: "reporter"`, with the plan's
/// tests that ran and were skipped and the versions; else the tables read the output.
/// A `reader` of `none` (the dry run found no reader that reads this runner right here) reads
/// every run as can't tell, `by: "binary"`, the tables' last line kept as its evidence.
#[allow(clippy::too_many_arguments)]
fn translated_by(root: &Path, run_id: &str, criterion_id: &str, commit: &str, command: &str, end: End, s: &Streams, on: &str, reader: Option<&str>, reported: Option<&str>, plan_files: &[String]) -> Option<TestLogTranslation> {
    if reader == Some("none") {
        let mut t = translated(root, run_id, criterion_id, commit, command, end, s, on)?;
        if t.by != "binary" {
            t.evidence = format!("no reader reads this runner right in this project (the dry run's choice); its output read: {}", t.evidence);
            t.by = "binary".to_string();
            t.label = "unclassified".to_string();
            t.location = None;
            t.missing_name = None;
        }
        return Some(t);
    }
    let exit = match end {
        End::Exited(c) => Some(c),
        _ => None,
    };
    if let Some(r) = reported.and_then(|text| crate::door::reported::read(text, exit, plan_files)) {
        let worktree = familiar_ai_git::worktree_root(root).ok().flatten();
        let mut t = crate::door::kept::test_log_translation(r.translation, run_id, criterion_id, commit, on, |c| worktree.as_deref().and_then(|w| tracked(root, w, commit, c)));
        t.ran_tests = Some(r.ran_tests);
        t.skipped_tests = Some(r.skipped_tests);
        t.versions = Some(r.versions);
        t.frames = (!r.frames.is_empty()).then_some(r.frames);
        return Some(t);
    }
    translated(root, run_id, criterion_id, commit, command, end, s, on)
}

/// The project's folder relative to its repository's, as `reader_chosen.root` names it (`""` at
/// the top).
pub fn root_in_repo(root: &Path) -> String {
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten();
    worktree.and_then(|w| root.strip_prefix(&w).ok().map(|p| p.to_string_lossy().into_owned())).unwrap_or_default()
}

/// A pipeline run's end and translation (plan §6.5): the same clean, cut and classify as a
/// laptop run's, each as the fields the binary writes beyond the move the rules offer (no
/// `run_id`, `criterion_id` or `commit`); no translation for a run that never started.
/// A run that ended `did_not_run` but ran (a crash, a timeout) is translated, as the laptop's is.
pub fn pipeline_reading(root: &Path, command: &str, commit: &str, out: &RunOutcome, report: &str) -> (serde_json::Map<String, serde_json::Value>, Option<serde_json::Map<String, serde_json::Value>>) {
    pipeline_reading_by(root, command, commit, out, report, None, &[])
}

/// [`pipeline_reading`] with Familiar's own reporter's result file `reported`, read first
/// ([`translated_by`]); `plan_files` are the plan's test files.
pub fn pipeline_reading_by(
    root: &Path,
    command: &str,
    commit: &str,
    out: &RunOutcome,
    report: &str,
    reported: Option<&str>,
    plan_files: &[String],
) -> (serde_json::Map<String, serde_json::Value>, Option<serde_json::Map<String, serde_json::Value>>) {
    pipeline_reading_with(root, command, commit, out, report, None, reported, plan_files)
}

/// [`pipeline_reading_by`] under the chosen `reader` (a `none` reads can't tell).
#[allow(clippy::too_many_arguments)]
pub fn pipeline_reading_with(
    root: &Path,
    command: &str,
    commit: &str,
    out: &RunOutcome,
    report: &str,
    reader: Option<&str>,
    reported: Option<&str>,
    plan_files: &[String],
) -> (serde_json::Map<String, serde_json::Value>, Option<serde_json::Map<String, serde_json::Value>>) {
    use crate::door::kept::clean;
    use familiar_ai_facts::FactFields;
    let o = if out.did_not_run().is_some() {
        "did_not_run"
    } else if out.verdict == familiar_ai_run::Verdict::Passed {
        "passed"
    } else {
        "failed"
    };
    let mut end = ended_fact("", o, end_name(out.end), Some(out)).to_fields().unwrap_or_default();
    end.remove("run_id");
    let (stdout, stderr, report) = (clean(&out.stdout), clean(&out.stderr), clean(report));
    let t = translated_by(root, "", "", commit, command, out.end, &Streams { stdout: &stdout, stderr: &stderr, report: &report }, "full", reader, reported, plan_files).and_then(|t| t.to_fields().ok()).map(|mut m| {
        for k in ["run_id", "criterion_id", "commit"] {
            m.remove(k);
        }
        m
    });
    (end, t)
}

#[allow(clippy::too_many_arguments)]
fn translated(root: &Path, run_id: &str, criterion_id: &str, commit: &str, command: &str, end: End, s: &Streams, on: &str) -> Option<TestLogTranslation> {
    let t = crate::door::kept::classify(end, s, familiar_ai_run::details_for(command))?;
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten();
    Some(crate::door::kept::test_log_translation(t, run_id, criterion_id, commit, on, |c| worktree.as_deref().and_then(|w| tracked(root, w, commit, c))))
}

/// The first of `candidates` whose file git tracks at `commit` ([`kept::in_project`]): read as
/// relative to the project, or, when it starts with the project's folder, to the repository;
/// stored relative to the project, with the function git's hunk header names above its line there
/// (`funcname_above`; plan §6.2) and the kind of the row that found it.
fn tracked(root: &Path, worktree: &Path, commit: &str, candidates: &[Location]) -> Option<Location> {
    // as the copy is laid out (verify_run::write_copy)
    let rel = root.strip_prefix(worktree).unwrap_or(Path::new("")).to_string_lossy().into_owned();
    let read = |p: &str| -> Option<Vec<u8>> {
        let in_repo = if rel.is_empty() { p.to_string() } else { format!("{rel}/{p}") };
        familiar_ai_git::file_at(worktree, commit, &in_repo).ok()
    };
    let inner: Vec<Location> = candidates
        .iter()
        .flat_map(|l| {
            let under = l.file_path.strip_prefix(&format!("{rel}/")).filter(|_| !rel.is_empty());
            std::iter::once(l.clone()).chain(under.map(|p| Location { file_path: p.to_string(), ..l.clone() }))
        })
        .collect();
    crate::door::kept::in_project(&inner, &read)
}

/// The backstop (plan §1.1): every run that ended with evidence and has no translation is
/// classified from its stored tails, `classified_on: cut`.
pub fn backstop(record: &Record, root: &Path, log: &mut Log, at: f64) -> Result<usize, String> {
    use serde_json::Value;
    let translated = |l: &Log, id: &str| l.iter().any(|f| f.kind == "test_log_translation" && f.fields.get("run_id").and_then(Value::as_str) == Some(id));
    let owed: Vec<familiar_ai_facts::Fact> = log
        .iter()
        .filter(|f| f.kind == "run_ended" && (f.fields.contains_key("stdout_tail") || f.fields.contains_key("stderr_tail")))
        .filter(|f| f.fields.get("run_id").and_then(Value::as_str).is_some_and(|id| !translated(log, id)))
        .cloned()
        .collect();
    let mut n = 0;
    for f in owed {
        let text = |k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let Some(run) = runs::find(log, &text("run_id")) else { continue };
        let num = |k: &str| f.fields.get(k).and_then(Value::as_i64).and_then(|n| i32::try_from(n).ok());
        let end = match (num("exit_code"), num("signal"), text("end").as_str()) {
            (Some(c), _, _) => End::Exited(c),
            (None, Some(s), _) => End::Signalled(s),
            (None, None, "timed_out") => End::TimedOut,
            _ => continue,
        };
        let (stdout, stderr) = (text("stdout_tail"), text("stderr_tail"));
        let Some(t) = translation_of(root, &run, end, &Streams { stdout: &stdout, stderr: &stderr, report: "" }, "cut") else { continue };
        if let Some(fact) = translate(record, &t, at)? {
            log.push(fact);
            n += 1;
        }
    }
    Ok(n)
}

/// The detached child's whole job for one run, then the chain.
pub fn execute(root: &Path, id: &str, around: &Surroundings) -> Result<(), String> {
    let began = Instant::now();
    let clock = || around.now + began.elapsed().as_secs_f64();
    let log = with(root, around.record_wait, |r| r.read_all().map_err(|e| crate::door::cannot("read its record", &e)))?;
    let Some(run) = runs::find(&log, id) else { return Ok(()) };
    if !run.can_start(clock()) {
        return Ok(());
    }
    if let Some(after) = run.after {
        // A retry waits for its time without holding the record.
        let wait = (after - clock()).clamp(0.0, runs::RETRY_AFTER * 2.0);
        std::thread::sleep(Duration::from_secs_f64(wait));
    }
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten().ok_or("not inside a git repository")?;
    let head = familiar_ai_git::head_commit(&worktree).ok().flatten().ok_or("the repository has no commit")?;
    let place = Place { worktree: &worktree, root, head: &head, wait: Duration::from_secs_f64(run.wait) };
    if run.dry() {
        return execute_dry_run(root, &run, &log, &place, around, &clock);
    }
    let mut on_start = |pgid: i32| -> bool {
        let now = clock();
        let fact = RunStarted { run_id: id.to_string(), pgid: pgid.to_string(), pgroup_start: procgroup::start_of(pgid).unwrap_or_else(|| "0".to_string()) };
        let rid = id.to_string();
        with(root, around.record_wait, |r| {
            r.append_if(&fact, now, TENANT, move |l| runs::find(l, &rid).is_some_and(|x| x.can_start(now))).map_err(|e| e.to_string())
        })
        .is_ok_and(|a| matches!(a, AppendIf::Appended(_)))
    };
    // The planned command names the runner whose env the run gets and whose tables read it; the
    // record's choice for its runner (the adapter, plan A) adds the reader.
    let details = familiar_ai_run::details_for(&run.command);
    let reader = details.and_then(|d| crate::door::readers::chosen_reader(&log, &root_in_repo(root), d.runner));
    let plan_files = crate::teach::example::planned_files(&log);
    let result = crate::door::verify_run::run_commit_read(&place, &run.command, &run.commit, details, reader.as_deref(), &mut on_start);
    let (fact, translation) = match &result {
        Err(why) => {
            note(root, &format!("run {id}: {why}"));
            (ended_fact(id, "did_not_run", "not_started", None), None)
        }
        Ok(w) if w.out.end == End::Cancelled => return Ok(()), // whoever cancelled or voided it writes its end
        Ok(Watched { out, report, reported }) => {
            crate::door::kept::keep_run(root, &run.commit, &out.stdout, &out.stderr);
            let o = if out.did_not_run().is_some() {
                "did_not_run"
            } else if out.verdict == familiar_ai_run::Verdict::Passed {
                "passed"
            } else {
                "failed"
            };
            // classified on the full cleaned capture; the record keeps the cut
            use crate::door::kept::clean;
            let (stdout, stderr, report) = (clean(&out.stdout), clean(&out.stderr), clean(report));
            let streams = Streams { stdout: &stdout, stderr: &stderr, report: &report };
            let translation = translated_by(root, &run.run_id, &run.criterion_id, &run.commit, &run.command, out.end, &streams, "full", reader.as_deref(), reported.as_deref(), &plan_files);
            (ended_fact(id, o, end_name(out.end), Some(out)), translation)
        }
    };
    ended(root, around, fact, translation, clock())?;
    chain(root, around, clock())
}

/// After a run: write the facts it settled and reserve and launch what is owed next.
fn chain(root: &Path, around: &Surroundings, at: f64) -> Result<(), String> {
    let quiet = Surroundings { run_inline: Duration::ZERO, ..around.clone() };
    let ids = with(root, around.record_wait, |r| {
        let mut log = r.read_all().map_err(|e| crate::door::cannot("read its record", &e))?;
        recover(r, &mut log, at)?;
        backstop(r, root, &mut log, at)?;
        Ok(crate::door::run::binary_facts(r, &mut log, at, TENANT, &quiet).map_err(|e| crate::door::cannot("write its record", &e))?.reserved)
    })?;
    spawn(root, &quiet, &ids);
    Ok(())
}

/// End void and expired reservations, killing each started group and confirming
/// it gone first.
pub fn recover(record: &Record, log: &mut Log, now: f64) -> Result<usize, String> {
    let mut n = 0;
    for r in runs::runs(log).into_iter().filter(|r| r.void(now) || r.expired(now)) {
        if let Some(s) = &r.started {
            if !procgroup::kill_and_confirm(s.pgid, &s.pgroup_start, Duration::from_secs(10)) {
                continue; // the reservation stays until the group is confirmed gone
            }
        }
        let fact = RunEnded { run_id: r.run_id.clone(), outcome: "did_not_run".into(), end: "killed".into(), ..Default::default() };
        let rid = r.run_id.clone();
        if let AppendIf::Appended(f) = record.append_if(&fact, now, TENANT, move |l| runs::find(l, &rid).is_some_and(|x| x.live())).map_err(|e| crate::door::cannot("write its record", &e))? {
            log.push(f);
            n += 1;
        }
    }
    Ok(n)
}

/// `familiar-ai run-child --run_id R`: the detached child.
pub fn run_child(id: &str, around: &Surroundings) -> Outcome {
    let Some(root) = find_project(&start_dir(around)) else {
        return plain(1, String::new(), "familiar-ai: run-child: no project here\n".into());
    };
    let around = Surroundings { run_inline: Duration::ZERO, ..around.clone() };
    match execute(&root, id, &around) {
        Ok(()) => plain(0, String::new(), String::new()),
        Err(e) => {
            note(&root, &format!("run {id}: {e}"));
            plain(1, String::new(), format!("familiar-ai: {e}\n"))
        }
    }
}

/// `familiar-ai cancel --run_id R`.
pub fn cancel(id: &str, around: &Surroundings) -> Outcome {
    let (root, record) = match open_project(around) {
        Ok(p) => p,
        Err(o) => return o,
    };
    let now = around.now;
    let rid = id.to_string();
    let got = record.append_if(&RunCancelRequested { run_id: id.to_string() }, now, TENANT, move |l| runs::find(l, &rid).is_some_and(|r| r.live()));
    match got {
        Err(e) => return plain(1, String::new(), format!("familiar-ai: {}\n", crate::door::cannot("write its record", &e))),
        Ok(AppendIf::Refused) => {
            let log = record.read_all().unwrap_or_default();
            let why = if runs::find(&log, id).is_some() { "already finished" } else { "no such run" };
            return plain(1, String::new(), format!("familiar-ai: run {id}: {why}\n"));
        }
        Ok(AppendIf::Appended(_)) => {}
    }
    let log = record.read_all().unwrap_or_default();
    if let Some(s) = runs::find(&log, id).and_then(|r| r.started) {
        if !procgroup::kill_and_confirm(s.pgid, &s.pgroup_start, Duration::from_secs(10)) {
            return plain(1, String::new(), format!("familiar-ai: run {id}: cancel requested, but its process group is not confirmed gone; the reservation stays\n"));
        }
    }
    let rid = id.to_string();
    let fact = RunEnded { run_id: id.to_string(), outcome: "did_not_run".into(), end: "cancelled".into(), ..Default::default() };
    let _ = record.append_if(&fact, now, TENANT, move |l| runs::find(l, &rid).is_some_and(|r| r.live()));
    let _ = root;
    plain(0, format!("run {id} cancelled; nothing it ends with counts as proof\n"), String::new())
}

/// `familiar-ai run`: one catch-up, then print the runs as they stand.
pub fn run_command(around: &Surroundings) -> Outcome {
    let (root, record) = match open_project(around) {
        Ok(p) => p,
        Err(o) => return o,
    };
    let log = match crate::door::catch_up(&record, &root, around) {
        Ok(l) => l,
        Err(m) => return plain(1, String::new(), format!("familiar-ai: {m}\n")),
    };
    let mut out = String::new();
    for r in runs::runs(&log) {
        let state = match (&r.ended, &r.started) {
            (Some(e), _) => format!("ended {} ({}){}", e.outcome, e.end, if r.held() { "" } else { ", not proof" }),
            (None, Some(_)) => "running".to_string(),
            (None, None) => "reserved".to_string(),
        };
        out.push_str(&format!("run {} · {} at {} · {state}\n", r.run_id, r.criterion_id, short(&r.commit)));
    }
    if out.is_empty() {
        out.push_str("no runs\n");
    }
    plain(0, out, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::{Stream, Truncated};
    use familiar_ai_run::{RunOutcome, Verdict};

    /// Two runs of one part at one commit under two commands, its own and its touching tests',
    /// never share an id (the owner's plan of 2026-10-21, step 0); one work asks for one id.
    #[test]
    fn the_own_run_and_the_touching_run_have_two_ids() {
        let w = |command: &str| Work { criterion_id: "c1".into(), criterion_rev: "r".into(), commit: "K".into(), command: command.into() };
        assert_ne!(run_id(&w("pytest tests/test_c1.py"), 0), run_id(&w("pytest tests/test_old.py"), 0));
        assert_eq!(run_id(&w("pytest tests/test_c1.py"), 0), run_id(&w("pytest tests/test_c1.py"), 0));
        assert_ne!(run_id(&w("pytest tests/test_c1.py"), 0), run_id(&w("pytest tests/test_c1.py"), 1));
    }

    fn outcome(end: End, stdout: &str, stderr: &str) -> RunOutcome {
        RunOutcome {
            verdict: Verdict::Failed,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            end,
            stdout_total_bytes: stdout.len() as u64,
            stdout_dropped_bytes: 0,
            stderr_total_bytes: stderr.len() as u64,
            stderr_dropped_bytes: 0,
        }
    }

    #[test]
    fn a_run_that_ran_ends_with_its_exit_code_and_its_cleaned_output() {
        let out = outcome(End::Exited(3), "\u{1b}[31mFAILED\u{1b}[0m test_a\r\n", "oops\r");
        let f = ended_fact("r-1", "failed", "exited", Some(&out));
        assert_eq!((f.run_id.as_str(), f.outcome.as_str(), f.end.as_str()), ("r-1", "failed", "exited"));
        assert_eq!((f.exit_code, f.signal), (Some(3), None));
        assert_eq!(f.stdout_tail.as_deref(), Some("FAILED test_a\n"));
        assert_eq!(f.stderr_tail.as_deref(), Some("oops\n"));
        assert_eq!(f.truncated, None, "nothing was cut or drained");
    }

    #[test]
    fn a_signal_is_kept_and_a_run_with_no_exit_has_no_code() {
        let f = ended_fact("r-1", "did_not_run", "signalled", Some(&outcome(End::Signalled(11), "", "")));
        assert_eq!((f.exit_code, f.signal), (None, Some(11)));
        assert_eq!((f.stdout_tail.as_deref(), f.stderr_tail.as_deref()), (Some(""), Some("")), "it ran: its output, empty, is evidence");
        let f = ended_fact("r-1", "did_not_run", "timed_out", Some(&outcome(End::TimedOut, "last test: t9\n", "")));
        assert_eq!((f.exit_code, f.signal, f.stdout_tail.as_deref()), (None, None, Some("last test: t9\n")));
    }

    #[test]
    fn a_stream_that_was_cut_or_drained_has_a_truncated_row() {
        let long = "x\n".repeat(5_000);
        let mut out = outcome(End::Exited(1), &long, "short\n");
        let f = ended_fact("r-1", "failed", "exited", Some(&out));
        assert_eq!(f.stdout_tail, Some(crate::door::kept::cut(&long).0));
        assert_eq!(f.truncated, Some(vec![Truncated { stream: Stream::Stdout, total_bytes: 10_000, dropped_bytes: 0 }]));
        out.stderr_total_bytes = 9_000_000;
        out.stderr_dropped_bytes = 42;
        let f = ended_fact("r-1", "failed", "exited", Some(&out));
        assert_eq!(
            f.truncated,
            Some(vec![
                Truncated { stream: Stream::Stdout, total_bytes: 10_000, dropped_bytes: 0 },
                Truncated { stream: Stream::Stderr, total_bytes: 9_000_000, dropped_bytes: 42 },
            ]),
            "a drained stream has its row even when its text fits"
        );
    }

    #[test]
    fn a_run_that_never_started_writes_no_evidence() {
        for (end, name) in [(End::NotStarted, "not_started"), (End::EmptyCommand, "empty_command"), (End::NoDirectory, "no_directory")] {
            let out = outcome(end, "sh: partial\n", "cannot start\n");
            let f = ended_fact("r-1", "did_not_run", name, Some(&out));
            assert_eq!(f, RunEnded { run_id: "r-1".into(), outcome: "did_not_run".into(), end: name.into(), ..Default::default() }, "{name}");
        }
    }

    #[test]
    fn an_end_with_no_run_carries_no_evidence() {
        let f = ended_fact("r-1", "did_not_run", "not_started", None);
        assert_eq!(f, RunEnded { run_id: "r-1".into(), outcome: "did_not_run".into(), end: "not_started".into(), ..Default::default() });
    }
}

