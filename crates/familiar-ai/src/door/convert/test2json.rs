//! `go test -json` (test2json) to the per-test result file (plan A, JSON Lines v1; lane akgorust).
//! The stream is read from the run's stdout (`GOFLAGS` gets ` -json` when `go test` is the
//! command's only go call, else `-json` goes into the `go test` part; `kits::go_test`), so there is
//! no side file and the version is not in it (`runner_version` is empty).
//!
//! What it writes, and when it writes nothing (no session line, the next reader takes over):
//! - no line is a test2json event: nothing. A setup failure up to go 1.23 is plain text
//!   (`FAIL pkg [setup failed]`), and so is the console of a run with no `-json`.
//! - a package that failed without a failing test and without a build failure (a `TestMain` that
//!   exits 1, an init panic, a timeout), a build failure whose reason is not in the stream (go 1.23
//!   prints the compiler's lines on stderr, only `[build failed]` is JSON), a test with no verdict
//!   in a package that passed: nothing says what happened, so nothing is read.
//! - a package that never ended (the stream was cut), a process that died by a signal (exit 128 + n
//!   or negative), a stream with `kept::cut`'s `[familiar: N chars dropped]` line in it (its middle
//!   is gone and what is left can look complete): the lines seen, and no `end` line, so nothing
//!   reads it.
//!
//! A test that failed is a `panic` (kind `exception`, the project's frames outermost first, the
//! testing and runtime frames dropped), else its first `file.go:line: message` output line (kind
//! `assertion`, Go has no assertion type: a `t.Error` or `t.Fatal` line), else an `exception` with
//! no frames. A test with no verdict in a package that failed died with the binary. go 1.24 prints a
//! build failure as `build-output` events (the compiler's words, kind `missing_name` for an
//! undefined name, `exception` otherwise, a missing module among them) and the package's `fail`
//! event names it in `FailedBuild`.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

/// `[./]file.go:LINE[:COL]: message`, as `t.Error` prints it (the file's base name only) and the
/// compiler prints a diagnostic.
fn location() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s*(?:\./)?(\S+?\.go):(\d+)(?::\d+)?: (.*)$").expect("location"))
}

/// What the compiler says of a name that does not exist.
fn missing_name() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"undefined: |has no field or method |not declared by package |undefined \(type ").expect("missing name"))
}

/// A goroutine trace's function line, `pkg.Func(0xc000...)`.
fn frame_function() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(\S+)\(.*\)$").expect("frame function"))
}

/// A goroutine trace's file line, `\t/path/file.go:LINE +0x1d`.
fn frame_file() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\t(\S+\.go):(\d+)(?: \+0x[0-9a-f]+)?$").expect("frame file"))
}

/// A panic's own line: at the start of a line, so a message that says "panic: " is not one.
fn panic_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^panic: (.*)$").expect("panic line"))
}

/// `kept::cut`'s line where a stream's middle was dropped: what is left can look complete.
fn is_cut_mark(line: &str) -> bool {
    line.starts_with("[familiar: ") && line.ends_with(" chars dropped]")
}

/// A process that died by a signal (the shell's 128 + n, or negative) did not finish.
fn died(exit: i32) -> bool {
    !(0..128).contains(&exit)
}

/// The actions test2json writes (go 1.24's two build actions among them); a line of JSON with any
/// other `Action` is not one of its events.
const ACTIONS: [&str; 12] = ["start", "run", "pause", "cont", "output", "pass", "fail", "skip", "bench", "attr", "build-output", "build-fail"];

/// Insertion-ordered map: the file lists tests in the order they first appeared.
struct Ordered<V> {
    index: HashMap<String, usize>,
    items: Vec<V>,
}

impl<V: Default> Ordered<V> {
    fn new() -> Self {
        Ordered { index: HashMap::new(), items: Vec::new() }
    }

    fn entry(&mut self, key: &str) -> &mut V {
        let at = match self.index.get(key) {
            Some(&at) => at,
            None => {
                self.items.push(V::default());
                self.index.insert(key.to_string(), self.items.len() - 1);
                self.items.len() - 1
            }
        };
        &mut self.items[at]
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.index.get(key).map(|&at| &self.items[at])
    }
}

#[derive(Default)]
struct Build {
    out: String,
}

#[derive(Default)]
struct Test {
    pkg: String,
    name: String,
    out: Vec<String>,
    outcome: Option<&'static str>,
}

#[derive(Default)]
struct Package {
    ended: Option<&'static str>,
    failed_build: Option<String>,
}

/// One frame of a goroutine trace.
struct Frame {
    file: String,
    line: u64,
    function: String,
}

/// The project's frames of the panicking goroutine's trace, outermost first (Go prints the innermost
/// first): the testing and runtime frames, the standard library and the module cache are not the
/// project's.
fn frames(out: &[String]) -> Vec<Frame> {
    let mut raw: Vec<Frame> = Vec::new();
    let mut function: Option<String> = None;
    let mut goroutines = 0;
    for text in out {
        let line = text.trim_end_matches(['\n', '\r']);
        // the dump lists the goroutine that panicked first, then every other: theirs are not its own
        if line.starts_with("goroutine ") && line.ends_with(':') {
            goroutines += 1;
            if goroutines > 1 {
                break;
            }
        }
        if let (Some(m), Some(f)) = (frame_file().captures(line), function.as_ref()) {
            raw.push(Frame { file: m[1].to_string(), line: m[2].parse().unwrap_or(0), function: f.clone() });
            function = None;
            continue;
        }
        function = frame_function().captures(line).map(|m| m[1].to_string());
    }
    // the testing package's own file says where GOROOT is on this machine
    let goroot_src = raw.iter().find_map(|f| f.file.strip_suffix("testing/testing.go").filter(|p| p.ends_with("/src/")).map(str::to_string));
    raw.retain(|f| !(f.file.contains("/pkg/mod/") || f.file.contains("/src/runtime/") || f.file.contains("/src/testing/") || goroot_src.as_deref().is_some_and(|g| f.file.starts_with(g))));
    raw.reverse();
    raw
}

fn frame_json(f: &Frame) -> Value {
    json!({"file": f.file, "line": f.line, "function": f.function})
}

/// A package's build output as `collect_error` lines: one per diagnostic (a location line and what
/// follows it up to the next), or one for the whole text when nothing in it has a location.
fn compile_errors(import_path: &str, text: &str) -> Vec<Value> {
    // (file, line, message)
    let mut found: Vec<(Option<(String, u64)>, String)> = Vec::new();
    let mut loose: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(m) = location().captures(line) {
            found.push((Some((m[1].to_string(), m[2].parse().unwrap_or(0))), m[3].to_string()));
        } else if let Some(last) = found.last_mut() {
            last.1.push('\n');
            last.1.push_str(line.trim_end());
        } else {
            loose.push(line.trim_end());
        }
    }
    if found.is_empty() {
        found.push((None, loose.join("\n")));
    }
    found
        .into_iter()
        .map(|(at, message)| {
            let kind = if missing_name().is_match(&message) { "missing_name" } else { "exception" };
            let message: String = message.chars().take(2000).collect();
            json!({
                "type": "collect_error",
                "file": at.as_ref().map_or(import_path, |(f, _)| f.as_str()),
                "package": import_path,
                "error": {
                    "type": "compile",
                    "message": message,
                    "kind": kind,
                    "frames": at.as_ref().map(|(f, l)| vec![json!({"file": f, "line": l, "function": null})]).unwrap_or_default(),
                },
            })
        })
        .collect()
}

/// The test's line in the file.
fn test_line(t: &Test, outcome: &str) -> Value {
    let mut file = Value::Null;
    let mut skip_reason = Value::Null;
    let mut error = Value::Null;
    let located: Vec<(String, u64, String)> = t
        .out
        .iter()
        .filter_map(|o| location().captures(o.trim_end_matches(['\n', '\r'])).map(|m| (m[1].to_string(), m[2].parse().unwrap_or(0), m[3].to_string())))
        .collect();
    if outcome == "skipped" {
        skip_reason = located.last().map_or(Value::Null, |l| json!(l.2));
    }
    if outcome == "failed" {
        let text: String = t.out.concat();
        if let Some(m) = panic_line().captures(&text) {
            let project = frames(&t.out);
            file = project.iter().find(|f| f.file.ends_with("_test.go")).map_or(Value::Null, |f| json!(f.file));
            error = json!({"type": "panic", "message": m[1].to_string(), "kind": "exception", "frames": project.iter().map(frame_json).collect::<Vec<_>>()});
        } else if let Some((f, l, msg)) = located.first() {
            file = json!(f);
            error = json!({"type": "t.Error", "message": msg, "kind": "assertion", "frames": [{"file": f, "line": l, "function": t.name}]});
        } else {
            let tail: String = text.chars().rev().take(500).collect::<Vec<_>>().into_iter().rev().collect();
            error = json!({"type": "unknown", "message": tail, "kind": "exception", "frames": []});
        }
    }
    json!({"type": "test", "id": format!("{}.{}", t.pkg, t.name), "file": file, "line": null, "outcome": outcome, "skip_reason": skip_reason, "error": error})
}

/// The per-test result file for a `go test -json` run's stdout and exit code, or the empty string
/// when the output holds no structured result (see the module's note).
pub fn to_report(native: &str, exit: i32) -> String {
    convert(native, exit).unwrap_or_default()
}

fn convert(native: &str, exit: i32) -> Option<String> {
    let mut builds: Ordered<Build> = Ordered::new();
    let mut tests: Ordered<Test> = Ordered::new();
    let mut packages: Ordered<Package> = Ordered::new();
    let mut saw_event = false;
    let mut cut = false;
    for line in native.lines() {
        let Ok(Value::Object(e)) = serde_json::from_str::<Value>(line) else {
            cut |= is_cut_mark(line);
            continue;
        };
        let Some(action) = e.get("Action").and_then(Value::as_str).filter(|a| ACTIONS.contains(a)) else { continue };
        saw_event = true;
        let text = |k: &str| e.get(k).and_then(Value::as_str);
        if action == "build-output" || action == "build-fail" {
            let b = builds.entry(text("ImportPath").unwrap_or(""));
            if action == "build-output" {
                b.out.push_str(text("Output").unwrap_or(""));
            }
            continue;
        }
        let pkg = text("Package").unwrap_or("");
        let package = packages.entry(pkg);
        match text("Test").filter(|t| !t.is_empty()) {
            Some(name) => {
                let t = tests.entry(&format!("{pkg}\u{0}{name}"));
                t.pkg = pkg.to_string();
                t.name = name.to_string();
                match action {
                    "output" => t.out.push(text("Output").unwrap_or("").to_string()),
                    "pass" => {
                        t.outcome = Some("passed");
                        t.out = Vec::new(); // a passing test's output is never read: a big run stays small
                    }
                    "fail" => t.outcome = Some("failed"),
                    "skip" => t.outcome = Some("skipped"),
                    _ => {}
                }
            }
            None => {
                match action {
                    "pass" => package.ended = Some("passed"),
                    "fail" => package.ended = Some("failed"),
                    "skip" => package.ended = Some("skipped"),
                    _ => {}
                }
                if let Some(b) = text("FailedBuild") {
                    package.failed_build = Some(b.to_string());
                }
            }
        }
    }
    if !saw_event {
        return None;
    }

    let package_ended = |pkg: &str| packages.get(pkg).and_then(|p| p.ended);
    let finished = packages.items.iter().all(|p| p.ended.is_some()) && !cut && !died(exit);

    // a package that failed says why by a failing test or by its build, or nothing is read
    let mut errors: Vec<Value> = Vec::new();
    let mut shown: Vec<&str> = Vec::new();
    let mut order: Vec<(&str, usize)> = packages.index.iter().map(|(k, &v)| (k.as_str(), v)).collect();
    order.sort_by_key(|&(_, at)| at);
    for (pkg, at) in order {
        let p = &packages.items[at];
        if p.ended != Some("failed") {
            continue;
        }
        let failing_test = tests.items.iter().any(|t| t.pkg == pkg && t.outcome != Some("passed") && t.outcome != Some("skipped"));
        match &p.failed_build {
            Some(key) => {
                let text = builds.get(key).map(|b| b.out.as_str()).filter(|t| !t.trim().is_empty())?;
                if !shown.contains(&key.as_str()) {
                    shown.push(key);
                    errors.extend(compile_errors(key, text));
                }
            }
            None if !failing_test => return None,
            None => {}
        }
    }

    let mut lines = vec![json!({"type": "session", "v": 1, "runner": "go-test", "runner_version": "", "language": "go", "language_version": "", "plugins": []})];
    let n_errors = errors.len();
    lines.extend(errors);
    let mut counts = [0u64; 3]; // passed, failed, skipped
    for t in &tests.items {
        let outcome = match (t.outcome, package_ended(&t.pkg)) {
            (Some(o), _) => o,
            // no verdict: the binary died with it, or the stream was cut with it running
            (None, Some("failed")) => "failed",
            (None, None) => "error",
            (None, Some(_)) => return None,
        };
        match outcome {
            "passed" => counts[0] += 1,
            "failed" => counts[1] += 1,
            "skipped" => counts[2] += 1,
            _ => {}
        }
        lines.push(test_line(t, outcome));
    }
    if finished {
        lines.push(json!({"type": "end", "exit": exit, "counts": {"passed": counts[0], "failed": counts[1], "skipped": counts[2], "deselected": 0, "errors": n_errors}}));
    }
    Some(lines.iter().map(|l| format!("{l}\n")).collect())
}
