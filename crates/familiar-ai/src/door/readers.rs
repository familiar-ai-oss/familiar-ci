//! The readers (plan A, `READERS` [draft]): Familiar's own list of the ways a runner's run can
//! be read, best first. The adapter names a reader by its id, never a pattern. Lane a-py wrote
//! pytest's; lane A-wire builds the rest from the runners' kit data modules (`door::kits::*`),
//! whose three vocabularies (the JS and Go/Rust tuples, unittest's, the `spec` structs) become one
//! [`Reader`] here.
//!
//! ```text
//!   kits::<runner>::READERS ──▶ READERS (one shape)   ──apply()──▶ env ops + the command with the
//!                                                                  reader's flags in its segment
//!   the run ──▶ result_file(): the kit's file, or the stdout / report files through a converter
//!          ──▶ reported::read (the per-test result file) ── None ──▶ the tables
//! ```
//!
//! Placeholders in values, flags and report globs: `{KIT}` the kit's folder (written once per
//! version outside the work tree), `{REPORT_DIR}` the run's own temp folder outside the copy,
//! `{REPORT}` the kit's result file there (`{REPORT_DIR}/results.jsonl`), and node's
//! `{NODE_DEFAULT_REPORTER}` (its version's own reporter).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

use familiar_ai_run::EnvOp;

use crate::door::aim;
use crate::door::kits::{self, spec};

/// One env change a reader makes, as data: `op` is `set`, `append`, `prepend` or `unset`; the
/// separator joins an appended or prepended value to the person's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Env {
    pub var: &'static str,
    pub op: &'static str,
    pub value: &'static str,
    pub separator: &'static str,
}

/// One reader: its id (the only thing `.familiar/adapter` names), its runner (as
/// `familiar_ai_run::details_for` names it), its kind, and how it reads.
#[derive(Debug, Clone, Copy)]
pub struct Reader {
    pub id: &'static str,
    pub runner: &'static str,
    /// `reporter` (Familiar's own, injected), `report` (the runner's own report), `pinned` (the
    /// console pinned for the tables) or `tables` (the regex tables alone).
    pub kind: &'static str,
    /// Applied to Familiar's own run only, in order.
    pub env: &'static [Env],
    /// Added to Familiar's own run in the runner's segment, after the runner and its subcommand.
    pub flags: &'static [&'static str],
    /// The files a `report` reader reads (`{REPORT_DIR}` the run's own folder, else relative to
    /// the folder the command runs in).
    pub reports: &'static [&'static str],
    /// How the run becomes the per-test result file: `schema` (the kit wrote it), `test2json`,
    /// `libtest_json` or `tap` (the run's stdout), `junit` or `trx` (the report files); `""` for
    /// the tables.
    pub converter: &'static str,
    /// The reader applies only when the runner's segment has one of these flags.
    pub only_when: &'static [&'static str],
    /// The reader does not apply when the runner's segment has one of these flags.
    pub skip_when: &'static [&'static str],
    /// The probes this reader cannot read by what it is (a compile error writes no report): the
    /// tables read those runs.
    pub cannot_read: &'static [&'static str],
    /// Whether a probe kit and recorded runs prove it; one that is not never becomes a choice.
    pub calibrated: bool,
}

const fn env(var: &'static str, op: &'static str, value: &'static str, separator: &'static str) -> Env {
    Env { var, op, value, separator }
}

const PYTEST_REPORTER: &[Env] = &[env("PYTEST_ADDOPTS", "append", "-p familiar_ai_pytest", " "), env("PYTHONPATH", "prepend", "{KIT}/pytest", ":"), env("FAMILIAR_AI_REPORT", "set", "{REPORT}", "")];

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// A tuple module's reader (id, kind, env ops, flags, report globs).
type Tuple = (&'static str, &'static str, &'static [(&'static str, &'static str, &'static str, &'static str)], &'static [&'static str], &'static [&'static str]);

fn from_tuple(runner: &'static str, r: &Tuple, converter: &'static str) -> Reader {
    // unittest's placeholders are its own folder and the result file
    let fill = |v: &'static str| -> &'static str {
        match v {
            "{kit}" => "{KIT}/unittest",
            "{report}" => "{REPORT}",
            v => v,
        }
    };
    let env: Vec<Env> = r.2.iter().map(|(var, op, value, sep)| Env { var, op, value: fill(value), separator: sep }).collect();
    let converter = match r.1 {
        "tables" | "pinned" => "",
        _ => converter,
    };
    Reader { id: r.0, runner, kind: r.1, env: leak(env), flags: r.3, reports: r.4, converter, only_when: &[], skip_when: &[], cannot_read: &[], calibrated: true }
}

fn from_spec(runner: &'static str, r: &spec::Reader, calibrated: bool) -> Reader {
    let op = |o: spec::Op| match o {
        spec::Op::Set => "set",
        spec::Op::Append => "append",
        spec::Op::Prepend => "prepend",
        spec::Op::Unset => "unset",
    };
    let kind = match r.kind {
        spec::Kind::Reporter => "reporter",
        spec::Kind::Report => "report",
        spec::Kind::Pinned => "pinned",
        spec::Kind::Tables => "tables",
    };
    let env: Vec<Env> = r.env.iter().map(|e| Env { var: e.var, op: op(e.op), value: e.value, separator: e.separator }).collect();
    Reader {
        id: r.id,
        runner,
        kind,
        env: leak(env),
        flags: r.flags,
        reports: r.report_globs,
        converter: r.converter,
        only_when: r.only_when_command_has,
        skip_when: r.skip_when_command_has,
        cannot_read: r.cannot_read,
        calibrated,
    }
}

/// The readers, each runner's best first: pytest, unittest, the JS four, go, cargo test and
/// nextest, PHPUnit, Maven, Gradle, dotnet, then the C/C++ report readers (not calibrated).
pub static READERS: LazyLock<Vec<Reader>> = LazyLock::new(|| {
    let mut out = vec![
        Reader { id: "pytest:reporter", runner: "pytest", kind: "reporter", env: PYTEST_REPORTER, flags: &[], reports: &[], converter: "schema", only_when: &[], skip_when: &[], cannot_read: &[], calibrated: true },
        Reader { id: "pytest:tables", runner: "pytest", kind: "tables", env: &[], flags: &[], reports: &[], converter: "", only_when: &[], skip_when: &[], cannot_read: &[], calibrated: true },
    ];
    out.extend(kits::unittest::READERS.iter().map(|r| from_tuple(kits::unittest::RUNNER, r, "schema")));
    out.extend(kits::jest::READERS.iter().map(|r| from_tuple(kits::jest::RUNNER, r, "schema")));
    out.extend(kits::vitest::READERS.iter().map(|r| from_tuple(kits::vitest::RUNNER, r, "schema")));
    out.extend(kits::node::READERS.iter().map(|r| {
        let mut x = from_tuple(kits::node::RUNNER, r, "schema");
        match x.id {
            "node:reporter" | "node:pinned" => x.skip_when = kits::node::OWN_REPORTER_FLAGS,
            "node:reporter-flags" => x.only_when = kits::node::OWN_REPORTER_FLAGS,
            _ => {}
        }
        // the pinned console is node's TAP on stdout, read by the TAP converter
        if x.id == "node:pinned" {
            x.converter = "tap";
        }
        x
    }));
    out.extend(kits::mocha::READERS.iter().map(|r| from_tuple(kits::mocha::RUNNER, r, "schema")));
    out.extend(kits::go_test::READERS.iter().map(|r| from_tuple(kits::go_test::RUNNER, r, kits::go_test::CONVERTER)));
    out.extend(kits::cargo_test::READERS.iter().map(|r| from_tuple(kits::cargo_test::RUNNER, r, kits::cargo_test::CONVERTER)));
    out.extend(kits::cargo_nextest::READERS.iter().map(|r| {
        let mut x = from_tuple(kits::cargo_nextest::RUNNER, r, kits::cargo_nextest::CONVERTER);
        if let Some((_, flags)) = kits::cargo_nextest::SKIP_WHEN.iter().find(|(id, _)| *id == x.id) {
            x.skip_when = flags;
        }
        x
    }));
    out.extend(kits::phpunit::READERS.iter().map(|r| from_spec(kits::phpunit::ID, r, true)));
    out.extend(kits::maven::READERS.iter().map(|r| from_spec(kits::maven::ID, r, true)));
    out.extend(kits::gradle::READERS.iter().map(|r| from_spec(kits::gradle::ID, r, true)));
    out.extend(kits::dotnet::READERS.iter().map(|r| from_spec(kits::dotnet::ID, r, true)));
    out.extend(kits::ctest::READERS.iter().map(|r| from_spec(kits::ctest::ID, r, kits::ctest::CALIBRATED)));
    out.extend(kits::gtest::READERS.iter().map(|r| from_spec(kits::gtest::ID, r, kits::gtest::CALIBRATED)));
    out.extend(kits::catch2::READERS.iter().map(|r| from_spec(kits::catch2::ID, r, kits::catch2::CALIBRATED)));
    out
});

/// The reader with this id.
pub fn reader(id: &str) -> Option<&'static Reader> {
    READERS.iter().find(|r| r.id == id)
}

/// The readers of `runner`, best first.
pub fn of_runner(runner: &str) -> Vec<&'static Reader> {
    READERS.iter().filter(|r| r.runner == runner).collect()
}

/// The kit's files, in the binary: (path under the kit folder, text, whether `{KIT}` in it is
/// filled with the folder).
fn kit_files() -> Vec<(&'static str, &'static str, bool)> {
    let mut out: Vec<(&'static str, &'static str, bool)> = vec![("pytest/familiar_ai_pytest.py", include_str!("../../kit/pytest/familiar_ai_pytest.py"), false), ("pytest/familiar_ai_calls.py", include_str!("../../kit/pytest/familiar_ai_calls.py"), false), ("pytest/sitecustomize.py", include_str!("../../kit/pytest/sitecustomize.py"), false)];
    for (p, t) in kits::unittest::KIT_FILES {
        out.push((if *p == "sitecustomize.py" { "unittest/sitecustomize.py" } else { p }, t, false));
    }
    for files in [kits::node::KIT_FILES, kits::jest::KIT_FILES, kits::vitest::KIT_FILES, kits::mocha::KIT_FILES] {
        out.extend(files.iter().map(|(p, t)| (*p, *t, false)));
    }
    out.extend(kits::phpunit::KIT_FILES.iter().map(|f| (f.path, f.text, f.template)));
    out.sort_by_key(|f| f.0);
    out.dedup_by_key(|f| f.0);
    out
}

/// The kit's folder for this version and these kit files, `familiar-ai-kit-<version>-<hash>`
/// under the temp folder, written once (a file already there is kept). The hash of the kit's
/// files names the folder, so a build whose kit changed under the same version (a dev build)
/// never runs the files an older build left.
pub fn kit_dir() -> Result<PathBuf, String> {
    kit_dir_in(&std::env::temp_dir())
}

/// The first 12 hex digits of the SHA-256 of the kit's files, each path and text in order.
fn kit_hash() -> &'static str {
    static HASH: LazyLock<String> = LazyLock::new(|| {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        for (path, text, template) in kit_files() {
            h.update(path.as_bytes());
            h.update([0, u8::from(template)]);
            h.update(text.as_bytes());
            h.update([0]);
        }
        h.finalize().iter().take(6).map(|b| format!("{b:02x}")).collect()
    });
    &HASH
}

/// [`kit_dir`] under `base`.
pub fn kit_dir_in(base: &Path) -> Result<PathBuf, String> {
    let dir = base.join(format!("familiar-ai-kit-{}-{}", env!("CARGO_PKG_VERSION"), kit_hash()));
    for (rel, text, template) in kit_files() {
        let path = dir.join(rel);
        if path.is_file() {
            continue;
        }
        let parent = path.parent().ok_or("the kit has no folder")?;
        std::fs::create_dir_all(parent).map_err(|e| format!("the kit's folder could not be made: {e}"))?;
        let text = if template { spec::render(text, &dir.to_string_lossy()) } else { text.to_string() };
        // written beside under a name of its own, then moved in, so a run never sees half a file
        static PARTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = PARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let part = parent.join(format!(".{}-{n}.{}", std::process::id(), path.file_name().and_then(|n| n.to_str()).unwrap_or("kit")));
        std::fs::write(&part, text).map_err(|e| format!("the kit could not be written: {e}"))?;
        if let Err(e) = std::fs::rename(&part, &path) {
            let _ = std::fs::remove_file(&part);
            if !path.is_file() {
                return Err(format!("the kit could not be written: {e}"));
            }
        }
    }
    Ok(dir)
}

/// One env change a reader makes, owned ([`EnvOp`] borrows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Set(String, String),
    Append(String, String),
    Prepend(String, String),
    Unset(String),
}

impl Op {
    pub fn as_env(&self) -> EnvOp<'_> {
        match self {
            Op::Set(n, v) => EnvOp::Set(n, v),
            Op::Append(n, v) => EnvOp::Append(n, v),
            Op::Prepend(n, v) => EnvOp::Prepend(n, v),
            Op::Unset(n) => EnvOp::Unset(n),
        }
    }
}

/// Whether `command` starts Python with `-I` or `-E`, which ignore `PYTHONPATH` and so the kit.
fn ignores_pythonpath(command: &str) -> bool {
    command.split("&&").flat_map(|p| p.split(';')).any(|segment| {
        let words: Vec<&str> = segment.split_whitespace().collect();
        words.iter().enumerate().any(|(i, w)| {
            let base = w.rsplit('/').next().unwrap_or(w);
            (base == "python" || base.starts_with("python3")) && words[i + 1..].iter().take_while(|o| o.starts_with('-') && **o != "-m" && **o != "-c").any(|o| !o.starts_with("--") && o[1..].chars().any(|c| c == 'I' || c == 'E'))
        })
    })
}

/// Why reader `r` cannot read `command`, when it cannot.
fn refused(r: &Reader, command: &str) -> Option<String> {
    let has = |f: &str| aim::has_flag(command, r.runner, f);
    if !r.only_when.is_empty() && !r.only_when.iter().any(|f| has(f)) {
        return Some(format!("{} applies only to a command naming {}", r.id, r.only_when.join(" or ")));
    }
    if let Some(f) = r.skip_when.iter().find(|f| has(f)) {
        return Some(format!("the command's own {f} beats {}", r.id));
    }
    match r.id {
        "pytest:reporter" if ignores_pythonpath(command) => Some("python -I or -E ignores PYTHONPATH, so the reporter cannot load".into()),
        "unittest:reporter" => kits::unittest::refused(command).map(str::to_string),
        // GOFLAGS reaches every go call: only for a command whose only go call is go test
        "go-test:reporter" if !aim::only_go_call_is_go_test(command) => Some("GOFLAGS would reach another go call of the command".into()),
        _ => None,
    }
}

/// Node's own reporter for the node on this PATH (`tap` before 23, `spec` from 23), asked once.
fn node_default_reporter() -> &'static str {
    static DEFAULT: LazyLock<&'static str> = LazyLock::new(|| {
        let major = std::process::Command::new("node")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|v| v.trim().trim_start_matches('v').split('.').next().and_then(|m| m.parse::<u32>().ok()));
        kits::node::default_reporter(major.unwrap_or(22))
    });
    *DEFAULT
}

/// What a reader does to Familiar's own run: its env ops and the command with its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub ops: Vec<Op>,
    pub command: String,
}

/// Reader `id` applied to Familiar's own run of `command`, the kit at `kit` and the run's own
/// report folder `report_dir`; an error when the reader cannot read this command.
pub fn apply(id: &str, command: &str, kit: &Path, report_dir: &Path) -> Result<Applied, String> {
    let r = reader(id).ok_or_else(|| format!("no reader {id}"))?;
    if let Some(why) = refused(r, command) {
        return Err(why);
    }
    let report = report_dir.join("results.jsonl");
    let fill = |s: &str| -> String {
        let s = s.replace("{KIT}", &kit.to_string_lossy()).replace("{REPORT_DIR}", &report_dir.to_string_lossy()).replace("{REPORT}", &report.to_string_lossy());
        if s.contains("{NODE_DEFAULT_REPORTER}") {
            s.replace("{NODE_DEFAULT_REPORTER}", node_default_reporter())
        } else {
            s
        }
    };
    let mut ops: Vec<Op> = Vec::new();
    for e in r.env {
        let (var, value) = (e.var.to_string(), fill(e.value));
        let own = familiar_ai_run::separator(e.var);
        ops.push(match e.op {
            "set" => Op::Set(var, value),
            "unset" => Op::Unset(var),
            // the run joins with the variable's own separator; another is joined here, from what
            // the run would inherit
            "append" | "prepend" if e.separator != own => {
                let now = std::env::var(e.var).ok().filter(|v| !v.is_empty());
                let joined = match (e.op, now) {
                    (_, None) => value,
                    ("append", Some(n)) => format!("{n}{}{value}", e.separator),
                    (_, Some(n)) => format!("{value}{}{n}", e.separator),
                };
                Op::Set(var, joined)
            }
            "append" => Op::Append(var, value),
            "prepend" => Op::Prepend(var, value),
            other => return Err(format!("{id}: no env op {other}")),
        });
    }
    // a kit that writes the result file is told where
    if r.converter == "schema" && !ops.iter().any(|o| matches!(o, Op::Set(n, _) if n == "FAMILIAR_AI_REPORT")) {
        ops.push(Op::Set("FAMILIAR_AI_REPORT".into(), report.to_string_lossy().into_owned()));
    }
    let flags: Vec<String> = r.flags.iter().map(|f| fill(f)).collect();
    let command = aim::insert_flags(command, r.runner, &flags).ok_or_else(|| format!("{id}: the command names no {}", r.runner))?;
    Ok(Applied { ops, command })
}

/// The env changes reader `id` makes to Familiar's own run of `command`, with the kit at `kit` and
/// the result file at `report` (its folder the run's report folder); an error when the reader
/// cannot read this command.
pub fn env_for(id: &str, command: &str, kit: &Path, report: &Path) -> Result<Vec<Op>, String> {
    let dir = report.parent().unwrap_or(Path::new(""));
    let mut ops = apply(id, command, kit, dir)?.ops;
    // the result file as the caller named it
    for o in ops.iter_mut() {
        if let Op::Set(n, v) = o {
            if n == "FAMILIAR_AI_REPORT" {
                *v = report.to_string_lossy().into_owned();
            }
        }
    }
    Ok(ops)
}

/// The language a runner's session line names.
fn language(runner: &str) -> &'static str {
    match runner {
        "pytest" | "unittest" => "python",
        "jest" | "vitest" | "mocha" | "node-test" => "javascript",
        "go-test" => "go",
        "cargo-test" | "cargo-nextest" => "rust",
        "phpunit" | "pest" => "php",
        "maven" | "gradle" => "java",
        "dotnet" => "csharp",
        _ => "cpp",
    }
}

/// What a run under a reader left: its stdout (whole, or `None` when the capture was cut), the
/// folder it ran in, its own report folder, when it started, its exit (`None` when a signal or a
/// timeout ended it), and the report files that were there before it ([`reports_before`]).
#[derive(Debug, Clone, Copy)]
pub struct Left<'a> {
    pub stdout: Option<&'a str>,
    pub run_dir: &'a Path,
    pub report_dir: &'a Path,
    pub started: SystemTime,
    pub exit: Option<i32>,
    pub before: &'a [(String, SystemTime)],
}

/// The report files reader `id` reads in `run_dir` that are there before the run, with when each
/// was last written: one the run leaves as it was is not the run's (a compile error leaves the
/// last run's reports behind, and a copy writes a tracked one just before the run starts).
pub fn reports_before(id: &str, run_dir: &Path) -> Vec<(String, SystemTime)> {
    let Some(r) = reader(id) else { return Vec::new() };
    r.reports
        .iter()
        .filter(|g| !g.starts_with("{REPORT_DIR}/"))
        .flat_map(|g| crate::door::verify_run::matching(run_dir, g))
        .filter_map(|rel| std::fs::metadata(run_dir.join(&rel)).and_then(|m| m.modified()).ok().map(|t| (rel, t)))
        .collect()
}

/// The per-test result file (JSON Lines v1) of a run under reader `id`, as `reported::read` takes
/// it: the kit's own file, or the run's stdout or report files through the reader's converter.
/// `None` when the reader writes none (the tables), when a signal or a timeout ended the run, or
/// when a stream it reads was cut.
pub fn result_file(id: &str, left: &Left<'_>) -> Option<String> {
    let r = reader(id)?;
    let exit = left.exit?;
    let root = left.run_dir.to_string_lossy();
    match r.converter {
        "schema" => std::fs::read_to_string(left.report_dir.join("results.jsonl")).ok(),
        "test2json" => Some(crate::door::convert::test2json::to_report(left.stdout?, exit)),
        "libtest_json" => Some(crate::door::convert::libtest_json::to_report(left.stdout?, exit)),
        "tap" => Some(crate::door::convert::tap::to_report_in(left.stdout?, exit, &root)),
        "junit" | "trx" => {
            let files = report_files(r, left);
            let natives: Vec<crate::door::convert::schema::Native<'_>> = files.iter().map(|(n, t, m)| crate::door::convert::schema::Native { name: n, text: t, modified: *m }).collect();
            let ctx = crate::door::convert::schema::Context::new(r.runner, language(r.runner)).with_root(&root);
            // a filesystem's coarse times: the run's start rounded down to its second
            let started = left.started.duration_since(SystemTime::UNIX_EPOCH).map(|d| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(d.as_secs())).unwrap_or(left.started);
            Some(if r.converter == "junit" {
                crate::door::convert::junit::to_report_files(&natives, &ctx, Some(started), exit)
            } else {
                crate::door::convert::trx::to_report_files(&natives, &ctx, Some(started), exit)
            })
        }
        _ => None,
    }
}

/// The report files reader `r` names, as (name, text, last written).
fn report_files(r: &Reader, left: &Left<'_>) -> Vec<(String, String, SystemTime)> {
    let mut out: Vec<(String, String, SystemTime)> = Vec::new();
    for g in r.reports {
        let (base, glob) = match g.strip_prefix("{REPORT_DIR}/") {
            Some(rest) => (left.report_dir, rest.to_string()),
            None => (left.run_dir, g.to_string()),
        };
        for rel in crate::door::verify_run::matching(base, &glob) {
            let path = base.join(&rel);
            let (Ok(text), Ok(meta)) = (std::fs::read_to_string(&path), std::fs::metadata(&path)) else { continue };
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if left.before.iter().any(|(n, t)| *n == rel && *t == modified) {
                continue; // there before the run, as it was
            }
            if !out.iter().any(|(n, _, _)| *n == rel) {
                out.push((rel, text, modified));
            }
        }
    }
    out
}

/// The reader the record chose for (`root`, `runner`): the latest `reader_chosen` for them.
// TODO(lane A5): switch to the rules' `reader_at(v, root, runner, commit)` when it lands.
pub fn chosen_reader(log: &[familiar_ai_facts::Fact], root: &str, runner: &str) -> Option<String> {
    let s = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(|v| v.as_str()).map(str::to_string);
    log.iter()
        .rev()
        .filter(|f| f.kind == "reader_chosen" && s(f, "ran_in").as_deref() != Some("pipeline"))
        .find(|f| s(f, "root").as_deref() == Some(root) && s(f, "runner").as_deref() == Some(runner))
        .and_then(|f| s(f, "reader"))
}
