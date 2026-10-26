//! Pure rules for the familiar-ai binary, derived from the fact log with no IO, clock or git.

pub mod action;
pub mod ask;
pub mod cases;
pub mod criterion;
pub mod dependency;
pub mod existing;
pub mod finding;
pub mod holds;
pub mod hooks;
pub mod ladder;
pub mod lifecycle;
pub mod moves;
pub mod outcome;
pub mod places;
pub mod plan;
pub mod reader;
pub mod reading;
pub mod review;
pub mod sample;
pub mod terminal;
pub mod trace;

use familiar_ai_facts::Log;

pub use hooks::{Clears, Decision, Event, Refusal};
pub use reader::View;

/// Decide what the hook answers to one event, from the log alone.
/// `now` is accepted for the caller's signature but no rule reads it.
pub fn decide(log: &Log, event: &Event, _now: f64) -> Decision {
    let v = View::new(log);
    hooks::decide(&v, event)
}

/// An anchor as the commit reader needs it, mirroring `familiar_ai_git::Anchor` so the rules depend on no git.
/// Lines 0..0 mean the whole file, which the teeth run puts back whole for new work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub criterion_id: String,
    pub file_path: String,
    pub line_start: u64,
    pub line_end: u64,
    pub sha: String,
}

/// What the commit reader needs from the log to read one commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WhatToRead {
    pub plan_test_files: Vec<String>,
    /// Each runnable criterion with its planned and written test files, in plan order.
    pub runnable_tests: Vec<(String, Vec<String>)>,
    /// Every criterion with its planned and written test files, in plan order; each commit's
    /// `tests_observed` holds one fingerprint per entry.
    pub all_tests: Vec<(String, Vec<String>)>,
    pub anchors: Vec<Anchor>,
}

impl WhatToRead {
    /// Return the files a commit's `tests_hash` covers: the first runnable criterion whose files
    /// the commit changed, else the first runnable criterion, else none.
    pub fn tests_for(&self, changed: &[String]) -> Vec<String> {
        self.runnable_tests
            .iter()
            .find(|(_, fs)| fs.iter().any(|f| changed.contains(f)))
            .or_else(|| self.runnable_tests.first())
            .map(|(_, fs)| fs.clone())
            .unwrap_or_default()
    }
}

/// Derive what the commit reader needs from the log.
/// An anchor is each place of a criterion's last `place_chosen`, at the sha of its latest
/// planning check, else the intent's sha.
pub fn what_to_read(log: &Log) -> WhatToRead {
    let v = View::new(log);
    let mut out = WhatToRead::default();
    let push_distinct = |xs: &mut Vec<String>, x: String| {
        if !xs.contains(&x) {
            xs.push(x);
        }
    };
    let runnable = lifecycle::runnable_criteria(&v);
    for c in plan::criteria(&v) {
        for fp in criterion::test_files(&v, &c) {
            push_distinct(&mut out.plan_test_files, fp);
        }
        let mut fs: Vec<String> = Vec::new();
        for f in v.of("test_planned", &c).chain(v.of("test_written", &c)) {
            if let Some(fp) = reader::text(f, "file_path") {
                push_distinct(&mut fs, fp.to_string());
            }
        }
        if runnable.contains(&c) {
            out.runnable_tests.push((c.clone(), fs.clone()));
        }
        out.all_tests.push((c.clone(), fs));
        let Some(a) = plan::anchor_of(&v, &c) else {
            continue;
        };
        if a.kind != "place_chosen" {
            // New work's file is put back whole: lines 0..0 mean the whole file.
            if let Some(fp) = plan::exit_files(a).and_then(|fs| fs.into_iter().next()) {
                let sha = v.last("intent_submitted").and_then(|i| reader::text(i, "sha")).unwrap_or("").to_string();
                out.anchors.push(Anchor { criterion_id: c.clone(), file_path: fp, line_start: 0, line_end: 0, sha });
            }
            continue;
        }
        for p in plan::places_of(a) {
            let (Some(start), Some(end)) = (p.line_start, p.line_end) else {
                continue;
            };
            let sha = plan::check_of(&v, &c, &p)
                .and_then(|ch| reader::text(ch, "sha"))
                .or_else(|| v.last("intent_submitted").and_then(|i| reader::text(i, "sha")))
                .unwrap_or("")
                .to_string();
            out.anchors.push(Anchor {
                criterion_id: c.clone(),
                file_path: p.file_path.to_string(),
                line_start: u64::try_from(start).unwrap_or(0),
                line_end: u64::try_from(end).unwrap_or(0),
                sha,
            });
        }
    }
    out
}
