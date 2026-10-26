//! H-2's reader (lane H5): a shell script's failing run read from the shell's own trace, which
//! Familiar's run writes to a side file read as the report stream (`familiar_ai_run::shell_trace`).
//! Two forms are read:
//!
//! - the bash kit's rows, `kind pid status file line command` (`err`, `ran`, `fail`, `exit`);
//! - xtrace lines, `+familiar-ai<TAB>file<TAB>line<TAB>command` (bash read as sh, through
//!   `BASH_XTRACEFD`; an `sh` that is not bash, under `-x`, given its file and line after the run).
//!
//! The script that decided is the last one the command itself ran (the outermost level of the
//! trace names it; the kit's last `exit` row is its shell). Its last command decides:
//!
//! - a check (`test`, `[`, `[[`, `grep -q`, `diff`, `cmp`) is an assertion at its line;
//! - `exit N` right after a check (within the two commands before it) is an assertion at the
//!   check's line;
//! - a command whose own trace ends in another script of the project (the place's script or a
//!   function sourced from it) is that script failing: an exception at its last line, for the
//!   place rule;
//! - anything else, or a trace that does not say, is not read here: can't tell.
//!
//! A missing command and a syntax error are read by the table's rows from the shell's own
//! stderr line; exit 0 is the sealed script's green (the table's `passed` row).

use familiar_ai_facts::{Location, LocationKind};

use crate::door::kept::Streams;

/// One command the trace saw run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Event {
    /// The script it is in, as the shell named it; empty at the command's own level.
    file: String,
    line: Option<u64>,
    cmd: String,
    /// The trace's line, verbatim: the reading's evidence.
    raw: String,
}

/// What the trace says decided a failing run: the deciding line of the trace, verbatim, and
/// where it points.
#[derive(Debug, Clone, PartialEq)]
pub struct ShReading {
    pub evidence: String,
    pub location: Location,
}

/// The words a check starts with.
const CHECKS: &[&str] = &["test", "[", "[[", "diff", "cmp"];

fn words(cmd: &str) -> Vec<String> {
    cmd.split_whitespace().map(|w| w.trim_matches(|c| c == '\'' || c == '"').to_string()).collect()
}

fn first_word(cmd: &str) -> String {
    let w = words(cmd);
    let w: Vec<&String> = w.iter().skip_while(|w| *w == "!").collect();
    w.first().map(|w| w.rsplit('/').next().unwrap_or(w).to_string()).unwrap_or_default()
}

/// Whether `cmd` (as the trace prints it) is a check: `test`, `[`, `[[`, `diff`, `cmp`, or a
/// quiet `grep`.
pub fn is_check(cmd: &str) -> bool {
    let first = first_word(cmd);
    if CHECKS.contains(&first.as_str()) {
        return true;
    }
    first == "grep" && words(cmd).iter().skip(1).any(|w| matches!(w.as_str(), "--quiet" | "--silent") || (w.starts_with('-') && !w.starts_with("--") && w.contains('q')))
}

fn is_exit(cmd: &str) -> bool {
    first_word(cmd) == "exit"
}

/// The xtrace lines of `text`, in order.
fn xtrace(text: &str) -> Vec<Event> {
    text.lines()
        .filter_map(|l| {
            let body = l.trim_start_matches('+');
            if body.len() == l.len() {
                return None;
            }
            let rest = body.strip_prefix(familiar_ai_run::shell_trace::MARK)?;
            let mut f = rest.splitn(3, '\t');
            let (file, line, cmd) = (f.next()?, f.next()?, f.next()?);
            Some(Event { file: file.to_string(), line: line.trim().parse().ok(), cmd: cmd.to_string(), raw: l.to_string() })
        })
        .collect()
}

/// The kit's rows of `text`: kind, pid, status, and the command.
fn kit(text: &str) -> Vec<(String, String, i64, Event)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.splitn(6, '\t');
            let kind = f.next()?;
            if !matches!(kind, "err" | "ran" | "fail" | "exit" | "nf") {
                return None;
            }
            let pid = f.next()?;
            let status: i64 = f.next()?.parse().ok()?;
            let (file, line, cmd) = (f.next()?, f.next()?, f.next()?);
            pid.parse::<u64>().ok()?;
            Some((kind.to_string(), pid.to_string(), status, Event { file: file.to_string(), line: line.trim().parse().ok(), cmd: cmd.to_string(), raw: l.to_string() }))
        })
        .collect()
}

fn at(file: &str, line: u64, kind: LocationKind) -> Location {
    Location { file_path: crate::door::kept::copy_relative(file), line, function: None, kind: Some(kind) }
}

/// The reading at `e`, of `kind`; none for a script outside the project (an absolute path, as a
/// tool written in bash prints its own, or one above the project): the trace says nothing there
/// about the planned test, and a red with no place must not seal.
fn read_of(e: &Event, kind: LocationKind) -> Option<ShReading> {
    let line = e.line.filter(|l| *l > 0)?;
    let location = at(&e.file, line, kind);
    let p = location.file_path.as_str();
    if p.is_empty() || p.starts_with('/') || p.split('/').any(|s| s == "..") {
        return None;
    }
    Some(ShReading { evidence: e.raw.clone(), location })
}

/// The decision over the deciding script's commands `own`, in order; `after` is every command
/// the trace saw after the script's last line began, in order, for the place rule.
fn decide(own: &[Event], after: &[Event]) -> Option<ShReading> {
    let (last, before) = own.split_last()?;
    if is_exit(&last.cmd) {
        return before.iter().rev().take(2).find(|e| is_check(&e.cmd)).and_then(|e| read_of(e, LocationKind::Assertion));
    }
    let group: Vec<&Event> = own.iter().rev().take_while(|e| e.line == last.line).collect();
    if let Some(check) = group.iter().find(|e| is_check(&e.cmd)) {
        return read_of(check, LocationKind::Assertion);
    }
    // the place rule: the last command run is in another script of the project
    let other = after.iter().rev().find(|e| !e.file.is_empty())?;
    if other.file == last.file {
        return None;
    }
    read_of(other, LocationKind::Exception)
}

/// H-2: what the trace in `s.report` says decided a run that ended with `code` (not 0, not a
/// signal), or `None`: can't tell.
pub fn read(code: i64, s: &Streams) -> Option<ShReading> {
    if code <= 0 {
        return None;
    }
    let rows = kit(s.report);
    if let Some((_, pid, status, last)) = rows.iter().rev().find(|(k, ..)| k == "exit") {
        // the outermost bash shell exits last; its status must be the run's
        if *status != code {
            return xtrace_read(s);
        }
        let mut own: Vec<Event> = rows.iter().filter(|(k, p, ..)| k == "ran" && p == pid).map(|(.., e)| e.clone()).collect();
        own.push(last.clone());
        // the kit's shell may be a tool the script ran (bats is bash): the script's own trace next
        return decide(&own, &[]).or_else(|| xtrace_read(s));
    }
    xtrace_read(s)
}

fn xtrace_read(s: &Streams) -> Option<ShReading> {
    let all = xtrace(s.report);
    let inner: Vec<String> = all.iter().filter(|e| !e.file.is_empty()).map(|e| e.file.clone()).collect();
    let outer: Vec<&Event> = all.iter().filter(|e| e.file.is_empty()).collect();
    let script = if outer.is_empty() {
        all.last()?.file.clone()
    } else {
        // the command's own last command, or the one before its `exit`, names the script
        let mut it = outer.iter().rev();
        let mut top = it.next()?;
        if is_exit(&top.cmd) {
            top = it.next()?;
        }
        words(&top.cmd).into_iter().find(|w| inner.contains(w))?
    };
    let own: Vec<Event> = all.iter().filter(|e| e.file == script).cloned().collect();
    let last = own.last()?;
    let first_of_group = all.iter().rposition(|e| e.file == script && e.line != last.line).map_or(0, |i| i + 1);
    decide(&own, &all[first_of_group..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_x(trace: &str) -> Option<(String, u64, Option<LocationKind>)> {
        read(1, &Streams { report: trace, ..Default::default() }).map(|r| (r.location.file_path, r.location.line, r.location.kind))
    }

    #[test]
    fn a_check_at_the_scripts_last_line_is_an_assertion_there() {
        let t = "+familiar-ai\t\t2\tsh tests/t.sh\n+familiar-ai\ttests/t.sh\t1\tgrep -qx hello\n+familiar-ai\ttests/t.sh\t1\tsh src/app.sh\n+familiar-ai\tsrc/app.sh\t3\tgreet\n+familiar-ai\tsrc/app.sh\t2\techo hi\n";
        assert_eq!(read_x(t), Some(("tests/t.sh".into(), 1, Some(LocationKind::Assertion))));
    }

    #[test]
    fn exit_after_a_check_is_the_checks_assertion_and_after_anything_else_cant_tell() {
        let t = "+familiar-ai\t\t1\tsh tests/t.sh\n+familiar-ai\ttests/t.sh\t2\tgrep -q x f\n+familiar-ai\ttests/t.sh\t2\techo no\n+familiar-ai\ttests/t.sh\t2\texit 1\n";
        assert_eq!(read_x(t), Some(("tests/t.sh".into(), 2, Some(LocationKind::Assertion))));
        let t = "+familiar-ai\t\t1\tsh tests/t.sh\n+familiar-ai\ttests/t.sh\t1\tsleep 1\n+familiar-ai\ttests/t.sh\t2\texit 1\n";
        assert_eq!(read_x(t), None);
    }

    #[test]
    fn the_places_script_failing_is_an_exception_there() {
        let t = "+familiar-ai\t\t1\tsh tests/t.sh\n+familiar-ai\ttests/t.sh\t1\tsh src/app.sh\n+familiar-ai\tsrc/app.sh\t4\tfalse\n";
        assert_eq!(read_x(t), Some(("src/app.sh".into(), 4, Some(LocationKind::Exception))));
    }

    #[test]
    fn a_script_that_passed_before_another_command_failed_says_nothing() {
        let t = "+familiar-ai\t\t1\tsh tests/t.sh\n+familiar-ai\ttests/t.sh\t1\tgrep -q x f\n+familiar-ai\t\t1\tfalse\n";
        assert_eq!(read_x(t), None);
    }

    #[test]
    fn the_kits_rows_read_the_outermost_shell() {
        let t = "ran\t9\t0\ttests/t.sh\t2\techo a\nran\t9\t1\ttests/t.sh\t3\tgrep -q x f\nexit\t9\t1\ttests/t.sh\t4\texit 1\n";
        assert_eq!(read_x(t), Some(("tests/t.sh".into(), 3, Some(LocationKind::Assertion))));
        assert_eq!(read(2, &Streams { report: t, ..Default::default() }), None, "a status that is not the run's: can't tell");
    }

    #[test]
    fn a_script_outside_the_project_says_nothing() {
        let t = "ran\t9\t1\t/usr/libexec/bats-core/bats-exec-suite\t70\t[[ $f -gt 0 ]]\nexit\t9\t1\t/usr/libexec/bats-core/bats-exec-suite\t71\texit 1\n";
        assert_eq!(read_x(t), None);
        let t = "+familiar-ai\t\t1\tsh ../t.sh\n+familiar-ai\t../t.sh\t1\tgrep -q x f\n";
        assert_eq!(read_x(t), None);
    }

    #[test]
    fn checks() {
        for c in ["[ 2 = 3 ]", "'[' hi = hello ']'", "test -f x", "[[ a == b ]]", "grep -qx hello", "grep -x -q hello", "grep --quiet a f", "diff a b", "cmp a b", "! grep -q a f"] {
            assert!(is_check(c), "{c}");
        }
        for c in ["grep hello f", "sh src/app.sh", "echo [", "exit 1", "grep --count a f"] {
            assert!(!is_check(c), "{c}");
        }
    }
}
