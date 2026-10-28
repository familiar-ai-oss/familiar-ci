//! What the agent and the person are told: the discipline, the door's forms, the whole picture, the cards, and the person's words.

pub mod card;
pub mod lens;
pub mod cases_card;
pub mod ci;
pub mod discipline;
pub mod example;
pub mod existing;
pub mod form;
pub mod full_path;
pub mod investigator;
pub mod judge;
pub mod person;
pub mod person_words;
pub mod picker;
pub mod precedents;
pub mod prove;
pub mod review;
pub mod sealed_test;
pub mod untouched;
pub mod verify;
pub mod words;

use std::path::Path;

use familiar_ai_facts::Log;
use familiar_ai_rules::{terminal, View};

use crate::door::switch::cleared;

/// The state lines for the facts a call appended, in the person's words, then
/// the first failed try since the seal when this call's run made it, then the
/// cards this call opened.
pub(crate) fn state_lines(before: &Log, after: &Log, judge_on: bool, root: &Path) -> Vec<String> {
    use familiar_ai_rules::{lifecycle, plan};
    let mut lines = words::state_lines(before, after, judge_on);
    let v = View::new(after);
    for c in plan::criteria(&v) {
        let tries = crate::door::investigator_brief::failed_tries(after, &c);
        // The second try holds two_failures, whose words already show.
        let first = tries.len() > crate::door::investigator_brief::failed_tries(before, &c).len() && lifecycle::attempts(&v, &c) == 1 && lifecycle::state(&v, &c) == "sealed";
        if let (true, Some(commit)) = (first, tries.last()) {
            lines.extend(words::failed_try(after, &c, crate::door::kept::kept_run(root, commit).as_deref()));
        }
    }
    lines.extend(opened_cards(before, after));
    lines.extend(started_lines(before, after));
    lines
}

/// The record's latest laptop choice per (root, runner) (plan A), sorted by both: CI's own
/// (`ran_in: pipeline`) is CI's.
pub(crate) struct Chosen {
    pub root: String,
    pub runner: String,
    pub reader: String,
    pub gaps: Vec<String>,
}

pub(crate) fn chosen(log: &Log) -> Vec<Chosen> {
    use serde_json::Value;
    let s = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut latest: std::collections::BTreeMap<(String, String), &familiar_ai_facts::Fact> = std::collections::BTreeMap::new();
    for f in log.iter().filter(|f| f.kind == "reader_chosen" && s(f, "ran_in") != "pipeline") {
        latest.insert((s(f, "root"), s(f, "runner")), f);
    }
    latest
        .into_iter()
        .map(|((root, runner), f)| Chosen {
            reader: s(f, "reader"),
            gaps: f.fields.get("gaps").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
            root,
            runner,
        })
        .collect()
}

/// The dry run going now (plan A), when there is one.
pub(crate) fn dry_run_going(log: &Log, now: f64) -> Option<String> {
    crate::door::runs::runs(log).into_iter().rfind(|r| r.dry() && r.live() && !r.void(now) && !r.expired(now)).map(|r| r.run_id)
}

/// How the runners are read (plan A) [draft words], one line: a dry run going, else the record's
/// latest choice per (root, runner) with the outcomes that ask, in words; `None` when there is
/// neither.
pub fn reading_line(log: &Log, now: f64) -> Option<String> {
    if let Some(id) = dry_run_going(log, now) {
        return Some(format!("reading: the dry run for pytest is running (run {id}); its choice goes in .familiar/adapter"));
    }
    let each: Vec<String> = chosen(log)
        .into_iter()
        .map(|c| {
            let at = if c.root.is_empty() { c.runner.clone() } else { format!("{} in {}", c.runner, c.root) };
            let kind = c.reader.rsplit_once(':').map_or("", |(_, k)| k);
            let how = match (c.reader.as_str(), kind) {
                ("none", _) => "by no reader: a plugin or setting changes what its runs print, so every run asks you".to_string(),
                (_, k) if k.starts_with("reporter") => "by Familiar's own reporter".to_string(),
                (_, "tables") => "by its output tables".to_string(),
                (_, "report" | "trx" | "trx-flag") => "by its report files".to_string(),
                (_, "pinned") => "by its output in a fixed format".to_string(),
                (r, _) => format!("by {r}"),
            };
            if c.gaps.is_empty() {
                format!("{at} {how}")
            } else {
                let outcomes: Vec<&str> = c.gaps.iter().map(|g| familiar_ai_rules::reading::outcome_words(g)).collect();
                format!("{at} {how}; asks you on: {}", outcomes.join(", "))
            }
        })
        .collect();
    (!each.is_empty()).then(|| format!("reading: {}", each.join(" · ")))
}

/// The reading as the status line's short segment [draft words]: each runner, with how many
/// outcomes ask, or a dry run going; `None` when there is neither.
pub fn reading_segment(log: &Log, now: f64) -> Option<String> {
    if dry_run_going(log, now).is_some() {
        return Some("reading: dry run going".to_string());
    }
    let each: Vec<String> = chosen(log)
        .into_iter()
        .map(|c| match (c.reader.as_str(), c.gaps.len()) {
            ("none", _) => format!("{}, every run asks you", c.runner),
            (_, 0) => c.runner,
            (_, 1) => format!("{}, 1 outcome asks you", c.runner),
            (_, n) => format!("{}, {n} outcomes ask you", c.runner),
        })
        .collect();
    (!each.is_empty()).then(|| format!("reading: {}", each.join("; ")))
}

/// The lines for the runs this call reserved that are still going: a dry run (plan A) in its own
/// words, first, then the parts' runs.
pub fn started_lines(before: &Log, after: &Log) -> Vec<String> {
    let dry: Vec<(String, Option<&str>)> = crate::door::runs::runs(after)
        .into_iter()
        .filter(|r| r.dry())
        .map(|r| (r.run_id, familiar_ai_run::details_for(&r.command).map(|d| d.runner)))
        .collect();
    let started = started_runs(before, after);
    let (dry_started, parts): (Vec<String>, Vec<String>) = started.into_iter().partition(|id| dry.iter().any(|(d, _)| d == id));
    let mut lines: Vec<String> =
        dry_started.iter().map(|id| words::dry_run_started(id, dry.iter().find(|(d, _)| d == id).and_then(|(_, r)| *r))).collect();
    if !parts.is_empty() {
        lines.push(words::started(&parts));
    }
    lines
}

/// The cards the facts of this call opened, as the person reads them.
pub(crate) fn opened_cards(before: &Log, after: &Log) -> Vec<String> {
    let (b, v) = (View::new(before), View::new(after));
    let was: Vec<(String, String)> = terminal::cards(&b).into_iter().map(|c| (c.criterion_id, c.hold)).collect();
    terminal::cards(&v)
        .iter()
        .filter(|c| !was.contains(&(c.criterion_id.clone(), c.hold.clone())))
        .map(|c| words::card(&v, c))
        .collect()
}

/// What the agent is told at SessionStart: the discipline, the whole
/// picture, the cards, and (while planning) the first missing line with the
/// command that claims it. `judge_on`: a judge is asked here.
pub(crate) fn session_start_context(log: &Log, root: &Path, now: f64, judge_on: bool) -> String {
    // the CI line, read from the record and HEAD, ends the one mode line
    let first = match ci::line_here(log, root) {
        Some(ci) => format!("{} · {ci}", mode_line(log)),
        None => mode_line(log),
    };
    let mut parts = vec![first, discipline::discipline().to_string()];
    parts.push(picture(log, root, now, judge_on, None));
    parts.join("\n\n")
}

/// The first line of the session's context: the mode, and the run's intent
/// while proving.
fn mode_line(log: &Log) -> String {
    match (person::mode(log), person::run_intent(log).and_then(|i| i.fields.get("text")).and_then(serde_json::Value::as_str)) {
        (person::PROVING, Some(text)) => format!("{}{} · the intent: \"{text}\"", words::LEAD, person::PROVING),
        (mode, _) => format!("{}{mode}", words::LEAD),
    }
}

/// The whole picture plus the cards and what comes next.
/// `judge_on`: a judge is asked here (a seal it has not read waits). `ci`:
/// the CI line, shown as its own line after the criteria (`status`).
pub(crate) fn picture(log: &Log, root: &Path, now: f64, judge_on: bool, ci: Option<&str>) -> String {
    let v = View::new(log);
    let mut parts = words::status_lines(log, judge_on);
    parts.extend(ci.map(|c| format!("{}{c}", words::LEAD)));
    parts.extend(reading_line(log, now).map(|r| format!("{}{r}", words::LEAD)));
    // each prompt of the person's that nothing answers yet
    parts.extend(person::status_lines(log));
    // the intent next to each sealed test, and the judge's answer
    parts.extend(judge::intent_next_to_test(log, judge_on));
    parts.extend(terminal::cards(&v).iter().map(|c| words::card(&v, c)));
    // each cleared hold whose answer no verified approval stands behind
    parts.extend(not_verified_lines(&v, &approved_if_answered(&v, root)));
    if let crate::decide::decision::Decision::Refuse { reason } =
        crate::decide::decision::decide(log, root, &[familiar_ai_rules::Event::FirstEditOrCommit], now, judge_on)
    {
        parts.push(reason);
    }
    // a test file a commit landed that the plan does not name
    parts.extend(unplanned_tests(log, root, 0));
    // the call to action, when every criterion is proven here
    parts.extend(prove::line(log, root, Some(prove::GH)));
    parts.join("\n")
}

/// The CI words of `status --json` while a part owes CI and there is no HEAD
/// to read the CI line at.
const CI_PENDING: &str = "CI pending";

/// `picture` as one JSON object: every value from the readers `picture` and
/// the status line use (`missing_line` is the refusal of a first edit, `next`
/// the call to action). Beside each state's words, its name and its holds;
/// the runs; each card's choices and claim line; and the counts, computed
/// here once so every surface reads the same ones. Each criterion,
/// card and run carries the name a person reads (`name`, "part 2"), and each
/// criterion its part number and its ask (`ask`: the prompt's time, its
/// words whole, its turn); the ids are the agent's.
/// `said`: every prompt the person typed and what it became. `counts.needs_you`
/// and `ci` are the only words a surface shows for what needs the person and
/// for CI. Each criterion's `held_by` says who holds it in words.
/// `checkouts`: each other checkout of the repository, as [`checkout_json`]
/// gives it. `first_done_commit` [draft name]: the head where every active
/// part was first done (`familiar_ai_rules::hooks::first_done_commit`), or null.
pub(crate) fn picture_json(log: &Log, root: &Path, now: f64, judge_on: bool, others: &[crate::door::project::Other]) -> serde_json::Value {
    use familiar_ai_rules::{lifecycle, plan};
    use serde_json::{json, Value};
    let v = View::new(log);
    let text = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).cloned().unwrap_or(Value::Null);
    let intent = person::run_intent(log).map_or(Value::Null, |i| json!({ "t": i.t, "text": text(i, "text"), "sha": text(i, "sha") }));
    let missing = match crate::decide::decision::decide(log, root, &[familiar_ai_rules::Event::FirstEditOrCommit], now, judge_on) {
        crate::decide::decision::Decision::Refuse { reason } => Value::from(reason),
        _ => Value::Null,
    };
    let asks = person_words::asks(&v);
    let name = |c: &str| person_words::name_of(&asks, c);
    let approved = approved_if_answered(&v, root);
    // a part the person cleared is left out (the owner, 2026-10-28); the record keeps it
    let live: Vec<String> = plan::criteria(&v).into_iter().filter(|c| !cleared(&v, c)).collect();
    let criteria: Vec<Value> = live.iter().map(|c| criterion_json(&v, &asks, c, judge_on, &approved)).collect();
    let cards = terminal::cards(&v);
    let runs = runs_json(log, now, &name);
    let waiting = live
        .iter()
        .filter(|c| match lifecycle::state(&v, c) {
            "waiting" => true,
            st @ ("sealed" | "green") => judge::shown(&v, c, st, judge_on).as_deref() == Some(judge::WAITING),
            _ => false,
        })
        .count();
    // the open cards are everything waiting on the person: one number, so no
    // surface counts again
    let counts = json!({
        "running": runs.iter().filter(|r| r["in_flight"] == true).count(),
        "waiting": waiting,
        "needs_you": cards.len(),
    });
    // a part green or beyond owes CI, so it always has CI words, even with no
    // HEAD to read the CI line at
    let owes_ci = live.iter().any(|c| matches!(lifecycle::state(&v, c), "green" | "proven"));
    let ci = ci::line_here(log, root).or_else(|| owes_ci.then(|| CI_PENDING.to_string()));
    json!({
        // the person's off switch
        "off": crate::door::switch::off_in(log),
        "mode": person::mode(log),
        "intent": intent,
        "plan": plan::plan_state(&v),
        "missing_line": missing,
        "criteria": criteria,
        "cards": cards.iter().map(|c| card_json(&v, c, name(&c.criterion_id))).collect::<Vec<_>>(),
        "next": prove::line(log, root, Some(prove::GH)),
        "waiting_on_you": cards.len(),
        "counts": counts,
        "runs": runs,
        "ci": ci,
        "reading": reading_line(log, now),
        "said": said_json(&v),
        "checkouts": others.iter().map(|o| checkout_json(o, now, judge_on)).collect::<Vec<_>>(),
        // the head where every active part was first done, as the rules read it (the CI bar)
        "first_done_commit": familiar_ai_rules::hooks::first_done_commit(&v),
    })
}

/// A criterion for `status --json`, beside its part and ask.
fn criterion_json(v: &View, asks: &[person_words::Ask], c: &str, judge_on: bool, approved: &[familiar_ai_rules::holds::Approved]) -> serde_json::Value {
    use familiar_ai_rules::{lifecycle, plan};
    use serde_json::{json, Value};
    let (ask, part) = person_words::part_of(asks, c).map_or((Value::Null, Value::Null), |(a, n)| (ask_json(a), Value::from(n)));
    let hold = words::holds_of(v, c);
    let judged = match judge::said(v, c) {
        None => Value::Null,
        Some(judge::Said::Nothing) => Value::from(if judge_on { "waiting" } else { "not judged yet" }),
        Some(judge::Said::Covers) => Value::from("covers"),
        Some(judge::Said::DoesNotCover { .. }) => Value::from("does not cover"),
        Some(judge::Said::NotJudged { .. }) => Value::from("not judged"),
    };
    let mut holds: Vec<String> = Vec::new();
    for w in lifecycle::whys(v, c) {
        if !holds.contains(&w) {
            holds.push(w);
        }
    }
    json!({
        "id": c,
        "name": person_words::name_of(asks, c),
        "part": part,
        "ask": ask,
        "text": words::text_of(v, c),
        "state": words::standing_on(v, c, judge_on),
        "state_name": lifecycle::state(v, c),
        "holds": holds,
        "hold": (!hold.is_empty()).then_some(hold),
        "holder": plan::assigned_to(v, c),
        "held_by": held_by(v, c),
        "path": plan::path_of(v, c),
        "judge": judged,
        "after": plan::string_before(v, c),
        "journey": journey_json(v, c),
        "answers": answers_json(v, c, approved),
        "try": try_json(v, c),
    })
}

/// The part's latest try (`familiar-ai try` [draft]) for `status --json`: its id, sha, whether
/// it was red, how many functions its failing tests ran, the ranker and the top 12; null before
/// the first.
fn try_json(v: &View, c: &str) -> serde_json::Value {
    use familiar_ai_rules::trace;
    use serde_json::{json, Value};
    let Some(f) = trace::latest(v, c) else { return Value::Null };
    let field = |k: &str| f.fields.get(k).cloned().unwrap_or(Value::Null);
    let top: Vec<Value> = trace::top_of(f)
        .into_iter()
        .take(crate::door::try_cmd::SHOWN)
        .map(|t| json!({ "rank": t.rank, "file_path": t.file_path, "symbol": t.symbol, "line_start": t.line_start, "line_end": t.line_end }))
        .collect();
    json!({ "run_id": field("run_id"), "sha": field("sha"), "red": field("red"), "path_size": field("path_size"), "ranker": field("ranker"), "top": top })
}

/// Each part's latest try (`familiar-ai try` [draft]) for `status`: red or not, and when red the
/// top 12 of what its failing tests ran as `rank. file:line_start-line_end symbol`, with how many
/// more `familiar-ai try --list all` prints.
pub(crate) fn try_lines(log: &Log) -> Vec<String> {
    use familiar_ai_rules::{plan, trace};
    use serde_json::Value;
    let v = View::new(log);
    let asks = person_words::asks(&v);
    let mut out = Vec::new();
    for c in plan::criteria(&v) {
        let Some(f) = trace::latest(&v, &c) else { continue };
        let name = person_words::name_of(&asks, &c).unwrap_or_else(|| c.clone());
        let s = |k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("");
        let at = if s("sha").is_empty() { String::new() } else { format!(" at {}", crate::door::git_program::short(s("sha"))) };
        let top = trace::top_of(f);
        if f.fields.get("red") != Some(&Value::Bool(true)) {
            out.push(format!("{}{name} · the try{at} ({}): not red: {}", words::LEAD, s("run_id"), crate::door::try_cmd::PASSES));
            continue;
        }
        let size = f.fields.get("path_size").and_then(Value::as_u64).unwrap_or(top.len() as u64);
        if top.is_empty() {
            out.push(format!("{}{name} · the try{at} ({}): red; the recorder saw no function of the project run, so there is no list", words::LEAD, s("run_id")));
            continue;
        }
        out.push(format!("{}{name} · the try{at} ({}): red · {size} functions its failing tests ran, best match to the person's words first ({}):", words::LEAD, s("run_id"), s("ranker")));
        let shown = crate::door::try_cmd::SHOWN;
        out.extend(top.iter().take(shown).map(|t| format!("  {}", crate::door::try_cmd::line_of(t.rank, &t.file_path, t.line_start, t.line_end, &t.symbol))));
        if size > shown as u64 {
            out.push(format!("  {} more: familiar-ai try --list all", size - shown as u64));
        }
    }
    out
}

/// The approvals verified here, read only when the record holds an answer.
pub(crate) fn approved_if_answered(v: &View, root: &Path) -> Vec<familiar_ai_rules::holds::Approved> {
    if v.kind("hold_answered").next().is_none() {
        return Vec::new();
    }
    crate::door::approval::approved_here(root)
}

/// [draft names, the owner's word owed]
pub(crate) const ANSWERED_VERIFIED: &str = "answered, verified";
pub(crate) const ANSWERED_NOT_VERIFIED: &str = "answered, not verified";

/// An answer's flag in the person's words: "answered, verified <t>" only by
/// a verified approval, else "answered, not verified".
pub(crate) fn answer_words(a: &familiar_ai_rules::holds::Answer) -> String {
    match &a.verified {
        Some(t) => format!("{ANSWERED_VERIFIED} {t}"),
        None => ANSWERED_NOT_VERIFIED.to_string(),
    }
}

/// Each answer to a card of `c` for `status --json`: its hold, choice, the
/// approval it names, whether that approval verified, and its words.
fn answers_json(v: &View, c: &str, approved: &[familiar_ai_rules::holds::Approved]) -> Vec<serde_json::Value> {
    familiar_ai_rules::holds::answers(v, c, approved)
        .iter()
        .map(|a| serde_json::json!({ "hold": a.hold, "choice": a.choice, "approval": a.approval, "verified": a.verified.is_some(), "words": answer_words(a) }))
        .collect()
}

/// How many answers in the record read not verified, the status line's count. The cases card's
/// answer is a choice about cases, not an approval of a hold, so it is not counted (the owner,
/// 2026-10-25) [draft].
pub(crate) fn not_verified_count(v: &View, approved: &[familiar_ai_rules::holds::Approved]) -> usize {
    let counted = |a: &&familiar_ai_rules::holds::Answer| a.verified.is_none() && a.hold != familiar_ai_rules::cases::CARD;
    familiar_ai_rules::plan::criteria(v).iter().filter(|c| !cleared(v, c)).map(|c| familiar_ai_rules::holds::answers(v, c, approved).iter().filter(counted).count()).sum()
}

/// One line per answer that reads not verified, for `status`.
fn not_verified_lines(v: &View, approved: &[familiar_ai_rules::holds::Approved]) -> Vec<String> {
    let asks = person_words::asks(v);
    let mut out = Vec::new();
    for c in familiar_ai_rules::plan::criteria(v).into_iter().filter(|c| !cleared(v, c)) {
        let name = person_words::name_of(&asks, &c).unwrap_or_else(|| c.clone());
        for a in familiar_ai_rules::holds::answers(v, &c, approved).iter().filter(|a| a.verified.is_none()) {
            out.push(format!("{}{name} · {} {}: {}", words::LEAD, a.hold, a.choice, answer_words(a)));
        }
    }
    out
}

/// The seven steps of a part's journey, by the names the person reads.
const STEPS: [&str; 7] = ["plan", "place", "test", "red", "fix", "green", "CI"];

/// A criterion's journey for `status --json`: each of the seven steps with
/// `done` (true; false, not done; null, not reached; "half", CI locally
/// verified but not hosted) and `at`, the short sha the step was done at; its
/// planned tests; its places; and `opened`, whether the pane opens it (the
/// part in play with a holder, or held). A part in play (its turn) has every
/// step not done false; any other has only its next step false, the rest null.
fn journey_json(v: &View, c: &str) -> serde_json::Value {
    use familiar_ai_rules::criterion::{self, Place};
    use familiar_ai_rules::{lifecycle, plan};
    use serde_json::{json, Value};
    let short = |f: &familiar_ai_facts::Fact| f.fields.get("commit").and_then(Value::as_str).map(|s| s.chars().take(7).collect::<String>());
    let st = lifecycle::state(v, c);
    let tests = criterion::test_files(v, c);
    let own = criterion::own_tests(v, c);
    let written = !tests.is_empty() && tests.iter().all(|t| own.iter().any(|o| &o.file_path == t));
    let seal = criterion::sealed_at(v, c, Place::Laptop);
    let fix = seal.and_then(|_| lifecycle::fix_commit(v, c));
    let proven = criterion::proven_raw(v, c);
    let green = proven || criterion::green_raw(v, c, Place::Laptop);
    // hosted as `words::standing_on` reads it; locally verified is half
    let hosted = v.kind("ci_report_seen").any(|f| f.fields.get("outcome").and_then(Value::as_str) == Some("proven"));
    let ci = match (proven, hosted) {
        (false, _) => Value::from(false),
        (true, true) => Value::from(true),
        (true, false) => Value::from("half"),
    };
    let done = [
        (Value::from(true), None),
        (Value::from(plan::anchor_of(v, c).is_some()), None),
        (Value::from(written), None),
        (Value::from(seal.is_some()), seal.and_then(short)),
        (Value::from(fix.is_some()), fix.and_then(short)),
        (Value::from(green), if green { criterion::stamp(v, c).and_then(short) } else { None }),
        (ci, None),
    ];
    let in_play = lifecycle::turn(v, c);
    let mut next_seen = false;
    let steps: Vec<Value> = STEPS
        .iter()
        .zip(done)
        .map(|(name, (d, at))| {
            let undone = d == false;
            let d = if undone && !in_play && next_seen {
                Value::Null
            } else {
                next_seen |= undone;
                d
            };
            let mut s = json!({ "name": name, "done": d });
            if let Some(at) = at {
                s["at"] = Value::from(at);
            }
            s
        })
        .collect();
    let places: Vec<Value> = match plan::anchor_of(v, c) {
        Some(a) if a.kind == "place_chosen" => plan::places_of(a).iter().map(|p| json!({ "file": p.file_path, "symbol": p.symbol })).collect(),
        Some(a) if a.kind == "new_work_spot_chosen" => {
            let text = |k: &str| a.fields.get(k).cloned().unwrap_or(Value::Null);
            vec![json!({ "file": text("file_path"), "symbol": text("beside") })]
        }
        _ => Vec::new(),
    };
    let opened = st == "held" || (matches!(st, "testing" | "sealed" | "green") && plan::assigned_to(v, c).is_some());
    json!({ "steps": steps, "tests": tests, "places": places, "opened": opened })
}

/// Who holds `c`, in words, from the record's assignment, transfers and the
/// models seen: `held by: opus, conversation cc9af915`, `held by: the
/// investigator (fable)`, `held by: a subagent (sonnet)`; `None` unheld.
pub(crate) fn held_by(v: &View, c: &str) -> Option<String> {
    use familiar_ai_rules::{ladder, plan};
    let h = plan::assigned_to(v, c)?;
    let model = ladder::rung_of(v, Some(h)).or_else(|| ladder::model_of(v, Some(h)));
    let who = |who: &str| model.map_or_else(|| format!("held by: {who}"), |m| format!("held by: {who} ({m})"));
    let investigating = ladder::investigator_of(v, c).and_then(|f| f.fields.get("to_conversation")).and_then(serde_json::Value::as_str) == Some(h);
    Some(if investigating {
        who("the investigator")
    } else if h.contains('/') {
        who("a subagent")
    } else {
        let short: String = h.chars().take(8).collect();
        model.map_or_else(|| format!("held by: conversation {short}"), |m| format!("held by: {m}, conversation {short}"))
    })
}

/// Another checkout for `status --json`: its folder, short name, branch and
/// switch (`off`, `switch_from`, `main_off`), what the person said there,
/// each ask with its parts in order (each as `criteria` gives it), its runs,
/// the open cards it relays (`relayed_cards`) and counts, and its mode; or `busy: true` alone when its
/// record was in use past the wait (`unreadable`, with why, when it could
/// not be read at all).
pub(crate) fn checkout_json(o: &crate::door::project::Other, now: f64, judge_on: bool) -> serde_json::Value {
    use familiar_ai_rules::plan;
    use serde_json::{json, Value};
    let mut out = json!({ "folder": o.folder, "name": o.name, "branch": o.branch, "root": o.root.display().to_string(), "busy": false });
    let (log, switch) = match &o.read {
        Ok(read) => read,
        Err(familiar_ai_facts::RecordError::Busy) => {
            out["busy"] = Value::from(true);
            return out;
        }
        Err(e) => {
            out["unreadable"] = Value::from(e.to_string());
            return out;
        }
    };
    switch.json_into(&mut out);
    let v = View::new(log);
    let asks = person_words::asks(&v);
    let known: Vec<String> = plan::criteria(&v).into_iter().filter(|c| !cleared(&v, c)).collect();
    let approved = approved_if_answered(&v, &o.root);
    let mut shown: Vec<Value> = asks
        .iter()
        .map(|a| {
            let parts: Vec<Value> = a.parts.iter().filter(|c| known.contains(c)).map(|c| criterion_json(&v, &asks, c, judge_on, &approved)).collect();
            let mut ask = ask_json(a);
            ask["parts"] = Value::from(parts);
            ask
        })
        .collect();
    if shown.is_empty() && person::mode(log) != person::IDLE {
        if let Some(intent) = person::run_intent(log) {
            let mut ask = ask_json(&person_words::ask_for(&v, intent));
            ask["parts"] = json!([]);
            shown.push(ask);
        }
    }
    let name = |c: &str| person_words::name_of(&asks, c);
    // only the cards this checkout relays (the owner, 2026-10-15); it is listed whatever
    let cards = crate::door::project::relayed_cards(o, log);
    out["mode"] = Value::from(person::mode(log));
    out["said"] = Value::from(said_json(&v));
    out["asks"] = Value::from(shown);
    out["runs"] = Value::from(runs_json(log, now, &name));
    out["cards"] = Value::from(cards.iter().map(|c| card_json(&v, c, name(&c.criterion_id))).collect::<Vec<_>>());
    out["counts"] = json!({ "needs_you": cards.len() });
    out
}

/// What the person said, for `status --json`: each prompt, oldest first,
/// whole, with its turn, its time, what it became and its ask's number.
fn said_json(v: &View) -> Vec<serde_json::Value> {
    person_words::said_and_became(v)
        .iter()
        .map(|p| serde_json::json!({ "for": p.turn, "at": crate::door::person::clock(p.at), "text": p.text, "became": p.became, "ask": p.ask }))
        .collect()
}

/// `familiar-ai status --said`: one line per prompt, oldest first, its
/// words on one line and whole.
pub(crate) fn said_lines(log: &Log) -> Vec<String> {
    person_words::said_and_became(&View::new(log))
        .iter()
        .map(|p| format!("  {}  {}  · {}", crate::door::person::clock(p.at), person_words::one_line(&p.text), p.label()))
        .collect()
}

/// `familiar-ai status --pr`: the pull request body as markdown. Each sealed
/// part, in criterion order, by its own text, then its ask in the person's
/// words, its test files and what the judge said.
pub(crate) fn pr_body(log: &Log) -> String {
    use person_words::{PR_ASKED_FOR, PR_CI, PR_CLOSING, PR_JUDGE, PR_LOGO, PR_NOT_ASKED, PR_ONE_PART, PR_PARTS, PR_TESTED_BY};
    let v = View::new(log);
    let asks = person_words::asks(&v);
    let mut parts = Vec::new();
    for c in familiar_ai_rules::plan::criteria(&v) {
        let Some(seal) = familiar_ai_rules::criterion::sealed_at(&v, &c, familiar_ai_rules::criterion::Place::Laptop) else {
            continue;
        };
        let own = words::text_of(&v, &c).map(person_words::one_line).or_else(|| person_words::name_of(&asks, &c)).unwrap_or_default();
        let asked = person_words::part_of(&asks, &c).map(|(a, _)| person_words::one_line(&a.text)).unwrap_or_default();
        let tested = judge::tested_by(&v, &c);
        let judge = judge::verdict_line(&v, &c, seal).unwrap_or_else(|| PR_NOT_ASKED.to_string());
        parts.push(format!("**{own}**\n\n- {PR_ASKED_FOR} {asked}\n- {PR_TESTED_BY} {tested}\n- {PR_JUDGE} {judge}\n"));
    }
    let count = if parts.len() == 1 { PR_ONE_PART.to_string() } else { PR_PARTS.replace("<n>", &parts.len().to_string()) };
    format!("{PR_LOGO}\n\n{count}\n\n{}\n{PR_CI}\n\n{PR_CLOSING}\n", parts.join("\n"))
}

/// An ask for `status --json`: the prompt's time (h:mm), its words whole,
/// and its turn.
fn ask_json(a: &person_words::Ask) -> serde_json::Value {
    serde_json::json!({ "at": a.at.map(crate::door::person::clock), "text": a.text, "for": a.turn })
}

/// A card for `status --json`: its part's name, its question in the
/// person's words, its choices by name in the rules' order (`no_test_stack`:
/// the typed command, `typed`), and the claim line for the first choice, as
/// the hook prints it.
fn card_json(v: &View, c: &terminal::Card, name: Option<String>) -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut choices: Vec<String> = c.choices.iter().map(|(ch, _)| ch.clone()).collect();
    if c.hold == "no_test_stack" {
        choices.push(picker::TYPED_CHOICE.to_string());
    }
    let mut known = Map::new();
    known.insert("criterion_id".into(), Value::from(c.criterion_id.as_str()));
    known.insert("hold".into(), Value::from(c.hold.as_str()));
    if let Some(first) = choices.first() {
        known.insert("choice".into(), Value::from(first.as_str()));
    }
    json!({
        "criterion_id": c.criterion_id,
        "name": name,
        "hold": c.hold,
        "question": words::named(v, &c.question),
        "choices": choices,
        "claim": crate::door::claim::command_form("hold_answered", &known, None),
        "finding": card_finding(v, c),
        // H-4: the can't-tell card's run in short sentences; empty on other cards
        "summary": c.summary,
        // the card as the person reads it on the picker (2026-10-20): its lines and each option's
        // short name and description, word for word
        "person": person_card(v, c),
    })
}

/// The card's lines and options exactly as the picker shows them to the person; the cases card's
/// options [draft] are its own, each label with its line.
fn person_card(v: &View, c: &terminal::Card) -> serde_json::Value {
    use crate::teach::person_words as pw;
    let a = pw::asked(v, c);
    let options: Vec<serde_json::Value> = match &a.cases {
        Some(s) => s.options.iter().map(|(ch, label, line)| serde_json::json!({ "choice": ch, "label": label, "description": line })).collect(),
        None => a.choices.iter().map(|(ch, line)| serde_json::json!({ "choice": ch, "label": pw::label_of(&c.hold, ch).unwrap_or(line), "description": line })).collect(),
    };
    serde_json::json!({ "lines": pw::lines(&a), "options": options })
}

/// The finding a two_tries_not_red card carries (S7b): its category, the choice it lists first,
/// the reason, its own fields and the quotes as the binary read them; null without one.
fn card_finding(v: &View, c: &terminal::Card) -> serde_json::Value {
    use serde_json::Value;
    if c.hold != familiar_ai_rules::finding::HOLD {
        return Value::Null;
    }
    let Some(f) = familiar_ai_rules::finding::finding(v, &c.criterion_id) else {
        return Value::Null;
    };
    let mut out = serde_json::Map::new();
    let category = familiar_ai_rules::finding::category(f).unwrap_or("");
    out.insert("category".into(), Value::from(category));
    let choice = crate::teach::person_words::FINDING_CHOICE.iter().find(|(k, _)| *k == category).map(|(_, ch)| *ch);
    out.insert("choice".into(), choice.map_or(Value::Null, Value::from));
    for k in ["model", "reason", "guidance", "missed", "scope", "quotes", "runs"] {
        if let Some(x) = f.fields.get(k) {
            out.insert(k.into(), x.clone());
        }
    }
    Value::Object(out)
}

/// The runs of this record, in the order reserved: in flight while reserved
/// or started and not ended (a void or expired run is not, as `any_live`
/// reads it); the outcome once ended, `cancelled` when a cancel came first;
/// its part by `name`.
fn runs_json(log: &Log, now: f64, name: &dyn Fn(&str) -> Option<String>) -> Vec<serde_json::Value> {
    crate::door::runs::runs(log)
        .iter()
        .map(|r| {
            let outcome = match &r.ended {
                None => None,
                Some(_) if r.cancelled() && !r.held() => Some("cancelled"),
                Some(e) => Some(e.outcome.as_str()),
            };
            serde_json::json!({
                "run_id": r.run_id,
                "criterion_id": r.criterion_id,
                "name": name(&r.criterion_id),
                "commit": r.commit.get(..7).unwrap_or(&r.commit),
                "in_flight": r.live() && !r.void(now) && !r.expired(now),
                "outcome": outcome,
            })
        })
        .collect()
}

/// The sha the plan was checked at: the latest planning check's, else the
/// latest intent's.
fn planning_sha(log: &Log) -> Option<&str> {
    fn sha(f: &familiar_ai_facts::Fact) -> Option<&str> {
        f.fields.get("sha").and_then(serde_json::Value::as_str)
    }
    let check = |f: &&familiar_ai_facts::Fact| f.kind == "anchor_sha_matched" && f.fields.get("checked_at").and_then(serde_json::Value::as_str) == Some("planning");
    log.iter().rev().find(check).and_then(sha).or_else(|| log.iter().rev().find(|f| f.kind == "intent_submitted").and_then(sha))
}

/// `file` in the working tree holds the same bytes as at `sha`. A file new
/// since, gone since, or a sha git cannot read: not untouched.
fn untouched(root: &Path, sha: &str, file: &str) -> bool {
    match familiar_ai_git::file_at(root, sha, file) {
        Ok(then) => std::fs::read(root.join(file)).is_ok_and(|now| now == then),
        Err(_) => false,
    }
}

/// The files in the working tree directly in each of `folders` that git
/// does not ignore, in order.
fn written_in(root: &Path, folders: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
            continue;
        };
        let mut names: Vec<String> = entries.flatten().filter(|e| e.path().is_file()).filter_map(|e| e.file_name().to_str().map(str::to_string)).collect();
        names.sort();
        for name in names {
            let file = format!("{folder}/{name}");
            if !familiar_ai_git::is_ignored(root, &file).unwrap_or(false) {
                out.push(file);
            }
        }
    }
    out
}

/// One line per test file the `commit_seen` facts from `from` on landed that
/// the plan does not name, with the claim that plans it and the planned file
/// to move it into. From the start (the status), also those written in the
/// working tree. A file whose bytes equal the planning sha's is never told,
/// whichever commit landed it; only the teaching changes, not the facts the
/// commit reader records.
pub(crate) fn unplanned_tests(log: &Log, root: &Path, from: usize) -> Vec<String> {
    use familiar_ai_rules::{criterion, lifecycle, plan};
    let v = View::new(log);
    let planned = familiar_ai_rules::what_to_read(log).plan_test_files;
    let folder = |f: &str| Path::new(f).parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf);
    let roots: Vec<String> = v.kind("test_stack_detected").filter_map(|f| f.fields.get("project_root").and_then(serde_json::Value::as_str)).map(|r| r.trim_start_matches("./").trim_end_matches('/').to_string()).collect();
    let named = |f: &str| {
        let base = Path::new(f).file_name().and_then(|b| b.to_str()).unwrap_or("");
        (base.starts_with("test_") && base.ends_with(".py")) || base.ends_with("_test.py")
    };
    let under_tests = |f: &str| roots.iter().any(|r| f.starts_with(&if r.is_empty() || r == "." { "tests/".to_string() } else { format!("{r}/tests/") }));
    let landed = log.iter().skip(from).filter(|f| f.kind == "commit_seen").flat_map(|f| {
        f.fields.get("files").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(serde_json::Value::as_str).map(str::to_string)
    });
    let mut files: Vec<String> = landed.collect();
    if from == 0 {
        let mut folders: Vec<String> = planned.iter().filter_map(|f| folder(f)).map(|f| f.to_string_lossy().into_owned()).collect();
        folders.extend(roots.iter().map(|r| if r.is_empty() || r == "." { "tests".to_string() } else { format!("{r}/tests") }));
        folders.sort();
        folders.dedup();
        files.extend(written_in(root, &folders));
    }
    let since = planning_sha(log);
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for file in files.iter().map(String::as_str) {
        if planned.iter().any(|p| p == file) || crate::door::commits::is_familiars_own(file) || seen.iter().any(|s| s == file) {
            continue;
        }
        if since.is_some_and(|sha| untouched(root, sha, file)) {
            continue;
        }
        let beside = plan::criteria(&v).into_iter().find(|c| criterion::test_files(&v, c).iter().any(|t| folder(t).is_some() && folder(t) == folder(file)));
        let Some(c) = beside.or_else(|| (named(file) || under_tests(file)).then(|| lifecycle::runnable_criteria(&v).into_iter().next()).flatten()) else {
            continue;
        };
        let files = criterion::test_files(&v, &c);
        let Some(first) = files.first() else {
            continue;
        };
        seen.push(file.to_string());
        out.push(words::unplanned_test(file, &files, &c, first));
    }
    out
}

/// The OUTCOMES lines of the runs `ids` (those this call told inline) that are neither
/// green nor a valid red: what the state lines do not already say (plan §6.1, S8). Read from
/// each run's translation, never from its output.
pub(crate) fn outcome_lines(ids: &[String], log: &Log) -> Vec<String> {
    use familiar_ai_rules::outcome::{family, translation_of, Family};
    let v = View::new(log);
    ids.iter()
        .filter(|id| !matches!(family(&v, id), Family::Green | Family::ValidRed))
        .filter_map(|id| translation_of(&v, id).and_then(words::outcome_line))
        .collect()
}

/// The runs this call reserved that are still going as it answers (plan §1.2).
fn started_runs(before: &Log, after: &Log) -> Vec<String> {
    let id = |f: &familiar_ai_facts::Fact| f.fields.get("run_id").and_then(serde_json::Value::as_str).map(str::to_string);
    let ended: Vec<String> = after.iter().filter(|f| f.kind == "run_ended").filter_map(id).collect();
    after.iter().skip(before.len()).filter(|f| f.kind == "run_reserved").filter_map(id).filter(|r| !ended.contains(r)).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::unplanned_tests;

    /// A root with no repository: the planning sha cannot be read, so a
    /// landed file is told as it was before the planning sha was asked.
    const NO_REPOSITORY: &str = "/nonexistent/familiar-ai-unit";

    fn committed(files: &[&str]) -> (familiar_ai_facts::Log, usize) {
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let from = log.len();
        let fields = serde_json::json!({ "commit": "k1", "files": files, "touches_tests": false, "touches_code": true, "anchor_overlap": [] });
        let serde_json::Value::Object(m) = fields else { panic!("object") };
        (familiar_ai_facts::append_raw(&log, "commit_seen", m, 1.0, "t").expect("commit_seen"), from)
    }

    #[test]
    fn a_landed_test_file_the_plan_does_not_name_is_taught_with_the_claim_that_plans_it() {
        let (log, from) = committed(&["tests/test_c1_by_alias.py", "src/login.py", ".familiar/plan"]);
        assert_eq!(
            unplanned_tests(&log, Path::new(NO_REPOSITORY), from),
            ["familiar-ai: tests/test_c1_by_alias.py is not a planned test file; the plan names tests/test_c1.py; plan it: familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_c1_by_alias.py --will create, or move the test into tests/test_c1.py"]
        );
        // the facts before the commit say nothing; from the start (the status) the same line
        assert!(unplanned_tests(&log, Path::new(NO_REPOSITORY), log.len()).is_empty());
        assert_eq!(unplanned_tests(&log, Path::new(NO_REPOSITORY), 0).len(), 1);
    }

    #[test]
    fn a_planned_test_file_or_a_code_file_says_nothing() {
        let (log, from) = committed(&["tests/test_c1.py", "src/login.py", "README.md"]);
        assert!(unplanned_tests(&log, Path::new(NO_REPOSITORY), from).is_empty());
    }
}
