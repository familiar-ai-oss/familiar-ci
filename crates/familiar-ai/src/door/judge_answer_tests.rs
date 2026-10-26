//! Prove the judge's answer is read from structured output or strictly from one
//! JSON object, and that every call with no verdict names why.

use super::*;
use serde_json::json;
use std::time::Duration;

fn ran(ended: Ended, stdout: &str) -> Ran {
    Ran { ended, wall: Duration::from_secs(1), stdout: stdout.to_string(), stderr: String::new() }
}

fn exit0(stdout: &str) -> Ran {
    ran(Ended::Exited(Some(0)), stdout)
}

#[test]
fn the_structured_output_is_read_first_and_the_text_is_the_fallback() {
    // A captured live answer with `--json-schema`: the answer in `structured_output` and again as text.
    let live = include_str!("../../tests/fixtures/judge_structured_live.json");
    let a = parse(live, "opus").unwrap_or_else(|| panic!("the live answer"));
    assert_eq!((a.model.as_str(), a.answers_intent), ("claude-opus-5-5", Some(AnswersIntent::No)));
    assert!(a.gap.starts_with("Path 2, refusing an empty name") && a.reason.starts_with("The intent covers two paths."), "{a:?}");
    // the structured field wins over a `result` text that is not the one object
    let mut outer: Value = serde_json::from_str(live).unwrap_or_default();
    outer["result"] = json!("Here is my answer: {\"answers_intent\": true}");
    assert_eq!(parse(&outer.to_string(), "opus").map(|a| a.answers_intent), Some(Some(AnswersIntent::No)));
    // a success with no structured output falls back to the text, strictly
    let text = json!({ "type": "result", "subtype": "success", "is_error": false, "result": "{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"r\"}" });
    assert_eq!(parse(&text.to_string(), "opus").and_then(|a| a.answers_intent), Some(AnswersIntent::Yes));
    // a structured output that is not the one form falls back too
    let off = json!({ "result": "{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"r\"}", "structured_output": { "answers_intent": true } });
    assert_eq!(parse(&off.to_string(), "opus").and_then(|a| a.answers_intent), Some(AnswersIntent::Yes));
    // a sentence before the JSON, with no structured output, is unreadable
    let sentence = json!({ "result": "Looking at the test:\n{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"r\"}" });
    assert_eq!(read(&exit0(&sentence.to_string()), "opus").map_err(|e| e.0), Err(JudgeUnansweredWhy::Unparseable));
}

#[test]
fn every_call_with_no_verdict_says_why_from_the_closed_set() {
    let why = |r: Ran| read(&r, "opus").map(|_| ());
    assert_eq!(why(ran(Ended::TimedOut, "")), Err((JudgeUnansweredWhy::Timeout, "stopped after 1 s".to_string())));
    assert_eq!(why(ran(Ended::Exited(Some(1)), "")), Err((JudgeUnansweredWhy::Error, "exit 1".to_string())));
    assert_eq!(why(ran(Ended::NotStarted("gone".into()), "")).map_err(|e| e.0), Err(JudgeUnansweredWhy::Error));
    let api = json!({ "type": "result", "subtype": "success", "is_error": true, "result": "API Error: 529 overloaded" }).to_string();
    assert_eq!(why(exit0(&api)), Err((JudgeUnansweredWhy::Error, "exit 0, but Claude Code reported an error".to_string())));
    let during = json!({ "type": "result", "subtype": "error_during_execution", "is_error": true }).to_string();
    assert_eq!(why(exit0(&during)), Err((JudgeUnansweredWhy::Error, "exit 0, error_during_execution".to_string())));
    let gave_up = json!({ "type": "result", "subtype": NO_STRUCTURED, "is_error": true }).to_string();
    assert_eq!(why(exit0(&gave_up)).map_err(|e| e.0), Err(JudgeUnansweredWhy::Unparseable));
    assert_eq!(why(ran(Ended::Exited(Some(1)), &gave_up)), Err((JudgeUnansweredWhy::Unparseable, format!("exit 1, {NO_STRUCTURED}"))));
    assert_eq!(why(exit0("not json")).map_err(|e| e.0), Err(JudgeUnansweredWhy::Unparseable));
    // cannot_see is an answer read (false, no gap); the caller names it
    let unseen = json!({ "structured_output": { "answers_intent": false, "gap": "", "reason": "the test is not visible" } }).to_string();
    assert_eq!(read(&exit0(&unseen), "opus").map(|a| a.answers_intent), Ok(None));
}

#[test]
fn only_one_json_object_with_the_three_fields_is_an_answer() {
    let wrap = |result: &str| json!({ "type": "result", "is_error": false, "result": result, "modelUsage": { "claude-haiku-4-5": {}, "claude-sonnet-4-5-20250929": {} } }).to_string();
    let answer = |model: &str, a: AnswersIntent, gap: &str, reason: &str| Some(Answer { model: model.into(), answers_intent: Some(a), gap: gap.into(), reason: reason.into(), cases: Vec::new(), places: Vec::new() });
    let yes = parse(&wrap(r#"{"answers_intent": true, "gap": "", "reason": "it checks the asked error."}"#), "sonnet");
    assert_eq!(yes, answer("claude-sonnet-4-5-20250929", AnswersIntent::Yes, "", "it checks the asked error."));
    let no = parse(&json!({ "result": r#"{"answers_intent": false, "gap": "the alias", "reason": "r"}"# }).to_string(), "opus");
    assert_eq!(no, answer("opus", AnswersIntent::No, "the alias", "r"), "no modelUsage: the alias");
    for fenced in ["```json\n{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"r\"}\n```", "```\n{\"answers_intent\": true, \"gap\": \" \", \"reason\": \"r\"}\n```\n"] {
        assert_eq!(parse(&wrap(fenced), "sonnet").and_then(|a| a.answers_intent), Some(AnswersIntent::Yes), "{fenced}");
    }
    // An older captured answer lacks the gap field, so it is refused as is and read once a gap is added.
    let captured = include_str!("../../tests/fixtures/judge_result_live.json");
    assert_eq!(parse(captured, "sonnet"), None, "a form with no gap is not an answer");
    let mut outer: Value = serde_json::from_str(captured).unwrap_or_default();
    let mut inner: Map<String, Value> = serde_json::from_str(outer["result"].as_str().unwrap_or("")).unwrap_or_default();
    inner.insert("gap".into(), json!("greet saying hello when it runs"));
    outer["result"] = json!(Value::Object(inner).to_string());
    let live = outer.to_string();
    let a = parse(&live, "sonnet").unwrap_or_else(|| panic!("the live answer"));
    assert_eq!((a.model.as_str(), a.answers_intent, a.gap.as_str()), ("claude-sonnet-5-5", Some(AnswersIntent::No), "greet saying hello when it runs"));
    assert!(a.reason.starts_with("The test only greps for the string 'hello'"), "{}", a.reason);
    // the first live call (full setup loaded) printed every message as a list
    let listed = json!([{ "type": "system" }, outer]).to_string();
    assert_eq!(parse(&listed, "sonnet").and_then(|a| a.answers_intent), Some(AnswersIntent::No));
    // false with an empty gap means the judge cannot see the tests
    let unseen = parse(&wrap(r#"{"answers_intent": false, "gap": "", "reason": "the test is not visible"}"#), "sonnet");
    assert_eq!(unseen.map(|a| (a.answers_intent, a.reason)), Some((None, "the test is not visible".to_string())));
    assert_eq!(parse(&json!([{ "type": "system" }]).to_string(), "sonnet"), None, "a list with no result");
    for bad in [
        wrap("yes"),
        wrap("```json\n{\"answers_intent\": yes, \"reason\": \"r\"}\n```"),
        wrap("```json {\"answers_intent\": true, \"reason\": \"r\"}```"),
        wrap("sure: ```json\n{\"answers_intent\": true, \"reason\": \"r\"}\n```"),
        wrap(r#"{"answers_intent": "yes", "gap": "", "reason": "r"}"#),
        wrap(r#"{"answers_intent": true, "gap": "", "reason": ""}"#),
        wrap(r#"{"answers_intent": true, "gap": "", "reason": "r", "more": 1}"#),
        wrap(r#"{"answers_intent": true, "reason": "r"}"#),
        wrap(r#"{"answers_intent": true, "gap": "the alias", "reason": "r"}"#),
        wrap(r#"{"answers_intent": false, "gap": 1, "reason": "r"}"#),
        json!({ "is_error": true, "result": r#"{"answers_intent": true, "reason": "r"}"# }).to_string(),
        "not json".to_string(),
    ] {
        assert_eq!(parse(&bad, "sonnet"), None, "{bad}");
    }
}

// the contract [draft names]: an answer may carry, for each case, the test that exercises it; the
// old answer of three fields still reads, with no cases
#[test]
fn an_answer_may_name_a_test_per_case_and_the_old_answer_still_reads() {
    let wrap = |result: &str| json!({ "type": "result", "is_error": false, "result": result, "modelUsage": { "claude-opus-4-6": {} } }).to_string();
    let old = parse(&wrap(r#"{"answers_intent": true, "gap": "", "reason": "r"}"#), "opus").unwrap_or_else(|| panic!("the old answer"));
    assert!(old.cases.is_empty());
    let with = parse(
        &wrap(r#"{"answers_intent": true, "gap": "", "reason": "r", "cases": [{"case_id": "n1", "test": "test_a", "ok": true}, {"case_id": "o1", "test": "none", "ok": false}]}"#),
        "opus",
    )
    .unwrap_or_else(|| panic!("the answer with cases"));
    assert_eq!(
        with.cases,
        vec![
            familiar_ai_facts::CaseTest { case_id: "n1".into(), test: "test_a".into(), ok: true },
            familiar_ai_facts::CaseTest { case_id: "o1".into(), test: "none".into(), ok: false }
        ]
    );
    let structured = json!({ "type": "result", "structured_output": { "answers_intent": false, "gap": "case o1", "reason": "r", "cases": [{"case_id": "o1", "test": "none", "ok": false}] } }).to_string();
    assert_eq!(parse(&structured, "opus").map(|a| a.cases.len()), Some(1), "read from the structured output too");
    for bad in [
        r#"{"answers_intent": true, "gap": "", "reason": "r", "cases": "n1"}"#,
        r#"{"answers_intent": true, "gap": "", "reason": "r", "cases": [{"case_id": "n1", "test": "t"}]}"#,
        r#"{"answers_intent": true, "gap": "", "reason": "r", "cases": [], "more": 1}"#,
    ] {
        assert_eq!(parse(&wrap(bad), "opus"), None, "{bad}");
    }
    // the schema asked when the criterion has a contract wants the cases; the old one stays as it was
    let schema: Value = serde_json::from_str(CONTRACT_SCHEMA).unwrap_or_default();
    assert_eq!(schema["required"], json!(["answers_intent", "gap", "reason", "cases"]));
    assert_eq!(schema["properties"]["cases"]["items"]["required"], json!(["case_id", "test", "ok"]));
    assert!(!SCHEMA.contains("cases"));
}
