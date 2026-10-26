//! What the investigator is shown, from the record, git and the kept runs; `teach::investigator` words it.
//! The investigator by layer: decide/ the spawn and the wait · door/ the facts, the run, the brief · teach/ the words.

use std::path::Path;

use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::criterion::{mine, sealed_at, Place};
use familiar_ai_rules::{lifecycle, plan, View};
use serde_json::Value;

use crate::teach::investigator::{Brief, Try};

pub const TRIES_SHOWN: usize = 2;

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

pub fn inputs(log: &Log, root: &Path, c: &str) -> Brief {
    let v = View::new(log);
    let seal = sealed_at(&v, c, Place::Laptop);
    let judged = seal.and_then(|s| crate::door::judge::inputs(log, root, c, usize::try_from(s.t).unwrap_or(usize::MAX)));
    let judge = seal.and_then(|s| crate::teach::judge::verdict_after(&v, c, s)).and_then(|f| match text(f, "answers_intent") {
        Some("yes") => Some((true, String::new())),
        // An older verdict names no gap, so its reason stands for it.
        Some("no") => Some((false, text(f, "gap").or_else(|| text(f, "reason")).unwrap_or("").trim().to_string())),
        _ => None,
    });
    let tries = failed_tries(log, c);
    let first = tries.len().saturating_sub(TRIES_SHOWN);
    let tries = tries
        .iter()
        .enumerate()
        .skip(first)
        .map(|(i, commit)| Try { n: i + 1, commit: commit.to_string(), files: changes(root, commit), output: crate::door::kept::kept_run(root, commit) })
        .collect();
    Brief {
        criterion_id: c.to_string(),
        said: crate::teach::person::words_and_picks(log),
        criterion: v.of("criterion", c).last().and_then(|f| text(f, "text")).unwrap_or("").to_string(),
        judge,
        place: place_now(&v, root, c),
        tests: judged.map(|i| i.tests).unwrap_or_default(),
        command: lifecycle::command_at(&v, c, i64::MAX).unwrap_or("none recorded").to_string(),
        planned: familiar_ai_rules::criterion::test_files(&v, c),
        category: lifecycle::fix_location(&v, c).and_then(|f| text(f, "category")).unwrap_or("none claimed").to_string(),
        tries,
    }
}

/// The commits of the failed tries since the seal, oldest first. A try is a
/// laptop stamp that raises `lifecycle::attempts` when the record includes it.
pub fn failed_tries<'a>(log: &'a Log, c: &str) -> Vec<&'a str> {
    let v = View::new(log);
    let Some(seal) = sealed_at(&v, c, Place::Laptop) else {
        return Vec::new();
    };
    let counted = |end: usize| log.get(..end).map_or(0, |l| lifecycle::attempts(&View::new(l), c));
    mine(&v, c, "fingerprint_stamped")
        .into_iter()
        .filter(|s| s.t > seal.t && text(s, "ran_in") == Some("laptop"))
        .filter(|s| {
            let at = usize::try_from(s.t).unwrap_or(usize::MAX);
            counted(at.saturating_add(1)) > counted(at)
        })
        .filter_map(|s| text(s, "commit"))
        .collect()
}

/// A commit's changed files against its first parent, with lines added and
/// removed; empty when unreadable.
fn changes(root: &Path, commit: &str) -> Vec<(String, u64, u64)> {
    familiar_ai_git::changed(root, commit)
        .map(|files| {
            files
                .into_iter()
                .map(|f| {
                    let added = f.hunks.iter().map(|h| h.new_count).sum();
                    let removed = f.hunks.iter().map(|h| h.old_count).sum();
                    (f.path, added, removed)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The anchor in words, each place followed by its lines at HEAD.
fn place_now(v: &View, root: &Path, c: &str) -> String {
    let Some(a) = plan::anchor_of(v, c) else {
        return "none chosen".to_string();
    };
    match a.kind {
        "place_chosen" => plan::places_of(a)
            .iter()
            .map(|p| {
                let (s, e) = (p.line_start.unwrap_or(0), p.line_end.unwrap_or(0));
                let at = |n: i64| u64::try_from(n).unwrap_or(0);
                let lines = familiar_ai_git::file_at(root, "HEAD", p.file_path)
                    .ok()
                    .and_then(|b| familiar_ai_git::line_bytes(&b, at(s), at(e)))
                    .map(|b| String::from_utf8_lossy(&b).trim_end().to_string());
                match lines {
                    Some(l) => format!("{} · {} · lines {s}–{e}, and those lines as they are now:\n{l}", p.file_path, p.symbol),
                    None => format!("{} · {} · lines {s}–{e} (not readable at HEAD)", p.file_path, p.symbol),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "new_work_spot_chosen" => format!(
            "new work in {} ({}), beside {}",
            text(a, "file_path").unwrap_or(""),
            text(a, "symbol").unwrap_or(""),
            text(a, "beside").unwrap_or("")
        ),
        _ => "new work with no spot in the existing code".to_string(),
    }
}
