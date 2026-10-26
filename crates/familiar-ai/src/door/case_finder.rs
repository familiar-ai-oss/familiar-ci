//! The case-finder's fact (the owner's plan of 2026-10-25, the cases card, §1b) [draft names]:
//! its answer, read at the main agent's PostToolUse on the Agent call that ran it (not at
//! SubagentStop, which does not fire when the turn limit ends the finder), checked against the
//! schema and recorded as `cases_found`: found, none or failed.
//! The case-finder by layer: decide/ the spawn and its reads · door/ the answer and its fact · teach/ the words.

use familiar_ai_facts::{Fact, Log, Record};
use familiar_ai_rules::{cases, plan, View};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::door::{cannot, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::case_finder::{left_out, MAX_CASES};

/// The plugin-scoped name of the case-finder agent [draft name], as `subagent_type` and
/// `agent_type` carry it.
pub const CASE_FINDER: &str = "familiar-ai:case-finder";

/// The prefix of the found cases' ids.
pub const FOUND_PREFIX: &str = "a";

/// The why of a failed call that carried no error [draft words].
pub const FAILED_CALL: &str = "the finder's call failed";

/// True for the case-finder's `agent_type`, plugin-scoped or the bare name a project-level copy carries.
pub fn is_case_finder(agent_type: Option<&str>) -> bool {
    matches!(agent_type, Some(CASE_FINDER) | Some("case-finder"))
}

/// The first part in plan order owed the case-finder that conversation `me` may release it for:
/// the part's holder, or anyone while it has none.
pub fn owed_here(v: &View, me: &str) -> Option<String> {
    plan::criteria(v).into_iter().find(|c| cases::owed(v, c) && plan::assigned_to(v, c).is_none_or(|h| h == me))
}

/// What the finder's answer came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The cases, without their ids.
    Found(Vec<Value>),
    /// The first `MAX_CASES` cases, without their ids, of the number the finder listed, more than
    /// `MAX_CASES` (the owner's ruling, 2026-10-25, "Keep what fits") [draft name]: recorded as
    /// found, its why the note of how many were left out.
    Kept(Vec<Value>, usize),
    /// None found, and why the planned cases cover it.
    Nothing(String),
    /// No valid answer, and why.
    Failed(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnswer {
    cases: Vec<RawCase>,
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

/// The finder's final text: the text blocks of `content` joined, or `content` itself as text.
pub fn final_text(response: &Value) -> String {
    match response.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks.iter().filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// True when Claude Code's note says the turn limit ended the agent ("this agent stopped at its
/// 3-turn limit before finishing ... PARTIAL output").
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

/// Check the finder's final text against the schema. Every case it lists must have each field;
/// past the brief's limits the answer keeps what fits (the owner's ruling, 2026-10-25): the first
/// `MAX_CASES` of a longer list, and a short line of any length whole.
pub fn parse(text: &str) -> Finding {
    if cut_off(text) {
        return Finding::Failed("the case-finder stopped at its turn limit before it answered".to_string());
    }
    let raw: RawAnswer = match serde_json::from_str(unfenced(text)) {
        Ok(raw) => raw,
        Err(e) => return Finding::Failed(format!("the case-finder's answer is not the JSON asked for: {e}")),
    };
    let mut cases = Vec::new();
    for (i, k) in raw.cases.iter().enumerate() {
        let n = i + 1;
        for (name, value) in [("short", &k.short), ("given", &k.given), ("expect", &k.expect), ("why", &k.why)] {
            if value.trim().is_empty() {
                return Finding::Failed(format!("the case-finder's case {n} has no {name}"));
            }
        }
        cases.push(json!({ "short": k.short.trim(), "given": k.given.trim(), "expect": k.expect.trim(), "why": k.why.trim() }));
    }
    if cases.len() > MAX_CASES {
        let listed = cases.len();
        cases.truncate(MAX_CASES);
        return Finding::Kept(cases, listed);
    }
    if !cases.is_empty() {
        return Finding::Found(cases);
    }
    match raw.none_because.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
        Some(why) => Finding::Nothing(why.to_string()),
        None => Finding::Failed("the case-finder listed no case and gave no none_because".to_string()),
    }
}

/// The why when the case-finder's own spawn did not return normally (Claude Code's
/// PostToolUseFailure on an Agent or Task call whose `subagent_type` is the finder): its error,
/// or `FAILED_CALL` when it carried none. The owner's ruling, 2026-10-25: "Record failed".
pub fn failed_spawn(input: &HookInput) -> Option<String> {
    if input.event != HookEvent::PostToolUseFailure || !input.is_spawn() || !is_case_finder(input.input_text("subagent_type")) {
        return None;
    }
    Some(input.error.as_deref().map(str::trim).filter(|e| !e.is_empty()).unwrap_or(FAILED_CALL).to_string())
}

/// The number after the found prefix of every case id the part's findings gave so far, the largest.
fn last_number(v: &View, c: &str) -> usize {
    v.of(cases::FOUND, c).flat_map(cases::found_ids).filter_map(|id| id.strip_prefix(FOUND_PREFIX).and_then(|n| n.parse().ok())).max().unwrap_or(0)
}

/// The `cases_found` fields for `finding` on `c`, the found cases' ids continuing past the part's
/// earlier findings; kept cases are found ones, their why the note of how many were left out.
pub fn fields(v: &View, c: &str, finding: &Finding, model: &str, tokens: u64) -> Map<String, Value> {
    let with_ids = |cs: &[Value]| -> Vec<Value> {
        let from = last_number(v, c);
        cs.iter()
            .enumerate()
            .map(|(i, k)| {
                let mut k = k.clone();
                if let Some(o) = k.as_object_mut() {
                    let mut with_id = Map::new();
                    with_id.insert("case_id".into(), json!(format!("{FOUND_PREFIX}{}", from + i + 1)));
                    with_id.extend(o.clone());
                    *o = with_id;
                }
                k
            })
            .collect()
    };
    let (outcome, cases, why) = match finding {
        Finding::Found(cs) => ("found", with_ids(cs), String::new()),
        Finding::Kept(cs, listed) => ("found", with_ids(cs), left_out(*listed)),
        Finding::Nothing(why) => ("none", Vec::new(), why.clone()),
        Finding::Failed(why) => ("failed", Vec::new(), why.clone()),
    };
    let f = json!({ "criterion_id": c, "outcome": outcome, "cases": cases, "why": why, "model": model, "tokens": tokens });
    f.as_object().cloned().unwrap_or_default()
}

/// At the main agent's PostToolUse on the Agent call that ran the case-finder to the end, record
/// its answer on the part owed it, and push the fact onto `log`; at its PostToolUseFailure, record
/// failed with the error as why. A background launch (`async_launched`), another agent's answer,
/// or an answer once nothing is owed records nothing.
pub(crate) fn hook_facts(record: &Record, input: &HookInput, log: &mut Log, now: f64) -> Result<(), String> {
    let finding = match failed_spawn(input) {
        Some(why) => (Finding::Failed(why), input.input_text("model").unwrap_or("").to_string(), 0),
        None => {
            if input.event != HookEvent::PostToolUse || !input.is_spawn() || !is_case_finder(input.response_text("agentType")) {
                return Ok(());
            }
            if input.response_text("status") != Some("completed") {
                return Ok(());
            }
            let Some(response) = input.tool_response.as_ref() else {
                return Ok(());
            };
            let model = input.response_text("resolvedModel").or_else(|| input.input_text("model")).unwrap_or("");
            let tokens = response.get("totalTokens").and_then(Value::as_u64).unwrap_or(0);
            (parse(&final_text(response)), model.to_string(), tokens)
        }
    };
    let Some(me) = input.agent_conversation_id() else {
        return Ok(());
    };
    let fields = {
        let v = View::new(log);
        let Some(c) = owed_here(&v, &me) else {
            return Ok(());
        };
        let (finding, model, tokens) = &finding;
        fields(&v, &c, finding, model, *tokens)
    };
    let fact = record.append_raw(cases::FOUND, fields, now, TENANT).map_err(|e| cannot("write its record", &e))?;
    log.push(fact);
    Ok(())
}

/// The part a finding this call recorded is on: a `cases_found` among `log`'s facts after the first `before`.
pub(crate) fn recorded_since(log: &[Fact], before: usize) -> Option<&str> {
    log.iter().skip(before).rev().find(|f| f.kind == cases::FOUND).and_then(|f| f.fields.get("criterion_id")).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;

    fn case(short: &str) -> Value {
        json!({ "short": short, "given": "g", "expect": "e", "why": "w" })
    }

    #[test]
    fn an_answer_bare_or_fenced_gives_its_cases() {
        let three = json!({ "cases": [case("one"), case("two"), case("three")] }).to_string();
        let want = Finding::Found(vec![case("one"), case("two"), case("three")]);
        assert_eq!(parse(&three), want);
        assert_eq!(parse(&format!("```json\n{three}\n```")), want);
        assert_eq!(parse(&format!("Found these.\n```\n{three}\n```\nDone.")), want);
        let none = json!({ "cases": [], "none_because": " n1 covers it " }).to_string();
        assert_eq!(parse(&none), Finding::Nothing("n1 covers it".to_string()));
    }

    #[test]
    fn a_cut_off_or_wrong_answer_is_failed_with_why() {
        let note = "NOTE: this agent stopped at its 25-turn limit before finishing. The text below is PARTIAL output; treat it as incomplete.\n{\"cases\":[]}";
        assert_eq!(parse(note), Finding::Failed("the case-finder stopped at its turn limit before it answered".to_string()));
        let failed = |t: &str| matches!(parse(t), Finding::Failed(w) if !w.is_empty());
        // updated with the definition (the owner's ruling, 2026-10-25, "Keep what fits"): nine
        // cases and a short line over 70 characters are kept, not failed (the test below)
        let eight = json!({ "cases": (0..8).map(|i| case(&format!("c{i}"))).collect::<Vec<_>>() }).to_string();
        assert!(matches!(parse(&eight), Finding::Found(cs) if cs.len() == 8));
        assert!(matches!(parse(&json!({ "cases": [case(&"x".repeat(70))] }).to_string()), Finding::Found(_)));
        assert!(failed(&json!({ "cases": [{ "short": "s", "given": "g", "expect": "e", "why": "w", "more": 1 }] }).to_string()));
        assert!(failed(&json!({ "cases": [], "extra": "" }).to_string()));
        assert!(failed(&json!({ "cases": [{ "short": "s", "given": "g", "expect": "e" }] }).to_string()));
        assert!(failed(&json!({ "cases": [{ "short": "s", "given": "", "expect": "e", "why": "w" }] }).to_string()));
        assert!(failed(&json!({ "cases": [] }).to_string()));
        assert!(failed(&json!({ "cases": [], "none_because": "  " }).to_string()));
        assert!(failed("everything is covered"));
        assert!(failed(""));
    }

    /// "Keep what fits" (the owner's ruling, 2026-10-25) [draft words]: past the brief's limits the
    /// answer is kept, not failed: the first `MAX_CASES` of a longer list, the found finding's why
    /// saying how many were left out, and a short line over the brief's 70 characters whole. A case
    /// missing a field still fails the whole answer, wherever it stands in the list.
    #[test]
    fn an_answer_past_the_briefs_limits_keeps_what_fits() {
        let log: Log = Vec::new();
        let v = View::new(&log);
        let listed = |n: usize| json!({ "cases": (1..=n).map(|i| case(&format!("c{i}"))).collect::<Vec<_>>() }).to_string();
        let recorded = |text: &str| fields(&v, "c1", &parse(text), "m", 0);
        let ids_and_shorts = |f: &Map<String, Value>| -> Vec<String> {
            f.get("cases").and_then(Value::as_array).map(|cs| cs.iter().map(|k| format!("{} {}", k["case_id"].as_str().unwrap_or(""), k["short"].as_str().unwrap_or(""))).collect()).unwrap_or_default()
        };
        let found = Some(&json!("found"));

        let nine = recorded(&listed(9));
        assert_eq!((nine.get("outcome"), nine.get("why")), (found, Some(&json!("the finder listed 9, the first 8 are kept: 1 left out"))), "{nine:?}");
        assert_eq!(ids_and_shorts(&nine), (1..=8).map(|i| format!("a{i} c{i}")).collect::<Vec<_>>(), "the first eight, in the finder's order");
        let twelve = recorded(&listed(12));
        assert_eq!((twelve.get("outcome"), twelve.get("why")), (found, Some(&json!("the finder listed 12, the first 8 are kept: 4 left out"))), "{twelve:?}");
        assert_eq!(ids_and_shorts(&twelve).len(), 8);
        let eight = recorded(&listed(8));
        assert_eq!((eight.get("outcome"), eight.get("why")), (found, Some(&json!(""))), "nothing left out, no note");

        let long = "the same greeting when the name comes in capitals, lower case or mixed case, as typed";
        assert!(long.chars().count() > 70);
        for short in [long.to_string(), "x".repeat(71), "x".repeat(400)] {
            let f = recorded(&json!({ "cases": [case(&short)] }).to_string());
            assert_eq!((f.get("outcome"), f.get("why")), (found, Some(&json!(""))), "{f:?}");
            assert_eq!(ids_and_shorts(&f), [format!("a1 {short}")], "the long line whole");
        }

        let mut blank_ninth: Vec<Value> = (1..=9).map(|i| case(&format!("c{i}"))).collect();
        blank_ninth[8]["given"] = json!(" ");
        assert_eq!(parse(&json!({ "cases": blank_ninth }).to_string()), Finding::Failed("the case-finder's case 9 has no given".to_string()));
        let mut no_why_ninth: Vec<Value> = (1..=9).map(|i| case(&format!("c{i}"))).collect();
        if let Some(o) = no_why_ninth[8].as_object_mut() {
            o.remove("why");
        }
        assert!(matches!(parse(&json!({ "cases": no_why_ninth }).to_string()), Finding::Failed(w) if w.contains("why")));
    }

    #[test]
    fn the_final_text_joins_the_text_blocks() {
        let r = json!({ "content": [{ "type": "text", "text": "a" }, { "type": "image" }, { "type": "text", "text": "b" }] });
        assert_eq!(final_text(&r), "a\nb");
        assert_eq!(final_text(&json!({ "content": "c" })), "c");
        assert_eq!(final_text(&json!({})), "");
    }

    #[test]
    fn found_ids_continue_past_the_parts_earlier_findings() {
        let log: Log = Vec::new();
        let v = View::new(&log);
        let first = fields(&v, "c1", &Finding::Found(vec![case("one"), case("two")]), "m", 7);
        assert_eq!(first.get("cases"), Some(&json!([
            { "case_id": "a1", "short": "one", "given": "g", "expect": "e", "why": "w" },
            { "case_id": "a2", "short": "two", "given": "g", "expect": "e", "why": "w" },
        ])));
        assert_eq!((first.get("outcome"), first.get("why"), first.get("tokens")), (Some(&json!("found")), Some(&json!("")), Some(&json!(7))));
        let log = append_raw(&log, "cases_found", first, 0.0, "t").unwrap_or_default();
        let v = View::new(&log);
        let again = fields(&v, "c1", &Finding::Found(vec![case("three")]), "m", 0);
        assert_eq!(again.get("cases").and_then(|cs| cs[0].get("case_id")), Some(&json!("a3")));
        let elsewhere = fields(&v, "c2", &Finding::Found(vec![case("x")]), "m", 0);
        assert_eq!(elsewhere.get("cases").and_then(|cs| cs[0].get("case_id")), Some(&json!("a1")), "another part counts its own");
        let none = fields(&v, "c1", &Finding::Nothing("covered".into()), "m", 0);
        assert_eq!((none.get("outcome"), none.get("cases"), none.get("why")), (Some(&json!("none")), Some(&json!([])), Some(&json!("covered"))));
    }

    /// A PostToolUseFailure as assumed from the Agent SDK's PostToolUseFailureHookInput (not a
    /// capture): every field but hook_event_name optional, `error` a string.
    fn failure(rest: Value) -> HookInput {
        let mut j = json!({ "hook_event_name": "PostToolUseFailure", "session_id": "s1", "tool_use_id": "toolu_1" });
        if let (Some(o), Some(r)) = (j.as_object_mut(), rest.as_object()) {
            o.extend(r.clone());
        }
        HookInput::parse(&j.to_string(), HookEvent::PostToolUseFailure).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn the_finders_failed_spawn_is_failed_with_the_error_as_why() {
        let spawn = |tool: &str, ty: &str| json!({ "tool_name": tool, "tool_input": { "subagent_type": ty, "model": "sonnet", "prompt": "brief" } });
        let mut with = spawn("Agent", CASE_FINDER);
        with["error"] = json!("Agent type 'familiar-ai:case-finder' not found");
        with["is_interrupt"] = json!(false);
        let input = failure(with);
        assert_eq!((input.error.as_deref(), input.is_interrupt), (Some("Agent type 'familiar-ai:case-finder' not found"), Some(false)));
        assert_eq!(failed_spawn(&input), Some("Agent type 'familiar-ai:case-finder' not found".to_string()));
        assert_eq!(failed_spawn(&failure(spawn("Task", "case-finder"))), Some(FAILED_CALL.to_string()), "Task, the bare name, no error");
        let mut blank = spawn("Agent", CASE_FINDER);
        blank["error"] = json!("  ");
        assert_eq!(failed_spawn(&failure(blank)), Some(FAILED_CALL.to_string()));
        let mut odd = spawn("Agent", CASE_FINDER);
        odd["error"] = json!({ "message": "boom" });
        assert_eq!(failed_spawn(&failure(odd)), Some(r#"{"message":"boom"}"#.to_string()), "an error that is not text is kept as its JSON");
        assert_eq!(failed_spawn(&failure(spawn("Agent", "general-purpose"))), None, "another agent's failure");
        assert_eq!(failed_spawn(&failure(json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } }))), None);
        assert_eq!(failed_spawn(&failure(json!({}))), None, "nothing named");
        let post = HookInput::parse(&json!({ "hook_event_name": "PostToolUse", "tool_name": "Agent", "tool_input": { "subagent_type": CASE_FINDER } }).to_string(), HookEvent::PostToolUse).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(failed_spawn(&post), None, "a PostToolUse is read by its answer");

        let log: Log = Vec::new();
        let f = fields(&View::new(&log), "c1", &Finding::Failed(FAILED_CALL.to_string()), "sonnet", 0);
        assert_eq!(Value::Object(f), json!({ "criterion_id": "c1", "outcome": "failed", "cases": [], "why": FAILED_CALL, "model": "sonnet", "tokens": 0 }));
    }

    #[test]
    fn the_finders_failed_call_is_recorded_while_it_is_owed() {
        let dir = std::env::temp_dir().join(format!("familiar-case-finder-failed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        let record = Record::open(&dir.join("record")).unwrap_or_else(|e| panic!("{e}"));
        let add = |kind: &str, f: Value| record.append_raw(kind, f.as_object().cloned().unwrap_or_default(), 0.5, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
        add("intent_submitted", json!({ "text": "greet with hello", "sha": "abc" }));
        add("behavior", json!({ "behavior_id": "b1", "text": "the greeting" }));
        let cases = json!([{ "case_id": "n1", "kind": "normal", "given": "Ada", "expect": "hello Ada" }, { "case_id": "o1", "kind": "opposite", "given": "no name", "expect": "no greeting" }]);
        add("promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing", "cases": cases, "contract_version": 1 }));
        add("criterion", json!({ "criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
        let mut log = record.read_all().unwrap_or_else(|e| panic!("{e}"));
        assert!(cases::owed(&View::new(&log), "c1"));
        let input = failure(json!({ "tool_name": "Agent", "tool_input": { "subagent_type": CASE_FINDER, "model": "haiku" }, "error": "Interrupted by user", "is_interrupt": true }));
        hook_facts(&record, &input, &mut log, 1.0).unwrap_or_else(|e| panic!("{e}"));
        let f = log.last().filter(|f| f.kind == cases::FOUND).unwrap_or_else(|| panic!("no finding: {log:?}"));
        assert_eq!((f.fields.get("outcome"), f.fields.get("why"), f.fields.get("model")), (Some(&json!("failed")), Some(&json!("Interrupted by user")), Some(&json!("haiku"))));
        assert_eq!(recorded_since(&log, log.len() - 1), Some("c1"));
        assert!(!cases::owed(&View::new(&log), "c1"), "the card, not the finder, is next");
        let n = log.len();
        hook_facts(&record, &input, &mut log, 2.0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(log.len(), n, "once nothing is owed, a failure records nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_finders_names() {
        assert!(is_case_finder(Some(CASE_FINDER)) && is_case_finder(Some("case-finder")));
        assert!(!is_case_finder(Some("familiar-ai:investigator")) && !is_case_finder(None));
    }
}
