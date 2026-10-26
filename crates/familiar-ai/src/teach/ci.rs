//! The CI line: what hosted CI said at the local HEAD, read only from the
//! record's `ci_report_seen` facts, which `familiar-ai ci fetch` writes. It
//! never touches the network.

use std::path::Path;

use familiar_ai_facts::{Fact, Log};
use serde_json::Value;

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn seven(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The CI line for the commit `head`; stale when only another commit has a
/// report, and `could_not` reads as not proven.
pub fn line(log: &Log, head: &str) -> String {
    let seen: Vec<&Fact> = log.iter().filter(|f| f.kind == "ci_report_seen").collect();
    let h7 = seven(head);
    let Some(here) = seen.iter().rev().find(|f| text(f, "sha") == head) else {
        return match seen.last() {
            Some(latest) => format!("CI: stale (ran at {})", seven(text(latest, "sha"))),
            None => format!("CI: no run for {h7}"),
        };
    };
    let id = text(here, "run_id");
    match text(here, "outcome") {
        "proven" => format!("CI: proven at {h7}, run {id}"),
        "cancelled" => format!("CI: cancelled (run {id})"),
        "timed_out" => format!("CI: timed out (run {id})"),
        _ => {
            let first = here.fields.get("report").and_then(Value::as_array).and_then(|r| r.first()).and_then(Value::as_str);
            match first {
                Some(l) => format!("CI: not proven at {h7}, run {id}: {l}"),
                None => format!("CI: not proven at {h7}, run {id}"),
            }
        }
    }
}

/// The CI line for the project's HEAD; `None` when the repository has no
/// commit yet.
pub fn line_here(log: &Log, root: &Path) -> Option<String> {
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten()?;
    let head = familiar_ai_git::head_commit(&worktree).ok().flatten()?;
    Some(line(log, &head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::CiReportSeen;

    /// When every report in these tests finished; the line never shows it.
    const FINISHED_AT: &str = "2026-10-10T12:00:00Z";

    fn seen(log: &Log, sha: &str, id: &str, outcome: &str, report: &[&str]) -> Log {
        let f = CiReportSeen { sha: sha.into(), run_id: id.into(), finished_at: FINISHED_AT.into(), outcome: outcome.into(), report: report.iter().map(|s| s.to_string()).collect() };
        familiar_ai_facts::append(log, &f, 1.0, "t").expect("append")
    }

    #[test]
    fn the_lines_are_the_pinned_words() {
        let h = "abcdef0123456789abcdef0123456789abcdef01";
        assert_eq!(line(&Vec::new(), h), "CI: no run for abcdef0");
        let log = seen(&Vec::new(), "1234567fff", "9", "proven", &[]);
        assert_eq!(line(&log, h), "CI: stale (ran at 1234567)");
        assert_eq!(line(&seen(&log, h, "10", "proven", &["x"]), h), "CI: proven at abcdef0, run 10");
        assert_eq!(line(&seen(&log, h, "11", "not_proven", &["first", "second"]), h), "CI: not proven at abcdef0, run 11: first");
        assert_eq!(line(&seen(&log, h, "12", "could_not", &[]), h), "CI: not proven at abcdef0, run 12");
        assert_eq!(line(&seen(&log, h, "13", "cancelled", &[]), h), "CI: cancelled (run 13)");
        assert_eq!(line(&seen(&log, h, "14", "timed_out", &[]), h), "CI: timed out (run 14)");
        let both = seen(&seen(&log, h, "15", "not_proven", &[]), h, "16", "proven", &[]);
        assert_eq!(line(&both, h), "CI: proven at abcdef0, run 16", "the latest for HEAD");
    }
}
