//! Runs the project's test command through `sh -c`; it passed only if it exits 0 within the
//! timeout, and its output is kept but never parsed.

use std::collections::VecDeque;
use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub mod shell_trace;

/// Names removed from the environment of every child the binary starts: the hook's own project
/// and the marker that it runs under Claude Code. A child finds its own project, and one that
/// sees `CLAUDECODE` may refuse to start.
pub const STRIPPED: [&str; 2] = ["CLAUDE_PROJECT_DIR", "CLAUDECODE"];

/// How many bytes of each stream are kept from its start (the owner's D19: 4 MiB).
pub const CAPTURE_HEAD: usize = 4 * 1024 * 1024;
/// How many bytes of each stream are kept from its end, rolling as more arrive (D19: 4 MiB).
pub const CAPTURE_TAIL: usize = 4 * 1024 * 1024;

/// One runner the planned command can name, and what its run needs so it prints the details
/// of a failure: env set on the child, and report files it writes beside the console. Familiar
/// never changes the command itself, but for `-x` on a shell script run by an `sh` that is not
/// bash, in Familiar's own run only ([`shell_trace`], H-2, A-2).
#[derive(Debug)]
pub struct RunnerDetails {
    /// The runner's name, as the recordings of its output name it.
    pub runner: &'static str,
    /// The command words (matched by their basename) that name this runner.
    pub words: &'static [&'static str],
    /// Set on the child, after the [`STRIPPED`] names are removed.
    pub env: &'static [(&'static str, &'static str)],
    /// Report files, as globs from the folder the command runs in.
    pub reports: &'static [&'static str],
}

const fn console(runner: &'static str, words: &'static [&'static str]) -> RunnerDetails {
    RunnerDetails { runner, words, env: &[], reports: &[] }
}

/// The runners a command can name (plan section 5, from the S0 recordings); most print every
/// detail to the console.
pub const RUNNER_DETAILS: &[RunnerDetails] = &[
    RunnerDetails {
        runner: "ctest",
        words: &["ctest", "cmake"],
        env: &[("CTEST_OUTPUT_ON_FAILURE", "1")],
        reports: &[],
    },
    RunnerDetails {
        runner: "maven",
        words: &["mvn", "mvnw"],
        env: &[],
        reports: &["target/surefire-reports/*.txt", "target/surefire-reports/*.xml"],
    },
    RunnerDetails {
        runner: "gradle",
        words: &["gradle", "gradlew"],
        env: &[],
        reports: &["build/test-results/**/*.xml"],
    },
    console("dotnet", &["dotnet"]),
    console("pytest", &["pytest", "py.test"]),
    console("unittest", &["unittest"]),
    console("jest", &["jest"]),
    console("vitest", &["vitest"]),
    console("mocha", &["mocha"]),
    console("node-test", &["node"]),
    console("cargo-test", &["cargo"]),
    console("cargo-nextest", &["cargo-nextest"]),
    console("go-test", &["go"]),
    console("phpunit", &["phpunit"]),
    console("pest", &["pest"]),
    // the owner, 2026-10-15: a test that is a shell script; make and bats are not sh
    console("sh", &["sh", "bash", "dash", "zsh"]),
];

/// The shell's loop and branch words, skipped before a segment's command word, so
/// `for f in …; do sh "$f"; done` names `sh`.
const SHELL_WORDS: &[&str] = &["for", "do", "done", "then"];

/// Wrappers that run the command after them: one word, or two.
const WRAPPERS: &[&[&str]] = &[&["npx"], &["bunx"], &["yarn"], &["pnpm", "exec"], &["uv", "run"], &["poetry", "run"], &["pipenv", "run"]];

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// A leading `NAME=value` word of the shell.
fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The row one segment of a command names: leading `NAME=value` words and wrappers (with their
/// options) skipped, `python -m <module>` read as its module, `cargo nextest` as nextest.
fn row_of(segment: &str) -> Option<&'static RunnerDetails> {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let mut i = words.iter().take_while(|w| SHELL_WORDS.contains(w)).count();
    i += words[i..].iter().take_while(|w| assignment(w)).count();
    while let Some(w) = WRAPPERS.iter().find(|w| words.len() >= i + w.len() && words[i..].iter().zip(w.iter()).all(|(a, b)| basename(a) == *b)) {
        i += w.len();
        i += words[i..].iter().take_while(|w| w.starts_with('-')).count();
    }
    let mut word = basename(words.get(i)?);
    if word == "python" || word == "python3" {
        let m = words[i + 1..].iter().position(|w| *w == "-m")?;
        word = words.get(i + 1 + m + 1)?;
    } else if word == "cargo" && words.get(i + 1) == Some(&"nextest") {
        word = "cargo-nextest";
    } else if word.ends_with(".sh") {
        // a `./…sh` script runs under the shell
        word = "sh";
    }
    RUNNER_DETAILS.iter().find(|row| row.words.contains(&word))
}

/// The [`RUNNER_DETAILS`] row the planned `command` names. Each `&&` / `;` segment is read, and
/// the last segment that names a runner picks it; `None` when no segment does (`make test`,
/// `./run_tests`): the console only.
pub fn details_for(command: &str) -> Option<&'static RunnerDetails> {
    let segments: Vec<&str> = command.split("&&").flat_map(|p| p.split(';')).collect();
    segments.into_iter().rev().find_map(row_of)
}

/// Whether the test command passed; any end other than exit 0 is `Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed,
}

/// How a run ended, which tells a failing test from a command that did not run.
/// CI reads it at the tests-first commit, where a timeout must not count as a red test; see
/// docs/decisions.md#exit-codes-not-output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The command exited with this code.
    Exited(i32),
    /// A signal ended the command before the time limit; this is its number.
    Signalled(i32),
    /// Still running at the time limit; its process group was killed.
    TimedOut,
    EmptyCommand,
    NoDirectory,
    /// The shell could not be started, or the child could not be read.
    NotStarted,
    /// The caller's `on_start` returned false, and the process group was killed.
    Cancelled,
}

/// What a run produced; `stdout` and `stderr` go to the local record only.
/// Each stream keeps its first [`CAPTURE_HEAD`] and last [`CAPTURE_TAIL`] bytes; when bytes
/// between them were dropped, the text is head, `\n[familiar: N bytes dropped]\n`, tail.
#[derive(Debug)]
pub struct RunOutcome {
    pub verdict: Verdict,
    pub stdout: String,
    pub stderr: String,
    /// How the run ended; it never changes `verdict`.
    pub end: End,
    /// Every byte read from stdout, kept or not.
    pub stdout_total_bytes: u64,
    /// The bytes of stdout between its head and its tail that were not kept.
    pub stdout_dropped_bytes: u64,
    /// Every byte read from stderr, kept or not.
    pub stderr_total_bytes: u64,
    /// The bytes of stderr between its head and its tail that were not kept.
    pub stderr_dropped_bytes: u64,
}

impl RunOutcome {
    fn failed(end: End) -> Self {
        RunOutcome {
            verdict: Verdict::Failed,
            stdout: String::new(),
            stderr: String::new(),
            end,
            stdout_total_bytes: 0,
            stdout_dropped_bytes: 0,
            stderr_total_bytes: 0,
            stderr_dropped_bytes: 0,
        }
    }

    /// Says in plain words why the command did not run, or `None` when its exit code decided.
    /// The shell exits 127 or 126 for a command it could not run, and 128 plus the signal for one
    /// a signal ended, so an out-of-memory kill is 137.
    pub fn did_not_run(&self) -> Option<String> {
        match self.end {
            End::Exited(127) => Some("the command was not found (exit 127)".into()),
            End::Exited(126) => Some("the command could not be executed (exit 126)".into()),
            End::Exited(code @ 129..=192) => Some(format!("it was ended by signal {} (exit {code})", code - 128)),
            End::Exited(_) => None,
            End::Signalled(n) => Some(format!("it was ended by signal {n}")),
            End::TimedOut => Some("it was still running at the time limit".into()),
            End::EmptyCommand => Some("the command is empty".into()),
            End::NoDirectory => Some("the folder to run it in does not exist".into()),
            End::NotStarted => Some("the shell could not be started".into()),
            End::Cancelled => Some("it was cancelled".into()),
        }
    }
}

/// Runs `test_command` through `sh -c` in `dir` for at most `timeout`, then kills its process group.
pub fn run_test_command(test_command: &str, dir: &Path, timeout: Duration) -> RunOutcome {
    run_test_command_watched(test_command, dir, timeout, None)
}

/// Runs like [`run_test_command`], passing the process group id to `on_start` as soon as it exists.
/// When `on_start` returns false the group is killed at once and the end is [`End::Cancelled`].
pub fn run_test_command_watched(
    test_command: &str,
    dir: &Path,
    timeout: Duration,
    on_start: Option<&mut dyn FnMut(i32) -> bool>,
) -> RunOutcome {
    run_test_command_with(test_command, dir, timeout, on_start, &[])
}

/// Runs like [`run_test_command_watched`], setting `env` on the child after the [`STRIPPED`]
/// names are removed; a [`STRIPPED`] name in `env` stays removed.
pub fn run_test_command_with(
    test_command: &str,
    dir: &Path,
    timeout: Duration,
    on_start: Option<&mut dyn FnMut(i32) -> bool>,
    env: &[(&str, &str)],
) -> RunOutcome {
    let ops: Vec<EnvOp> = env.iter().map(|(n, v)| EnvOp::Set(n, v)).collect();
    run_inner(test_command, dir, timeout, on_start, &ops, None)
}

/// One change a reader makes to the child's environment (plan A2): `Append` and `Prepend` join
/// the value to the one the child would inherit with the variable's own [`separator`], so the
/// person's own value survives; with none inherited the value stands alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvOp<'a> {
    Set(&'a str, &'a str),
    Append(&'a str, &'a str),
    Prepend(&'a str, &'a str),
    Unset(&'a str),
}

/// The separator a variable's list uses: `:` for a search path (`PATH`, `PYTHONPATH`,
/// `NODE_PATH`, …), a space for an option list (`PYTEST_ADDOPTS`, `NODE_OPTIONS`, `GOFLAGS`, …).
pub fn separator(name: &str) -> &'static str {
    if name.ends_with("PATH") || name == "PHP_INI_SCAN_DIR" {
        ":"
    } else {
        " "
    }
}

/// The value each name in `ops` takes in the child, in order, reading what it would inherit
/// from `inherited`; `None` removes it.
pub fn env_values(ops: &[EnvOp], inherited: &dyn Fn(&str) -> Option<String>) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for op in ops {
        let name = match op {
            EnvOp::Set(n, _) | EnvOp::Append(n, _) | EnvOp::Prepend(n, _) | EnvOp::Unset(n) => *n,
        };
        let now = out.iter().find(|(n, _)| n == name).map_or_else(|| inherited(name), |(_, v)| v.clone()).filter(|v| !v.is_empty());
        let value = match op {
            EnvOp::Set(_, v) => Some(v.to_string()),
            EnvOp::Unset(_) => None,
            EnvOp::Append(_, v) => Some(now.map_or_else(|| v.to_string(), |c| format!("{c}{}{v}", separator(name)))),
            EnvOp::Prepend(_, v) => Some(now.map_or_else(|| v.to_string(), |c| format!("{v}{}{c}", separator(name)))),
        };
        out.retain(|(n, _)| n != name);
        out.push((name.to_string(), value));
    }
    out
}

/// Runs like [`run_test_command_watched`], applying `ops` to the child's environment after the
/// [`STRIPPED`] names are removed; a [`STRIPPED`] name stays removed.
pub fn run_test_command_ops(
    test_command: &str,
    dir: &Path,
    timeout: Duration,
    on_start: Option<&mut dyn FnMut(i32) -> bool>,
    ops: &[EnvOp],
) -> RunOutcome {
    run_inner(test_command, dir, timeout, on_start, ops, None)
}

impl RunnerDetails {
    /// Whether this runner's run is read through the shell's own trace (H-2): a test that is a
    /// shell script.
    pub fn traced(&self) -> bool {
        self.runner == "sh"
    }
}

/// Runs like [`run_test_command_with`]; a `runner` read through the shell's trace
/// ([`RunnerDetails::traced`]) gets Familiar's trace in its run ([`shell_trace`]), and the
/// trace's text is returned beside the outcome as the run's report stream (empty otherwise).
pub fn run_test_command_traced(
    test_command: &str,
    dir: &Path,
    timeout: Duration,
    on_start: Option<&mut dyn FnMut(i32) -> bool>,
    env: &[(&str, &str)],
    runner: Option<&RunnerDetails>,
) -> (RunOutcome, String) {
    let trace = runner.filter(|r| r.traced()).and_then(|_| shell_trace::Trace::new(test_command, dir));
    let Some(trace) = trace else {
        let ops: Vec<EnvOp> = env.iter().map(|(n, v)| EnvOp::Set(n, v)).collect();
        return (run_inner(test_command, dir, timeout, on_start, &ops, None), String::new());
    };
    let extra = trace.env();
    let mut all: Vec<EnvOp> = env.iter().filter(|(n, _)| !extra.iter().any(|(e, _)| e == n)).map(|(n, v)| EnvOp::Set(n, v)).collect();
    all.extend(extra.iter().map(|(n, v)| EnvOp::Set(n.as_str(), v.as_str())));
    let file = if trace.opens_fd() { std::fs::File::create(trace.xtrace_file()).ok() } else { None };
    let command = trace.command(test_command);
    let mut out = run_inner(&command, dir, timeout, on_start, &all, file.as_ref());
    drop(file);
    let report = trace.finish(&mut out.stderr, dir);
    (out, report)
}

fn run_inner(
    test_command: &str,
    dir: &Path,
    timeout: Duration,
    on_start: Option<&mut dyn FnMut(i32) -> bool>,
    ops: &[EnvOp],
    trace_file: Option<&std::fs::File>,
) -> RunOutcome {
    if test_command.trim().is_empty() {
        return RunOutcome::failed(End::EmptyCommand);
    }
    if !dir.is_dir() {
        return RunOutcome::failed(End::NoDirectory);
    }
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(test_command)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in STRIPPED {
        cmd.env_remove(name);
    }
    let inherited = |n: &str| std::env::var(n).ok();
    for (name, value) in env_values(ops, &inherited).into_iter().filter(|(name, _)| !STRIPPED.contains(&name.as_str())) {
        match value {
            Some(v) => cmd.env(name, v),
            None => cmd.env_remove(name),
        };
    }
    // The child gets its own process group so killing it also ends every process it started.
    // Soundness: `pre_exec` runs in the forked child before exec, and `setpgid(0, 0)` is
    // async-signal-safe and affects only the child; if it fails the child pid is still killable.
    // The trace's side file, when there is one, is the child's [`shell_trace::TRACE_FD`]:
    // `dup2` is async-signal-safe too, and the duplicate is not closed on exec.
    let fd = trace_file.map(std::os::fd::AsRawFd::as_raw_fd);
    unsafe {
        cmd.pre_exec(move || {
            libc::setpgid(0, 0);
            if let Some(fd) = fd {
                libc::dup2(fd, shell_trace::TRACE_FD);
            }
            Ok(())
        });
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return RunOutcome::failed(End::NotStarted),
    };

    if let Some(on_start) = on_start {
        if !on_start(child.id() as i32) {
            kill_group(&mut child);
            return RunOutcome::failed(End::Cancelled);
        }
    }

    // Both pipes drain on their own threads so a command printing megabytes never blocks.
    let out = Arc::new(Mutex::new(Capture::default()));
    let err = Arc::new(Mutex::new(Capture::default()));
    let out_thread = spawn_drain(child.stdout.take(), Arc::clone(&out));
    let err_thread = spawn_drain(child.stderr.take(), Arc::clone(&err));

    let start = Instant::now();
    let (verdict, end) = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // A reaped status with no exit code was ended by a signal, which has a number.
                let end = status.code().map_or_else(|| End::Signalled(status.signal().unwrap_or(0)), End::Exited);
                break if status.success() {
                    (Verdict::Passed, end)
                } else {
                    (Verdict::Failed, end)
                };
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    break (Verdict::Failed, End::TimedOut);
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break (Verdict::Failed, End::NotStarted),
        }
    };
    // A background process left behind would outlive the run and hold the pipes open.
    kill_group(&mut child);

    // A process that left the group can still hold a pipe open, so the wait for EOF is bounded.
    let deadline = Instant::now() + READ_GRACE;
    while !(out_thread.is_finished() && err_thread.is_finished()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    let (stdout, stdout_total_bytes, stdout_dropped_bytes) = take_text(&out);
    let (stderr, stderr_total_bytes, stderr_dropped_bytes) = take_text(&err);
    RunOutcome {
        verdict,
        stdout,
        stderr,
        end,
        stdout_total_bytes,
        stdout_dropped_bytes,
        stderr_total_bytes,
        stderr_dropped_bytes,
    }
}

/// How long to wait for the pipes to close once the command has ended.
const READ_GRACE: Duration = Duration::from_secs(2);

/// One stream's capture: its first [`CAPTURE_HEAD`] bytes, a ring of its last
/// [`CAPTURE_TAIL`] bytes, and a count of every byte read.
#[derive(Default)]
struct Capture {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    total: u64,
}

impl Capture {
    fn push(&mut self, bytes: &[u8]) {
        self.total += bytes.len() as u64;
        let to_head = CAPTURE_HEAD.saturating_sub(self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..to_head]);
        let rest = &bytes[to_head..];
        // Only the last CAPTURE_TAIL bytes can stay, so the ring never grows past its cap.
        let rest = &rest[rest.len().saturating_sub(CAPTURE_TAIL)..];
        let over = (self.tail.len() + rest.len()).saturating_sub(CAPTURE_TAIL);
        self.tail.drain(..over);
        self.tail.extend(rest);
    }

    fn dropped(&self) -> u64 {
        self.total - (self.head.len() + self.tail.len()) as u64
    }
}

/// Reads a pipe to EOF on its own thread, adding to `buf` as bytes arrive.
fn spawn_drain(
    pipe: Option<impl Read + Send + 'static>,
    buf: Arc<Mutex<Capture>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let Some(mut p) = pipe else { return };
        let mut chunk = [0u8; 8192];
        loop {
            match p.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => match buf.lock() {
                    Ok(mut b) => b.push(&chunk[..n]),
                    Err(_) => break,
                },
            }
        }
    })
}

/// Returns what has been read so far with its total and dropped byte counts, converting
/// invalid UTF-8 lossily. When bytes were dropped, the head and the tail are decoded each on
/// its own around the marker.
fn take_text(buf: &Mutex<Capture>) -> (String, u64, u64) {
    let Ok(c) = buf.lock() else { return (String::new(), 0, 0) };
    let (tail_a, tail_b) = c.tail.as_slices();
    let dropped = c.dropped();
    let text = if dropped == 0 {
        String::from_utf8_lossy(&[c.head.as_slice(), tail_a, tail_b].concat()).into_owned()
    } else {
        format!(
            "{}\n[familiar: {dropped} bytes dropped]\n{}",
            String::from_utf8_lossy(&c.head),
            String::from_utf8_lossy(&[tail_a, tail_b].concat()),
        )
    };
    (text, c.total, dropped)
}

/// Kills the command's whole process group, then reaps the command.
fn kill_group(child: &mut Child) {
    let pid = child.id() as libc::pid_t;
    // Soundness: `kill` with a negated pgid sends SIGKILL to every process in the child's group,
    // and fails harmlessly with ESRCH if the group is already gone.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;

    fn dir() -> PathBuf {
        std::env::temp_dir()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "familiar-ai-run-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&d).expect("create scratch dir");
        d
    }

    #[test]
    fn true_passes() {
        let r = run_test_command("true", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Passed);
    }

    #[test]
    fn false_fails() {
        let r = run_test_command("false", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn nonzero_exit_fails() {
        let r = run_test_command("exit 3", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn missing_command_fails() {
        let r = run_test_command(
            "definitely-not-a-real-command-xyz123",
            &dir(),
            Duration::from_secs(5),
        );
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn empty_command_fails() {
        let r = run_test_command("", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
        let r = run_test_command("   ", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn missing_directory_fails() {
        let d = dir().join("familiar-ai-run-no-such-dir-xyz123");
        let r = run_test_command("true", &d, Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn killed_by_signal_fails() {
        let r = run_test_command("kill -9 $$", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Failed);
    }

    #[test]
    fn timeout_fails_and_returns_fast() {
        let start = Instant::now();
        let r = run_test_command("sleep 30", &dir(), Duration::from_secs(1));
        assert_eq!(r.verdict, Verdict::Failed);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn timeout_ends_children_too() {
        let d = scratch("child");
        let pidfile = d.join("child.pid");
        let cmd = format!("sleep 300 & echo $! > {}; wait", pidfile.display());
        let start = Instant::now();
        let r = run_test_command(&cmd, &d, Duration::from_secs(1));
        assert_eq!(r.verdict, Verdict::Failed);
        assert!(start.elapsed() < Duration::from_secs(5));
        let pid: libc::pid_t = fs::read_to_string(&pidfile)
            .expect("pid file written")
            .trim()
            .parse()
            .expect("pid is a number");
        // After a moment for the reaper, kill(pid, 0) must report ESRCH.
        thread::sleep(Duration::from_millis(100));
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        let gone = !alive && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        let _ = fs::remove_dir_all(&d);
        assert!(gone, "child process {pid} should be gone");
    }

    #[test]
    fn stdout_and_stderr_are_captured() {
        let r = run_test_command("echo out; echo err >&2", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Passed);
        assert_eq!(r.stdout.trim(), "out");
        assert_eq!(r.stderr.trim(), "err");
    }

    #[test]
    fn megabytes_of_output_do_not_hang() {
        let start = Instant::now();
        let r = run_test_command(
            // ~5 MiB on stdout, ~1 MiB on stderr, mixed.
            "i=0; while [ $i -lt 5120 ]; do printf '%1024s\\n' x; i=$((i+1)); done; \
             i=0; while [ $i -lt 1024 ]; do printf '%1024s\\n' y >&2; i=$((i+1)); done",
            &dir(),
            Duration::from_secs(30),
        );
        assert_eq!(r.verdict, Verdict::Passed);
        assert!(r.stdout.len() >= 5 * 1024 * 1024, "{}", r.stdout.len());
        assert!(r.stderr.len() >= 1024 * 1024, "{}", r.stderr.len());
        assert!(start.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn a_big_output_keeps_its_head_and_its_tail_and_counts_what_it_dropped() {
        assert_eq!((CAPTURE_HEAD, CAPTURE_TAIL), (4 * 1024 * 1024, 4 * 1024 * 1024));
        let r = run_test_command(
            "head -c 20971520 /dev/zero | tr '\\0' 'a'; echo; echo END",
            &dir(),
            Duration::from_secs(60),
        );
        assert_eq!(r.verdict, Verdict::Passed);
        let total: u64 = 20971520 + "\nEND\n".len() as u64;
        assert_eq!(r.stdout_total_bytes, total);
        let dropped = total - (CAPTURE_HEAD + CAPTURE_TAIL) as u64;
        assert_eq!(r.stdout_dropped_bytes, dropped);
        let marker = format!("\n[familiar: {dropped} bytes dropped]\n");
        // Bounded: the kept text is the head, the marker and the tail, never the whole 20 MiB.
        assert_eq!(r.stdout.len(), CAPTURE_HEAD + marker.len() + CAPTURE_TAIL);
        assert_eq!(r.stdout.find(&marker), Some(CAPTURE_HEAD), "the marker sits after the head");
        assert!(r.stdout[..CAPTURE_HEAD].bytes().all(|b| b == b'a'));
        assert!(r.stdout.ends_with("a\nEND\n"), "{:?}", &r.stdout[r.stdout.len() - 16..]);
        assert_eq!((r.stderr_total_bytes, r.stderr_dropped_bytes), (0, 0));
    }

    #[test]
    fn a_small_output_is_kept_whole_and_drops_nothing() {
        let r = run_test_command("echo out; echo err >&2", &dir(), Duration::from_secs(5));
        assert_eq!((r.stdout.as_str(), r.stderr.as_str()), ("out\n", "err\n"));
        assert_eq!((r.stdout_total_bytes, r.stdout_dropped_bytes), (4, 0));
        assert_eq!((r.stderr_total_bytes, r.stderr_dropped_bytes), (4, 0));
    }

    #[test]
    fn runs_in_the_given_directory() {
        let d = scratch("cwd");
        fs::write(d.join("marker.txt"), "here").expect("write marker");
        let r = run_test_command("cat marker.txt", &d, Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Passed);
        assert_eq!(r.stdout.trim(), "here");
        let r = run_test_command("pwd", &d, Duration::from_secs(5));
        let got = fs::canonicalize(r.stdout.trim()).expect("pwd path");
        let want = fs::canonicalize(&d).expect("scratch path");
        let _ = fs::remove_dir_all(&d);
        assert_eq!(got, want);
    }

    #[test]
    fn non_utf8_output_is_a_result_not_an_error() {
        let r = run_test_command("printf '\\377\\376ok\\n'", &dir(), Duration::from_secs(5));
        assert_eq!(r.verdict, Verdict::Passed);
        assert!(r.stdout.ends_with("ok\n"), "{:?}", r.stdout);
    }

    #[test]
    fn how_it_ended_tells_a_failing_exit_from_a_run_that_did_not_happen() {
        let d = dir();
        let w = Duration::from_secs(5);
        assert_eq!(run_test_command("exit 1", &d, w).end, End::Exited(1));
        assert_eq!(run_test_command("exit 1", &d, w).did_not_run(), None);
        assert_eq!(run_test_command("true", &d, w).end, End::Exited(0));
        assert_eq!(run_test_command("kill -9 $$", &d, w).end, End::Signalled(9));
        assert_eq!(run_test_command("sleep 30", &d, Duration::from_millis(200)).end, End::TimedOut);
        assert_eq!(run_test_command("", &d, w).end, End::EmptyCommand);
        assert_eq!(run_test_command("true", &d.join("familiar-ai-run-no-such-dir-xyz123"), w).end, End::NoDirectory);
        let missing = run_test_command("definitely-not-a-real-command-xyz123", &d, w);
        assert_eq!((missing.verdict, missing.end), (Verdict::Failed, End::Exited(127)));
        assert!(missing.did_not_run().is_some_and(|w| w.contains("not found")));
        // A child of the shell ended by a signal makes the shell exit 128 plus the signal.
        let killed = run_test_command("sh -c 'kill -9 $$'; exit $?", &d, w);
        assert!(killed.did_not_run().is_some_and(|w| w.contains("signal 9 (exit 137)")), "{:?}", killed.end);
        assert_eq!(run_test_command("exit 101", &d, w).did_not_run(), None, "a failing cargo test is a run");
        for cmd in ["kill -9 $$", ""] {
            assert!(run_test_command(cmd, &d, w).did_not_run().is_some(), "{cmd}");
        }
    }

    #[test]
    fn a_signal_that_ends_the_command_keeps_its_number() {
        let d = dir();
        let w = Duration::from_secs(5);
        // The runner's `sh -c` execs the command, so the signal ends the child itself.
        let segv = run_test_command(r#"exec sh -c "kill -SEGV \$\$""#, &d, w);
        assert_eq!(segv.end, End::Signalled(11));
        let words = segv.did_not_run();
        assert!(words.as_deref().is_some_and(|w| w.contains("ended by signal 11")), "{words:?}");
        let killed = run_test_command("kill -KILL $$", &d, w);
        assert_eq!(killed.end, End::Signalled(9));
        assert!(killed.did_not_run().is_some());
        // Through a shell that goes on, the same signal is the shell's exit 128 plus its number.
        let through = run_test_command(r#"sh -c "kill -SEGV \$\$"; exit $?"#, &d, w);
        assert_eq!(through.end, End::Exited(139));
        assert!(through.did_not_run().is_some());
    }

    #[test]
    fn a_background_process_left_behind_does_not_hold_the_call() {
        let d = scratch("left-behind");
        let pidfile = d.join("bg.pid");
        let cmd = format!("sleep 300 & echo $! > {}; echo done", pidfile.display());
        let start = Instant::now();
        let r = run_test_command(&cmd, &d, Duration::from_secs(20));
        assert_eq!(r.verdict, Verdict::Passed);
        assert_eq!(r.stdout.trim(), "done");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "took {:?}",
            start.elapsed()
        );
        let pid: libc::pid_t = fs::read_to_string(&pidfile)
            .expect("pid file")
            .trim()
            .parse()
            .expect("pid");
        thread::sleep(Duration::from_millis(100));
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        let _ = fs::remove_dir_all(&d);
        assert!(!alive, "the background process {pid} should be gone");
    }

    #[test]
    fn a_process_that_left_the_group_cannot_hold_the_call() {
        // `setsid` is not on every machine (macOS has none): nothing to check there.
        if run_test_command("command -v setsid", &dir(), Duration::from_secs(5)).verdict
            == Verdict::Failed
        {
            return;
        }
        let d = scratch("escaped");
        let pidfile = d.join("esc.pid");
        let cmd = format!(
            "setsid sh -c 'echo $$ > {}; exec sleep 300' & echo started",
            pidfile.display()
        );
        let start = Instant::now();
        let r = run_test_command(&cmd, &d, Duration::from_secs(20));
        assert_eq!(r.verdict, Verdict::Passed);
        assert!(r.stdout.contains("started"));
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "took {:?}",
            start.elapsed()
        );
        thread::sleep(Duration::from_millis(200));
        if let Ok(text) = fs::read_to_string(&pidfile) {
            if let Ok(pid) = text.trim().parse::<libc::pid_t>() {
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn output_before_a_timeout_is_kept() {
        let r = run_test_command("echo before; sleep 30", &dir(), Duration::from_secs(1));
        assert_eq!(r.verdict, Verdict::Failed);
        assert_eq!(r.stdout.trim(), "before");
    }

    #[test]
    fn the_env_given_reaches_the_child_and_the_stripped_names_stay_removed() {
        let w = Duration::from_secs(5);
        let r = run_test_command_with(
            r#"echo "${FAMILIAR_AI_RUN_GIVEN-unset}|${CLAUDE_PROJECT_DIR-unset}|${CLAUDECODE-unset}""#,
            &dir(),
            w,
            None,
            &[("FAMILIAR_AI_RUN_GIVEN", "here"), ("CLAUDE_PROJECT_DIR", "/given"), ("CLAUDECODE", "1")],
        );
        assert_eq!(r.verdict, Verdict::Passed);
        assert_eq!(r.stdout.trim(), "here|unset|unset");
        let plain = run_test_command_with("echo ${FAMILIAR_AI_RUN_GIVEN-unset}", &dir(), w, None, &[]);
        assert_eq!(plain.stdout.trim(), "unset");
    }

    /// The runner a command names, by its row's name; `None` when it names none.
    fn runner(command: &str) -> Option<&'static str> {
        details_for(command).map(|row| row.runner)
    }

    #[test]
    fn a_runner_that_prints_its_details_elsewhere_has_them_in_its_row() {
        let ctest = details_for("ctest --test-dir build").expect("ctest is in the table");
        assert_eq!(ctest.env, &[("CTEST_OUTPUT_ON_FAILURE", "1")]);
        let maven = details_for("mvn -q test").expect("mvn is in the table");
        assert_eq!(maven.reports, &["target/surefire-reports/*.txt", "target/surefire-reports/*.xml"]);
        let gradle = details_for("gradle test").expect("gradle is in the table");
        assert_eq!(gradle.reports, &["build/test-results/**/*.xml"]);
        let pytest = details_for("pytest -q").expect("pytest is in the table");
        assert!(pytest.env.is_empty() && pytest.reports.is_empty(), "its details are in the console");
    }

    #[test]
    fn each_runner_is_found_by_the_first_word_of_its_command_by_basename() {
        for (command, want) in [
            ("ctest --output-on-failure", "ctest"),
            ("mvn -q test", "maven"),
            ("./mvnw test", "maven"),
            ("gradle test", "gradle"),
            ("./gradlew test --info", "gradle"),
            ("dotnet test", "dotnet"),
            ("pytest -q tests/test_a.py", "pytest"),
            (".venv/bin/pytest", "pytest"),
            ("jest --ci", "jest"),
            ("node_modules/.bin/jest", "jest"),
            ("vitest run", "vitest"),
            ("mocha test/a.spec.js", "mocha"),
            ("node --test", "node-test"),
            ("cargo test --locked", "cargo-test"),
            ("cargo nextest run", "cargo-nextest"),
            ("cargo-nextest nextest run", "cargo-nextest"),
            ("go test ./...", "go-test"),
            ("vendor/bin/phpunit", "phpunit"),
            ("./vendor/bin/pest", "pest"),
        ] {
            assert_eq!(runner(command), Some(want), "{command}");
        }
    }

    #[test]
    fn leading_assignments_and_wrappers_are_skipped() {
        for (command, want) in [
            ("CI=1 FORCE_COLOR=0 jest", "jest"),
            ("npx vitest run", "vitest"),
            ("npx --yes jest", "jest"),
            ("pnpm exec vitest run", "vitest"),
            ("yarn jest --ci", "jest"),
            ("bunx vitest", "vitest"),
            ("uv run pytest -q", "pytest"),
            ("poetry run pytest", "pytest"),
            ("pipenv run pytest", "pytest"),
            ("python -m pytest tests/", "pytest"),
            ("python3 -m pytest", "pytest"),
            ("python -m unittest discover", "unittest"),
            (".venv/bin/python -m pytest", "pytest"),
            ("uv run python -m pytest", "pytest"),
            ("PYTHONPATH=src uv run python3 -m unittest", "unittest"),
        ] {
            assert_eq!(runner(command), Some(want), "{command}");
        }
    }

    #[test]
    fn each_segment_is_read_and_the_last_match_wins() {
        assert_eq!(runner("cd build && ctest"), Some("ctest"));
        assert_eq!(runner("npm ci && npx vitest run"), Some("vitest"));
        assert_eq!(runner("cmake -S . -B build && cmake --build build && ctest --test-dir build"), Some("ctest"));
        assert_eq!(runner("pytest -q; jest"), Some("jest"));
        assert_eq!(runner("jest && echo done"), Some("jest"), "a later segment naming no runner does not hide an earlier one");
        assert_eq!(
            runner("cmake -S . -B build && cmake --build build && ./build/tests"),
            Some("ctest"),
            "plan section 5: the project's own binary names no runner, so the cmake segment before it picks the C++ row"
        );
    }

    #[test]
    fn a_command_that_names_no_runner_has_no_row() {
        // `./run_tests.sh` and `sh tests/test_c1.sh` name the sh runner (the owner, 2026-10-15)
        for command in ["make test", "./run_tests", "npm test", "yarn test", "python tests/run.py", "", "   ", "FOO=1", "npx", "python -m"] {
            assert_eq!(runner(command), None, "{command:?}");
        }
    }

    #[test]
    fn a_passing_word_in_the_output_does_not_make_a_pass() {
        let r = run_test_command(
            "echo 'test result: ok. 5 passed'; exit 1",
            &dir(),
            Duration::from_secs(5),
        );
        assert_eq!(r.verdict, Verdict::Failed);
        let r = run_test_command(
            "echo 'FAILED: 3 errors'; exit 0",
            &dir(),
            Duration::from_secs(5),
        );
        assert_eq!(r.verdict, Verdict::Passed);
    }
}
