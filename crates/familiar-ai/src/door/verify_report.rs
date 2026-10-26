//! Write the report `verify` leaves for CI to upload, `.familiar/verify-report.json`,
//! at every exit, and the job's summary when `GITHUB_STEP_SUMMARY` names a
//! file. The report is written even when the record cannot be.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::door::project::find_project;
use crate::door::{setup, FAMILIAR_FOLDER, PLAN_FILE};
use crate::teach::verify::Row;
use crate::teach::words::LEAD;

pub const REPORT_FILE: &str = "verify-report.json";

/// What GitHub's CI variables say about the run; each field is `None`
/// outside CI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ci {
    pub run_id: Option<String>,
    pub sha: Option<String>,
    pub runner: Option<String>,
    pub summary: Option<std::path::PathBuf>,
    /// The signers whose approvals CI trusts (`FAMILIAR_AI_ALLOWED_SIGNERS`,
    /// a repository secret or variable mapped into the job), never a file in
    /// the tree; see door/approval.rs.
    pub allowed_signers: Option<String>,
}

impl Ci {
    /// Read GitHub's variables; `verify` is the one command that runs in CI,
    /// and only its report reads them.
    pub fn from_env() -> Ci {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Ci {
            run_id: var("GITHUB_RUN_ID"),
            sha: var("GITHUB_SHA"),
            runner: var("RUNNER_NAME"),
            summary: var("GITHUB_STEP_SUMMARY").map(std::path::PathBuf::from),
            allowed_signers: var(crate::door::approval::SIGNERS_VAR),
        }
    }
}

/// The evidence for a criterion a hosted run proved; any other row's evidence
/// is its own line without the lead.
pub fn hosted_label(sha: &str, run_id: &str) -> String {
    format!("hosted CI-proven at {}, run {run_id}", sha.chars().take(7).collect::<String>())
}

/// The rows when the criteria could be read, else the one line saying why
/// nothing could be proven.
pub enum Came<'a> {
    Rows(&'a [Row]),
    CouldNot(&'a str),
}

/// Write the report, and the summary in CI, for a run in `start` that exits
/// `exit`. `Err` is the one line saying why the report cannot be trusted: it
/// could not be written, or git tracks its path (a committed report would be
/// uploaded as this run's); verify then exits 2, and a tracked path's report
/// is still overwritten with exit 2. Outside a repository nothing is written.
pub fn write(start: &Path, now: f64, ci: &Ci, came: &Came, exit: i32) -> Result<(), String> {
    let worktree = familiar_ai_git::worktree_root(start).ok().flatten();
    let head = worktree.as_deref().and_then(|w| familiar_ai_git::head_commit(w).ok().flatten());
    let sha = ci.sha.clone().or_else(|| head.clone());
    // Outside any checkout there is nowhere beside a record to write.
    let Some(root) = find_project(start).or_else(|| worktree.clone()) else {
        return Ok(());
    };
    let plan_rel = worktree.as_deref().and_then(|w| setup::paths_in_repository(&root, w).ok()).map(|(_, plan)| plan).unwrap_or_else(|| format!("{FAMILIAR_FOLDER}/{PLAN_FILE}"));
    let report_rel = format!("{}{REPORT_FILE}", plan_rel.strip_suffix(PLAN_FILE).unwrap_or(&format!("{FAMILIAR_FOLDER}/")));
    let tracked = match (&worktree, &head) {
        (Some(w), Some(h)) => familiar_ai_git::file_at(w, h, &report_rel).is_ok(),
        _ => false,
    };
    let exit = if tracked { 2 } else { exit };
    let plan_hash = match (&worktree, &head) {
        (Some(w), Some(h)) => familiar_ai_git::file_at(w, h, &plan_rel).ok().map(|b| hex(&Sha256::digest(&b))),
        _ => None,
    };
    let evidence = |row: &Row| match (&ci.run_id, &sha) {
        (Some(id), Some(sha)) if row.outcome == "proven" => hosted_label(sha, id),
        _ => without_lead(&row.line),
    };
    let mut report = json!({
        "run": { "id": ci.run_id.as_deref().unwrap_or("local"), "at": now, "runner": ci.runner.as_deref().unwrap_or("local") },
        "sha": sha,
        "plan_hash": plan_hash,
        "criteria": [],
        "exit": exit,
    });
    let mut summary = vec![format!("### familiar-verify: exit {exit} (0 proven, 1 not proven, 2 could not prove)"), String::new()];
    match came {
        Came::Rows(rows) => {
            report["criteria"] = Value::Array(rows.iter().map(|r| json!({ "id": r.id, "text": r.text, "outcome": r.outcome, "evidence": evidence(r) })).collect());
            summary.extend(rows.iter().map(|r| format!("- {}", without_lead(&r.line))));
            if let (Some(id), Some(sha)) = (&ci.run_id, &sha) {
                if rows.iter().any(|r| r.outcome == "proven") {
                    summary.push(String::new());
                    summary.extend(rows.iter().filter(|r| r.outcome == "proven").map(|r| format!("{}: {}", r.id, hosted_label(sha, id))));
                }
            }
        }
        Came::CouldNot(line) => {
            report["could_not"] = Value::from(without_lead(line));
            summary.push(format!("- {}", without_lead(line)));
        }
    }
    let folder = root.join(FAMILIAR_FOLDER);
    let written = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(folder.join(REPORT_FILE), format!("{report:#}\n")));
    if let Some(path) = &ci.summary {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(format!("{}\n", summary.join("\n")).as_bytes());
        }
    }
    if tracked {
        return Err(format!("{report_rel} is tracked by git, so a committed report could be uploaded as this run's"));
    }
    written.map_err(|e| format!("{report_rel} could not be written: {e}"))
}

fn without_lead(line: &str) -> String {
    line.strip_prefix(LEAD).unwrap_or(line).to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
