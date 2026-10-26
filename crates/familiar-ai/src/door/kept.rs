//! What the binary keeps on the laptop beside the record: the end of each test
//! run it makes and the transcript of each investigator it launches. Each is
//! written only where git ignores it, and a failed write changes nothing decided.

use std::path::Path;

pub use crate::door::runner_tables::{Table, TABLES};
use crate::door::{FAMILIAR_FOLDER, RECORD_FILE};

/// The folder of kept runs inside [`FAMILIAR_FOLDER`].
pub const RUNS_FOLDER: &str = "runs";
/// The folder of the investigators' transcripts inside [`FAMILIAR_FOLDER`].
pub const TRANSCRIPTS_FOLDER: &str = "investigator";
/// How many trailing characters of a run's output are kept.
pub const KEPT_OUTPUT: usize = 4_000;

/// How many characters of a stream's start a run's evidence keeps (plan §3).
pub const CUT_HEAD: usize = 2_000;
/// How many characters of a stream's end a run's evidence keeps (plan §3).
pub const CUT_TAIL: usize = 6_000;

/// A run's output as the evidence reads it: ANSI escapes removed, and every
/// `\r\n` and lone `\r` made a `\n`.
pub fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\u{1b}' => match it.next() {
                // CSI: parameters and intermediates up to a final byte @..~
                Some('[') => skip_csi(&mut it),
                // OSC, DCS, SOS, PM, APC: a string ended by BEL or ST (ESC \)
                Some(']' | 'P' | 'X' | '^' | '_') => {
                    while let Some(d) = it.next() {
                        if d == '\u{7}' || (d == '\u{1b}' && it.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                // a charset designation takes one more character
                Some('(' | ')' | '*' | '+' | '-' | '.' | '/' | '#' | '%' | ' ') => {
                    it.next();
                }
                _ => {}
            },
            '\u{9b}' => skip_csi(&mut it),
            '\r' => {
                it.next_if_eq(&'\n');
                out.push('\n');
            }
            _ => out.push(c),
        }
    }
    out
}

fn skip_csi(it: &mut std::iter::Peekable<std::str::Chars>) {
    for d in it.by_ref() {
        if ('\u{40}'..='\u{7e}').contains(&d) {
            break;
        }
    }
}

/// The stored cut of a cleaned stream: its first [`CUT_HEAD`] and last
/// [`CUT_TAIL`] characters, each ending or starting at a line boundary when the
/// budget holds one, joined by the line `[familiar: N chars dropped]`; and N.
/// A stream that fits is returned whole with 0.
pub fn cut(s: &str) -> (String, usize) {
    let total = s.chars().count();
    if total <= CUT_HEAD + CUT_TAIL {
        return (s.to_string(), 0);
    }
    let at = |n: usize| s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    let head_window = &s[..at(CUT_HEAD)];
    let head = match head_window.rfind('\n') {
        Some(i) => &head_window[..=i],
        None => head_window,
    };
    let tail_window = &s[at(total - CUT_TAIL)..];
    let at_line_start = s[..s.len() - tail_window.len()].ends_with('\n');
    let tail = match tail_window.find('\n') {
        Some(i) if !at_line_start && i + 1 < tail_window.len() => &tail_window[i + 1..],
        _ => tail_window,
    };
    let dropped = total - head.chars().count() - tail.chars().count();
    let sep = if head.ends_with('\n') { "" } else { "\n" };
    (format!("{head}{sep}[familiar: {dropped} chars dropped]\n{tail}"), dropped)
}

/// A run's three cleaned texts, as [`classify`] searches them: stdout, stderr, and the text of
/// the runner's report files.
#[derive(Debug, Clone, Copy, Default)]
pub struct Streams<'a> {
    pub stdout: &'a str,
    pub stderr: &'a str,
    pub report: &'a str,
}

impl<'a> Streams<'a> {
    fn get(&self, name: &str) -> &'a str {
        match name {
            "stdout" => self.stdout,
            "stderr" => self.stderr,
            _ => self.report,
        }
    }
}

/// The streams a row or location row searches, in order: `either` is stderr, stdout, report.
fn order(stream: &'static str) -> &'static [&'static str] {
    match stream {
        "either" => &["stderr", "stdout", "report"],
        "stdout" => &["stdout"],
        "stderr" => &["stderr"],
        _ => &["report"],
    }
}

/// What a run's output reads as (plan §2, §3, §6.2): the label, one line of evidence verbatim
/// from the text searched, the failing tests when the deciding row names them, and where the
/// output points with the name it says is missing. `by` is `regex`, or `binary` for an end
/// Familiar itself knows (timed out, empty command, no directory).
#[derive(Debug, Clone, PartialEq)]
pub struct Translation {
    pub by: &'static str,
    pub label: String,
    pub evidence: String,
    pub failing_tests: Vec<String>,
    pub location: Option<familiar_ai_facts::Location>,
    pub missing_name: Option<String>,
    /// Every place the output points, most wanted first (plan §6.2: the deepest frame first for
    /// an exception, the first error for a compile error), each with the kind of the row that
    /// found it; `location` is the first of them. The binary keeps the first one git tracks at the
    /// run's commit ([`in_project`]), under the location rules of [`test_log_translation`].
    pub candidates: Vec<familiar_ai_facts::Location>,
    /// What the runner's own summary counted ([`count`]); empty when it counted nothing.
    pub counts: familiar_ai_facts::Counts,
}

/// One table's reading: the row that decided (its index, from 0) or `None` for the exit
/// default, the label after the contradiction rule, and what the row matched.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub row: Option<usize>,
    pub label: &'static str,
    pub evidence: String,
    pub failing_tests: Vec<String>,
}

struct Compiled {
    rows: Vec<Option<regex::Regex>>,
    locs: Vec<Option<regex::Regex>>,
    counts: Vec<Option<regex::Regex>>,
}

impl Compiled {
    fn of(t: &Table) -> Compiled {
        Compiled {
            rows: t.rows.iter().map(|r| compile(r.regex)).collect(),
            locs: t.locs.iter().map(|l| compile(l.regex)).collect(),
            counts: t.counts.iter().map(|c| compile(c.regex)).collect(),
        }
    }
}

/// The fields a count row may add to ([`crate::door::runner_tables::Count`]).
pub const COUNT_FIELDS: &[&str] = &["passed", "failed", "errors", "skipped", "deselected", "total"];

/// Whether count row `row` of `table` compiles and names one of [`COUNT_FIELDS`]; `None` when the
/// table or the row does not exist.
pub fn count_compiles(table: &str, row: usize) -> Option<bool> {
    let t = table_named(table)?;
    let c = compiled(t)?;
    let rx = c.counts.get(row)?;
    Some(rx.is_some() && COUNT_FIELDS.contains(&t.counts[row].field))
}

/// What `t`'s count rows read in the output (lane H3, D7): each row's matches in the first stream
/// of its order where it matches, each adding its `n` (or one) to its field. `None` when no row
/// matched. Passed, when no row gave it, is total − failed − errors − skipped; errors are counted
/// as failed.
pub fn count(t: &'static Table, s: &Streams) -> Option<familiar_ai_facts::Counts> {
    let rxs = compiled(t).map_or(&[][..], |c| &c.counts[..]);
    let mut sums: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for (row, rx) in t.counts.iter().zip(rxs) {
        let Some(rx) = rx else { continue };
        let Some(text) = order(row.stream).iter().map(|n| s.get(n)).find(|text| rx.is_match(text)) else { continue };
        let named = rx.capture_names().any(|n| n == Some("n"));
        let add: u64 = rx.captures_iter(text).map(|c| if named { c.name("n").and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0) } else { 1 }).sum();
        *sums.entry(row.field).or_default() += add;
    }
    if sums.is_empty() {
        return None;
    }
    let get = |f: &str| sums.get(f).copied();
    let failed = match (get("failed"), get("errors")) {
        (None, None) => None,
        (f, e) => Some(f.unwrap_or(0) + e.unwrap_or(0)),
    };
    let passed = get("passed").or_else(|| get("total").map(|n| n.saturating_sub(failed.unwrap_or(0)).saturating_sub(get("skipped").unwrap_or(0))));
    Some(familiar_ai_facts::Counts { passed, failed, skipped: get("skipped"), deselected: get("deselected") })
}

fn compile(rx: &str) -> Option<regex::Regex> {
    regex::RegexBuilder::new(rx).size_limit(1 << 28).dfa_size_limit(1 << 26).build().ok()
}

/// A table's patterns, compiled once per process when first read; `None` for a table that is not
/// one of [`TABLES`]. A pattern that does not compile never matches (the tables' own test
/// compiles every one).
fn compiled(t: &'static Table) -> Option<&'static Compiled> {
    static ALL: std::sync::OnceLock<Vec<std::sync::OnceLock<Compiled>>> = std::sync::OnceLock::new();
    let all = ALL.get_or_init(|| TABLES.iter().map(|_| std::sync::OnceLock::new()).collect());
    let i = TABLES.iter().position(|x| std::ptr::eq(x, t))?;
    Some(all[i].get_or_init(|| Compiled::of(t)))
}

/// Whether row `row` of `t` matches any text (a guard row such as Catch2's exit-0 `^`).
fn matches_anything(t: &'static Table, row: usize) -> bool {
    compiled(t).and_then(|c| c.rows.get(row)).and_then(Option::as_ref).is_some_and(|rx| rx.is_match(""))
}

/// Whether a pattern of `table` compiles; `None` when the table or the pattern does not exist.
pub fn compiles(table: &str, row: usize, loc: bool) -> Option<bool> {
    let t = table_named(table)?;
    let c = compiled(t)?;
    let list = if loc { &c.locs } else { &c.rows };
    list.get(row).map(Option::is_some)
}

/// The table recorded under `runner`'s name.
pub fn table_named(runner: &str) -> Option<&'static Table> {
    TABLES.iter().find(|t| t.runner == runner)
}

/// The tables a run's runner selects: the tables of its `RUNNER_DETAILS` row (one, or a
/// family's three), or every table but sh's when the command names no runner (make is not sh:
/// sh's exit-1 row would read any runner's silent failure).
pub fn tables_for(runner: Option<&familiar_ai_run::RunnerDetails>) -> Vec<&'static Table> {
    match runner {
        Some(r) => TABLES.iter().filter(|t| t.row == r.runner).collect(),
        None => TABLES.iter().filter(|t| t.row != "sh").collect(),
    }
}

fn exit_holds(cond: &str, code: i64) -> bool {
    match cond.trim() {
        "any" => true,
        "0" => code == 0,
        "nonzero" => code != 0,
        "sig" => code < 0,
        list => list.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).any(|n| n == code),
    }
}

/// The whole line of `text` that holds byte `at`.
fn line_at(text: &str, at: usize) -> &str {
    let a = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let b = text[at..].find('\n').map_or(text.len(), |i| at + i);
    &text[a..b]
}

/// A red label at exit 0, or `passed` at any other exit, is `unclassified` (plan §2).
fn contradicts(label: &str, code: i64) -> bool {
    (code == 0 && !matches!(label, "passed" | "no_tests_collected" | "unclassified")) || (code != 0 && label == "passed")
}

/// The exit defaults when no row matched (plan §2, H-1: exit 0 is unclassified); 126 and 127 are split later by what is missing.
fn exit_default(code: i64) -> &'static str {
    match code {
        126 | 127 => "environment_error",
        132 | 134 | 135 | 136 | 139 | -4 | -6 | -7 | -8 | -11 => "crashed",
        130 | 137 | 143 | -2 | -9 | -15 => "killed",
        // H-1 (2026-10-19): exit 0 with output no row knows is not a pass; a table says "passed" itself
        0 => "unclassified",
        _ => "unclassified",
    }
}

/// `t`'s reading of a run that exited with `code` (a signal N is -N): its rows in order, the
/// first whose exit condition holds and whose pattern is found in its stream; else the exit
/// default.
pub fn read_table(t: &'static Table, code: i64, s: &Streams) -> Reading {
    let rows = compiled(t).map_or(&[][..], |c| &c.rows[..]);
    for (i, (row, rx)) in t.rows.iter().zip(rows).enumerate() {
        let Some(rx) = rx else { continue };
        if !exit_holds(row.exit, code) {
            continue;
        }
        for name in order(row.stream) {
            let text = s.get(name);
            let Some(caps) = rx.captures(text) else { continue };
            let at = caps.get(0).map_or(0, |m| m.start());
            let failing_tests = if rx.capture_names().any(|n| n == Some("test")) {
                let mut seen: Vec<String> = Vec::new();
                for cap in rx.captures_iter(text) {
                    if let Some(name) = cap.name("test").map(|m| m.as_str().trim().to_string()).filter(|n| !n.is_empty() && !seen.contains(n)) {
                        seen.push(name);
                    }
                }
                seen
            } else {
                Vec::new()
            };
            let label = if contradicts(row.outcome, code) { "unclassified" } else { row.outcome };
            return Reading { row: Some(i), label, evidence: line_at(text, at).to_string(), failing_tests };
        }
    }
    Reading { row: None, label: exit_default(code), evidence: String::new(), failing_tests: Vec::new() }
}

/// The shell's line for a command it could not run, and the word it could not run.
fn missing_command<'a>(s: &Streams<'a>) -> Option<(&'a str, String)> {
    static RX: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    let rx = RX
        .get_or_init(|| {
            compile(r"(?m)^(?:[^:\n]*: )?(?:line )?(?:\d+: )?(?P<word>[^:\n]+): (?:command not found|not found|No such file or directory|Permission denied)$")
        })
        .as_ref()?;
    ["stderr", "stdout"].iter().find_map(|name| {
        let text = s.get(name);
        let caps = rx.captures(text)?;
        let line = line_at(text, caps.get(0)?.start());
        let word = caps.name("word")?.as_str().trim_matches(|c| matches!(c, '‘' | '’' | '\'' | '"' | '`')).to_string();
        Some((line, word))
    })
}

/// The last line of stderr, else of stdout, that is not blank.
fn last_line(s: &Streams) -> String {
    [s.stderr, s.stdout].iter().find_map(|t| t.lines().rev().find(|l| !l.trim().is_empty())).unwrap_or("").to_string()
}

/// Strip the run copy's prefix (`…/familiar-ai-run-<pid>-<n>/`, plan §6.2), a `file://`
/// scheme and a leading `./`.
pub(crate) fn copy_relative(path: &str) -> String {
    let p = path.strip_prefix("file://").unwrap_or(path);
    // a run's copy, verify's copy and the planning run's copy are named alike (`<kind>-<pid>-<n>`)
    let at = ["familiar-ai-run-", "familiar-ai-verify-", "familiar-ai-at-planning-"].iter().find_map(|k| p.find(k).map(|i| (i, k.len())));
    let p = match at {
        Some((i, len)) => {
            let rest = &p[i + len..];
            let mut parts = rest.splitn(2, '/');
            let id = parts.next().unwrap_or("");
            let well_formed = id.split_once('-').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()));
            match (well_formed, parts.next()) {
                (true, Some(after)) => after,
                _ => p,
            }
        }
        None => p,
    };
    p.strip_prefix("./").unwrap_or(p).to_string()
}

/// Where `t`'s location rows say the output points, for a run read as `label`: the first of
/// [`locations`], with the name it gives.
pub fn locate(t: &'static Table, label: &str, s: &Streams) -> (Option<familiar_ai_facts::Location>, Option<String>) {
    let (all, name) = locations(t, label, s);
    (all.into_iter().next(), name)
}

/// The stored kind of a location row's kind: a `frame` is an exception's (lane H3).
pub fn kind_of(row_kind: &str) -> Option<familiar_ai_facts::LocationKind> {
    use familiar_ai_facts::LocationKind;
    match row_kind {
        "assertion" => Some(LocationKind::Assertion),
        "exception" | "frame" => Some(LocationKind::Exception),
        "compile" => Some(LocationKind::Compile),
        "signal" => Some(LocationKind::Signal),
        _ => None,
    }
}

/// Add `at` to `out` unless its file and line are there already. Location rule (2) of lane H3: for
/// a failed test, one file:line that an assertion row and another row both give is an assertion
/// (node-test's exception and assertion rows read the same block).
fn keep_place(out: &mut Vec<familiar_ai_facts::Location>, at: familiar_ai_facts::Location, label: &str) {
    use familiar_ai_facts::LocationKind::Assertion;
    match out.iter_mut().find(|l| l.file_path == at.file_path && l.line == at.line) {
        Some(l) if label == "tests_failed" && at.kind == Some(Assertion) => l.kind = Some(Assertion),
        Some(_) => {}
        None => out.push(at),
    }
}

/// Every place `t`'s location rows say the output points, for a run read as `label`, most wanted
/// first: a compile row for a missing name or a compile error; an exception, assertion or frame
/// row for a failed test or a crash; a signal row for a crash only. The rows in table order, each
/// row's matches in text order (a row marked first) or from the last. A match with no file or
/// line is no place. A JVM frame's class names its package, which becomes the file's folders
/// (`calc.Calc` and `Calc.java` give `calc/Calc.java`). Each place carries its row's kind
/// ([`kind_of`]). The name is the first matching row's, kept for a missing name only. For a failed
/// test, when the first exception or frame row that matches gives no file or line, the output has
/// no place at all (location rule (3) of lane H3, the tables' own contract: cargo, ctest, catch2,
/// googletest, nunit).
pub fn locations(t: &'static Table, label: &str, s: &Streams) -> (Vec<familiar_ai_facts::Location>, Option<String>) {
    let (all, name, blind) = located(t, label, s);
    (if blind { Vec::new() } else { all }, name)
}

/// [`locations`] before rule (3) is applied, and whether it applies.
fn located(t: &'static Table, label: &str, s: &Streams) -> (Vec<familiar_ai_facts::Location>, Option<String>, bool) {
    let kinds: &[&str] = match label {
        "missing_target_symbol" | "collection_error" => &["compile"],
        "tests_failed" => &["exception", "assertion", "frame"],
        "crashed" => &["exception", "assertion", "frame", "signal"],
        _ => return (Vec::new(), None, false),
    };
    let locs = compiled(t).map_or(&[][..], |c| &c.locs[..]);
    let mut out: Vec<familiar_ai_facts::Location> = Vec::new();
    let mut name: Option<Option<String>> = None;
    let mut exception_seen = false;
    let mut blind = false;
    for (loc, rx) in t.locs.iter().zip(locs) {
        let Some(rx) = rx else { continue };
        if !kinds.contains(&loc.kind) {
            continue;
        }
        let mut hits: Vec<regex::Captures> = order(loc.stream).iter().flat_map(|n| rx.captures_iter(s.get(n))).collect();
        if !loc.first {
            hits.reverse();
        }
        let Some(top) = hits.first() else { continue };
        if name.is_none() {
            // a name the output qualifies with its module (`module 'os' has no attribute 'mul'`)
            // keeps the module, so the place rule can tell a chance match (H-1)
            let module = top.name("module").map(|m| m.as_str()).filter(|m| !m.is_empty());
            name = Some(
                top.name("name")
                    .map(|m| module.map_or_else(|| m.as_str().to_string(), |module| format!("{module}.{}", m.as_str())))
                    .filter(|n| !n.is_empty() && label == "missing_target_symbol"),
            );
        }
        let place = |caps: &regex::Captures| -> Option<familiar_ai_facts::Location> {
            let file = caps.name("file").map(|m| m.as_str()).filter(|f| !f.is_empty());
            let line = caps.name("line").and_then(|m| m.as_str().parse::<u64>().ok());
            let (f, l) = file.zip(line)?;
            let mut path = copy_relative(f);
            if let Some((pkg, _)) = caps.name("class").and_then(|c| c.as_str().rsplit_once('.')) {
                if !path.contains('/') {
                    path = format!("{}/{path}", pkg.replace('.', "/"));
                }
            }
            Some(familiar_ai_facts::Location { file_path: path, line: l, function: None, kind: kind_of(loc.kind) })
        };
        if label == "tests_failed" && matches!(loc.kind, "exception" | "frame") && !exception_seen {
            exception_seen = true;
            blind = place(top).is_none();
        }
        for caps in &hits {
            if let Some(at) = place(caps) {
                keep_place(&mut out, at, label);
            }
        }
    }
    (out, name.flatten(), blind)
}

/// The source folders a JVM path (`calc/Calc.java`, from a frame's package) is looked for
/// under, after the path itself: Maven's and Gradle's layout.
const JVM_ROOTS: &[&str] = &["src/main/java/", "src/test/java/", "src/main/kotlin/", "src/test/kotlin/", "src/main/groovy/", "src/test/groovy/", "src/main/scala/", "src/test/scala/"];

/// The first of `candidates` inside the project (plan §6.2): a relative path, never absolute,
/// above the root or under a `.venv` or `node_modules` folder, whose file `read` finds (git's blob at the run's commit, so `.venv` and
/// `node_modules`, linked into the run's copy, are never in it) and whose line is one of that
/// file's (a transformed module's line is not the source's), with the function git's hunk
/// header names above its line (`funcname_above`). A JVM path is also looked for under the JVM
/// source folders.
pub fn in_project(candidates: &[familiar_ai_facts::Location], read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<familiar_ai_facts::Location> {
    candidates.iter().find_map(|loc| {
        let p = loc.file_path.as_str();
        if p.is_empty() || p.starts_with('/') || p.split('/').any(|s| matches!(s, ".." | ".venv" | "node_modules")) {
            return None;
        }
        let jvm = [".java", ".kt", ".groovy", ".scala"].iter().any(|e| p.ends_with(e));
        let roots: &[&str] = if jvm { JVM_ROOTS } else { &[] };
        std::iter::once(p.to_string()).chain(roots.iter().map(|r| format!("{r}{p}"))).find_map(|path| {
            let bytes = read(&path)?;
            if loc.line == 0 || loc.line > familiar_ai_git::line_count(&bytes) {
                return None;
            }
            let function = function_at(&path, &bytes, loc.line).map(|(_, f, _)| f);
            Some(familiar_ai_facts::Location { file_path: path, line: loc.line, function, kind: loc.kind })
        })
    })
}

/// The function `line` sits in, as git's hunk header reads it: the header's line number, its
/// text, and the last line of its extent, which runs to the line before the next header (or the
/// file's end), blank lines at its end left out. `None` when no line above qualifies.
pub fn function_extent(bytes: &[u8], line: u64) -> Option<(u64, String, u64)> {
    let (start, header) = familiar_ai_git::funcname_above(bytes, line.saturating_add(1))?;
    let lines = familiar_ai_git::split_lines(bytes);
    let header_like = |l: &&[u8]| l.first().is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_' || *c == b'$');
    let from = usize::try_from(start).ok()?;
    let next = lines.iter().skip(from).position(header_like).map_or(lines.len(), |i| from + i);
    let mut end = next;
    while end > from && lines[end - 1].iter().all(u8::is_ascii_whitespace) {
        end -= 1;
    }
    Some((start, header, (end as u64).max(start)))
}

/// The languages whose methods sit indented inside a class (Java, C#, PHP): git's header above a
/// line in a method is the class's, so the method around the line is read instead ([`method_around`]).
const BRACED_METHODS: &[&str] = &[".java", ".cs", ".php"];

/// The function `line` of `path` sits in, as [`function_extent`] gives it: for Java, C# and PHP
/// the innermost method whose braces enclose the line ([`method_around`]), else (and when no
/// method encloses it) git's hunk header.
pub fn function_at(path: &str, bytes: &[u8], line: u64) -> Option<(u64, String, u64)> {
    let braced = BRACED_METHODS.iter().any(|e| path.ends_with(e));
    braced.then(|| method_around(bytes, line)).flatten().or_else(|| function_extent(bytes, line))
}

/// Words that open a statement or a type, never a method's header.
const NOT_A_METHOD: &[&str] = &[
    "if", "else", "for", "foreach", "while", "do", "switch", "case", "catch", "try", "using", "lock", "fixed", "return", "new", "throw", "await", "yield", "echo", "print", "assert", "synchronized", "class", "interface", "enum", "record", "struct", "trait", "namespace", "package", "import", "use",
];

/// The innermost method header at or above `line` (1-based) whose body encloses it: a line whose
/// text before its first `(` is only modifiers, a type and the method's name (no statement
/// word, no `.`, `=` or `->`), its body running from it to the brace that closes its first `{`,
/// or to its `;` when it is an expression body (`=>`). Its line, its trimmed text cut to 80
/// bytes, and its last line; `None` when no method encloses the line.
pub fn method_around(bytes: &[u8], line: u64) -> Option<(u64, String, u64)> {
    let lines = familiar_ai_git::split_lines(bytes);
    let at = usize::try_from(line).ok()?.checked_sub(1).filter(|i| *i < lines.len())?;
    (0..=at).rev().find_map(|i| {
        let text = String::from_utf8_lossy(lines[i]);
        let t = text.trim();
        let (before, _) = t.split_once('(')?;
        let name_ok = before.trim_end().ends_with(|c: char| c.is_alphanumeric() || c == '_');
        let plain = before.chars().all(|c| c.is_alphanumeric() || " \t_$<>[],?@:".contains(c));
        let words = before.split(|c: char| !(c.is_alphanumeric() || c == '_'));
        if !name_ok || !plain || words.clone().any(|w| NOT_A_METHOD.contains(&w)) {
            return None;
        }
        let end = body_end(&lines, i)?;
        (end >= at).then(|| {
            let mut cut = t.len().min(80);
            while !t.is_char_boundary(cut) {
                cut -= 1;
            }
            (i as u64 + 1, t[..cut].trim_end().to_string(), end as u64 + 1)
        })
    })
}

/// The 0-based last line of the body a header at `from` opens: where the brace its first `{`
/// opened closes (the file's end when it never does), or its `;` when no `{` came first and an
/// `=>` did; `None` for a statement (a `;` with no `=>`).
fn body_end(lines: &[&[u8]], from: usize) -> Option<usize> {
    let (mut depth, mut opened, mut arrow) = (0i64, false, false);
    for (j, l) in lines.iter().enumerate().skip(from) {
        arrow |= l.windows(2).any(|w| w == b"=>");
        for c in l.iter() {
            match c {
                b'{' => {
                    depth += 1;
                    opened = true;
                }
                b'}' => depth -= 1,
                b';' if !opened => return arrow.then_some(j),
                _ => {}
            }
            if opened && depth <= 0 {
                return Some(j);
            }
        }
    }
    opened.then(|| lines.len().saturating_sub(1))
}

/// A `place_chosen` candidate from a crash's or an exception's translation (plan §7.3, line 4):
/// its location's file, the function git's hunk header names there as the symbol, and that
/// function's lines ([`function_extent`]), read from `bytes`, the file at the run's commit.
/// `None` for any other label (a missing name or a compile error points at the test, not at the
/// code that crashed) or when no function is above the location.
pub fn place_candidate(t: &familiar_ai_facts::TestLogTranslation, bytes: &[u8]) -> Option<familiar_ai_facts::Place> {
    if !matches!(t.label.as_str(), "tests_failed" | "crashed") {
        return None;
    }
    let loc = t.location.as_ref()?;
    let (line_start, symbol, line_end) = function_at(&loc.file_path, bytes, loc.line)?;
    Some(familiar_ai_facts::Place { file_path: loc.file_path.clone(), symbol, line_start, line_end })
}

/// Classify a run (plan §2, §3): `end` with its cleaned output, read by the tables `runner`
/// selects ([`tables_for`]). `None` for an end that says nothing about the tests (cancelled, not
/// started).
pub fn classify(end: familiar_ai_run::End, s: &Streams, runner: Option<&familiar_ai_run::RunnerDetails>) -> Option<Translation> {
    classify_with(&tables_for(runner), runner.is_none(), end, s)
}

/// [`classify`] with the tables given. One table decides alone. Several (a family's, or every
/// table when the command names no runner) agree or the run is `unclassified`: each table whose
/// row matched gives its label, and one label among them wins. In a family a guard row that
/// matches any text gives no label, the table named as the row (ctest) decides when it reads the
/// run, and `tests_failed` yields to a sibling's other label. When no row of any table matched,
/// a family's run takes the exit default and an unknown runner's is `unclassified`.
pub fn classify_with(tables: &[&'static Table], unknown: bool, end: familiar_ai_run::End, s: &Streams) -> Option<Translation> {
    use familiar_ai_run::End;
    let binary = |label: &str, evidence: String| Translation { by: "binary", label: label.to_string(), evidence, failing_tests: Vec::new(), location: None, missing_name: None, candidates: Vec::new(), counts: familiar_ai_facts::Counts::default() };
    let code: i64 = match end {
        End::Exited(c) => i64::from(c),
        End::Signalled(n) => -i64::from(n),
        // the timeout kills the group; the rows are read for the evidence only
        End::TimedOut => -9,
        End::EmptyCommand | End::NoDirectory => return Some(binary("usage_error", String::new())),
        End::NotStarted | End::Cancelled => return None,
    };
    let readings: Vec<(&'static Table, Reading)> = tables.iter().map(|t| (*t, read_table(t, code, s))).collect();
    let mut voted: Vec<&(&'static Table, Reading)> = readings.iter().filter(|(_, r)| r.row.is_some()).collect();
    if !unknown && readings.len() > 1 {
        // A family's tables read one runner's output. A row whose pattern matches any text is its
        // table's guard against output it does not know, not a reading of it. The table keyed by
        // the row's own name (ctest, which runs the binaries) reads first; the framework tables
        // under it (or dotnet's three) agree, where `tests_failed`, the reading every sibling
        // gives to the runner's shared failure block, yields to a sibling's other reading.
        voted.retain(|(t, r)| !r.row.is_some_and(|i| matches_anything(t, i)));
        if let Some(own) = voted.iter().find(|(t, _)| t.runner == t.row).copied() {
            voted = vec![own];
        } else if voted.iter().any(|(_, r)| r.label != "tests_failed") {
            voted.retain(|(_, r)| r.label != "tests_failed");
        }
    }
    let (table, mut reading) = match (readings.len(), voted.first()) {
        (1, _) => readings[0].clone(),
        (_, Some(first)) if voted.iter().all(|(_, r)| r.label == first.1.label) => (*first).clone(),
        (_, Some(first)) => (first.0, Reading { label: "unclassified", failing_tests: Vec::new(), ..first.1.clone() }),
        (_, None) if unknown || readings.is_empty() => (&TABLES[0], Reading { row: None, label: "unclassified", evidence: String::new(), failing_tests: Vec::new() }),
        (_, None) => (readings[0].0, Reading { row: None, label: exit_default(code), evidence: String::new(), failing_tests: Vec::new() }),
    };
    // H-2 (lane H5): a shell script's run that no row read is read from the shell's own trace
    if let ([only], None, "unclassified") = (tables, reading.row, reading.label) {
        if only.runner == "sh" {
            if let Some(r) = crate::door::sh_trace::read(code, s) {
                let location = Some(r.location.clone());
                return Some(Translation { by: "regex", label: "tests_failed".to_string(), evidence: r.evidence, failing_tests: Vec::new(), location, missing_name: None, candidates: vec![r.location], counts: familiar_ai_facts::Counts::default() });
            }
        }
    }
    // Exit 126/127: a bare command is the environment, a local path the command (the owner, 2026-10-14).
    // The shell's line decides whatever the command names: a command naming no runner, or tables
    // that disagree, read it too (plan §2: the shell's line is the evidence either way).
    if matches!(code, 126 | 127) && matches!(reading.label, "environment_error" | "usage_error" | "unclassified") {
        if let Some((line, word)) = missing_command(s) {
            reading.label = if word.contains('/') { "usage_error" } else { "environment_error" };
            reading.evidence = line.to_string();
        }
    }
    if reading.evidence.is_empty() {
        reading.evidence = last_line(s);
    }
    // the deciding table's counts, else the first of its family's that counts anything (D7)
    let counts = std::iter::once(table).chain(tables.iter().copied()).find_map(|t| count(t, s)).unwrap_or_default();
    if end == End::TimedOut {
        return Some(Translation { failing_tests: reading.failing_tests, counts, ..binary("timed_out", reading.evidence) });
    }
    // the deciding table's places first, then its family's; the name from the first that gives one
    let mut candidates: Vec<familiar_ai_facts::Location> = Vec::new();
    let mut missing_name = None;
    let mut blind = false;
    for t in std::iter::once(table).chain(tables.iter().copied()) {
        let (all, name, no_place) = located(t, reading.label, s);
        missing_name = missing_name.or(name);
        blind |= no_place;
        for l in all {
            keep_place(&mut candidates, l, reading.label);
        }
    }
    if blind {
        // rule (3): an exception the output gives no place for leaves the run with none
        candidates.clear();
    }
    let location = candidates.first().cloned();
    Some(Translation { by: "regex", label: reading.label.to_string(), evidence: reading.evidence, failing_tests: reading.failing_tests, location, missing_name, candidates, counts })
}

/// A run's `test_log_translation` from its classification `t` (plan §6.2): written under the
/// run's, the criterion's and the commit's ids, `classified_on` `on`. `locate` says where one
/// candidate is inside the project (the binary's: a file git tracks at the commit, with the
/// function above its line), `None` when it is not. The location rules of lane H3:
///
/// 1. the location is the first candidate inside the project, with its kind;
/// 2. (one file:line from an assertion row and another is an assertion: [`locations`]);
/// 3. (an exception with no file or line leaves no candidates: [`locations`]);
/// 4. for a failed test, candidates inside the project of different kinds at different places
///    leave no location: the output does not say which one failed.
///
/// `counts` is written on every translation (`{}` when the output counted nothing, D7), except one
/// `classified_on` `cut` (the backstop's), which writes none.
pub fn test_log_translation(
    t: Translation,
    run_id: &str,
    criterion_id: &str,
    commit: &str,
    on: &str,
    locate: impl Fn(&[familiar_ai_facts::Location]) -> Option<familiar_ai_facts::Location>,
) -> familiar_ai_facts::TestLogTranslation {
    let one = |c: &familiar_ai_facts::Location| locate(std::slice::from_ref(c));
    let mut location = None;
    for (i, c) in t.candidates.iter().enumerate() {
        let Some(first) = one(c) else { continue };
        let mixed = t.label == "tests_failed"
            && t.candidates[i + 1..].iter().filter(|o| o.kind != c.kind).filter_map(one).any(|o| o.kind != first.kind && (o.file_path != first.file_path || o.line != first.line));
        location = (!mixed).then_some(first);
        break;
    }
    familiar_ai_facts::TestLogTranslation {
        run_id: run_id.to_string(),
        criterion_id: criterion_id.to_string(),
        commit: commit.to_string(),
        by: t.by.to_string(),
        label: t.label,
        evidence: t.evidence,
        failing_tests: t.failing_tests,
        classified_on: on.to_string(),
        location,
        missing_name: t.missing_name,
        counts: (on != "cut").then_some(t.counts),
        ran_tests: None,
        skipped_tests: None,
        versions: None,
        frames: None,
    }
}

pub fn tail(s: &str, n: usize) -> &str {
    let count = s.chars().count();
    match count.checked_sub(n).and_then(|skip| s.char_indices().nth(skip)) {
        Some((i, _)) => &s[i..],
        None => s,
    }
}

/// Whether git ignores `.familiar/<name>` of the project at `root`.
pub(crate) fn ignored(root: &Path, name: &str) -> bool {
    let Ok(Some(worktree)) = familiar_ai_git::worktree_root(root) else {
        return false;
    };
    let Ok((record_rel, _)) = crate::door::setup::paths_in_repository(root, &worktree) else {
        return false;
    };
    let folder = record_rel.strip_suffix(RECORD_FILE).unwrap_or_default();
    familiar_ai_git::is_ignored(&worktree, &format!("{folder}{name}")).unwrap_or(false)
}

/// The kept run's file for `commit`, which must be hex so it cannot escape the folder.
fn run_path(root: &Path, commit: &str) -> Option<std::path::PathBuf> {
    let hex = !commit.is_empty() && commit.chars().all(|c| c.is_ascii_hexdigit());
    hex.then(|| root.join(FAMILIAR_FOLDER).join(RUNS_FOLDER).join(format!("{commit}.log")))
}

/// Keep the end of the test command's run at `commit`: stdout, then stderr.
pub fn keep_run(root: &Path, commit: &str, stdout: &str, stderr: &str) {
    let Some(path) = run_path(root, commit) else {
        return;
    };
    if !ignored(root, RUNS_FOLDER) {
        return;
    }
    let mut both = stdout.to_string();
    if !both.is_empty() && !both.ends_with('\n') && !stderr.is_empty() {
        both.push('\n');
    }
    both.push_str(stderr);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, tail(&both, KEPT_OUTPUT)));
    }
}

/// The kept end of the run at `commit`, when one was kept.
pub fn kept_run(root: &Path, commit: &str) -> Option<String> {
    std::fs::read_to_string(run_path(root, commit)?).ok()
}

/// Copy session `id`'s transcript from `<claude_dir>/projects/*/` to
/// `.familiar/investigator/` and return where it is, or why it is not there.
pub fn keep_transcript(root: &Path, claude_dir: Option<&Path>, id: &str) -> String {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return format!("transcript not looked for: {id:?} is not a session id");
    }
    let Some(dir) = claude_dir else {
        return "transcript not looked for: no home folder is known (CLAUDE_CONFIG_DIR or HOME)".to_string();
    };
    let projects = dir.join("projects");
    let name = format!("{id}.jsonl");
    let found = std::fs::read_dir(&projects)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join(&name))
        .find(|p| p.is_file());
    let Some(from) = found else {
        return format!("transcript not found: no {}/*/{name}", projects.display());
    };
    let kept = format!("{FAMILIAR_FOLDER}/{TRANSCRIPTS_FOLDER}");
    if !ignored(root, TRANSCRIPTS_FOLDER) {
        return format!("transcript not copied: {kept} is not ignored by git (familiar-ai setup adds the line); it is at {}", from.display());
    }
    let to = root.join(FAMILIAR_FOLDER).join(TRANSCRIPTS_FOLDER);
    match std::fs::create_dir_all(&to).and_then(|()| std::fs::copy(&from, to.join(&name))) {
        Ok(_) => format!("transcript {kept}/{name}"),
        Err(e) => format!("transcript not copied from {}: {e}", from.display()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_verify_copy_and_a_planning_copy_are_stripped_as_a_run_copy_is() {
        assert_eq!(copy_relative("/tmp/familiar-ai-run-12-3/tests/test_add.py"), "tests/test_add.py");
        assert_eq!(copy_relative("/tmp/familiar-ai-verify-12-3/tests/test_add.py"), "tests/test_add.py");
        assert_eq!(copy_relative("/var/tmp/familiar-ai-at-planning-12-3/src/calc.py"), "src/calc.py");
        assert_eq!(copy_relative("/home/x/familiar-ai-verify-notanid/a.py"), "/home/x/familiar-ai-verify-notanid/a.py");
    }

    use super::*;
    use std::process::Command;

    fn repo(name: &str, ignore: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("familiar-ai-kept-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join(FAMILIAR_FOLDER));
        let _ = Command::new("git").args(["init", "-q"]).current_dir(&dir).status();
        let _ = std::fs::write(dir.join(".gitignore"), ignore);
        dir
    }

    #[test]
    fn a_run_is_kept_by_its_commit_its_end_only_and_only_where_git_ignores_it() {
        assert_eq!(tail("héllo", 3), "llo");
        assert_eq!(tail("hi", 3), "hi");
        let dir = repo("runs", ".familiar/runs\n");
        let long = format!("{}END", "x".repeat(5_000));
        keep_run(&dir, "abc123", &long, "boom\n");
        let kept = kept_run(&dir, "abc123").unwrap_or_default();
        assert_eq!(kept.chars().count(), KEPT_OUTPUT);
        assert!(kept.ends_with("xEND\nboom\n"), "stdout, then stderr: {kept}");
        keep_run(&dir, "../escape", "x", "");
        assert_eq!(kept_run(&dir, "../escape"), None, "a commit id is hex only");
        assert_eq!(kept_run(&dir, "def456"), None);
        let open = repo("runs-open", "");
        keep_run(&open, "abc123", "out", "");
        assert_eq!(kept_run(&open, "abc123"), None, "not ignored: nothing kept");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&open);
    }

    #[test]
    fn clean_strips_escapes_and_makes_every_line_end_a_newline() {
        assert_eq!(clean("\u{1b}[31mFAILED\u{1b}[0m test_x\r\n"), "FAILED test_x\n", "colour and CRLF");
        assert_eq!(clean("50%\r100%\rdone\n"), "50%\n100%\ndone\n", "a lone CR is a line end");
        assert_eq!(clean("a\u{1b}[2K\u{1b}[1Gb"), "ab", "erase line, cursor column");
        assert_eq!(clean("\u{1b}]0;title\u{7}x\u{1b}]8;;http://e\u{1b}\\link\u{1b}]8;;\u{1b}\\"), "xlink", "OSC to BEL and to ST");
        assert_eq!(clean("\u{1b}(Bplain\u{1b}=\u{1b}>"), "plain", "charset and keypad escapes");
        assert_eq!(clean("\u{9b}1mx"), "x", "the 8-bit CSI");
        assert_eq!(clean("héllo ✓\n"), "héllo ✓\n", "other text unchanged");
        assert_eq!(clean("tail\u{1b}"), "tail", "a lone escape at the end");
    }

    #[test]
    fn a_short_stream_is_not_cut() {
        let s = "line\n".repeat(1_000);
        assert_eq!(cut(&s), (s.clone(), 0));
        let s = "é".repeat(CUT_HEAD + CUT_TAIL);
        assert_eq!(cut(&s), (s.clone(), 0), "counted in characters, not bytes");
    }

    #[test]
    fn a_long_stream_keeps_its_head_and_tail_at_line_boundaries_with_a_marker() {
        // 10-character lines numbered from 0
        let lines: Vec<String> = (0..2_000).map(|i| format!("{i:09}\n")).collect();
        let s = lines.concat();
        let (kept, dropped) = cut(&s);
        let head = lines[..CUT_HEAD / 10].concat();
        let tail = lines[lines.len() - CUT_TAIL / 10..].concat();
        assert_eq!(dropped, s.chars().count() - CUT_HEAD - CUT_TAIL);
        assert_eq!(kept, format!("{head}[familiar: {dropped} chars dropped]\n{tail}"));
        // lines that do not fall on the budget: the head stops before, the tail starts after
        let lines: Vec<String> = (0..2_000).map(|i| format!("{i:010}\n")).collect();
        let s = lines.concat();
        let (kept, dropped) = cut(&s);
        let head = lines[..CUT_HEAD / 11].concat();
        let tail = lines[lines.len() - CUT_TAIL / 11..].concat();
        assert_eq!(kept, format!("{head}[familiar: {dropped} chars dropped]\n{tail}"));
        assert_eq!(dropped, s.chars().count() - head.chars().count() - tail.chars().count());
        assert!(head.chars().count() <= CUT_HEAD && tail.chars().count() <= CUT_TAIL);
    }

    #[test]
    fn a_line_longer_than_the_budget_is_cut_at_a_character() {
        let s = format!("{}\n{}", "α".repeat(5_000), "ω".repeat(9_000));
        let (kept, dropped) = cut(&s);
        assert_eq!(dropped, 14_001 - CUT_HEAD - CUT_TAIL);
        assert_eq!(kept, format!("{}\n[familiar: {dropped} chars dropped]\n{}", "α".repeat(CUT_HEAD), "ω".repeat(CUT_TAIL)));
    }

    #[test]
    fn the_transcript_is_copied_from_claude_codes_projects_folder_by_its_session_id() {
        let dir = repo("transcript", ".familiar/investigator\n");
        let home = dir.join("home/.claude");
        let id = "0b9c6a1e-4f2d-4c3b-9a8e-123456789abc";
        assert_eq!(keep_transcript(&dir, Some(&home), id), format!("transcript not found: no {}/projects/*/{id}.jsonl", home.display()));
        let _ = std::fs::create_dir_all(home.join("projects/-tmp-p"));
        let _ = std::fs::write(home.join(format!("projects/-tmp-p/{id}.jsonl")), "{\"type\":\"user\"}\n");
        assert_eq!(keep_transcript(&dir, Some(&home), id), format!("transcript .familiar/investigator/{id}.jsonl"));
        let copied = std::fs::read_to_string(dir.join(format!(".familiar/investigator/{id}.jsonl"))).unwrap_or_default();
        assert_eq!(copied, "{\"type\":\"user\"}\n");
        assert!(keep_transcript(&dir, None, id).starts_with("transcript not looked for: no home folder"));
        assert!(keep_transcript(&dir, Some(&home), "../x").starts_with("transcript not looked for: "));
        let open = repo("transcript-open", "");
        assert!(keep_transcript(&open, Some(&home), id).starts_with("transcript not copied: .familiar/investigator is not ignored by git"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&open);
    }
}
