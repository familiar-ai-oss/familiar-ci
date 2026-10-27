//! The cases card asks the lens panel's questions (the owner's plan of 2026-10-26, lens-panel.md,
//! approved) [draft words]: the agent asks the card with ONE AskUserQuestion call; the hook sets the
//! card's own question (its lines, as before) and one question per open panel question, each with
//! the panel's options (Claude Code adds its own "Other" for words typed). The picker records the
//! card's own choice as `hold_answered`, as before, and each question's pick as
//! `question_answered`: the picked option's text, or `typed` with the words. The card stays open
//! until its choice and every question has an answer; a partial answer asks again only what is
//! open. The pick is the person's: `familiar-ai claim question_answered` from an agent's shell is
//! refused, as a card's answer is.

mod cases_fixture;
mod common;

use common::Project;
use familiar_ai::teach::{cases_card, picker};
use familiar_ai_facts::Fact;
use familiar_ai_rules::terminal::{self, Card};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const CARD: &str = "cases_unconfirmed";
const Q1: &str = "Should a name typed in capitals be greeted in capitals?";
const Q2: &str = "Should an empty name be greeted?";

fn the_card(log: &familiar_ai_facts::Log) -> Card {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == CARD).unwrap_or_else(|| panic!("the cases card"))
}

/// c1's synthesis (after its five lenses) with one found case and the two questions.
fn asked(log: &familiar_ai_facts::Log) -> familiar_ai_facts::Log {
    let log = cases_fixture::lensed(log);
    let qs = json!([
        { "question_id": "q1", "text": Q1, "options": ["as typed", "in lower case"], "why": "main lowercases it" },
        { "question_id": "q2", "text": Q2, "options": ["yes", "no", "only with a warning"], "why": "" }
    ]);
    let cs = json!([{ "case_id": "a1", "short": "a name in capitals", "given": "ADA", "expect": "hello ADA", "why": "w" }]);
    cases_fixture::raw(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "found", "cases": cs, "why": "", "model": "claude-opus-5-5", "tokens": 6000, "questions": qs }))
}

const Q1_SHOWN: &str = "Should a name typed in capitals be greeted in capitals?\n\nWhy it matters: main lowercases it";

#[test]
fn the_card_input_holds_the_cards_question_then_one_per_open_panel_question() {
    let log = asked(&cases_fixture::planned());
    let v = View::new(&log);
    let card = the_card(&log);
    let input = picker::card_input(&v, &card, "the card's own question");
    let qs = input["questions"].as_array().cloned().unwrap_or_default();
    assert_eq!(qs.len(), 3, "{input}");
    assert_eq!(qs[0]["question"], json!("the card's own question"));
    assert_eq!(qs[0]["options"], picker::card_options(&v, &card), "the card's own options, as before");
    assert_eq!(
        qs[1],
        json!({ "question": Q1_SHOWN, "header": "familiar-ai", "options": [{ "label": "as typed", "description": "" }, { "label": "in lower case", "description": "" }], "multiSelect": false })
    );
    assert_eq!(qs[2]["question"], json!(Q2), "no why, the text alone");
    assert_eq!(qs[2]["options"].as_array().map(Vec::len), Some(3));
    assert!(qs.iter().all(|q| q["header"] == json!("familiar-ai")));

    // once the choice is answered, only the open questions; once q1 is answered, only q2
    let log = cases_fixture::answered(&log, "keep_planned", "");
    let v = View::new(&log);
    let input = picker::card_input(&v, &the_card(&log), "ignored");
    assert_eq!(input["questions"].as_array().map(|qs| qs.iter().map(|q| q["question"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>()), Some(vec![Q1_SHOWN.to_string(), Q2.to_string()]));
    let log = cases_fixture::raw(&log, "question_answered", json!({ "criterion_id": "c1", "question_id": "q1", "choice": "as typed", "typed": "" }));
    let input = picker::card_input(&View::new(&log), &the_card(&log), "ignored");
    assert_eq!(input["questions"].as_array().map(|qs| qs.iter().map(|q| q["question"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>()), Some(vec![Q2.to_string()]));
}

#[test]
fn the_agent_is_told_the_hook_adds_the_panels_questions_to_the_same_call() {
    let log = asked(&cases_fixture::planned());
    let v = View::new(&log);
    let f = picker::facts(&v, &the_card(&log));
    assert!(f.contains("The hook adds the lens panel's 2 questions to the same call, one question each, with their options."), "{f}");
    let log = cases_fixture::answered(&log, "keep_planned", "");
    let f = picker::facts(&View::new(&log), &the_card(&log));
    assert!(f.starts_with("familiar-ai: the lens panel's questions are open for part 1 of what they asked. Ask the person with AskUserQuestion"), "{f}");
    assert!(f.contains("do not claim question_answered yourself"), "{f}");
}

/// A full-path project whose panel asked two questions, through the hooks: the card is open.
fn project(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_LENS_MODEL").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"the name Ada","expect":"hello Ada"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
    ] {
        p.claim(&args);
    }
    p.lenses_answer_none();
    let answer = json!({ "cases": [], "none_because": "n1 and o1 cover it", "questions": [
        { "text": Q1, "options": ["as typed", "in lower case"], "why": "main lowercases it" },
        { "text": Q2, "options": ["yes", "no", "only with a warning"] }
    ] });
    p.synthesis_answers(&answer.to_string());
    p
}

fn hooked(p: &Project, event: &str, fixture: &str, set: Value) -> Value {
    let (code, out, err) = p.hook_raw(event, &p.fixture(fixture, set));
    assert_eq!(code, 0, "{event} {fixture}: {out}{err}");
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{event} printed no JSON ({e}): {out}"))
}

/// The agent's AskUserQuestion with the card's lines word for word: the hook's updatedInput.
fn ask(p: &Project) -> Value {
    let log = p.facts();
    let v = View::new(&log);
    let question = cases_card::shape(&cases_card::shown(&v, &the_card(&log)));
    let asking = json!({ "tool_input": { "questions": [{ "question": question, "header": "x", "options": [{ "label": "yes", "description": "Continue" }], "multiSelect": false }] } });
    let pre = hooked(p, "PreToolUse", "pre_tool_use_ask_user_question", asking);
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("ask"), "{pre}");
    pre["hookSpecificOutput"]["updatedInput"].clone()
}

/// The person's answers in the picker on `asked`: the hook's answer to the PostToolUse.
fn answer(p: &Project, asked: &Value, answers: Value) -> Value {
    let response = json!({ "questions": asked["questions"], "answers": answers, "annotations": {} });
    let mut input = asked.clone();
    input["answers"] = response["answers"].clone();
    hooked(p, "PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
}

fn of_kind(p: &Project, kind: &str) -> Vec<Value> {
    p.facts().into_iter().filter(|f: &Fact| f.kind == kind).map(|f| Value::Object(f.fields)).collect()
}

#[test]
fn one_call_asks_the_card_and_the_panels_questions_and_each_pick_is_recorded() {
    let p = project("one-call");
    let asked = ask(&p);
    let qs = asked["questions"].as_array().cloned().unwrap_or_default();
    assert_eq!(qs.len(), 3, "{asked}");
    let own = qs[0]["question"].as_str().unwrap_or("").to_string();
    let out = answer(&p, &asked, json!({ own: "Looks right (Recommended)", Q1_SHOWN: "as typed", Q2: "only when the name is blank, say so" }));
    assert_eq!(of_kind(&p, "hold_answered"), [json!({ "criterion_id": "c1", "hold": CARD, "choice": "keep_planned", "typed": "" })]);
    assert_eq!(
        of_kind(&p, "question_answered"),
        [
            json!({ "criterion_id": "c1", "question_id": "q1", "choice": "as typed", "typed": "" }),
            json!({ "criterion_id": "c1", "question_id": "q2", "choice": "typed", "typed": "only when the name is blank, say so" })
        ]
    );
    let told = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(told.contains(&format!("the person answered the lens panel's questions for \"greet says hello\": \"{Q1}\": \"as typed\"; \"{Q2}\": \"only when the name is blank, say so\". The answers are recorded.")), "{out}");
    let holds = p.criterion_json("c1")["holds"].to_string();
    assert!(!holds.contains("cases_unconfirmed"), "the card is answered: {holds}");
}

#[test]
fn a_partial_answer_asks_again_only_what_is_open() {
    let p = project("partial");
    let asked = ask(&p);
    let own = asked["questions"][0]["question"].as_str().unwrap_or("").to_string();
    // the card's own and q1; q2 left without an answer
    answer(&p, &asked, json!({ own: "Looks right (Recommended)", Q1_SHOWN: "in lower case" }));
    assert_eq!(of_kind(&p, "question_answered").len(), 1);
    assert!(p.criterion_json("c1")["holds"].to_string().contains("cases_unconfirmed"), "q2 is open");

    // the next call is q2 alone, whatever the agent wrote
    let pre = hooked(&p, "PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": { "questions": [{ "question": "anything", "header": "x", "options": [{ "label": "a", "description": "b" }], "multiSelect": false }] } }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("ask"), "{pre}");
    let again = pre["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(again["questions"].as_array().map(|qs| qs.iter().map(|q| q["question"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>()), Some(vec![Q2.to_string()]), "{again}");
    let out = answer(&p, &again, json!({ Q2: "no" }));
    assert_eq!(of_kind(&p, "question_answered").last(), Some(&json!({ "criterion_id": "c1", "question_id": "q2", "choice": "no", "typed": "" })));
    assert_eq!(of_kind(&p, "hold_answered").len(), 1, "the card's choice is not asked again");
    assert!(!p.criterion_json("c1")["holds"].to_string().contains("cases_unconfirmed"), "the card is answered");
    let told = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(told.contains("The answers are recorded. The planned cases stand for \"greet says hello\": write its test."), "the card's choice leads on: {out}");
}

#[test]
fn an_answer_to_a_question_not_asked_as_the_hook_set_it_records_nothing() {
    let p = project("not-set");
    let asked = ask(&p);
    let mut changed = asked.clone();
    changed["questions"][1]["options"] = json!([{ "label": "as typed", "description": "" }, { "label": "something else", "description": "" }]);
    answer(&p, &changed, json!({ Q1_SHOWN: "as typed" }));
    assert!(of_kind(&p, "question_answered").is_empty());
}

#[test]
fn the_agents_own_question_answered_is_refused_and_told_to_ask_with_the_picker() {
    let p = project("claim");
    let (code, _, err) = p.run_as(Some("84be6339-0cc7-40ec-91b9-cae44365931b"), &["claim", "question_answered", "--criterion_id", "c1", "--question_id", "q1", "--choice", "as typed", "--typed", ""], "");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("this card is the person's to answer: ask the person with AskUserQuestion"), "{err}");
    assert!(of_kind(&p, "question_answered").is_empty());
    // the shell command itself is refused by the hook, read from its text
    let pre = p.tool("PreToolUse", "Bash", json!({ "command": "familiar-ai claim question_answered --criterion_id c1 --question_id q1 --choice 'as typed' --typed ''" }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{pre}");
    // the panel's own facts are the binary's
    for kind in ["lens_released", "lens_found"] {
        let (code, _, err) = p.try_claim(&[kind, "--criterion_id", "c1"]);
        assert_ne!(code, 0, "{kind}");
        assert!(err.contains("is written by familiar-ai itself"), "{kind}: {err}");
    }
}
