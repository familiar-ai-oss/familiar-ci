//! Everything that touches the record, the repository or a child process. Facts
//! are appended one at a time, and the committed plan mirrors the record's claims
//! after each.

pub mod aim;
pub mod anchor;
pub mod approval;
pub mod lens;
pub mod child;
pub mod ci;
pub mod claim;
pub mod claim_cmd;
pub mod commits;
pub mod convert;
pub mod dependency;
pub mod dry_run;
pub mod existing;
pub mod diagnosis;
pub mod documentation;
pub mod folder;
pub mod git_program;
pub mod investigator;
pub mod investigator_brief;
pub mod investigator_run;
pub mod judge;
pub mod judge_answer;
pub mod kept;
pub mod kits;
pub mod lane;
pub mod marker;
pub mod person;
pub mod picker;
pub mod places;
pub mod precedents;
#[cfg(test)]
mod precedents_tests;
pub mod procgroup;
pub mod project;
pub mod readers;
pub mod review;
pub mod reported;
pub mod run;
pub mod runner;
pub mod runner_tables;
pub mod runs;
pub mod set_up;
pub mod setup;
pub mod share;
pub mod siblings;
pub mod sh_trace;
pub mod shell_forms;
pub mod shell_write;
pub mod stack;
pub mod statusline;
pub mod switch;
pub mod tidy;
pub mod told;
pub mod touching;
pub mod try_cmd;
pub mod try_rank;
pub mod tidy_plan;
pub mod verify;
pub mod verify_report;
pub mod verify_run;

/// The folder holding the plan, which is committed, and the record, which is not.
pub const FAMILIAR_FOLDER: &str = ".familiar";
pub const RECORD_FILE: &str = "record";
pub const PLAN_FILE: &str = "plan";
/// The judge's call log inside [`FAMILIAR_FOLDER`], laptop-only like the record.
pub const JUDGE_LOG: &str = "judge.log";
/// The investigator's run log inside [`FAMILIAR_FOLDER`], laptop-only like the record.
pub const INVESTIGATOR_LOG: &str = "investigator.log";
/// Every fact's tenant in this variant.
pub const TENANT: &str = "_default_";

/// Before anything is decided, record landed commits, write the facts they owe,
/// run the owed anchor checks, and return the log.
pub(crate) fn catch_up(record: &familiar_ai_facts::Record, root: &std::path::Path, around: &crate::input::Surroundings) -> Result<familiar_ai_facts::Log, String> {
    catch_up_with_run(record, root, around, None).map(|(log, _)| log)
}

/// `catch_up`, also returning where the results this call settled begin in the log; `by`
/// goes on every commit it records.
pub(crate) fn catch_up_with_run(record: &familiar_ai_facts::Record, root: &std::path::Path, around: &crate::input::Surroundings, by: Option<&str>) -> Result<(familiar_ai_facts::Log, usize), String> {
    let landed = commits::record_commits(record, root, &what_to_read, around.now, TENANT, by)
        .map_err(|e| cannot("read the repository", &e))?;
    let mut log: familiar_ai_facts::Log = record.read_all().map_err(|e| cannot("read its record", &e))?;
    // the touching tests the fix's imports name, found when commits land (the owner, 2026-10-20) [draft]
    if !landed.is_empty() {
        touching::derive(record, root, &mut log, around.now, TENANT).map_err(|e| cannot("write its record", &e))?;
    }
    runner::recover(record, &mut log, around.now)?;
    runner::backstop(record, root, &mut log, around.now)?;
    // c_dep (plan §7.3): a draft with missing lines is written again, an approval with no draft drafted again
    dependency::backstop(record, root, &mut log, around)?;
    // Only results settled after this point are reported.
    let settled_from = log.len();
    // Reserve and launch owed runs with the record released, then wait within one
    // `run_inline` budget for the chain to settle; a slow run is left live. The
    // runner launches successors, so the hook only waits and reads.
    let until = std::time::Instant::now() + around.run_inline;
    let mut children = Vec::new();
    loop {
        let moves = run::binary_facts(record, &mut log, around.now, TENANT, around).map_err(|e| cannot("write its record", &e))?;
        let launching = !moves.reserved.is_empty();
        if launching {
            children.extend(record.while_released(around.record_wait, || runner::spawn(root, around, &moves.reserved)).map_err(|e| cannot("reopen its record", &e))?);
            log = record.read_all().map_err(|e| cannot("read its record", &e))?;
        }
        // A child exits only after its last append, so settling waits for them.
        runner::reap(&mut children);
        if !launching && !runs::any_live(&log, around.now) && children.is_empty() {
            break;
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        record.while_released(around.record_wait, || runner::nap(&mut children, left.min(std::time::Duration::from_millis(50)))).map_err(|e| cannot("reopen its record", &e))?;
        log = record.read_all().map_err(|e| cannot("read its record", &e))?;
    }
    anchor::binary_checks(record, root, &mut log, around.now, TENANT)
        .map_err(|e| cannot("write its record", &e))?;
    Ok((log, settled_from))
}

/// Write the committed plan file from the record's claims; the binary never reads it.
pub(crate) fn project_plan(root: &std::path::Path, log: &familiar_ai_facts::Log) -> Result<(), String> {
    let path = root.join(FAMILIAR_FOLDER).join(PLAN_FILE);
    familiar_ai_plan::write(&path, &familiar_ai_plan::claims_from_log(log))
        .map_err(|e| cannot("write the plan file", &e))?;
    // the adapter is mirrored beside the plan, at the same moments (plan A)
    project_adapter(root, log)
}

/// The adapter's file inside [`FAMILIAR_FOLDER`] (plan A [draft]), committed like the plan.
pub const ADAPTER_FILE: &str = "adapter";

/// The adapter's text (plan A): one JSON line per (root, runner) the record chose a reader for,
/// the latest choice each, sorted by (root, runner), keys in a fixed order, reader ids only.
pub fn adapter_lines(log: &[familiar_ai_facts::Fact]) -> String {
    use serde_json::Value;
    let s = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut latest: std::collections::BTreeMap<(String, String), &familiar_ai_facts::Fact> = std::collections::BTreeMap::new();
    // the laptop's choices: CI's own (`ran_in: pipeline`) are its, never the committed file's
    for f in log.iter().filter(|f| f.kind == "reader_chosen" && s(f, "ran_in") != "pipeline") {
        latest.insert((s(f, "root"), s(f, "runner")), f);
    }
    let mut out = String::new();
    for ((root, runner), f) in latest {
        let q = |v: &str| Value::from(v).to_string();
        let gaps: Vec<String> = f.fields.get("gaps").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(q).collect()).unwrap_or_default();
        let setup = f.fields.get("setup").and_then(Value::as_str).map(|v| format!(",\"setup\":{}", q(v))).unwrap_or_default();
        out.push_str(&format!(
            "{{\"root\":{},\"runner\":{},\"reader\":{}{setup},\"familiar\":{},\"gaps\":[{}]}}\n",
            q(&root),
            q(&runner),
            q(&s(f, "reader")),
            q(&s(f, "familiar")),
            gaps.join(",")
        ));
    }
    out
}

/// Write `.familiar/adapter` from the record's choices (plan A): the binary's, never by hand;
/// nothing is written before the first choice, and an unchanged file is left as it is.
pub fn project_adapter(root: &std::path::Path, log: &[familiar_ai_facts::Fact]) -> Result<(), String> {
    let text = adapter_lines(log);
    let path = root.join(FAMILIAR_FOLDER).join(ADAPTER_FILE);
    if text.is_empty() || std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
        return Ok(());
    }
    std::fs::write(&path, text).map_err(|e| cannot("write the adapter file", &e))
}

/// One plain line for what could not be read.
pub(crate) fn cannot(what: &str, e: &dyn std::fmt::Display) -> String {
    format!("Familiar cannot {what}: {e}")
}

/// The rules' `what_to_read(log)` in the shape the commit reader takes.
pub(crate) fn what_to_read(log: &familiar_ai_facts::Log) -> commits::WhatToRead {
    let w = familiar_ai_rules::what_to_read(log);
    let v = familiar_ai_rules::View::new(log);
    commits::WhatToRead {
        plan_test_files: w.plan_test_files,
        all_tests: w
            .all_tests
            .into_iter()
            .map(|(c, files)| commits::CriterionTests { criterion_rev: familiar_ai_rules::criterion::criterion_rev(&v, &c), criterion_id: c, files })
            .collect(),
        anchors: w
            .anchors
            .into_iter()
            .map(|a| {
                let (sha, line_start, line_end) = where_it_is(&v, &a).unwrap_or((a.sha, a.line_start, a.line_end));
                familiar_ai_git::Anchor { criterion_id: a.criterion_id, file_path: a.file_path, line_start, line_end, sha }
            })
            .collect(),
    }
}

/// Where the start check found an anchor's lines, when it followed the planning
/// check of the same place, matched, and hashed the same lines; `None` keeps the plan's.
fn where_it_is(v: &familiar_ai_rules::View, a: &familiar_ai_rules::Anchor) -> Option<(String, u64, u64)> {
    use serde_json::Value;
    if a.line_start == 0 && a.line_end == 0 {
        return None; // a whole file is put back whole
    }
    let text = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_str).map(str::to_string);
    let num = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_u64);
    let of_this_place = |planned: &familiar_ai_facts::Fact| {
        text(planned, "file_path").as_deref() == Some(a.file_path.as_str())
            && num(planned, "line_start") == Some(a.line_start)
            && num(planned, "line_end") == Some(a.line_end)
            && text(planned, "sha").as_deref() == Some(a.sha.as_str())
    };
    // F7: this place's own planning check and its own start check
    let (planned, start) = familiar_ai_rules::lifecycle::place_checks(v, &a.criterion_id)
        .into_iter()
        .find_map(|p| p.planning.filter(|pl| of_this_place(pl)).zip(p.start))?;
    let found = start.t > planned.t
        && start.fields.get("matched") == Some(&Value::Bool(true))
        && text(start, "file_path").as_deref() == Some(a.file_path.as_str())
        && text(start, "lines_hash").is_some_and(|h| !h.is_empty() && Some(&h) == text(planned, "lines_hash").as_ref());
    let (sha, line_start, line_end) = (text(start, "sha")?, num(start, "line_start")?, num(start, "line_end")?);
    (found && !sha.is_empty() && line_start > 0 && line_end >= line_start).then_some((sha, line_start, line_end))
}
