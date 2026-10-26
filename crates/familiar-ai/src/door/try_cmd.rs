//! `familiar-ai try` [draft name] (the owner's plan of 2026-10-23, change C): a look at a pytest
//! part's planned test before its place is chosen.
//!
//! ```text
//!   the part's command (trace::command_of) ──▶ the working tree, as it is, under the pytest kit
//!                                              with FAMILIAR_AI_TRACE=1 (nothing committed)
//!   the kit's report ──▶ red: a test of the planned files failed or errored
//!                    ──▶ the path: the union of the calls of those tests
//!                    ──▶ ranked by `rank` (the seam; words now) against trace::answer_words
//!                    ──▶ .familiar/runs/<run_id>.trace.json (the whole ranked path, not committed)
//!                    ──▶ trace_listed (the top 25), the binary's fact
//! ```
//!
//! A try is never a run of the record: it writes no `run_reserved`, `run_started`, `run_ended`,
//! `test_log_translation` or result, only `trace_listed`, whose `run_id` is the try's own id
//! (`try-<part>-<n>`) and names its trace file, never a reserved run. Every reader of runs, the
//! seal's among them (`outcome::seal_run_of` reads the results), stays blind to it.

use std::path::Path;
use std::time::Instant;

use familiar_ai_facts::{Log, TraceListed, TracedFunction};
use familiar_ai_rules::{lifecycle, plan, trace, View};
use serde_json::{json, Value};

use crate::Outcome;
use crate::door::git_program::short;
use crate::door::kept::RUNS_FOLDER;
use crate::door::project::{open_project, plain};
use crate::door::try_rank::{Called, Ranked};
use crate::door::{catch_up, cannot, FAMILIAR_FOLDER, TENANT};
use crate::input::Surroundings;
use crate::teach::words::LEAD;

/// The reader a try runs under: the pytest kit, where the call recorder lives.
pub const TRY_READER: &str = "pytest:reporter";
/// How many listed functions the agent and `status` are shown.
pub const SHOWN: usize = 12;
/// The words for a try that ran its test and saw it pass [draft words].
pub const PASSES: &str = "the test passes: it doesn't show the problem yet; change the test and try again";

/// THE RANKER SEAM [draft]: rank the functions a failing test ran (`called`, each one's text read
/// through `read`, a file path relative to the root) against `query` (the person's words, with
/// their answer to the one question); returns the ranker's name, as `trace_listed.ranker` names
/// it, and every called function at its 1-based rank, best first.
///
/// Words only for now (`try_rank::rank_words`, BM25, "words"). The embedder + words ranker of the
/// search-by-meaning lane (`lane-search-int`: cosine with the local embedder, 0.6, rank-fused
/// with word match, 0.4, as measured) plugs in here when it is set up, and then returns
/// "embedder+words"; nothing else changes.
pub fn rank(query: &str, called: &[Called], read: impl Fn(&str) -> Option<String>) -> (&'static str, Vec<Ranked>) {
    ("words", crate::door::try_rank::rank_words(query, called, read))
}

/// `familiar-ai try [--criterion <id>] [--list all]`.
pub fn run_try(criterion: Option<&str>, list: Option<&str>, around: &Surroundings) -> Outcome {
    let began = Instant::now();
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    if let Some(l) = list {
        if l != "all" {
            return refused(&format!("--list takes all, not {l}"));
        }
        let log = match record.read_all() {
            Ok(log) => log,
            Err(e) => return refused(&cannot("read its record", &e)),
        };
        drop(record);
        return list_all(&root, &log, criterion, around);
    }
    let log = match catch_up(&record, &root, around) {
        Ok(log) => log,
        Err(m) => return refused(&m),
    };
    let v = View::new(&log);
    let c = match criterion.map(str::to_string).or_else(|| part_of(&v, around.session_id.as_deref())) {
        Some(c) if plan::criteria(&v).contains(&c) => c,
        Some(c) => return refused(&format!("no part {c} in the plan")),
        None => return refused("no part here runs its tests under pytest, the one runner with a call recorder: no try is asked; choose the place as before"),
    };
    if let Some(why) = why_not(&v, &c) {
        return refused(&why);
    }
    let Some(command) = trace::command_of(&v, &c) else {
        return refused(&format!("{c} has no test command to try: claim test_stack_detected with its --test_command"));
    };
    let kit = match crate::door::readers::kit_dir() {
        Ok(k) => k,
        Err(e) => return refused(&format!("the try cannot write the pytest kit: {e}")),
    };
    if let Err(why) = crate::door::readers::apply(TRY_READER, &command, &kit, &std::env::temp_dir()) {
        return refused(&format!("the try cannot record `{command}`: {why}"));
    }
    let Some(worktree) = familiar_ai_git::worktree_root(&root).ok().flatten() else {
        return refused("the project is not inside a git repository");
    };
    let head = familiar_ai_git::head_commit(&worktree).ok().flatten().unwrap_or_default();
    let files = familiar_ai_rules::criterion::test_files(&v, &c);
    let query = query_of(&v, &c);
    let run_id = format!("try-{}-{}", safe(&c), v.of(trace::LISTED, &c).count() + 1);
    // the record is let go while the test runs
    let ran = record.while_released(around.record_wait, || {
        // nothing written beside the sources: no bytecode from the try's own run
        // the recorder is rooted at the project, whatever pytest's rootdir: the list's files are
        // read from here
        let at = root.to_string_lossy().into_owned();
        let extra = [("FAMILIAR_AI_TRACE", "1"), ("PYTHONDONTWRITEBYTECODE", "1"), ("FAMILIAR_AI_TRACE_ROOT", at.as_str())];
        crate::door::verify_run::run_here_read(&root, &command, around.test_run_wait, TRY_READER, &extra)
    });
    let watched = match ran {
        Ok(w) => w,
        Err(e) => return refused(&cannot("reopen its record", &e)),
    };
    let Some(report) = watched.reported.as_deref().filter(|r| has_session(r)) else {
        let why = watched.out.did_not_run().unwrap_or_else(|| "the kit's report was not written".to_string());
        let tail = crate::door::kept::clean(&watched.out.stderr);
        let tail = tail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        return refused(&format!("the try of {c} could not be read ({why}){}; nothing was listed", if tail.is_empty() { String::new() } else { format!(": {tail}") }));
    };
    let read = read_report(report, &files);
    let (ranker, ranked) = if read.red { rank(&query, &read.path, |f| std::fs::read_to_string(root.join(f)).ok()) } else { ("words", Vec::new()) };
    let fact = TraceListed {
        criterion_id: c.clone(),
        run_id: run_id.clone(),
        sha: head.clone(),
        red: read.red,
        path_size: ranked.len() as u64,
        ranker: ranker.to_string(),
        query: query.clone(),
        top: ranked.iter().take(trace::MOST_LISTED).map(listed).collect(),
    };
    let kept = keep(&root, &run_id, &fact, &command, &read.failing, &ranked);
    let at = around.now + began.elapsed().as_secs_f64();
    if let Err(e) = record.append(&fact, at, TENANT) {
        return refused(&cannot("write its record", &e));
    }
    plain(0, told(&c, &head, &files, &read, &fact, &ranked, kept), String::new())
}

/// A refusal: one line, exit 1.
fn refused(why: &str) -> Outcome {
    plain(1, String::new(), format!("{LEAD}try: {why}\n"))
}

/// The id as a file name's part: letters, digits, `-` and `_`, anything else `_`.
fn safe(id: &str) -> String {
    id.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' }).collect()
}

/// Why `c` may not be tried now, in one line; `None` when it may.
fn why_not(v: &View, c: &str) -> Option<String> {
    if !trace::traced(v, c) {
        return Some(format!("{c}'s tests do not run under pytest, the one runner with a call recorder: no try is asked; choose its place as before"));
    }
    if !plan::test_may_be_written(v, c) {
        let missing: Vec<&str> = ["line_1", "line_2", "line_3", "line_6", "line_7", "line_8", "line_9", "line_10"]
            .into_iter()
            .filter(|l| !plan::line(v, l).unwrap_or(true))
            .collect();
        let missing = if missing.is_empty() { String::new() } else { format!(" ({} not held)", missing.join(", ").replace("line_", "line ")) };
        return Some(format!("{c}'s test may not be written yet: lines 1-3 and 6-10 come first{missing}; familiar-ai status shows the missing line"));
    }
    // the person's card on the part, as the gate on the planned test reads it
    let card = lifecycle::holds(v, c).iter().find(|h| h.why == trace::REPRO || familiar_ai_rules::criterion::answers_of(&h.why).is_some()).map(|h| h.why.clone());
    card.map(|h| format!("the person's card {h} is open on {c}; the try waits for their answer"))
}

/// The part a try with no `--criterion` is of: among the traced parts whose test may be written,
/// held by this session first (any, with no session), then those whose turn on their string has
/// come, the first that waits on a try (`try_owed`, `try_not_red`), else the first.
fn part_of(v: &View, session: Option<&str>) -> Option<String> {
    let cs: Vec<String> = plan::criteria(v).into_iter().filter(|c| trace::traced(v, c) && plan::test_may_be_written(v, c)).collect();
    let mine = |c: &String| session.is_none_or(|s| plan::assigned_to(v, c).and_then(|h| familiar_ai_rules::ladder::session_of(Some(h))) == Some(s));
    let turn = |c: &String| plan::string_before(v, c).is_none_or(|b| lifecycle::released(v, &b) || matches!(lifecycle::state(v, &b), "green" | "proven"));
    let waits = |c: &String| trace::try_hold(v, c).is_some();
    cs.iter()
        .find(|c| mine(c) && turn(c) && waits(c))
        .or_else(|| cs.iter().find(|c| mine(c) && turn(c)))
        .or_else(|| cs.iter().find(|c| mine(c)))
        .or_else(|| cs.iter().find(|c| turn(c) && waits(c)))
        .or_else(|| cs.first())
        .cloned()
}

/// The ranking query: the person's words for `c` (the prompt its intent names, else the intent's
/// text), then the words they typed on the one question's card.
pub fn query_of(v: &View, c: &str) -> String {
    let words = familiar_ai_rules::ask::words_of(v, c)
        .map(str::to_string)
        .or_else(|| familiar_ai_rules::ask::intent_of(v, c).and_then(|i| i.fields.get("text")).and_then(Value::as_str).map(str::to_string));
    [words, trace::answer_words(v, c)].into_iter().flatten().filter(|w| !w.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

/// What a try's report says about the planned files.
#[derive(Debug, Default)]
struct Read {
    /// A test of the planned files failed or errored, or one of them could not be collected.
    red: bool,
    /// How many tests of the planned files ran.
    ran: usize,
    /// The failing tests' ids.
    failing: Vec<String>,
    /// The union of the failing tests' calls, each function once at its latest call.
    path: Vec<Called>,
}

/// Whether the kit wrote its report: a session line.
fn has_session(report: &str) -> bool {
    report.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).any(|l| l.get("type").and_then(Value::as_str) == Some("session"))
}

fn same_file(a: &str, b: &str) -> bool {
    a.trim_start_matches("./") == b.trim_start_matches("./")
}

/// Read the kit's report lines: the planned files' tests, the failing ones' calls.
fn read_report(report: &str, files: &[String]) -> Read {
    let lines: Vec<Value> = report.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let s = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let planned = |f: &str| files.iter().any(|p| same_file(p, f));
    let mut out = Read::default();
    for l in &lines {
        match s(l, "type").as_str() {
            "test" if planned(&s(l, "file")) => {
                out.ran += 1;
                if matches!(s(l, "outcome").as_str(), "failed" | "error") {
                    out.failing.push(s(l, "id"));
                }
            }
            "collect_error" if planned(&s(l, "file")) => out.red = true,
            _ => {}
        }
    }
    out.red |= !out.failing.is_empty();
    let mut path: Vec<Called> = Vec::new();
    for l in lines.iter().filter(|l| s(l, "type") == "calls" && out.failing.contains(&s(l, "id"))) {
        for row in l.get("funcs").and_then(Value::as_array).into_iter().flatten() {
            let (Some(file), Some(symbol), Some(def_line)) = (row.get(0).and_then(Value::as_str), row.get(1).and_then(Value::as_str), row.get(2).and_then(Value::as_u64)) else { continue };
            let last = row.get(3).and_then(Value::as_u64).unwrap_or(0);
            match path.iter_mut().find(|c| c.file == file && c.symbol == symbol && u64::from(c.def_line) == def_line) {
                Some(c) => c.last_call = c.last_call.max(last),
                None => path.push(Called { file: file.to_string(), symbol: symbol.to_string(), def_line: u32::try_from(def_line).unwrap_or(u32::MAX), last_call: last }),
            }
        }
    }
    out.path = path;
    out
}

fn listed(r: &Ranked) -> TracedFunction {
    TracedFunction { rank: u64::from(r.rank), file_path: r.file.clone(), symbol: r.symbol.clone(), line_start: u64::from(r.line_start), line_end: u64::from(r.line_end) }
}

/// The trace file of try `run_id`.
pub fn trace_path(root: &Path, run_id: &str) -> std::path::PathBuf {
    root.join(FAMILIAR_FOLDER).join(RUNS_FOLDER).join(format!("{}.trace.json", safe(run_id)))
}

/// Keep the whole ranked path at `.familiar/runs/<run_id>.trace.json`, only where git ignores
/// the runs' folder (as a kept run is); whether it was kept.
fn keep(root: &Path, run_id: &str, f: &TraceListed, command: &str, failing: &[String], ranked: &[Ranked]) -> bool {
    if !crate::door::kept::ignored(root, RUNS_FOLDER) {
        return false;
    }
    let all: Vec<Value> = ranked.iter().map(|r| json!({ "rank": r.rank, "file_path": r.file, "symbol": r.symbol, "line_start": r.line_start, "line_end": r.line_end })).collect();
    let body = json!({
        "run_id": run_id, "criterion_id": f.criterion_id, "sha": f.sha, "red": f.red, "ranker": f.ranker,
        "query": f.query, "command": command, "failing": failing, "path_size": f.path_size, "ranked": all,
    });
    let path = trace_path(root, run_id);
    path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, format!("{body}\n")).is_ok()
}

/// One listed function as the agent reads it: `rank. file:line_start-line_end symbol`.
pub fn line_of(rank: u64, file: &str, start: u64, end: u64, symbol: &str) -> String {
    format!("{rank}. {file}:{start}-{end} {symbol}")
}

/// What the agent is told after a try.
fn told(c: &str, head: &str, files: &[String], read: &Read, f: &TraceListed, ranked: &[Ranked], kept: bool) -> String {
    let at = if head.is_empty() { String::new() } else { format!(" at {}", short(head)) };
    let mut out = String::new();
    if !f.red {
        if read.ran == 0 {
            out.push_str(&format!("{LEAD}the try of {c}{at} ({}): no test of {} ran: write the planned test there and try again\n", f.run_id, files.join(", ")));
        } else {
            out.push_str(&format!("{LEAD}the try of {c}{at} ({}): not red. {PASSES}\n", f.run_id));
        }
        return out;
    }
    let failing = match read.failing.len() {
        0 => "the planned test could not be collected".to_string(),
        1 => "1 failing test".to_string(),
        n => format!("{n} failing tests"),
    };
    if ranked.is_empty() {
        out.push_str(&format!("{LEAD}the try of {c}{at} ({}): red ({failing}); the recorder saw no function of the project run, so there is no list: choose the place as before\n", f.run_id));
        out.push_str(&format!("next: familiar-ai claim place_chosen --criterion_id {c} --places '[{{\"file_path\":…,\"symbol\":…,\"line_start\":…,\"line_end\":…}}]'\n"));
        return out;
    }
    out.push_str(&format!("{LEAD}the try of {c}{at} ({}): red ({failing}); the functions it ran, best match to the person's words first ({}):\n", f.run_id, f.ranker));
    for r in ranked.iter().take(SHOWN) {
        out.push_str(&format!("  {}\n", line_of(u64::from(r.rank), &r.file, u64::from(r.line_start), u64::from(r.line_end), &r.symbol)));
    }
    if ranked.len() > SHOWN {
        let more = ranked.len() - SHOWN;
        out.push_str(&if kept { format!("  {more} more: familiar-ai try --list all\n") } else { format!("  {more} more, not kept: .familiar/runs is not git-ignored here\n") });
    }
    out.push_str(&format!(
        "next: pick the place from this list: familiar-ai claim place_chosen --criterion_id {c} --places '[{{\"file_path\":…,\"symbol\":…,\"line_start\":…,\"line_end\":…}}]', or familiar-ai claim off_list_reason --criterion_id {c} --reason '…' if the fix lies elsewhere\n"
    ));
    out
}

/// `try --list all`: the latest try's whole ranked path, read from its trace file; nothing runs.
fn list_all(root: &Path, log: &Log, criterion: Option<&str>, around: &Surroundings) -> Outcome {
    let v = View::new(log);
    let latest = match criterion {
        Some(c) => trace::latest(&v, c),
        None => {
            let c = part_of(&v, around.session_id.as_deref());
            c.and_then(|c| trace::latest(&v, &c)).or_else(|| v.kind(trace::LISTED).last())
        }
    };
    let Some(f) = latest else {
        return refused("no try yet: familiar-ai try runs one");
    };
    let run_id = f.fields.get("run_id").and_then(Value::as_str).unwrap_or("");
    let c = f.fields.get("criterion_id").and_then(Value::as_str).unwrap_or("");
    let stored: Option<Value> = std::fs::read_to_string(trace_path(root, run_id)).ok().and_then(|t| serde_json::from_str(&t).ok());
    let Some(stored) = stored else {
        return refused(&format!("the whole list of try {run_id} was not kept (.familiar/runs/{run_id}.trace.json); familiar-ai try runs a new one"));
    };
    let red = stored.get("red").and_then(Value::as_bool).unwrap_or(false);
    let ranked = stored.get("ranked").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = format!("{LEAD}the try of {c} ({run_id}): {}, {} functions, best match to the person's words first:\n", if red { "red" } else { "not red" }, ranked.len());
    for r in &ranked {
        let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0);
        let s = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
        out.push_str(&format!("  {}\n", line_of(n("rank"), s("file_path"), n("line_start"), n("line_end"), s("symbol"))));
    }
    plain(0, out, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_is_the_union_of_the_failing_tests_calls_and_red_reads_only_the_planned_files() {
        let report = [
            r#"{"type":"session","v":1}"#,
            r#"{"type":"test","id":"tests/test_a.py::test_x","file":"tests/test_a.py","outcome":"failed"}"#,
            r#"{"type":"test","id":"tests/test_a.py::test_y","file":"tests/test_a.py","outcome":"error"}"#,
            r#"{"type":"test","id":"tests/test_a.py::test_z","file":"tests/test_a.py","outcome":"passed"}"#,
            r#"{"type":"test","id":"tests/test_b.py::test_q","file":"tests/test_b.py","outcome":"failed"}"#,
            r#"{"type":"calls","id":"tests/test_a.py::test_x","funcs":[["m.py","f",1,3],["m.py","g",4,5]]}"#,
            r#"{"type":"calls","id":"tests/test_a.py::test_y","funcs":[["m.py","g",4,9],["n.py","h",2,1]]}"#,
            r#"{"type":"calls","id":"tests/test_a.py::test_z","funcs":[["m.py","passing",7,2]]}"#,
            r#"{"type":"calls","id":"tests/test_b.py::test_q","funcs":[["m.py","other",9,2]]}"#,
        ]
        .join("\n");
        let r = read_report(&report, &["./tests/test_a.py".to_string()]);
        assert!(r.red);
        assert_eq!(r.ran, 3);
        assert_eq!(r.failing, ["tests/test_a.py::test_x", "tests/test_a.py::test_y"]);
        let path: Vec<(&str, u64)> = r.path.iter().map(|c| (c.symbol.as_str(), c.last_call)).collect();
        assert_eq!(path, [("f", 3), ("g", 9), ("h", 1)], "each function once, at its latest call; a passing or unplanned test's calls left out");
        let r = read_report(&report, &["tests/test_c.py".to_string()]);
        assert!(!r.red && r.ran == 0 && r.path.is_empty());
        let r = read_report(r#"{"type":"collect_error","file":"tests/test_c.py","error":{}}"#, &["tests/test_c.py".to_string()]);
        assert!(r.red && r.path.is_empty(), "a planned file that cannot be collected is red, with nothing to list");
    }

    #[test]
    fn the_seam_ranks_by_words_now() {
        let called = [Called { file: "m.py".into(), symbol: "coupon".into(), def_line: 1, last_call: 1 }];
        let (ranker, ranked) = rank("the coupon", &called, |_| Some("def coupon():\n    return 1\n".into()));
        assert_eq!(ranker, "words");
        assert_eq!(ranked.len(), 1);
    }
}
