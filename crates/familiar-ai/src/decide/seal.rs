//! The judge's gate over the rules' decision: while a red seal's verdict is pending or "no", code for that
//! criterion is refused, and a stop is refused while a "no" stands against a green criterion, the judge's or
//! the reviewer's verified one on the fix. The reference model has no judge; see
//! docs/decisions.md#a-judge-one-rung-up.

use std::path::Path;

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::test_files;
use familiar_ai_rules::{lifecycle, plan, View};
use serde_json::Value;

use crate::decide::decision::{mentions_git_commit, relative_to, Decision};
use crate::door::commits::is_familiars_own;
use crate::door::shell_write::{inside_project, written_paths, Written};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::judge::{self, Said};

/// Apply the gate on code at PreToolUse when the judge is on, and the "no" check at Stop.
/// A "no" already written blocks a stop even with the judge off, because it is a fact.
pub fn decide(decided: Decision, input: &HookInput, log: &Log, root: &Path, judge_on: bool) -> Decision {
    if matches!(decided, Decision::Refuse { .. }) {
        return decided;
    }
    let v = View::new(log);
    match input.event {
        HookEvent::PreToolUse if judge_on => match gated_code(&v, input, root) {
            Some((c, said)) => Decision::Refuse { reason: judge::gate(&v, &c, &said) },
            None => decided,
        },
        // an announced change to an existing test waits for the judge's word (the owner, 2026-10-20) [draft]
        HookEvent::Stop if judge_on && existing_owed(&v).is_some() => Decision::Refuse { reason: existing_owed(&v).unwrap_or_default() },
        HookEvent::Stop => match no_after_green(&v) {
            Some(reason) => Decision::Refuse { reason },
            None => decided,
        },
        _ => decided,
    }
}

/// Unreleased criteria with a red seal whose verdict is pending or "no".
fn gated(v: &View) -> Vec<(String, Said)> {
    plan::criteria(v)
        .into_iter()
        .filter(|c| !lifecycle::released(v, c))
        .filter_map(|c| match judge::said(v, &c)? {
            // After two misses with no verdict the gate lifts.
            Said::Covers | Said::NotJudged { .. } => None,
            said => Some((c, said)),
        })
        .collect()
}

/// The gated criterion this call's code lands on: a non-test edit of the active criterion or its anchor,
/// or a commit while an anchor file differs from HEAD, since a commit's content is known only after it lands.
fn gated_code(v: &View, input: &HookInput, root: &Path) -> Option<(String, Said)> {
    let gated = gated(v);
    if gated.is_empty() {
        return None;
    }
    let tests: Vec<String> = plan::criteria(v).iter().flat_map(|c| test_files(v, c)).collect();
    let is_code = |rel: &String| !tests.contains(rel) && !is_familiars_own(rel);
    let lane = lifecycle::lane(v, input.agent_conversation_id().as_deref());
    let on = |files: &[String]| gated.iter().find(|(c, _)| lane.as_deref() == Some(c.as_str()) || files.iter().any(|f| anchor_files(v, c).contains(f))).cloned();
    if let Some(fp) = input.file_path() {
        let rel = relative_to(root, fp);
        return if is_code(&rel) { on(&[rel]) } else { None };
    }
    let command = input.command()?;
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    let written: Vec<String> = written_paths(command)
        .into_iter()
        .filter_map(|w| match w {
            Written::Path(p) => inside_project(root, cwd, &p),
            Written::Unknown => None, // `patch`/`git apply`: no path to read
        })
        .filter(is_code)
        .collect();
    if !written.is_empty() {
        if let Some(hit) = on(&written) {
            return Some(hit);
        }
    }
    if !mentions_git_commit(command) {
        return None;
    }
    gated.into_iter().find(|(c, _)| anchor_changed(v, c, root))
}

/// `c`'s anchor files, relative to the project.
fn anchor_files(v: &View, c: &str) -> Vec<String> {
    let Some(a) = plan::anchor_of(v, c) else {
        return Vec::new();
    };
    match a.kind {
        "place_chosen" => plan::places_of(a).iter().map(|p| p.file_path.to_string()).collect(),
        "new_work_spot_chosen" => a.fields.get("file_path").and_then(Value::as_str).map(str::to_string).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// True when an anchor file differs from HEAD, or HEAD cannot be read.
fn anchor_changed(v: &View, c: &str, root: &Path) -> bool {
    let head = familiar_ai_git::head_commit(root).ok().flatten();
    anchor_files(v, c).iter().any(|f| {
        let disk = std::fs::read(root.join(f)).ok();
        let at_head = head.as_deref().and_then(|h| familiar_ai_git::file_at(root, h, f).ok());
        head.is_none() || disk != at_head
    })
}

/// The stop refusal while the judge owes a word on an announced change to an existing test.
fn existing_owed(v: &View) -> Option<String> {
    let owed = familiar_ai_rules::existing::owed(v, judge::TWICE);
    let (e, _) = owed.first()?;
    Some(format!(
        "familiar-ai: not done yet · the judge has not read the change to the existing test {} yet; it reads it at your next stop",
        familiar_ai_rules::existing::words(e)
    ))
}

/// The stop refusal when a "no" stands against a green or proven criterion: the judge's on its
/// seal, else the reviewer's verified one on its fix (the owner's plan of 2026-10-20, step 3).
fn no_after_green(v: &View) -> Option<String> {
    plan::criteria(v).into_iter().filter(|c| !lifecycle::released(v, c)).find_map(|c| {
        if !matches!(lifecycle::state(v, &c), "green" | "proven") {
            return None;
        }
        if let Some(Said::DoesNotCover { model, gap }) = judge::said(v, &c) {
            return Some(judge::not_done(v, &c, &model, &gap));
        }
        familiar_ai_rules::review::standing_no(v, &c).map(|no| crate::teach::review::not_done(v, &c, no))
    })
}
