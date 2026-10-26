//! The case-finder released through the orchestrator (the owner's plan of 2026-10-25, the cases
//! card, §1b) [draft names]: once a full-path part's contract is whole, the main agent's Agent call
//! is rewritten to the finder with the binary's brief in place of its own prompt, on the model the
//! environment names (sonnet by default; the owner's ruling, 2026-10-26, "keep the new brief on sonnet"); the finder only reads; its answer is read at the main
//! agent's PostToolUse on that call and recorded as `cases_found` (found, none or failed), and the
//! person's card opens. Payloads are the spike's captures (session 6b42509f, spike0).

mod common;

use common::Project;
use familiar_ai_facts::{Fact, Record};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const FINDER: &str = "familiar-ai:case-finder";
/// The finder's agent id in the captures.
const AGENT: &str = "a09dfaf2a8482404c";

/// A full-path project planned to c1's whole contract: the finder is owed.
fn project(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_CASE_FINDER_MODEL").create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"the name Ada","expect":"hello Ada"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
    ] {
        p.claim(&args);
    }
    p
}

impl Project {
    /// A captured payload re-pointed at this project, `set` over it; the hook's JSON answer.
    fn captured(&self, event: &str, fixture: &str, set: Value) -> Value {
        let (code, out, err) = self.hook_raw(event, &self.fixture(fixture, set));
        assert_eq!(code, 0, "{event} {fixture}: {out}{err}");
        serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{event} {fixture} printed no JSON ({e}): {out}"))
    }

    fn of_kind(&self, kind: &str) -> Vec<Fact> {
        self.facts().into_iter().filter(|f| f.kind == kind).collect()
    }

    /// The person's answer on c1's cases card, appended as the picker writes it.
    fn persons_answer(&self, choice: &str) {
        let record = Record::open_waiting(&self.record_path(), std::time::Duration::from_secs(5)).expect("the record");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0) + 1.0;
        let fields = json!({ "criterion_id": "c1", "hold": "cases_unconfirmed", "choice": choice, "typed": "" });
        record.append_raw("hold_answered", fields.as_object().cloned().unwrap_or_default(), now, familiar_ai::TENANT).expect("the answer");
    }

    /// The brief the binary makes for c1 from the record as it stands.
    fn brief(&self) -> String {
        let log = self.facts();
        familiar_ai::teach::case_finder::brief(&View::new(&log), "c1")
    }
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The finder's answer as its final text, in place of the found capture's.
fn answering(text: &str) -> Value {
    let mut f = common::fixture("post_tool_use_agent_case_finder_found");
    f["tool_response"]["content"] = json!([{ "type": "text", "text": text }]);
    json!({ "tool_response": f["tool_response"] })
}

fn a_case(short: &str) -> Value {
    json!({ "short": short, "given": "the name Ada", "expect": "hello Ada", "why": "the person's words" })
}

/// Whatever the hook said to the agent (`additionalContext` or a decision's reason).
fn said(out: &Value) -> String {
    out.to_string()
}

#[test]
fn the_main_agents_spawn_becomes_the_case_finder_with_the_binarys_brief() {
    let p = project("spawn");
    let out = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    let h = &out["hookSpecificOutput"];
    assert_eq!(h["permissionDecision"], json!("allow"), "{out}");
    // updated with the definition (the owner's ruling, 2026-10-26): the default model is sonnet again, was opus
    let want = json!({ "description": "Say hi", "prompt": p.brief(), "subagent_type": FINDER, "model": "sonnet", "run_in_background": false });
    assert_eq!(h["updatedInput"], want, "the exact rewrite: {out}");
    assert!(!said(&out).contains("say hi"), "the main agent's prompt is dropped: {out}");
    let reason = h["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: ") && reason.contains(FINDER) && reason.contains("sonnet"), "{reason}");
    assert!(!reason.contains("you must") && !reason.contains("spawn the"), "a fact, not a command: {reason}");

    // a spawn that already names the finder, with a prompt of the agent's own, is still given the brief
    let own = json!({ "tool_input": { "description": "find cases", "prompt": "look at greet and tell me what I missed", "subagent_type": FINDER, "model": "haiku", "run_in_background": true } });
    let out = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", own);
    let want = json!({ "description": "find cases", "prompt": p.brief(), "subagent_type": FINDER, "model": "sonnet", "run_in_background": false });
    assert_eq!(out["hookSpecificOutput"]["updatedInput"], want, "{out}");
}

#[test]
fn the_finders_model_is_the_environments_and_sonnet_when_it_names_none() {
    let p = project("model");
    p.set_env("FAMILIAR_AI_CASE_FINDER_MODEL", Some("haiku"));
    let out = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    assert_eq!(out["hookSpecificOutput"]["updatedInput"]["model"], json!("haiku"), "{out}");
    p.set_env("FAMILIAR_AI_CASE_FINDER_MODEL", Some("  "));
    let out = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    // updated with the definition (the owner's ruling, 2026-10-26): sonnet, was opus
    assert_eq!(out["hookSpecificOutput"]["updatedInput"]["model"], json!("sonnet"), "a blank value: {out}");
}

#[test]
fn the_finder_starts_with_the_brief_only_and_only_reads() {
    let p = project("reads");
    let started = p.captured("SubagentStart", "subagent_start_case_finder", json!({}));
    assert!(!said(&started).contains("THE PERSON ASKED") && !said(&started).contains("works for the person"), "no context beside the brief: {started}");

    let write = p.captured("PreToolUse", "pre_tool_use_write_in_case_finder", json!({ "tool_input": { "file_path": p.path("notes.txt").to_string_lossy(), "content": "x" } }));
    assert_eq!(write["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{write}");
    let who = json!({ "agent_id": AGENT, "agent_type": FINDER });
    for (tool, input) in [
        ("Edit", json!({ "file_path": p.path("src/app.sh").to_string_lossy(), "old_string": "hi", "new_string": "hello" })),
        ("Bash", json!({ "command": "ls" })),
        ("Agent", json!({ "description": "d", "prompt": "p", "subagent_type": "general-purpose" })),
    ] {
        let mut set = who.clone();
        set["tool_name"] = json!(tool);
        set["tool_input"] = input;
        let out = p.captured("PreToolUse", "pre_tool_use_write_in_case_finder", set);
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{tool}: {out}");
    }
    let mut read = who.clone();
    read["tool_name"] = json!("Read");
    read["tool_input"] = json!({ "file_path": p.path("src/app.sh").to_string_lossy() });
    // Read is not among the plugin's hook matchers; were it sent, it goes ahead
    let out = p.captured("PreToolUse", "pre_tool_use_write_in_case_finder", read);
    assert_ne!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "a read goes ahead: {out}");

    let stop = p.captured("SubagentStop", "subagent_stop_case_finder", json!({}));
    assert_ne!(stop["decision"], json!("block"), "the finder's stop is not held on the finder it is: {stop}");
    assert!(p.of_kind("cases_found").is_empty(), "the answer is read at the PostToolUse, not the stop");
}

#[test]
fn the_finders_cases_are_recorded_and_the_persons_card_opens() {
    let p = project("found");
    p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    let out = p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", json!({}));
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 1, "{out}");
    let f = &found[0];
    assert_eq!((text(f, "criterion_id"), text(f, "outcome"), text(f, "model")), ("c1", "found", "claude-sonnet-5-5"));
    assert_eq!(f.fields.get("tokens"), Some(&json!(31250)));
    let cases = f.fields.get("cases").cloned().unwrap_or_default();
    assert_eq!(cases[0], json!({ "case_id": "a1", "short": "the same with a name in capitals", "given": "the name ADA", "expect": "hello ADA, the name as given", "why": "the person asked for the name as typed" }));
    let ids: Vec<&str> = cases.as_array().map(|cs| cs.iter().filter_map(|c| c["case_id"].as_str()).collect()).unwrap_or_default();
    assert_eq!(ids, ["a1", "a2", "a3"]);
    let log = p.facts();
    let told = familiar_ai::teach::case_finder::after_finding(&View::new(&log), "c1");
    // updated with the definition (the cases card's words, step 4) [draft words]: what the agent is
    // told runs over several lines (the finding, then the open card), so it is read in the
    // context's own text, not in its JSON, where each line break is escaped
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.contains(&told), "the agent is told the card is open: {out}");

    let status = p.status_json().to_string();
    assert!(status.contains("cases_unconfirmed"), "the person's card: {status}");
    assert!(!status.contains("\"cases_owed\""), "nothing more is owed: {status}");

    // nothing is owed any more: a second answer is not recorded, and a spawn goes as asked
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", json!({}));
    assert_eq!(p.of_kind("cases_found").len(), 1);
    let out = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    assert!(out["hookSpecificOutput"]["updatedInput"].is_null(), "no rewrite once the finding stands: {out}");
    assert_ne!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "the spawn goes as asked: {out}");
}

/// The person's card in `status --json` (its `person`) reads as the picker shows it: each option's
/// label and its own line (the wording the owner approved, plan §3).
#[test]
fn the_persons_card_in_status_gives_each_option_its_own_line() {
    let p = project("person");
    p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", json!({}));
    let status = p.status_json();
    let card = status["cards"].as_array().and_then(|cs| cs.iter().find(|c| c["hold"] == "cases_unconfirmed")).cloned().unwrap_or_else(|| panic!("the cases card: {status}"));
    assert_eq!(
        card["person"]["options"],
        json!([
            { "choice": "add_all", "label": "Add all three (Recommended)", "description": "the test checks all five" },
            { "choice": "keep_planned", "label": "Just the first two", "description": "the test checks the two above" }
        ]),
        "{card}"
    );
}

/// The person's last answer comes through the picker: every other line of the plan held, the
/// finder found nothing more, and the person picks "Looks right" on the card in Claude Code's
/// picker. The plan is confirmed at that PostToolUse, and the places it owes are listed there, as
/// a claim that confirms a plan lists them (the owner's plan of 2026-10-21, step 3), not first at
/// a later claim.
#[test]
fn the_persons_last_answer_on_the_picker_confirms_the_plan_and_lists_its_places() {
    let p = project("picked");
    for args in [
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1"],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", answering(r#"{"cases": [], "none_because": "n1 and o1 cover every path greet has"}"#));
    assert_eq!(p.status_json()["plan"], json!("planning"), "the card is open");
    assert!(p.of_kind("places_listed").is_empty(), "nothing is listed before the plan is confirmed");

    // the agent asks with the card's lines word for word; the hook puts the card in the picker
    let log = p.facts();
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().find(|c| c.hold == "cases_unconfirmed").expect("the cases card");
    let question = familiar_ai::teach::cases_card::shape(&familiar_ai::teach::cases_card::shown(&v, &card));
    let asking = json!({ "tool_input": { "questions": [{ "question": question, "header": "x", "options": [{ "label": "yes", "description": "Continue" }], "multiSelect": false }] } });
    let pre = p.captured("PreToolUse", "pre_tool_use_ask_user_question", asking);
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("ask"), "{pre}");
    let asked = pre["hookSpecificOutput"]["updatedInput"].clone();

    // the person picks "Looks right" in the picker
    let q = asked["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let response = json!({ "questions": asked["questions"], "answers": { q: "Looks right (Recommended)" }, "annotations": {} });
    let mut input = asked.clone();
    input["answers"] = response["answers"].clone();
    input["annotations"] = json!({});
    p.captured("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }));
    let answers: Vec<(String, String)> = p.of_kind("hold_answered").iter().map(|a| (text(a, "hold").to_string(), text(a, "choice").to_string())).collect();
    assert_eq!(answers, [("cases_unconfirmed".to_string(), "keep_planned".to_string())]);
    let listed = p.of_kind("places_listed");
    assert_eq!(listed.iter().map(|f| text(f, "criterion_id")).collect::<Vec<_>>(), ["c1"], "listed at the pick that confirmed the plan");
    assert_eq!(p.status_json()["plan"], json!("confirmed"));
}

#[test]
fn the_turn_limit_records_failed_and_look_again_owes_one_more_finder() {
    let p = project("cut");
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_cut", json!({}));
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 1);
    let f = &found[0];
    assert_eq!((text(f, "outcome"), text(f, "model")), ("failed", "claude-haiku-4-5-20251001"));
    assert!(text(f, "why").contains("turn limit"), "{f:?}");
    assert_eq!(f.fields.get("cases"), Some(&json!([])));
    assert_eq!(f.fields.get("tokens"), Some(&json!(5320)));
    assert!(p.status_json().to_string().contains("cases_unconfirmed"));

    // the person's pick, as the picker records it (the card's picker is not this test's)
    p.persons_answer("look_again");
    let spawn = p.captured("PreToolUse", "pre_tool_use_agent_case_finder", json!({}));
    assert_eq!(spawn["hookSpecificOutput"]["updatedInput"]["subagent_type"], json!(FINDER), "one more finder: {spawn}");
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", json!({}));
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 2);
    assert_eq!(text(&found[1], "outcome"), "found");
}

#[test]
fn a_background_launch_records_nothing_and_another_agents_answer_is_not_the_finders() {
    let p = project("async");
    let launched = json!({ "tool_response": { "isAsync": true, "status": "async_launched", "agentId": AGENT, "agentType": FINDER, "resolvedModel": "claude-sonnet-5-5", "prompt": "p", "outputFile": "/tmp/claude/tasks/output", "canReadOutputFile": true } });
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", launched);
    let mut other = common::fixture("post_tool_use_agent_case_finder_found");
    other["tool_response"]["agentType"] = json!("general-purpose");
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", json!({ "tool_response": other["tool_response"] }));
    assert!(p.of_kind("cases_found").is_empty());
}

#[test]
fn every_answer_is_checked_against_the_schema() {
    let three = json!({ "cases": [a_case("one"), a_case("two"), a_case("three")] }).to_string();
    let none = json!({ "cases": [], "none_because": "n1 and o1 cover every path greet has" }).to_string();
    let nine = json!({ "cases": (1..=9).map(|i| a_case(&format!("case {i}"))).collect::<Vec<_>>() }).to_string();
    let long = json!({ "cases": [a_case(&"x".repeat(71))] }).to_string();
    let extra = json!({ "cases": [{ "short": "s", "given": "g", "expect": "e", "why": "w", "id": "z9" }] }).to_string();
    let missing = json!({ "cases": [{ "short": "s", "given": "g", "expect": "e" }] }).to_string();
    let blank = json!({ "cases": [{ "short": "s", "given": " ", "expect": "e", "why": "w" }] }).to_string();
    let unexplained = json!({ "cases": [] }).to_string();
    for (name, answer, outcome, n) in [
        ("bare", three.as_str(), "found", 3),
        ("fenced", &format!("Here is what I found.\n```json\n{three}\n```\n"), "found", 3),
        ("none", none.as_str(), "none", 0),
        // updated with the definition (the owner's ruling, 2026-10-25, "Keep what fits"): nine
        // cases keep the first eight, a short line over 70 characters is kept whole
        ("nine", nine.as_str(), "found", 8),
        ("long", long.as_str(), "found", 1),
        ("extra", extra.as_str(), "failed", 0),
        ("missing", missing.as_str(), "failed", 0),
        ("blank", blank.as_str(), "failed", 0),
        ("unexplained", unexplained.as_str(), "failed", 0),
        ("prose", "I looked at greet and everything is covered.", "failed", 0),
    ] {
        let p = project(&format!("schema-{name}"));
        p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", answering(answer));
        let found = p.of_kind("cases_found");
        assert_eq!(found.len(), 1, "{name}");
        let f = &found[0];
        assert_eq!(text(f, "outcome"), outcome, "{name}: {f:?}");
        assert_eq!(f.fields.get("cases").and_then(Value::as_array).map(Vec::len), Some(n), "{name}");
        if outcome == "none" {
            assert_eq!(text(f, "why"), "n1 and o1 cover every path greet has");
        }
        if outcome == "failed" {
            assert!(!text(f, "why").trim().is_empty(), "{name}: a failure says why");
        }
        if outcome == "found" {
            let note = if name == "nine" { "the finder listed 9, the first 8 are kept: 1 left out" } else { "" };
            assert_eq!(text(f, "why"), note, "{name}");
        }
        if name == "long" {
            assert_eq!(f.fields["cases"][0]["short"], json!("x".repeat(71)), "the long line whole");
        }
    }
}

/// "Keep what fits" (the owner's ruling, 2026-10-25) [draft words]: the finder's answer past the
/// brief's limits is kept, not failed: its first eight cases as a1..a8, a short line over 70
/// characters whole, and the found finding's why saying how many were left out; the person's card
/// shows that line after the found checks, and the agent is told it with the card's lines.
#[test]
fn an_answer_past_the_briefs_limits_keeps_what_fits_and_the_card_says_what_was_left_out() {
    let p = project("fits");
    let long = "the same greeting when the name comes in capitals, lower case or mixed case, as typed";
    assert!(long.chars().count() > 70);
    let mut listed: Vec<Value> = (1..=10).map(|i| a_case(&format!("case {i}"))).collect();
    listed[1] = a_case(long);
    let out = p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", answering(&json!({ "cases": listed }).to_string()));
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 1, "{out}");
    let f = &found[0];
    let note = "the finder listed 10, the first 8 are kept: 2 left out";
    assert_eq!((text(f, "outcome"), text(f, "why")), ("found", note), "{f:?}");
    let kept: Vec<String> = f.fields["cases"].as_array().map(|cs| cs.iter().map(|k| format!("{} {}", k["case_id"].as_str().unwrap_or(""), k["short"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
    let mut want: Vec<String> = (1..=8).map(|i| format!("a{i} case {i}")).collect();
    want[1] = format!("a2 {long}");
    assert_eq!(kept, want, "the first eight, the long line whole");

    let status = p.status_json();
    let card = status["cards"].as_array().and_then(|cs| cs.iter().find(|c| c["hold"] == "cases_unconfirmed")).cloned().unwrap_or_else(|| panic!("the cases card: {status}"));
    let lines: Vec<String> = card["person"]["lines"].as_array().map(|ls| ls.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    assert!(lines.contains(&format!("  2. {long}")), "{lines:?}");
    assert_eq!(lines[lines.len() - 3..], ["  8. case 8".to_string(), String::new(), note.to_string()], "{lines:?}");
    assert_eq!(card["person"]["options"][0], json!({ "choice": "add_all", "label": "Add all eight (Recommended)", "description": "the test checks all ten" }), "{card}");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.contains(&format!("  {note}\n")), "the agent is told the line with the card's: {out}");
}
