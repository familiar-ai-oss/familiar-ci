//! c_dep's draft (plan §7.3; D13–D18, D21): on the person's `a_dependency`, the binary asks the
//! holder's model once, with the record let go, for the dependency's words, its test file and,
//! when the crash names none, its place; then appends `dependency_drafted` and, when the whole
//! plan holds with them, the dependency's lines in one batch. The backstop, run as the record is
//! caught up, rewrites a draft whose lines are missing and drafts an approval with no draft again;
//! two misses end the approval and its card opens again. See docs/decisions.md.

use std::path::Path;
use std::time::Duration;

use familiar_ai_facts::{AppendIf, DependencyDrafted, DependencyUnanswered, Fact, Log, Place, Record, TestLogTranslation};
use familiar_ai_rules::dependency::{self as rules, PLAN_INCOMPLETE};
use familiar_ai_rules::{ladder, plan, View};
use serde_json::{json, Map, Value};

use crate::door::child::{self, Ran};
use crate::door::{cannot, FAMILIAR_FOLDER, JUDGE_LOG, TENANT};
use crate::input::Surroundings;

/// The one-shot's answer: the dependency's words, its test file, and its place when it names one.
pub const SCHEMA: &str = r#"{"type":"object","properties":{"text":{"type":"string"},"test_file":{"type":"string"},"place":{"type":["object","null"],"properties":{"file_path":{"type":"string"},"symbol":{"type":"string"},"line_start":{"type":"integer"},"line_end":{"type":"integer"}},"required":["file_path","symbol","line_start","line_end"],"additionalProperties":false}},"required":["text","test_file","place"],"additionalProperties":false}"#;

/// The model asked when the holder's is not known [draft: the owner's word owed].
pub const UNKNOWN_HOLDER: &str = "opus";

/// The judge's lean flags (D15) with this schema: no settings, MCP servers or tools, 50 cents.
pub fn args(alias: &str) -> Vec<String> {
    let mut lean: Vec<&str> = crate::door::judge::LEAN.to_vec();
    if let Some(i) = lean.iter().position(|a| *a == "--json-schema") {
        lean[i + 1] = SCHEMA;
    }
    ["-p", "--model", alias].into_iter().chain(lean).map(str::to_string).collect()
}

/// What the one-shot answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub text: String,
    pub test_file: String,
    pub place: Option<Place>,
}

fn text<'a>(f: &'a Fact, k: &str) -> Option<&'a str> {
    f.fields.get(k).and_then(Value::as_str)
}

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

/// The holder's model alias (D15), read from its `model_seen`.
pub fn alias(v: &View, c: &str) -> &'static str {
    ladder::rung_of(v, plan::assigned_to(v, c)).unwrap_or(UNKNOWN_HOLDER)
}

/// Read the answer: `structured_output`, else the `result` text as one JSON object.
pub fn read(ran: &Ran) -> Result<Draft, String> {
    let Some(out) = ran.answer() else {
        return Err(format!("the one-shot gave no answer: {}", ran.how()));
    };
    let outer = match serde_json::from_str::<Value>(out.trim()) {
        Ok(Value::Array(xs)) => xs.into_iter().rev().find(|x| x.get("type").and_then(Value::as_str) == Some("result")),
        Ok(one) => Some(one),
        Err(_) => None,
    }
    .ok_or("the one-shot's answer was not JSON")?;
    if outer.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err("the one-shot reported an error".to_string());
    }
    let answer = match outer.get("structured_output").filter(|s| s.is_object()) {
        Some(s) => s.clone(),
        None => {
            let t = outer.get("result").and_then(Value::as_str).unwrap_or("").trim();
            let t = t.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
            serde_json::from_str(t).map_err(|_| "the one-shot's answer was not the one JSON object".to_string())?
        }
    };
    let s = |k: &str| answer.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
    let (text, test_file) = (s("text"), s("test_file"));
    if text.is_empty() || test_file.is_empty() {
        return Err("the one-shot's answer had no text or no test file".to_string());
    }
    let place = answer.get("place").filter(|p| p.is_object()).and_then(|p| serde_json::from_value::<Place>(p.clone()).ok());
    Ok(Draft { text, test_file, place })
}

/// The prompt: the person's words, the part, what its run said, where its code and test are.
pub fn prompt(log: &Log, c: &str, hold: &str) -> String {
    let v = View::new(log);
    let crit = v.of("criterion", c).last().and_then(|f| text(f, "text")).unwrap_or("").to_string();
    let run = familiar_ai_rules::outcome::card_run(&v, c, hold);
    let said = crate::teach::person::words_and_picks(log).join("\n");
    let tests = familiar_ai_rules::criterion::test_files(&v, c).join(", ");
    let place = plan::anchor_of(&v, c)
        .map(|a| plan::places_of(a).iter().map(|p| format!("{} {}", p.file_path, p.symbol)).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    let evidence = run.and_then(|t| text(t, "evidence")).unwrap_or("");
    let label = run.and_then(|t| text(t, "label")).unwrap_or(hold);
    let location = run.and_then(|t| t.fields.get("location")).map(Value::to_string).unwrap_or_default();
    format!(
        "A part of the work cannot go on: something it depends on must be built first. The person approved planning that dependency as a part of its own, to be done before this one.\n\n\
         The person asked:\n{said}\n\nThe part: {crit}\nIts code: {place}\nIts tests: {tests}\nIts last run read {label}: {evidence}\nThe run points at: {location}\n\n\
         Answer with the dependency as one JSON object: \"text\", one sentence of what the dependency must do so the part can go on; \"test_file\", the path of a new test file for it, relative to the project's root; \"place\", the existing function the dependency's fix goes in ({{\"file_path\", \"symbol\", \"line_start\", \"line_end\"}}), or null when you cannot tell."
    )
}

/// The one-shot's test file must be a relative path inside the project, under no `.venv` or
/// `node_modules`, and no other part's planned test (S9c review 3); otherwise the answer is a miss.
pub fn test_file_ok(log: &Log, f: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("the one-shot's test file {f} {why}"));
    let parts: Vec<&str> = f.split(['/', '\\']).collect();
    if f.starts_with('/') || f.starts_with('\\') || f.contains(':') || parts.contains(&"..") {
        return bad("is not a relative path inside the project");
    }
    if parts.iter().any(|p| matches!(*p, ".venv" | "node_modules")) {
        return bad("is under .venv or node_modules");
    }
    let norm = |s: &str| s.trim_start_matches("./").to_string();
    if log.iter().any(|t| t.kind == "test_planned" && text(t, "file_path").map(norm) == Some(norm(f))) {
        return bad("is another part's planned test");
    }
    Ok(())
}

/// The next free criterion id: `c<n>` past every numbered one.
pub fn next_id(v: &View) -> String {
    let cs = plan::criteria(v);
    let top = cs.iter().filter_map(|c| c.strip_prefix('c').and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
    (top + 1..).map(|n| format!("c{n}")).find(|id| !cs.contains(id)).unwrap_or_else(|| "c_dep".to_string())
}

/// Line 4's place (plan §7.3): the crash's own place when the run names one in the project, else
/// the one-shot's.
fn place_for(log: &Log, root: &Path, c: &str, hold: &str, draft: &DependencyDrafted) -> Option<Place> {
    let v = View::new(log);
    let from_run = familiar_ai_rules::outcome::card_run(&v, c, hold).and_then(|t| {
        let tr: TestLogTranslation = serde_json::from_value(Value::Object(t.fields.clone())).ok()?;
        let loc = tr.location.as_ref()?;
        let bytes = familiar_ai_git::file_at(root, &tr.commit, &loc.file_path).ok()?;
        crate::door::kept::place_candidate(&tr, &bytes)
    });
    from_run.or_else(|| draft.place.clone())
}

/// The dependency's lines for the draft (plan §7.3, lines 2, 4, 6, 9, 10, 11), in batch order.
fn lines(log: &Log, root: &Path, approval: &Fact, draft: &DependencyDrafted) -> Vec<(&'static str, Map<String, Value>)> {
    let v = View::new(log);
    let (c, d) = (approval.criterion_id().unwrap_or(""), draft.criterion_id.as_str());
    let hold = text(approval, "hold").unwrap_or("");
    let mut crit = fields(json!({ "criterion_id": d, "text": draft.text }));
    if let Some(src) = v.of("criterion", c).last() {
        for k in ["path", "behavior_id", "promise_id", "for"] {
            if let Some(x) = src.fields.get(k) {
                crit.insert(k.into(), x.clone());
            }
        }
    }
    let mut out: Vec<(&'static str, Map<String, Value>)> = vec![("criterion", crit)];
    if let Some(p) = place_for(log, root, c, hold, draft) {
        out.push(("place_chosen", fields(json!({ "criterion_id": d, "places": [p] }))));
    }
    out.push(("test_planned", fields(json!({ "criterion_id": d, "file_path": draft.test_file, "will": "create" }))));
    out.extend(rules::restring(&v, d, c));
    if let Some(h) = plan::assigned_to(&v, c) {
        out.push(("criterion_assigned", fields(json!({ "criterion_id": d, "agent_conversation_id": h }))));
    }
    let category = if hold == "environment_error" { "environment" } else { "this_repository" };
    out.push(("fix_location_category", fields(json!({ "criterion_id": d, "category": category, "where": "" }))));
    out
}

/// One fact to write: its kind and fields.
type Line = (String, Map<String, Value>);

/// Step 6: the lines and their planning checks applied to the log in memory; `Ok` with every fact
/// to write when the whole plan holds, else why not.
fn validated(log: &Log, root: &Path, approval: &Fact, draft: &DependencyDrafted, at: f64) -> Result<Vec<Line>, String> {
    let mut mem = log.clone();
    let mut out: Vec<Line> = Vec::new();
    for (kind, f) in lines(log, root, approval, draft) {
        mem = familiar_ai_facts::append_raw(&mem, kind, f.clone(), at, TENANT).map_err(|e| format!("{PLAN_INCOMPLETE}: {kind}: {e}"))?;
        out.push((kind.to_string(), f));
        if kind == "place_chosen" {
            // line 5: the planning checks, as anchor::binary_checks writes them
            let sha = mem.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| text(f, "commit")).unwrap_or("").to_string();
            while let Some(check) = crate::door::anchor::next_check(&mem).filter(|k| k.criterion_id == draft.criterion_id && k.checked_at == familiar_ai_facts::CheckedAt::Planning) {
                let fact = crate::door::anchor::look(root, &sha, &check, None);
                let f = familiar_ai_facts::FactFields::to_fields(&fact).map_err(|e| format!("{PLAN_INCOMPLETE}: {e}"))?;
                mem = familiar_ai_facts::append_raw(&mem, "anchor_sha_matched", f.clone(), at, TENANT).map_err(|e| format!("{PLAN_INCOMPLETE}: {e}"))?;
                out.push(("anchor_sha_matched".to_string(), f));
            }
        }
    }
    let v = View::new(&mem);
    let failing: Vec<&str> = plan::PLAN_LINES.iter().copied().filter(|n| !plan::line(&v, n).unwrap_or(true)).collect();
    if !failing.is_empty() {
        return Err(format!("{PLAN_INCOMPLETE}: {}", failing.join(", ")));
    }
    if !rules::keeps_string(&View::new(log), &v) {
        return Err(format!("{PLAN_INCOMPLETE}: the insert leaves a part off the string"));
    }
    Ok(out)
}

fn miss(record: &Record, log: &mut Log, approval: u64, why: String, at: f64) -> Result<(), String> {
    let f = DependencyUnanswered { approval, why };
    log.push(record.append(&f, at, TENANT).map_err(|e| cannot("write its record", &e))?);
    Ok(())
}

/// Steps 6 and on for a draft in the record: the lines as one batch when the plan holds with
/// them, else `dependency_unanswered` (and the environment_error card reads it).
fn write_lines(record: &Record, root: &Path, log: &mut Log, approval: &Fact, at: f64) -> Result<(), String> {
    let v = View::new(log);
    let Some(d) = rules::draft_of(&v, approval.t).cloned() else {
        return Ok(());
    };
    let Ok(draft) = serde_json::from_value::<DependencyDrafted>(Value::Object(d.fields.clone())) else {
        return miss(record, log, approval.t, format!("{PLAN_INCOMPLETE}: the draft cannot be read"), at);
    };
    match validated(log, root, approval, &draft, at) {
        Ok(facts) => {
            let batch: Vec<(&str, Map<String, Value>, f64)> = facts.iter().map(|(k, f)| (k.as_str(), f.clone(), at)).collect();
            let written = record.append_all(&batch, TENANT).map_err(|e| cannot("write its record", &e))?;
            log.extend(written);
            crate::door::anchor::binary_checks(record, root, log, at, TENANT).map_err(|e| cannot("write its record", &e))?;
            Ok(())
        }
        Err(why) => miss(record, log, approval.t, why, at),
    }
}

/// Steps 2–6 for the approval `a`: the one-shot with the record let go, then the draft and its
/// lines with the record held again. A miss is written as `dependency_unanswered`.
fn draft_once(record: &Record, root: &Path, log: &mut Log, a: &Fact, around: &Surroundings) -> Result<(), String> {
    let c = a.criterion_id().unwrap_or("").to_string();
    let hold = text(a, "hold").unwrap_or("").to_string();
    let Some(dir) = around.judge_dir.clone() else {
        // without `claude` (or with the judge off), as the judge: a miss
        return miss(record, log, a.t, "no claude to ask".to_string(), around.now);
    };
    let v = View::new(log);
    let (alias, ask) = (alias(&v, &c), prompt(log, &c, &hold));
    let argv = args(alias);
    let ran = record
        .while_released(around.record_wait, || {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return Ran { ended: child::Ended::NotStarted(format!("{}: {e}", dir.display())), wall: Duration::ZERO, stdout: String::new(), stderr: String::new() };
            }
            child::run(crate::door::judge::PROGRAM, &argv, &dir, &[("FAMILIAR_AI_JUDGE", "off")], &ask, around.judge_wait)
        })
        .map_err(|e| cannot("reopen its record", &e))?;
    let path = root.join(FAMILIAR_FOLDER).join(JUDGE_LOG);
    let shown: Vec<String> = std::iter::once(crate::door::judge::PROGRAM.to_string()).chain(argv.iter().cloned()).collect();
    child::log(&path, &format!("at {:.0} · dependency for {c} · {}", around.now, child::shown(&shown)), &ran, "");
    *log = record.read_all().map_err(|e| cannot("read its record", &e))?;
    let answer = match read(&ran) {
        Ok(x) => x,
        Err(why) => return miss(record, log, a.t, why, around.now),
    };
    if let Err(why) = test_file_ok(log, &answer.test_file) {
        return miss(record, log, a.t, why, around.now);
    }
    // the new part's id is read with the record held again, as the draft is written (S9c review 2)
    let d = next_id(&View::new(log));
    let fact = DependencyDrafted { approval: a.t, criterion_id: d.clone(), text: answer.text, test_file: answer.test_file, place: answer.place };
    let t = a.t;
    let none_yet = move |l: &Log| {
        let v = View::new(l);
        rules::draft_of(&v, t).is_none() && rules::misses(&v, t).len() < rules::TWICE && !plan::criteria(&v).contains(&d)
    };
    match record.append_if(&fact, around.now, TENANT, none_yet).map_err(|e| cannot("write its record", &e))? {
        AppendIf::Appended(f) => log.push(f),
        AppendIf::Refused => return Ok(()), // another call drafted it first
    }
    write_lines(record, root, log, a, around.now)
}

/// After the person's pick: when it is an approval, draft it now (plan §7.3, steps 2–6).
pub(crate) fn after_pick(record: &Record, root: &Path, log: &mut Log, around: &Surroundings) -> Result<(), String> {
    let Some(a) = log.last().filter(|f| rules::is_approval(f)).cloned() else {
        return Ok(());
    };
    draft_once(record, root, log, &a, around)
}

/// How long the asking call may still be drafting an approval it has not missed: the one-shot's
/// wait and a margin (D16).
pub fn drafting_for(around: &Surroundings) -> Duration {
    around.judge_wait + Duration::from_secs(30)
}

/// The approval whose draft another call is writing now (D16): no draft, no miss, and younger
/// than the one-shot's wait. Other conversations are told to wait.
pub fn in_flight(log: &Log, now: f64, wait: Duration) -> Option<&Fact> {
    let v = View::new(log);
    let all: Vec<&Fact> = log.iter().filter(|f| rules::is_approval(f)).collect();
    all.into_iter().find(|a| {
        rules::draft_of(&v, a.t).is_none() && rules::misses(&v, a.t).is_empty() && !rules::ended(&v, a) && now - a.at < wait.as_secs_f64()
    })
}

/// The backstop (plan §7.3), as the record is caught up: a draft whose lines are missing is
/// written again (not while its environment_error card stands); an approval with no draft and
/// fewer than two misses is drafted again, unless its asking call is still drafting it.
pub(crate) fn backstop(record: &Record, root: &Path, log: &mut Log, around: &Surroundings) -> Result<(), String> {
    let approvals: Vec<Fact> = {
        let v = View::new(log);
        plan::criteria(&v).iter().flat_map(|c| rules::approvals(&v, c)).cloned().collect()
    };
    for a in approvals {
        let v = View::new(log);
        let c = a.criterion_id().unwrap_or("").to_string();
        if rules::ended(&v, &a) || rules::landed(&v, a.t) || familiar_ai_rules::lifecycle::released(&v, &c) {
            continue;
        }
        if rules::draft_of(&v, a.t).is_some() {
            if rules::lines_owed(&v, &a) {
                write_lines(record, root, log, &a, around.now)?;
            }
        } else if in_flight(log, around.now, drafting_for(around)).is_none_or(|f| f.t != a.t) {
            draft_once(record, root, log, &a, around)?;
        }
    }
    Ok(())
}
