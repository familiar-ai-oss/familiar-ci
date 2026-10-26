//! Check the place an agent named in `place_chosen`; never find one. The
//! planning check reads the committed lines at the sha; the start check reads
//! the working tree, following a pure shift of the lines. Each check writes one
//! `anchor_sha_matched` fact with the lines' hash, and the rules compare the
//! hashes to derive `anchor_moved`.

use std::path::Path;

use familiar_ai_facts::{AnchorShaMatched, CheckedAt, Fact, Log, Record, RecordError};
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::{plan, View};
use serde_json::Value;

use crate::door::commits::last_commit_seen;

/// One check the rules say is owed, with real values in place.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub criterion_id: String,
    pub file_path: String,
    pub symbol: String,
    pub line_start: u64,
    pub line_end: u64,
    pub checked_at: CheckedAt,
}

fn as_u64(v: Option<i64>) -> Option<u64> {
    v.and_then(|n| u64::try_from(n).ok())
}

/// The next check owed at this log, planning checks first; `None` when none.
pub fn next_check(log: &Log) -> Option<Check> {
    let v = View::new(log);
    for c in plan::criteria(&v) {
        let Some(a) = plan::anchor_of(&v, &c) else {
            continue;
        };
        if a.kind != "place_chosen" {
            continue;
        }
        for p in plan::places_of(a) {
            let (Some(line_start), Some(line_end)) = (as_u64(p.line_start), as_u64(p.line_end)) else {
                continue;
            };
            if plan::check_of(&v, &c, &p).is_none() {
                return Some(Check {
                    criterion_id: c.clone(),
                    file_path: p.file_path.to_string(),
                    symbol: p.symbol.to_string(),
                    line_start,
                    line_end,
                    checked_at: CheckedAt::Planning,
                });
            }
        }
    }
    // The start check, as the rules offer it.
    let start = criterion_admissible(&v).into_iter().find(|m| {
        m.kind == "anchor_sha_matched"
            && m.fields.get("checked_at").and_then(Value::as_str) == Some("start")
    })?;
    let text = |k: &str| start.fields.get(k).and_then(Value::as_str).map(str::to_string);
    let num = |k: &str| start.fields.get(k).and_then(Value::as_u64);
    Some(Check {
        criterion_id: text("criterion_id")?,
        file_path: text("file_path")?,
        symbol: text("symbol")?,
        line_start: num("line_start")?,
        line_end: num("line_end")?,
        checked_at: CheckedAt::Start,
    })
}

/// The lines' hash and whether the symbol's text is on them; a missing file
/// or out-of-range lines give an empty hash and `false`.
fn looked(bytes: Option<Vec<u8>>, check: &Check) -> (String, bool) {
    let Some(bytes) = bytes else {
        return (String::new(), false);
    };
    let Some(lines) = familiar_ai_git::line_bytes(&bytes, check.line_start, check.line_end) else {
        return (String::new(), false);
    };
    let hash = familiar_ai_git::lines_hash_of(&bytes, check.line_start, check.line_end)
        .unwrap_or_default();
    let symbol_there = check.symbol.is_empty()
        || lines.windows(check.symbol.len()).any(|w| w == check.symbol.as_bytes());
    (hash, symbol_there)
}

/// Where the planned lines `line_start..=line_end` of `old` sit in `new`, or
/// `None` when every planned line was deleted. Lines added or removed above
/// the place, or added right after it, only shift it.
pub fn moved_to(old: &[u8], new: &[u8], line_start: u64, line_end: u64) -> Option<(u64, u64)> {
    let hunks = familiar_ai_git::line_hunks(old, new);
    let mut shift: i64 = 0;
    for h in &hunks {
        let (start, end) = (h.old_start, h.old_start + h.old_count);
        let (old_n, new_n) = (i64::try_from(h.old_count).ok()?, i64::try_from(h.new_count).ok()?);
        if end <= line_start && (h.old_count > 0 || start <= line_start) {
            shift += new_n - old_n; // above the place
        } else if start > line_end {
            // below the place, or added right after it
        } else {
            return match familiar_ai_git::map_range(&hunks, line_start, line_end) {
                familiar_ai_git::MappedRange::Range { line_start, line_end } => Some((line_start, line_end)),
                familiar_ai_git::MappedRange::Deleted => None,
            };
        }
    }
    let at = |n: u64| u64::try_from(i64::try_from(n).ok()? + shift).ok();
    Some((at(line_start)?, at(line_end)?))
}

/// Build the check's fact. The start check reads the working tree, since
/// another's edit since planning may not be committed yet, and finds the
/// lines from `planned_at`, the planning check's sha. The start check also
/// says whether those lines are the lines at `sha`, the record's tip commit
/// (`tip_matched`, the owner, 2026-10-11): the planned lines found in the tip
/// the same way, and their text the same as the working tree's.
pub fn look(root: &Path, sha: &str, check: &Check, planned_at: Option<&str>) -> AnchorShaMatched {
    let bytes = match check.checked_at {
        CheckedAt::Planning => familiar_ai_git::file_at(root, sha, &check.file_path).ok(),
        CheckedAt::Start => std::fs::read(root.join(&check.file_path)).ok(),
    };
    let planned = planned_at.filter(|_| check.checked_at == CheckedAt::Start).and_then(|from| familiar_ai_git::file_at(root, from, &check.file_path).ok());
    let found_in = |new: &[u8]| match planned.as_deref() {
        Some(old) => moved_to(old, new, check.line_start, check.line_end).unwrap_or((0, 0)),
        None => (check.line_start, check.line_end),
    };
    let (line_start, line_end) = bytes.as_deref().map_or((check.line_start, check.line_end), found_in);
    let tip_matched = (check.checked_at == CheckedAt::Start).then(|| {
        let Some(now) = bytes.as_deref() else {
            return false;
        };
        let Ok(tip) = familiar_ai_git::file_at(root, sha, &check.file_path) else {
            return false;
        };
        let (a, b) = found_in(&tip);
        let here = familiar_ai_git::line_bytes(now, line_start, line_end);
        here.is_some() && here == familiar_ai_git::line_bytes(&tip, a, b)
    });
    let found = Check { line_start, line_end, ..check.clone() };
    let (lines_hash, matched) = looked(bytes, &found);
    AnchorShaMatched {
        criterion_id: check.criterion_id.clone(),
        file_path: check.file_path.clone(),
        symbol: check.symbol.clone(),
        line_start,
        line_end,
        sha: sha.to_string(),
        lines_hash,
        matched,
        checked_at: check.checked_at,
        tip_matched,
    }
}

/// The planning check of the place a start check is owed for (the move names
/// the place as planned); the criterion's latest when no place matches.
fn planning_of<'a>(v: &View<'a>, check: &Check) -> Option<&'a Fact> {
    let place = plan::PlaceRef {
        file_path: &check.file_path,
        symbol: &check.symbol,
        line_start: i64::try_from(check.line_start).ok(),
        line_end: i64::try_from(check.line_end).ok(),
    };
    plan::check_of(v, &check.criterion_id, &place).or_else(|| familiar_ai_rules::lifecycle::planning_check(v, &check.criterion_id))
}

/// The places where the change landed since the seal, read from the
/// repository (F12): for each file the rules name, the hunks from the
/// tests-first commit `base` to `tip`; each hunk's symbol is the line git
/// prints after its `@@` (`funcname_above`) and its lines run from that line
/// to the hunk's end, numbered at `base`; a hunk with no such line is a
/// file-level place (no symbol, the hunk's own lines). Hunks under one symbol
/// make one place. A file absent at `base` or `tip` names no place.
pub fn landed_places(root: &Path, base: &str, tip: &str, files: &[String]) -> Vec<familiar_ai_facts::Place> {
    let mut out: Vec<familiar_ai_facts::Place> = Vec::new();
    for file in files.iter().filter(|f| !crate::door::commits::is_familiars_own(f)) {
        let (Ok(old), Ok(new)) = (familiar_ai_git::file_at(root, base, file), familiar_ai_git::file_at(root, tip, file)) else {
            continue;
        };
        for h in familiar_ai_git::line_hunks(&old, &new) {
            // an insertion follows line old_start - 1, which then belongs to the place
            let end = if h.old_count > 0 { h.old_start + h.old_count - 1 } else { h.old_start.saturating_sub(1) };
            let (start, symbol) = match familiar_ai_git::funcname_above(&old, h.old_start) {
                Some((line, text)) => (line, text),
                None if h.old_count > 0 => (h.old_start, String::new()),
                None => (end, String::new()),
            };
            let (start, end) = (start.max(1), end.max(start).max(1));
            match out.iter_mut().find(|p| p.file_path == *file && p.symbol == symbol) {
                Some(p) => {
                    p.line_start = p.line_start.min(start);
                    p.line_end = p.line_end.max(end);
                }
                None => out.push(familiar_ai_facts::Place { file_path: file.clone(), symbol, line_start: start, line_end: end }),
            }
        }
    }
    out
}

/// Right after the person's keep_going on `anchor_untouched`, append the
/// place where the change landed (the rules offer it as the binary's move),
/// carrying the answer's hold and choice, then each place's planning check at
/// the tests-first commit, so putting the lines back takes the change out.
/// The owner (2026-10-13): "we update the plan, with a stamp that the user
/// basically overrode it." Nothing is written when no landed line is found.
fn kept_places(record: &Record, root: &Path, log: &mut Log, at: f64, tenant: &str) -> Result<Vec<Fact>, RecordError> {
    let mut written = Vec::new();
    let v = View::new(log);
    let mut owed: Vec<(String, Vec<String>, String)> = Vec::new();
    for c in plan::criteria(&v) {
        let Some(m) = familiar_ai_rules::moves::kept_place_move(&v, &c) else {
            continue;
        };
        let files: Vec<String> = m.fields.get("places").and_then(Value::as_array).into_iter().flatten().filter_map(|p| p.get("file_path").and_then(Value::as_str).map(str::to_string)).collect();
        let Some(base) = familiar_ai_rules::criterion::tests_first_commit(&v, &c).and_then(|tf| tf.ids.last().cloned()) else {
            continue;
        };
        owed.push((c, files, base));
    }
    drop(v);
    let Some(tip) = last_commit_seen(log) else {
        return Ok(written);
    };
    for (c, files, base) in owed {
        let places = landed_places(root, &base, &tip, &files);
        if places.is_empty() {
            continue;
        }
        let kept = familiar_ai_facts::PlaceChosen { criterion_id: c.clone(), places: places.clone(), hold: Some("anchor_untouched".into()), choice: Some("keep_going".into()) };
        let fact = record.append(&kept, at, tenant)?;
        log.push(fact.clone());
        written.push(fact);
        for p in places {
            let check = Check { criterion_id: c.clone(), file_path: p.file_path, symbol: p.symbol, line_start: p.line_start, line_end: p.line_end, checked_at: CheckedAt::Planning };
            let fact = record.append(&look(root, &base, &check, None), at, tenant)?;
            log.push(fact.clone());
            written.push(fact);
        }
    }
    Ok(written)
}

/// Write every owed check until none is left, and return them. Nothing is
/// written before a commit is seen, since there is no sha yet. The place
/// keep_going appends comes first, with its own planning checks.
pub fn binary_checks(
    record: &Record,
    root: &Path,
    log: &mut Log,
    at: f64,
    tenant: &str,
) -> Result<Vec<Fact>, RecordError> {
    let mut written = kept_places(record, root, log, at, tenant)?;
    loop {
        let Some(check) = next_check(log) else {
            // the owner (2026-10-11): a light anchor's documentation check, read at the same sha
            match crate::door::documentation::write_one(record, root, log, at, tenant)? {
                Some(fact) => {
                    written.push(fact);
                    continue;
                }
                None => break,
            }
        };
        let Some(sha) = last_commit_seen(log) else {
            break;
        };
        let planned_at = match check.checked_at {
            // F7: the planning check of this place, not the latest of any place
            CheckedAt::Start => planning_of(&View::new(log), &check)
                .and_then(|f| f.fields.get("sha"))
                .and_then(Value::as_str)
                .map(str::to_string),
            CheckedAt::Planning => None,
        };
        let fact = record.append(&look(root, &sha, &check, planned_at.as_deref()), at, tenant)?;
        log.push(fact.clone());
        written.push(fact);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append;
    use familiar_ai_rules::sample::criterion_fixture;

    #[test]
    fn a_confirmed_plan_owes_the_start_check_of_the_active_criterion() {
        let log = criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture");
        let check = next_check(&log).expect("the start check is owed");
        assert_eq!(check.checked_at, CheckedAt::Start);
        assert_eq!(check.criterion_id, "c1");
        assert_eq!((check.file_path.as_str(), check.symbol.as_str()), ("src/login.py", "check_password"));
        assert_eq!((check.line_start, check.line_end), (40, 62));
    }

    #[test]
    fn the_planned_lines_are_found_where_the_commits_put_them() {
        let old = b"a\nb\nc\nd\n";
        assert_eq!(moved_to(old, b"x\ny\na\nb\nc\nd\n", 2, 3), Some((4, 5)), "two lines above shift it");
        assert_eq!(moved_to(old, b"b\nc\nd\n", 2, 3), Some((1, 2)), "a line removed above");
        assert_eq!(moved_to(old, b"a\nb\nc\nnew\nd\n", 2, 3), Some((2, 3)), "added right after: not the place's");
        assert_eq!(moved_to(old, b"a\nb\nnew\nc\nd\n", 2, 3), Some((2, 4)), "added inside: the place grew");
        assert_eq!(moved_to(old, b"a\nd\n", 2, 3), None, "every planned line deleted");
    }

    #[test]
    fn once_the_start_check_is_written_nothing_is_owed() {
        let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let check = next_check(&log).expect("owed");
        let fact = AnchorShaMatched {
            criterion_id: check.criterion_id,
            file_path: check.file_path,
            symbol: check.symbol,
            line_start: check.line_start,
            line_end: check.line_end,
            sha: "base".into(),
            lines_hash: "L0".into(),
            matched: true,
            checked_at: CheckedAt::Start,
            tip_matched: None,
        };
        let log = append(&log, &fact, 0.0, "t").expect("append");
        assert!(next_check(&log).is_none());
    }

    #[test]
    fn a_place_without_a_planning_check_owes_one_first() {
        let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        // the fixture's planning check is for c1's place; a new place_chosen has none
        let log = append(
            &log,
            &familiar_ai_facts::PlaceChosen {
                criterion_id: "c1".into(),
                places: vec![familiar_ai_facts::Place {
                    file_path: "src/login.py".into(),
                    symbol: "check_password".into(),
                    line_start: 41,
                    line_end: 63,
                }],
                hold: None,
                choice: None,
            },
            0.0,
            "t",
        )
        .expect("re-anchor");
        let check = next_check(&log).expect("owed");
        assert_eq!(check.checked_at, CheckedAt::Planning);
        assert_eq!((check.line_start, check.line_end), (41, 63));
    }

    #[test]
    fn looked_reads_the_lines_the_symbol_and_a_missing_file() {
        let check = Check {
            criterion_id: "c1".into(),
            file_path: "src/app.sh".into(),
            symbol: "greet".into(),
            line_start: 2,
            line_end: 2,
            checked_at: CheckedAt::Start,
        };
        let (hash, matched) = looked(Some(b"#!/bin/sh\ngreet() { echo hi; }\n".to_vec()), &check);
        assert_eq!(hash.len(), 64);
        assert!(matched);
        let (_, matched) = looked(Some(b"#!/bin/sh\nhello() { echo hi; }\n".to_vec()), &check);
        assert!(!matched, "the symbol is not on the lines");
        let (hash, matched) = looked(Some(b"one line\n".to_vec()), &check);
        assert_eq!((hash.as_str(), matched), ("", false), "the range is outside");
        let (hash, matched) = looked(None, &check);
        assert_eq!((hash.as_str(), matched), ("", false), "no file");
    }
}
