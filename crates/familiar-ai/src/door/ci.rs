//! `familiar-ai ci fetch [--sha <sha>]`: ask GitHub through `gh` what hosted CI
//! said about a commit and write one `ci_report_seen` fact per new completed
//! run. `status` never runs this; it reads what this wrote. See
//! docs/decisions.md#ci-proves-the-laptop-guides

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai_facts::CiReportSeen;
use serde_json::Value;

use crate::Outcome;
use crate::door::child::{self, Ended};
use crate::door::project::{open_project, plain};
use crate::door::{cannot, TENANT};
use crate::input::Surroundings;
use crate::teach::prove::{FAMILIAR_VERIFY, GH};
use crate::teach::words::LEAD;

/// The artifact name verify uploads its report under, as in action/action.yml.
pub const REPORT_ARTIFACT: &str = "familiar-verify-report";
/// How long one `gh` call of the fetch may take; longer than `status` waits,
/// since a download is slower.
pub const FETCH_WAIT: Duration = Duration::from_secs(60);

/// One completed run from `gh run list`.
#[derive(Debug, Clone, PartialEq)]
struct Finished {
    id: String,
    conclusion: String,
    updated_at: String,
}

pub fn run_fetch(sha: Option<&str>, around: &Surroundings) -> Outcome {
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let could_not = |why: &str| plain(1, String::new(), format!("{LEAD}CI: could not ask GitHub ({why})\n"));
    let sha = match sha.map(str::to_string).or_else(|| head(&root)) {
        Some(s) => s,
        None => return could_not("the repository has no commit"),
    };
    let log = match record.read_all() {
        Ok(l) => l,
        Err(e) => return plain(1, String::new(), format!("{LEAD}{}\n", cannot("read its record", &e))),
    };
    let known = |log: &familiar_ai_facts::Log, id: &str| log.iter().any(|f| f.kind == "ci_report_seen" && f.fields.get("run_id").and_then(Value::as_str) == Some(id));
    // Ask GitHub with the record released, so no hook waits on the network.
    let asked = record.while_released(around.record_wait, || ask(&root, &sha, |id| known(&log, id)));
    let seen = match asked {
        Ok(Ok(seen)) => seen,
        Ok(Err(why)) => return could_not(&why),
        Err(e) => return plain(1, String::new(), format!("{LEAD}{}\n", cannot("reopen its record", &e))),
    };
    let mut log = match record.read_all() {
        Ok(l) => l,
        Err(e) => return plain(1, String::new(), format!("{LEAD}{}\n", cannot("read its record", &e))),
    };
    for f in seen {
        if known(&log, &f.run_id) {
            continue;
        }
        match record.append(&f, around.now, TENANT) {
            Ok(fact) => log.push(fact),
            Err(e) => return plain(1, String::new(), format!("{LEAD}{}\n", cannot("write its record", &e))),
        }
    }
    plain(0, format!("{LEAD}{}\n", crate::teach::ci::line(&log, &sha)), String::new())
}

fn head(root: &Path) -> Option<String> {
    let worktree = familiar_ai_git::worktree_root(root).ok().flatten()?;
    familiar_ai_git::head_commit(&worktree).ok().flatten()
}

fn gh(root: &Path, args: &[&str]) -> Result<String, String> {
    let ran = child::run(GH, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), root, &[], "", FETCH_WAIT);
    match &ran.ended {
        Ended::Exited(Some(0)) => Ok(ran.stdout),
        _ => {
            let said = ran.stderr.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string);
            Err(said.map_or_else(|| format!("gh {}: {}", args.first().copied().unwrap_or(""), ran.how()), |s| s.to_string()))
        }
    }
}

/// The completed runs of `sha` the record has not seen, oldest first.
fn ask(root: &Path, sha: &str, known: impl Fn(&str) -> bool) -> Result<Vec<CiReportSeen>, String> {
    let out = gh(root, &["run", "list", "--commit", sha, "--workflow", FAMILIAR_VERIFY, "--json", "databaseId,status,conclusion,updatedAt"])?;
    let mut runs = finished(&out)?;
    runs.retain(|r| !known(&r.id));
    runs.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
    Ok(runs.into_iter().map(|r| read(root, sha, r)).collect())
}

/// The completed runs in `gh run list --json` output.
fn finished(out: &str) -> Result<Vec<Finished>, String> {
    let v: Value = serde_json::from_str(out.trim()).map_err(|e| format!("gh run list said something unreadable: {e}"))?;
    let runs = v.as_array().ok_or("gh run list did not give a list")?;
    let s = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(runs
        .iter()
        .filter(|r| s(r, "status") == "completed")
        .filter_map(|r| {
            let id = match r.get("databaseId")? {
                Value::Number(n) => n.to_string(),
                Value::String(t) => t.clone(),
                _ => return None,
            };
            Some(Finished { id, conclusion: s(r, "conclusion"), updated_at: s(r, "updatedAt") })
        })
        .collect())
}

/// The fact for one run: its outcome from the run's conclusion, never from a
/// report, and the evidence lines of its uploaded report only when that report
/// names this sha and this run and agrees with the conclusion. A report a
/// commit carried in (a forged one) is dropped, and a downloaded report that
/// disagrees with a green run makes it not proven.
fn read(root: &Path, sha: &str, run: Finished) -> CiReportSeen {
    let outcome = match run.conclusion.as_str() {
        "cancelled" | "timed_out" => run.conclusion.as_str(),
        "success" => "proven",
        _ => "not_proven",
    };
    let (outcome, report) = match outcome {
        "cancelled" | "timed_out" => (outcome, Vec::new()),
        _ => match downloaded(root, &run.id).map(|r| agreeing(&r, sha, &run.id, outcome == "proven")) {
            None => (outcome, Vec::new()),
            Some(Some(lines)) => (outcome, lines),
            // a report that disagrees with its run turns a green run down
            // (verify masked by continue-on-error, or a report verify did not write)
            Some(None) => ("not_proven", Vec::new()),
        },
    };
    CiReportSeen { sha: sha.to_string(), run_id: run.id.clone(), finished_at: run.updated_at.clone(), outcome: outcome.to_string(), report }
}

/// Download a run's report.
fn downloaded(root: &Path, id: &str) -> Option<Value> {
    let dir: PathBuf = std::env::temp_dir().join(format!("familiar-ai-ci-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let got = gh(root, &["run", "download", id, "-n", REPORT_ARTIFACT, "-D", &dir.to_string_lossy()])
        .ok()
        .and_then(|_| std::fs::read_to_string(dir.join(crate::door::verify_report::REPORT_FILE)).ok());
    let _ = std::fs::remove_dir_all(&dir);
    serde_json::from_str(&got?).ok()
}

/// The report's evidence lines when it names `sha` and run `id` and its exit
/// agrees with the run's conclusion (0 exactly when the run succeeded).
fn agreeing(report: &Value, sha: &str, id: &str, succeeded: bool) -> Option<Vec<String>> {
    let run_id = match report.get("run").and_then(|r| r.get("id"))? {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let exit = report.get("exit").and_then(Value::as_i64)?;
    if report.get("sha").and_then(Value::as_str) != Some(sha) || run_id != id || (exit == 0) != succeeded {
        return None;
    }
    let mut lines: Vec<String> = report
        .get("criteria")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("evidence").and_then(Value::as_str).map(str::to_string))
        .collect();
    if lines.is_empty() {
        lines.extend(report.get("could_not").and_then(Value::as_str).map(str::to_string));
    }
    Some(lines)
}
