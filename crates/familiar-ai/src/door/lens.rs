//! The lens panel's facts (the owner's plan of 2026-10-26, lens-panel.md, approved with its
//! names): which Agent call got which lens (`lens_released`, written at the call's PreToolUse with
//! its tool_use_id, so parallel calls each get a different lens), and each answer, read at the
//! main agent's PostToolUse on that call (or its PostToolUseFailure) and checked against its
//! schema: a lens's as `lens_found` (found, none or failed), the synthesis's as `cases_found` (its
//! cases, the first `MAX_CASES` kept, and its questions, the first `MAX_QUESTIONS` well-formed ones).
//! The lens panel by layer: decide/ the spawn and its reads · door/ the answers and their facts · teach/ the words.

use familiar_ai_facts::{Fact, Log, Record};
use familiar_ai_rules::{cases, plan, View};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::door::{cannot, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::lens::{left_out, named, MAX_CASES, MAX_QUESTIONS};

/// The plugin-scoped name of the lens agent, as `subagent_type` and `agent_type` carry it.
pub const LENS: &str = "familiar-ai:lens";

/// The prefix of the found cases' ids.
pub const FOUND_PREFIX: &str = "a";

/// The why of a failed call that carried no error [draft words].
pub const FAILED_CALL: &str = "the lens's call failed";

/// The kind of the one finding a failed lens's answer carries, its note the why [draft].
pub const FAILED: &str = "failed";

/// True for the lens agent's `agent_type`, plugin-scoped or the bare name a project-level copy carries.
pub fn is_lens(agent_type: Option<&str>) -> bool {
    matches!(agent_type, Some(LENS) | Some("lens"))
}

/// The time after which `c`'s current panel's releases count: its latest synthesis that closed a
/// panel (a later look again begins the next one; nothing is released between them), else its intent.
pub fn panel_since(v: &View, c: &str) -> i64 {
    let intent = familiar_ai_rules::ask::intent_of(v, c).map_or(-1, |f| f.t as i64);
    let synthesis = cases::findings(v, c).last().map_or(-1, |f| f.t as i64);
    intent.max(synthesis)
}

/// Whether `lens` (one of the five, or the synthesis) is still owed on `c`.
fn owed(v: &View, c: &str, lens: &str) -> bool {
    if lens == cases::SYNTHESIS {
        cases::owed_next(v, c) == Some(cases::SYNTHESIS)
    } else {
        cases::lenses_owed(v, c).contains(&lens)
    }
}

/// Whether `lens` is running on `c`: released since the panel began, its answer still owed.
pub fn running(v: &View, c: &str, lens: &str) -> bool {
    let since = panel_since(v, c);
    owed(v, c, lens) && v.of(cases::RELEASED, c).any(|r| r.t as i64 > since && r.fields.get("lens").and_then(Value::as_str) == Some(lens))
}

/// What the Agent call `id` was released as, `(part, lens)`, from its latest `lens_released`.
pub fn released_as(v: &View, id: &str) -> Option<(String, String)> {
    let r = v.kind(cases::RELEASED).filter(|r| r.fields.get("tool_use_id").and_then(Value::as_str) == Some(id)).last()?;
    let s = |k: &str| r.fields.get(k).and_then(Value::as_str).map(str::to_string);
    Some((s("criterion_id")?, s("lens")?))
}

/// What the Agent call `id` was released as while its answer is still owed.
pub fn owed_release(v: &View, id: &str) -> Option<(String, String)> {
    released_as(v, id).filter(|(c, lens)| owed(v, c, lens))
}

/// Whether conversation `me` may release the panel's calls on `c`: its holder, or anyone while it has none.
fn mine(v: &View, c: &str, me: &str) -> bool {
    plan::assigned_to(v, c).is_none_or(|h| h == me)
}

/// The first part in plan order owed the lens panel that conversation `me` may release it for.
pub fn owed_here(v: &View, me: &str) -> Option<String> {
    plan::criteria(v).into_iter().find(|c| cases::owed(v, c) && mine(v, c, me))
}

/// The first part in plan order with a lens (or the synthesis) to release that `me` may release, and that lens.
pub fn to_release_here(v: &View, me: &str) -> Option<(String, &'static str)> {
    plan::criteria(v).into_iter().filter(|c| mine(v, c, me)).find_map(|c| cases::to_release(v, &c).map(|l| (c, l)))
}

/// Whether the only lens agents out are syntheses, which have no tools [draft]: some part's
/// synthesis is running and no part's lens is. While a lens of any part runs too, a lens agent's
/// call cannot be told apart from the synthesis's, and the reads go ahead.
pub fn only_the_synthesis_out(v: &View) -> bool {
    let parts = plan::criteria(v);
    let synthesis = parts.iter().any(|c| running(v, c, cases::SYNTHESIS));
    synthesis && !parts.iter().any(|c| cases::LENSES.iter().any(|l| running(v, c, l)))
}

/// At the main agent's PreToolUse on an Agent call while a lens (or the synthesis) is to be
/// released on a part it may release it for, record which one this call is: `lens_released` with
/// the call's tool_use_id; a call already released and still owed its answer records nothing again.
pub(crate) fn release(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    if input.event != HookEvent::PreToolUse || !input.is_spawn() || is_lens(input.agent_type.as_deref()) {
        return Ok(());
    }
    let (Some(id), Some(me)) = (input.tool_use_id.as_deref(), input.agent_conversation_id()) else {
        return Ok(());
    };
    let fields = {
        let v = View::new(log);
        if owed_release(&v, id).is_some() {
            return Ok(());
        }
        let Some((c, lens)) = to_release_here(&v, &me) else {
            return Ok(());
        };
        json!({ "criterion_id": c, "lens": lens, "tool_use_id": id }).as_object().cloned().unwrap_or_default()
    };
    log.push(record.append_raw(cases::RELEASED, fields, now, TENANT).map_err(|e| cannot("write its record", &e))?);
    Ok(())
}

/// A lens's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum LensAnswer {
    /// Its findings, each with every field of `lens_found`'s finding.
    Found(Vec<Value>),
    /// No finding: a valid, good answer.
    Nothing,
    /// No valid answer, and why.
    Failed(String),
}

/// What the synthesis's cases came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The cases, without their ids.
    Found(Vec<Value>),
    /// The first `MAX_CASES` cases, without their ids, of the number listed, more than `MAX_CASES`
    /// (the owner's ruling, 2026-10-25, "Keep what fits"): recorded as found, its why the note of
    /// how many were left out.
    Kept(Vec<Value>, usize),
    /// None found, and why the planned cases cover it.
    Nothing(String),
    /// No valid answer, and why.
    Failed(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLens {
    findings: Vec<RawFinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFinding {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    target_symbol: Option<String>,
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    candidate_question: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSynthesis {
    cases: Vec<RawCase>,
    #[serde(default)]
    questions: Vec<Value>,
    #[serde(default)]
    none_because: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    short: String,
    given: String,
    expect: String,
    why: String,
}

/// The subagent's final text: the text blocks of `content` joined, or `content` itself as text.
pub fn final_text(response: &Value) -> String {
    match response.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks.iter().filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// True when Claude Code's note says the turn limit ended the agent ("this agent stopped at its
/// 25-turn limit before finishing ... PARTIAL output").
fn cut_off(text: &str) -> bool {
    text.contains("stopped at its") && text.contains("-turn limit")
}

/// The JSON inside the first code fence (```json … ``` or ``` … ```), else the whole text.
fn unfenced(text: &str) -> &str {
    let Some(open) = text.find("```") else {
        return text.trim();
    };
    let rest = &text[open + 3..];
    // the fence's language word, up to the end of its line
    let body = rest.find('\n').map_or(rest, |n| &rest[n + 1..]);
    body.find("```").map_or(body, |close| &body[..close]).trim()
}

fn trimmed(s: &Option<String>) -> String {
    s.as_deref().map(str::trim).unwrap_or("").to_string()
}

/// Check lens `lens`'s final text against its schema (the trial's FINDINGS_OUT): every finding
/// needs its note; a kind left out is the lens's own, a null field is empty.
pub fn parse_lens(lens: &str, text: &str) -> LensAnswer {
    let who = named(lens);
    if cut_off(text) {
        return LensAnswer::Failed(format!("{who} stopped at its turn limit before it answered"));
    }
    let raw: RawLens = match serde_json::from_str(unfenced(text)) {
        Ok(raw) => raw,
        Err(e) => return LensAnswer::Failed(format!("{who}'s answer is not the JSON asked for: {e}")),
    };
    let mut out = Vec::new();
    for (i, f) in raw.findings.iter().enumerate() {
        let note = trimmed(&f.note);
        if note.is_empty() {
            return LensAnswer::Failed(format!("{who}'s finding {} has no note", i + 1));
        }
        let kind = Some(trimmed(&f.kind)).filter(|k| !k.is_empty()).unwrap_or_else(|| lens.to_string());
        out.push(json!({ "kind": kind, "note": note, "file_path": trimmed(&f.file_path), "target_symbol": trimmed(&f.target_symbol), "candidate_question": trimmed(&f.candidate_question) }));
    }
    if out.is_empty() {
        LensAnswer::Nothing
    } else {
        LensAnswer::Found(out)
    }
}

/// One of the synthesis's questions when it is well-formed: its text, two to four options, each
/// once and none blank, and its why (blank when left out).
fn question(q: &Value) -> Option<(String, Vec<String>, String)> {
    let text = q.get("text")?.as_str()?.trim().to_string();
    let options: Vec<String> = q.get("options")?.as_array()?.iter().map(|o| o.as_str().map(str::trim).map(str::to_string)).collect::<Option<_>>()?;
    let distinct = options.iter().enumerate().all(|(i, o)| !o.is_empty() && !options[..i].contains(o));
    let why = match q.get("why") {
        None | Some(Value::Null) => String::new(),
        Some(w) => w.as_str()?.trim().to_string(),
    };
    (!text.is_empty() && (2..=4).contains(&options.len()) && distinct).then_some((text, options, why))
}

/// Check the synthesis's final text against its schema: its cases as the finder's were (every
/// field of each, the first `MAX_CASES` of a longer list, a short line of any length whole), and
/// its questions, the first `MAX_QUESTIONS` well-formed ones with distinct texts, as q1..q3.
pub fn parse_synthesis(text: &str) -> (Finding, Vec<Value>) {
    let failed = |why: String| (Finding::Failed(why), Vec::new());
    if cut_off(text) {
        return failed("the synthesis stopped at its turn limit before it answered".to_string());
    }
    let raw: RawSynthesis = match serde_json::from_str(unfenced(text)) {
        Ok(raw) => raw,
        Err(e) => return failed(format!("the synthesis's answer is not the JSON asked for: {e}")),
    };
    let mut cases = Vec::new();
    for (i, k) in raw.cases.iter().enumerate() {
        for (name, value) in [("short", &k.short), ("given", &k.given), ("expect", &k.expect), ("why", &k.why)] {
            if value.trim().is_empty() {
                return failed(format!("the synthesis's case {} has no {name}", i + 1));
            }
        }
        cases.push(json!({ "short": k.short.trim(), "given": k.given.trim(), "expect": k.expect.trim(), "why": k.why.trim() }));
    }
    let mut questions: Vec<Value> = Vec::new();
    for (text, options, why) in raw.questions.iter().filter_map(question) {
        if questions.len() == MAX_QUESTIONS {
            break;
        }
        if questions.iter().any(|q| q["text"] == json!(text)) {
            continue;
        }
        questions.push(json!({ "question_id": format!("q{}", questions.len() + 1), "text": text, "options": options, "why": why }));
    }
    let finding = if cases.len() > MAX_CASES {
        let listed = cases.len();
        cases.truncate(MAX_CASES);
        Finding::Kept(cases, listed)
    } else if !cases.is_empty() {
        Finding::Found(cases)
    } else {
        match raw.none_because.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
            Some(why) => Finding::Nothing(why.to_string()),
            // the panel asks the person first: its questions stand without a none_because
            None if !questions.is_empty() => Finding::Nothing(String::new()),
            None => return failed("the synthesis listed no case and gave no none_because".to_string()),
        }
    };
    (finding, questions)
}

/// The `lens_found` fields for `answer` of `lens` on `c`; a failed answer carries its why as its
/// one finding, of kind `FAILED`.
pub fn lens_fields(c: &str, lens: &str, answer: &LensAnswer, model: &str, tokens: u64) -> Map<String, Value> {
    let (outcome, findings) = match answer {
        LensAnswer::Found(fs) => ("found", fs.clone()),
        LensAnswer::Nothing => ("none", Vec::new()),
        LensAnswer::Failed(why) => ("failed", vec![json!({ "kind": FAILED, "note": why, "file_path": "", "target_symbol": "", "candidate_question": "" })]),
    };
    json!({ "criterion_id": c, "lens": lens, "outcome": outcome, "findings": findings, "model": model, "tokens": tokens }).as_object().cloned().unwrap_or_default()
}

/// The number after the found prefix of every case id the part's findings gave so far, the largest.
fn last_number(v: &View, c: &str) -> usize {
    v.of(cases::FOUND, c).flat_map(cases::found_ids).filter_map(|id| id.strip_prefix(FOUND_PREFIX).and_then(|n| n.parse().ok())).max().unwrap_or(0)
}

/// The `cases_found` fields for the synthesis's `finding` and `questions` on `c`, the found cases'
/// ids continuing past the part's earlier findings; kept cases are found ones, their why the note
/// of how many were left out; `questions` only when it asks any.
pub fn fields(v: &View, c: &str, finding: &Finding, questions: &[Value], model: &str, tokens: u64) -> Map<String, Value> {
    let with_ids = |cs: &[Value]| -> Vec<Value> {
        let from = last_number(v, c);
        cs.iter()
            .enumerate()
            .map(|(i, k)| {
                let mut with_id = Map::new();
                with_id.insert("case_id".into(), json!(format!("{FOUND_PREFIX}{}", from + i + 1)));
                with_id.extend(k.as_object().cloned().unwrap_or_default());
                Value::Object(with_id)
            })
            .collect()
    };
    let (outcome, cases, why) = match finding {
        Finding::Found(cs) => ("found", with_ids(cs), String::new()),
        Finding::Kept(cs, listed) => ("found", with_ids(cs), left_out(*listed)),
        Finding::Nothing(why) => ("none", Vec::new(), why.clone()),
        Finding::Failed(why) => ("failed", Vec::new(), why.clone()),
    };
    let mut f = json!({ "criterion_id": c, "outcome": outcome, "cases": cases, "why": why, "model": model, "tokens": tokens });
    if !questions.is_empty() {
        f["questions"] = Value::Array(questions.to_vec());
    }
    f.as_object().cloned().unwrap_or_default()
}

/// The why when a released call did not return normally (Claude Code's PostToolUseFailure on an
/// Agent or Task call): its error, or `FAILED_CALL` when it carried none. The owner's ruling,
/// 2026-10-25: "Record failed".
pub fn failed_spawn(input: &HookInput) -> Option<String> {
    if input.event != HookEvent::PostToolUseFailure || !input.is_spawn() {
        return None;
    }
    Some(input.error.as_deref().map(str::trim).filter(|e| !e.is_empty()).unwrap_or(FAILED_CALL).to_string())
}

/// At the main agent's PostToolUse on an Agent call released as a lens (or the synthesis) whose
/// answer is still owed, record its answer, and push the fact onto `log`; at its
/// PostToolUseFailure, record failed with the error as why. A background launch
/// (`async_launched`), a call that was not released, or an answer once its lens is answered
/// records nothing.
pub(crate) fn hook_facts(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    if !matches!(input.event, HookEvent::PostToolUse | HookEvent::PostToolUseFailure) || !input.is_spawn() {
        return Ok(());
    }
    let Some(id) = input.tool_use_id.as_deref() else {
        return Ok(());
    };
    let (kind, fields) = {
        let v = View::new(log);
        let Some((c, lens)) = owed_release(&v, id) else {
            return Ok(());
        };
        let (text, model, tokens) = match failed_spawn(input) {
            Some(why) => (Err(why), input.input_text("model").unwrap_or("").to_string(), 0),
            None => {
                if input.response_text("status") != Some("completed") {
                    return Ok(());
                }
                let Some(response) = input.tool_response.as_ref() else {
                    return Ok(());
                };
                let model = input.response_text("resolvedModel").or_else(|| input.input_text("model")).unwrap_or("");
                (Ok(final_text(response)), model.to_string(), response.get("totalTokens").and_then(Value::as_u64).unwrap_or(0))
            }
        };
        if lens == cases::SYNTHESIS {
            let (finding, questions) = match text {
                Ok(t) => parse_synthesis(&t),
                Err(why) => (Finding::Failed(why), Vec::new()),
            };
            (cases::FOUND, fields(&v, &c, &finding, &questions, &model, tokens))
        } else {
            let answer = match text {
                Ok(t) => parse_lens(&lens, &t),
                Err(why) => LensAnswer::Failed(why),
            };
            (cases::LENS_FOUND, lens_fields(&c, &lens, &answer, &model, tokens))
        }
    };
    log.push(record.append_raw(kind, fields, now, TENANT).map_err(|e| cannot("write its record", &e))?);
    Ok(())
}

/// The part an answer this call recorded is on: a `lens_found` or `cases_found` among `log`'s
/// facts after the first `before`.
pub(crate) fn recorded_since(log: &[Fact], before: usize) -> Option<&str> {
    log.iter().skip(before).rev().find(|f| f.kind == cases::FOUND || f.kind == cases::LENS_FOUND).and_then(|f| f.fields.get("criterion_id")).and_then(Value::as_str)
}
