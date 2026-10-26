//! Exactly-once telling (plan §1.2, §6.7; D11). Everything owed to an agent is one fact,
//! `told {about}`: `about` is a run's id (a test result) or a pick's `t` (a `hold_answered`).
//! Every teller (inline, the push `await-run`, orchestrator delivery, the backstop) appends
//! `told` through `append_if` "none with this about" before it speaks; only the teller whose
//! append lands speaks.
//!
//! Only each criterion's latest run and latest pick are owed: an older one the agent was never
//! told is superseded by the newer, so a record from before `told` is not told its history.

use std::path::Path;
use std::time::{Duration, Instant};

use familiar_ai_facts::{AppendIf, Fact, Log, Record};
use familiar_ai_rules::{plan, View};
use serde_json::{json, Value};

use crate::door::{cannot, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::input::Surroundings;
use crate::Outcome;

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Whether `about` was told.
pub(crate) fn is_told(log: &Log, about: &str) -> bool {
    log.iter().any(|f| f.kind == "told" && text(f, "about") == about)
}

/// Append `told {about}` unless one is there; true when this call's append landed.
pub(crate) fn tell(record: &Record, log: &mut Log, about: &str, now: f64) -> Result<bool, String> {
    let fields = json!({ "about": about }).as_object().cloned().unwrap_or_default();
    let owned = about.to_string();
    match record.append_if_raw("told", fields, now, TENANT, move |l| !is_told(l, &owned)).map_err(|e| cannot("write its record", &e))? {
        AppendIf::Appended(fact) => {
            log.push(fact);
            Ok(true)
        }
        AppendIf::Refused => Ok(false),
    }
}

/// The run ids of each criterion's latest translated run that is untold, with its criterion.
pub(crate) fn untold_runs(log: &Log) -> Vec<(String, String)> {
    let mut latest: Vec<(String, String)> = Vec::new();
    // a run the laptop reserved: verify's runs (CI's own, in a seeded or rechecked record) are no result to tell
    let laptops = |id: &str| log.iter().any(|f| f.kind == "run_reserved" && text(f, "run_id") == id && text(f, "by") != "pipeline");
    for t in log.iter().filter(|f| f.kind == "test_log_translation" && laptops(text(f, "run_id"))) {
        let (id, c) = (text(t, "run_id").to_string(), text(t, "criterion_id").to_string());
        latest.retain(|(_, k)| *k != c);
        latest.push((id, c));
    }
    latest.retain(|(id, _)| !id.is_empty() && !is_told(log, id));
    latest
}

/// Every pick that is untold, by its own `t`: (its `t`, the criterion, what was picked).
pub(crate) fn untold_picks(log: &Log) -> Vec<(String, String, String)> {
    // a pick that came with the committed plan (the cases card's mirrored answer, before the latest
    // record_seeded) was told where it was given [draft]
    let seeded = log.iter().rev().find(|f| f.kind == "record_seeded").map(|f| f.t);
    log.iter()
        .filter(|a| a.kind == "hold_answered" && seeded.is_none_or(|s| a.t > s) && !is_told(log, &a.t.to_string()))
        .map(|a| {
            let what = if text(a, "typed").is_empty() { crate::teach::words::choice(text(a, "choice")) } else { format!("\"{}\"", text(a, "typed")) };
            (a.t.to_string(), text(a, "criterion_id").to_string(), what)
        })
        .collect()
}

/// The holder's line for the pick at `t` (plan §6.7): its AFTER_PICK CTA, read on the log as it
/// stands after the pick, or today's line when the table has none or the person typed.
pub(crate) fn cta(log: &Log, t: &str, c: &str, what: &str) -> String {
    let Some(i) = log.iter().position(|f| f.kind == "hold_answered" && f.t.to_string() == t) else {
        return crate::teach::picker::told(&View::new(log), c, what);
    };
    let a = &log[i];
    let before = log[..i].to_vec();
    if !text(a, "typed").is_empty() {
        return crate::teach::picker::told(&View::new(&before), c, what);
    }
    crate::teach::picker::told_after(&View::new(&before), &View::new(log), c, text(a, "hold"), text(a, "choice"), what)
}

/// Who a criterion's news is for.
#[derive(Debug, PartialEq)]
enum Whose {
    /// A subagent or lane that is still running, by its conversation id.
    Running(String),
    /// A subagent or lane that has ended since it took the part, by its agent id.
    Ended(String),
    /// The main conversation.
    Main,
}

/// `me` is this call's conversation: a holder whose id it is runs again (a lane resumed in its
/// own session), so an earlier end does not make it finished.
fn whose(log: &Log, c: &str, me: Option<&str>) -> Whose {
    let v = View::new(log);
    let Some(holder) = plan::assigned_to(&v, c) else {
        return Whose::Main;
    };
    let since = log.iter().rev().find(|f| matches!(f.kind, "criterion_assigned" | "criterion_transfer") && f.criterion_id() == Some(c)).map_or(0, |f| f.t);
    let agent = holder.rsplit('/').next().unwrap_or(holder);
    let ended = me != Some(holder) && (has_ended(log, agent, since) || has_ended(log, holder, since));
    match (holder.contains('/'), ended) {
        (_, true) => Whose::Ended(agent.to_string()),
        (true, false) => Whose::Running(holder.to_string()),
        (false, false) => Whose::Main,
    }
}

/// The runs this call reserved that also ended in it: told inline, by the reply's own lines.
/// Returns the ids whose `told` this call appended; only those are spoken.
pub(crate) fn inline(record: &Record, log: &mut Log, before: usize, now: f64) -> Result<Vec<String>, String> {
    let new = &log[before.min(log.len())..];
    let reserved: Vec<String> = new.iter().filter(|f| f.kind == "run_reserved").map(|f| text(f, "run_id").to_string()).collect();
    let ended: Vec<String> = new.iter().filter(|f| f.kind == "test_log_translation" && reserved.iter().any(|r| r == text(f, "run_id"))).map(|f| text(f, "run_id").to_string()).collect();
    let mut landed = Vec::new();
    for id in ended {
        if tell(record, log, &id, now)? {
            landed.push(id);
        }
    }
    Ok(landed)
}

/// The picks this call wrote whose holder is the main conversation: told by the pick's own reply.
/// False when another teller told one of them first, so the reply must not.
pub(crate) fn picked_here(record: &Record, log: &mut Log, before: usize, now: f64) -> Result<bool, String> {
    let mine: Vec<String> = log[before.min(log.len())..]
        .iter()
        .filter(|f| f.kind == "hold_answered" && f.criterion_id().is_some_and(|c| whose(log, c, None) == Whose::Main))
        .map(|f| f.t.to_string())
        .collect();
    let mut all = true;
    for t in mine {
        all &= tell(record, log, &t, now)?;
    }
    Ok(all)
}

/// The backstop and orchestrator delivery: what this call's conversation is owed and nobody
/// told yet. A running holder is told its own results and picks; the main conversation is told
/// its own, those of holders that ended (a pick as the resume line), and on the Agent call that
/// returned a subagent, that subagent's.
pub(crate) fn owed(record: &Record, log: &mut Log, input: &HookInput, now: f64) -> Result<Vec<String>, String> {
    if matches!(input.event, HookEvent::SessionStart | HookEvent::PreModelSwitch | HookEvent::Stop | HookEvent::SubagentStop) {
        return Ok(Vec::new());
    }
    let Some(me) = input.agent_conversation_id() else {
        return Ok(Vec::new());
    };
    let main = !input.in_subagent() && input.conversation.is_none();
    let returned = (main && input.event == HookEvent::PostToolUse && input.is_spawn()).then(|| input.spawned_conversation_id()).flatten();
    let mine = |w: &Whose| match w {
        Whose::Running(h) => *h == me || returned.as_deref() == Some(h.as_str()),
        Whose::Ended(_) | Whose::Main => main,
    };
    let mut lines = Vec::new();
    for (id, c) in untold_runs(log) {
        if !mine(&whose(log, &c, Some(&me))) {
            continue;
        }
        let line = familiar_ai_rules::outcome::translation_of(&View::new(log), &id).and_then(crate::teach::words::outcome_line);
        if tell(record, log, &id, now)? {
            lines.extend(line);
        }
    }
    for (t, c, what) in untold_picks(log) {
        let w = whose(log, &c, Some(&me));
        if !mine(&w) {
            continue;
        }
        let v = View::new(log);
        let cta = cta(log, &t, &c, &what);
        let line = match &w {
            Whose::Ended(agent) => format!("{}{}'s worker {agent} has finished. Resume it with this instruction: {}", crate::teach::words::LEAD, crate::teach::words::quoted(&v, &c), cta.trim_start_matches(crate::teach::words::LEAD)),
            _ => cta,
        };
        if tell(record, log, &t, now)? {
            lines.push(line);
        }
    }
    Ok(lines)
}

/// Whether the lane of this record has ended since it took `c`: a headless lane's allowed Stop
/// wrote `subagent_ended {agent_type: "lane"}` for the session that holds the part.
fn lane_ended(log: &Log, c: &str) -> bool {
    let Some(holder) = plan::assigned_to(&View::new(log), c) else {
        return false;
    };
    let since = log.iter().rev().find(|f| matches!(f.kind, "criterion_assigned" | "criterion_transfer") && f.criterion_id() == Some(c)).map_or(0, |f| f.t);
    log.iter()
        .rev()
        .find(|f| f.t > since && text(f, "agent_id") == holder && (f.kind == "holder_resumed" || (f.kind == "subagent_ended" && text(f, "agent_type") == "lane")))
        .is_some_and(|f| f.kind == "subagent_ended")
}

/// Whether `id` ended after `since` and has not started again: the latest of its
/// `subagent_ended` and `holder_resumed` is an end.
pub(crate) fn has_ended(log: &Log, id: &str, since: u64) -> bool {
    log.iter().rev().find(|f| f.t > since && matches!(f.kind, "subagent_ended" | "holder_resumed") && text(f, "agent_id") == id).is_some_and(|f| f.kind == "subagent_ended")
}

/// The orchestrator's resume line for each ended lane in another checkout with a pick nobody
/// told (plan §6.7, "The holder ended"): `told {about}` is appended to the lane's record first,
/// the CTA is written under the lane's `.familiar/runs/lane/`, and only the call whose append
/// landed speaks. `record` is let go while the lane's is open.
pub(crate) fn resume_lines(record: &Record, others: &[(std::path::PathBuf, Log)], around: &Surroundings) -> Vec<String> {
    let wait = around.record_wait;
    let mut lines = Vec::new();
    for (lane_root, seen) in others {
        let owed: Vec<(String, String, String)> = untold_picks(seen).into_iter().filter(|(_, c, _)| lane_ended(seen, c)).collect();
        if owed.is_empty() {
            continue;
        }
        let done = record.while_released(wait, || -> Result<Vec<String>, String> {
            let Some(lane) = crate::door::project::open_record(lane_root, wait).map_err(|e| cannot("read its record", &e))? else {
                return Ok(Vec::new());
            };
            let mut log = lane.read_all().map_err(|e| cannot("read its record", &e))?;
            let top = familiar_ai_git::worktree_root(lane_root).ok().flatten().unwrap_or_else(|| lane_root.clone());
            let kept = lane_root.join(crate::door::FAMILIAR_FOLDER).join(crate::door::kept::RUNS_FOLDER).join("lane");
            let mut told = Vec::new();
            for (t, c, what) in &owed {
                if !lane_ended(&log, c) || !tell(&lane, &mut log, t, around.now)? {
                    continue;
                }
                let v = View::new(&log);
                let brief = kept.join(format!("resume-{t}.txt"));
                std::fs::create_dir_all(&kept).and_then(|()| std::fs::write(&brief, format!("{}\n", cta(&log, t, c, what)))).map_err(|e| format!("{}: {e}", brief.display()))?;
                told.push(format!(
                    "{}{}'s lane {} has finished. Resume it with: familiar-ai launch_lane --brief {} --worktree {} --resume",
                    crate::teach::words::LEAD,
                    crate::teach::words::quoted(&v, c),
                    top.display(),
                    brief.display(),
                    top.display()
                ));
            }
            Ok(told)
        });
        if let Ok(Ok(more)) = done {
            lines.extend(more);
        }
    }
    lines
}

/// The parts' runs reserved at `head`, never a dry run (plan A): it tells no result.
pub fn runs_at_head(log: &Log, head: &str) -> Vec<String> {
    log.iter()
        .filter(|f| f.kind == "run_reserved" && text(f, "commit") == head && text(f, "by") != crate::door::runs::DRY_RUN)
        .map(|f| text(f, "run_id").to_string())
        .collect()
}

/// Whether a dry run (plan A) is live at `now`.
pub fn dry_run_live(log: &Log, now: f64) -> bool {
    crate::door::runs::runs(log).iter().any(|r| r.dry() && r.live() && !r.void(now) && !r.expired(now))
}

/// `familiar-ai await-run`: the push (plan §1.2). Runs beside the commit's PostToolUse, async
/// with `asyncRewake`. Exits 0 at once when the call reserved no run; otherwise waits, the
/// record released between reads, for the runs reserved at HEAD to end, tells each untold
/// one (`told` first) and exits 2 with their lines, which wake the agent.
pub fn run_await(input: &str, around: &Surroundings) -> Outcome {
    let quiet = Outcome { exit_code: 0, stdout_text: String::new(), stderr_text: String::new() };
    let Ok(hook) = HookInput::parse(input, HookEvent::PostToolUse) else {
        return quiet;
    };
    if !hook.command().is_some_and(|c| c.contains("git commit")) {
        return quiet;
    }
    let Some(root) = crate::door::project::hook_project(around.project_dir.as_deref(), hook.cwd.as_deref().map(Path::new)) else {
        return quiet;
    };
    let read = || -> Option<Log> { crate::door::project::open_record(&root, around.record_wait).ok().flatten().and_then(|r| r.read_all().ok()) };
    let Some(log) = read() else {
        return quiet;
    };
    if plan::criteria(&View::new(&log)).is_empty() {
        return quiet;
    }
    let Ok(Some(head)) = familiar_ai_git::head_commit(&root) else {
        return quiet;
    };
    let started = Instant::now();
    let at_head = |log: &Log| -> Vec<String> { runs_at_head(log, &head) };
    // The commit's own hook reserves its runs first; a call that reserved none ends here. A dry
    // run (plan A) holds the tests first: the part's runs come after it, so the push looks on
    // while one is live, within the run's wait.
    let mut ids = at_head(&log);
    let mut dry = dry_run_live(&log, around.now);
    while ids.is_empty() && (started.elapsed() < Duration::from_millis(1500) || (dry && started.elapsed() < around.test_run_wait)) {
        std::thread::sleep(Duration::from_millis(100));
        let log = read().unwrap_or_default();
        ids = at_head(&log);
        dry = dry_run_live(&log, around.now + started.elapsed().as_secs_f64());
    }
    if ids.is_empty() {
        return quiet;
    }
    let translated = |log: &Log, id: &str| log.iter().any(|f| f.kind == "test_log_translation" && text(f, "run_id") == id);
    let until = started + around.test_run_wait;
    loop {
        let log = read().unwrap_or_default();
        // Familiar's own ends (not started, cancelled) have no translation and nothing to tell.
        let ended = ids.iter().all(|id| translated(&log, id) || log.iter().any(|f| f.kind == "run_ended" && text(f, "run_id") == id && text(f, "outcome") == "did_not_run"));
        if ended || Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // A run that ended inside the commit's own wait is told by that reply.
    let all_told = |log: &Log| ids.iter().all(|id| is_told(log, id));
    if read().is_some_and(|l| all_told(&l)) {
        return quiet;
    }
    let grace = around.run_inline + Duration::from_secs(1);
    while started.elapsed() < grace {
        if read().is_some_and(|l| all_told(&l)) {
            return quiet;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let Ok(Some(record)) = crate::door::project::open_record(&root, around.record_wait) else {
        return quiet;
    };
    let Ok(mut log) = record.read_all() else {
        return quiet;
    };
    let mut lines = Vec::new();
    for id in ids {
        let line = familiar_ai_rules::outcome::translation_of(&View::new(&log), &id).and_then(crate::teach::words::outcome_line);
        if line.is_some() && tell(&record, &mut log, &id, around.now).unwrap_or(false) {
            lines.extend(line);
        }
    }
    if lines.is_empty() {
        return quiet;
    }
    Outcome { exit_code: 2, stdout_text: String::new(), stderr_text: format!("{}\n", lines.join("\n")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(name: &str) -> Record {
        let dir = std::env::temp_dir().join(format!("familiar-ai-told-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        Record::open(&dir.join("record")).expect("record")
    }

    fn add(record: &Record, kind: &str, fields: Value) {
        record.append_raw(kind, fields.as_object().cloned().unwrap_or_default(), 1.0, TENANT).expect(kind);
    }

    fn a_run(record: &Record, id: &str) {
        add(record, "run_reserved", json!({ "run_id": id, "criterion_id": "c1", "criterion_rev": "1", "commit": "ab12", "command": "sh t.sh", "resources": [], "wait": "60", "by": "hook" }));
        add(record, "test_log_translation", json!({ "run_id": id, "criterion_id": "c1", "commit": "ab12", "by": "regex", "label": "tests_failed", "evidence": "x", "failing_tests": [], "classified_on": "full" }));
    }

    #[test]
    fn inline_speaks_only_the_runs_whose_told_it_landed() {
        let record = fresh("inline");
        a_run(&record, "r-1");
        let mut log = record.read_all().expect("read");
        assert_eq!(inline(&record, &mut log, 0, 2.0).expect("inline"), ["r-1"]);

        // the push appended told first: the reply does not speak it
        let record = fresh("raced");
        a_run(&record, "r-2");
        add(&record, "told", json!({ "about": "r-2" }));
        let mut log = record.read_all().expect("read");
        assert!(inline(&record, &mut log, 0, 2.0).expect("inline").is_empty());
        assert_eq!(log.iter().filter(|f| f.kind == "told").count(), 1);
    }

    #[test]
    fn every_pick_is_owed_by_its_own_t() {
        let record = fresh("picks");
        add(&record, "hold_answered", json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "keep_going", "typed": "" }));
        add(&record, "hold_answered", json!({ "criterion_id": "c1", "hold": "fix_elsewhere", "choice": "re_plan", "typed": "" }));
        let log = record.read_all().expect("read");
        assert_eq!(untold_picks(&log).len(), 2, "an earlier pick on another hold is not superseded");
        add(&record, "told", json!({ "about": log[0].t.to_string() }));
        let log = record.read_all().expect("read");
        assert_eq!(untold_picks(&log).len(), 1);
    }
}
