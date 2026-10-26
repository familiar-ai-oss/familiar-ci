//! Read the judge's answer from Claude Code's result, or say why a call gave
//! none. The `structured_output` is read first; a `success` result without it
//! falls back to the `result` text, where Claude Code's docs call it a failure.

use familiar_ai_facts::{AnswersIntent, CaseTest, JudgeUnansweredWhy, PlaceOk};
use familiar_ai_rules::ladder::family_of;
use serde_json::{Map, Value};

use crate::door::child::{Ended, Ran};

/// The `--json-schema` for the answer: three required fields, nothing else.
pub const SCHEMA: &str = r#"{"type":"object","properties":{"answers_intent":{"type":"boolean"},"gap":{"type":"string"},"reason":{"type":"string"}},"required":["answers_intent","gap","reason"],"additionalProperties":false}"#;

/// The `--json-schema` when the criterion's promise gives cases (the contract [draft names]): the
/// three fields and, for each case, the test function that exercises it. `SCHEMA` stays as it was
/// for a criterion with no contract.
pub const CONTRACT_SCHEMA: &str = r#"{"type":"object","properties":{"answers_intent":{"type":"boolean"},"gap":{"type":"string"},"reason":{"type":"string"},"cases":{"type":"array","items":{"type":"object","properties":{"case_id":{"type":"string"},"test":{"type":"string"},"ok":{"type":"boolean"}},"required":["case_id","test","ok"],"additionalProperties":false}}},"required":["answers_intent","gap","reason","cases"],"additionalProperties":false}"#;

/// The `--json-schema` when places were judged not affected (the owner's plan of 2026-10-21, step
/// 3) [draft names]: `CONTRACT_SCHEMA` and, for each such place, the judge's word on its reason.
pub const PLACES_SCHEMA: &str = r#"{"type":"object","properties":{"answers_intent":{"type":"boolean"},"gap":{"type":"string"},"reason":{"type":"string"},"cases":{"type":"array","items":{"type":"object","properties":{"case_id":{"type":"string"},"test":{"type":"string"},"ok":{"type":"boolean"}},"required":["case_id","test","ok"],"additionalProperties":false}},"places":{"type":"array","items":{"type":"object","properties":{"file":{"type":"string"},"line":{"type":"integer"},"ok":{"type":"boolean"}},"required":["file","line","ok"],"additionalProperties":false}}},"required":["answers_intent","gap","reason","cases","places"],"additionalProperties":false}"#;

/// The result subtype Claude Code gives when no answer matched the schema.
pub const NO_STRUCTURED: &str = "error_max_structured_output_retries";

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub model: String,
    /// `None` when the judge cannot see the tests (false with an empty gap), so
    /// the seal stands unjudged rather than refused.
    pub answers_intent: Option<AnswersIntent>,
    /// The path no test exercises; empty when it answers.
    pub gap: String,
    pub reason: String,
    /// For each case of the contract, the test the judge named; empty in an answer without cases.
    pub cases: Vec<CaseTest>,
    /// For each place judged not affected, the judge's word on its reason; empty without places.
    pub places: Vec<PlaceOk>,
}

/// The call's answer, or why there is none with a few words on how it ended.
pub fn read(ran: &Ran, alias: &str) -> Result<Answer, (JudgeUnansweredWhy, String)> {
    let out = answered(ran)?;
    parse(out, alias).ok_or((JudgeUnansweredWhy::Unparseable, "exit 0, but its answer was not the one JSON object".to_string()))
}

/// The call's stdout when Claude Code ended with a result that is not an error, or why not with a
/// few words on how it ended; the reviewer's answer is read from it the same way.
pub fn answered(ran: &Ran) -> Result<&str, (JudgeUnansweredWhy, String)> {
    let gave_up = |out: &str| result_of(out).and_then(|r| r.get("subtype").and_then(Value::as_str).map(str::to_string)).as_deref() == Some(NO_STRUCTURED);
    let Some(out) = ran.answer() else {
        return Err(match ran.ended {
            Ended::TimedOut => (JudgeUnansweredWhy::Timeout, ran.how()),
            _ if gave_up(&ran.stdout) => (JudgeUnansweredWhy::Unparseable, format!("{}, {NO_STRUCTURED}", ran.how())),
            _ => (JudgeUnansweredWhy::Error, ran.how()),
        });
    };
    let outer = result_of(out);
    let subtype = outer.as_ref().and_then(|o| o.get("subtype").and_then(Value::as_str));
    let is_error = outer.as_ref().and_then(|o| o.get("is_error").and_then(Value::as_bool)) == Some(true);
    match (subtype, is_error) {
        (Some(NO_STRUCTURED), _) => return Err((JudgeUnansweredWhy::Unparseable, format!("exit 0, {NO_STRUCTURED}"))),
        (Some(s), _) if s.starts_with("error") => return Err((JudgeUnansweredWhy::Error, format!("exit 0, {s}"))),
        (_, true) => return Err((JudgeUnansweredWhy::Error, "exit 0, but Claude Code reported an error".to_string())),
        _ => {}
    }
    Ok(out)
}

/// Claude Code's result object, alone or the last `result` in a list of messages.
pub(crate) fn result_of(stdout: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(stdout.trim()).ok()? {
        Value::Array(xs) => xs.into_iter().rev().find(|x| x.get("type").and_then(Value::as_str) == Some("result")),
        one => Some(one),
    }
}

/// Read the answer from `structured_output`, else strictly from the `result` text
/// as exactly one JSON object, optionally in one code fence. False with an empty
/// gap means the judge cannot see the tests.
pub fn parse(stdout: &str, alias: &str) -> Option<Answer> {
    let outer = result_of(stdout)?;
    if outer.get("is_error").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let structured = outer.get("structured_output").and_then(Value::as_object).and_then(fields);
    let (yes, gap, reason, cases, places) = match structured {
        Some(f) => f,
        None => fields(&serde_json::from_str(unfenced(outer.get("result")?.as_str()?)).ok()?)?,
    };
    let model = model_of(&outer, alias);
    let answers_intent = match (yes, gap.is_empty()) {
        (true, _) => Some(AnswersIntent::Yes),
        (false, false) => Some(AnswersIntent::No),
        (false, true) => None,
    };
    Some(Answer { model, answers_intent, gap, reason, cases, places })
}

/// The model that answered, as Claude Code's `modelUsage` names it: the one of the alias's family,
/// else the first, else the alias.
pub(crate) fn model_of(outer: &Value, alias: &str) -> String {
    let used: Vec<&String> = outer.get("modelUsage").and_then(Value::as_object).map(|m| m.keys().collect()).unwrap_or_default();
    used.iter().find(|k| family_of(Some(k.as_str())) == family_of(Some(alias))).or(used.first()).map(|k| k.to_string()).unwrap_or_else(|| alias.to_string())
}

/// The answer object of a result: its `structured_output`, else its `result` text as exactly one
/// JSON object, optionally in one code fence; `None` when it is an error or neither.
pub(crate) fn answer_object(outer: &Value) -> Option<Map<String, Value>> {
    if outer.get("is_error").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    if let Some(m) = outer.get("structured_output").and_then(Value::as_object) {
        return Some(m.clone());
    }
    serde_json::from_str::<Value>(unfenced(outer.get("result")?.as_str()?)).ok()?.as_object().cloned()
}

/// The answer's fields, when they are exactly the one form: the three alone (an answer without a
/// contract, or one written before it), with `cases`, or with `cases` and `places`.
type Fields = (bool, String, String, Vec<CaseTest>, Vec<PlaceOk>);

fn fields(answer: &Map<String, Value>) -> Option<Fields> {
    let cases = match answer.get("cases") {
        Some(c) => serde_json::from_value::<Vec<CaseTest>>(c.clone()).ok()?,
        None => Vec::new(),
    };
    let places = match answer.get("places") {
        Some(p) => serde_json::from_value::<Vec<PlaceOk>>(p.clone()).ok()?,
        None => Vec::new(),
    };
    if answer.len() != 3 + usize::from(answer.contains_key("cases")) + usize::from(answer.contains_key("places")) {
        return None;
    }
    let yes = answer.get("answers_intent")?.as_bool()?;
    let gap = answer.get("gap")?.as_str()?.trim().to_string();
    let reason = answer.get("reason")?.as_str()?.trim().to_string();
    if reason.is_empty() || (yes && !gap.is_empty()) {
        return None;
    }
    Some((yes, gap, reason, cases, places))
}

/// `text` without one surrounding code fence (```` ```json ```` or ```` ``` ````).
fn unfenced(text: &str) -> &str {
    let t = text.trim();
    let Some(rest) = t.strip_prefix("```") else {
        return t;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    match rest.strip_suffix("```") {
        Some(inside) if rest.starts_with('\n') => inside.trim(),
        _ => t,
    }
}

#[cfg(test)]
#[path = "judge_answer_tests.rs"]
mod tests;
