//! The investigator diagnoses a test that never went red [draft words] (the owner, 2026-10-16,
//! S7b): on `two_tries_not_red` the binary runs a read-only investigator one rung up, never below
//! opus, and writes `investigator_finding` from its structured output after re-reading every
//! quote; two misses give a bare card, and with the investigator off the card comes at once. The
//! card is always the person's: the finding's choice first, the question refused until it quotes
//! the proof. A finding clears no hold and relabels no run; the plan file never holds it.

mod common;

use common::Project;
use familiar_ai_facts::Log;
use familiar_ai_rules::terminal::Card;
use familiar_ai_rules::{terminal, View};
use serde_json::{json, Value};

const SESSION: &str = "5e1f0c3a-7b2d-4c6e-9a8b-0d1e2f3a4b5c";
const ASKED: &str = "greet the person with hello";
const TEST: &str = "out=$(sh src/app.sh)\n[ -n \"$out\" ]\n";

/// The fake `claude`: every call's argv to `calls` and its prompt to `prompt`, then the answer
/// written for that call (`answer.<n>`), else `answer`.
const FAKE: &str = r#"#!/bin/sh
D="$(cd "$(dirname "$0")/.." && pwd)"
printf '%s\n' "$*" >> "$D/calls"
cat > "$D/prompt"
n=$(wc -l < "$D/calls" | tr -d ' ')
if [ -e "$D/answer.$n" ]; then cat "$D/answer.$n"; else cat "$D/answer"; fi
"#;

/// A planned project, c1 on the full path held by SESSION on haiku; the fake `claude` first on
/// PATH and the investigator on unless `off`.
fn project(name: &str, off: bool) -> Project {
    let b = Project::builder(name).session(SESSION).env_remove("CLAUDE_CODE_SESSION_ID");
    std::fs::create_dir_all(b.dir().join("plugin")).expect("plugin folder");
    let plugin = b.dir().join("plugin");
    let b = b.env_remove("FAMILIAR_AI_INVESTIGATOR_BUDGET").env_remove("CLAUDE_CONFIG_DIR").env("CLAUDE_PLUGIN_ROOT", plugin).bin("claude", FAKE);
    let p = if off { b.env("FAMILIAR_AI_INVESTIGATOR", "off").create() } else { b.env_remove("FAMILIAR_AI_INVESTIGATOR").create() };
    p.hook_fixture("UserPromptSubmit", "user_prompt_submit", json!({ "prompt": ASKED }));
    let said = said_at(&p);
    for args in [
        vec!["intent_submitted", "--for", &said.to_string(), "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "nothing", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.hook("PostModelSwitch", json!({ "from_model": "claude-sonnet-5-5", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

fn said_at(p: &Project) -> u64 {
    p.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded")
}

impl Project {
    /// The agent's commit; what the hook told.
    fn agent_commit(&self, message: &str) -> Value {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        let payload = self.payload("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {}, "permission_mode": "bypassPermissions" }));
        self.hook_with("PostToolUse", &payload)
    }

    /// Try 1, a tests-first run that passed; returns its commit. The answers are written after it.
    fn tests_first(&self) -> String {
        self.write("tests/test_c1.sh", TEST);
        self.agent_commit("c1: tests first");
        self.head()
    }

    /// Try 2, a code commit before any seal: the hold rises and the diagnosis runs.
    fn second_try(&self) -> Value {
        self.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
        self.agent_commit("c1: a fix before a seal")
    }

    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }

    fn answer(&self, n: Option<usize>, structured: Value) {
        let out = json!({ "type": "result", "subtype": "success", "is_error": false, "total_cost_usd": 0.4, "modelUsage": { "claude-opus-4-6": {} }, "structured_output": structured });
        let name = n.map_or_else(|| "answer".to_string(), |n| format!("answer.{n}"));
        std::fs::write(self.dir.join(name), format!("{out}\n")).expect("answer");
    }

    fn card(&self) -> Card {
        let log: Log = self.facts();
        terminal::cards(&View::new(&log)).into_iter().find(|c| c.hold == "two_tries_not_red").expect("the card")
    }

    /// The question in the shape the agent is told, with or without the finding's lines.
    fn question(&self, with_found: bool) -> String {
        let log: Log = self.facts();
        let v = View::new(&log);
        let a = familiar_ai::teach::person_words::asked(&v, &self.card());
        let mut lines: Vec<String> = familiar_ai::teach::person_words::lines(&a);
        if !with_found {
            lines.retain(|l| !l.starts_with(familiar_ai::teach::person_words::FOUND));
        }
        format!("{}\nIn the agent's words: \"the test only checks that greet prints something\"\nwhat now?", lines.join("\n"))
    }

    fn pre_ask(&self, question: &str) -> Value {
        let input = json!({ "questions": [{ "question": question, "header": "familiar-ai", "options": [{ "label": "a", "description": "a" }, { "label": "b", "description": "b" }], "multiSelect": false }] });
        self.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": input }))
    }

    fn picked(&self, card: &Value, answer: &str) -> Value {
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        input["annotations"] = json!({});
        self.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
    }
}

fn labels(card: &Value) -> Vec<String> {
    card["questions"][0]["options"].as_array().map(|o| o.iter().filter_map(|x| x["label"].as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn person_quote(said: u64) -> Value {
    json!({ "role": "person", "said": said, "text": "greet the person with hello" })
}

fn test_quote(commit: &str, text: &str) -> Value {
    json!({ "role": "test", "file_path": "tests/test_c1.sh", "line_start": 2, "line_end": 2, "commit": commit, "text": text })
}

fn told(out: &Value) -> String {
    format!("{} {}", out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or(""), out["systemMessage"].as_str().unwrap_or(""))
}

#[test]
fn a_wrong_test_finding_is_written_from_the_quotes_the_binary_read_again() {
    let p = project("wrong", false);
    let tf = p.tests_first();
    assert!(p.file("calls").is_empty(), "one try: nothing owed");
    let said = said_at(&p);
    p.answer(None, json!({ "category": "wrong_test", "reason": "the test passes for any output", "quotes": [test_quote(&tf, "[ -n   \"$out\" ]"), person_quote(said)], "guidance": "check that greet prints hello" }));
    let translations = |p: &Project| p.facts_of("test_log_translation");
    let out = p.second_try();
    let runs_before = translations(&p);
    // the launch: read-only, one rung up and never below opus, no plugin, the budget
    let calls = p.file("calls");
    assert_eq!(calls.lines().count(), 1, "{calls}");
    let argv = calls.lines().next().unwrap_or("");
    for piece in ["-p", "--model opus", "--output-format json", "--json-schema", "--strict-mcp-config", "--tools Read,Grep,Glob", "--max-budget-usd 3.00"] {
        assert!(argv.contains(piece), "{piece} in {argv}");
    }
    assert!(argv.contains("--setting-sources  ") || argv.contains("--setting-sources ''"), "{argv}");
    assert!(!argv.contains("--plugin-dir"), "no plugin: {argv}");
    // the brief: the person's words, the part and its promise, the place, the test, the runs, the tree
    let prompt = p.file("prompt");
    for piece in [ASKED, "greet says hello", "a greeting", "src/app.sh", "greet() { echo hi; }", "greet() { echo hey; }", "[ -n \"$out\" ]", "sh tests/test_c1.sh", "wrong_test", "behavior_exists", "requirement_missed", &tf[..7]] {
        assert!(prompt.contains(piece), "{piece:?} in the brief:\n{prompt}");
    }
    assert!(!prompt.contains("test_change_announced"), "no stale advice:\n{prompt}");
    // the finding, as the binary read it
    let found = p.facts_of("investigator_finding");
    assert_eq!(found.len(), 1, "{found:?}");
    let f = &found[0];
    assert_eq!((f["criterion_id"].as_str(), f["category"].as_str(), f["model"].as_str()), (Some("c1"), Some("wrong_test"), Some("claude-opus-4-6")));
    assert_eq!(f["guidance"], "check that greet prints hello");
    assert_eq!(f["quotes"][0]["text"], "[ -n \"$out\" ]", "the binary's own bytes, not the model's spacing");
    assert_eq!(f["quotes"][1], person_quote(said));
    let runs = f["runs"].as_array().cloned().unwrap_or_default();
    assert_eq!(runs.len(), 2, "the two runs: {runs:?}");
    assert!(runs.iter().all(|r| r["run_id"].as_str().is_some_and(|id| id.starts_with("r-"))), "{runs:?}");
    // nothing else moved: no transfer, the hold stands, the runs read the same, the plan file never holds it
    assert!(p.facts_of("criterion_transfer").is_empty());
    assert_eq!(translations(&p), runs_before, "a finding relabels no run");
    assert_eq!(p.card().choices.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["try_again", "re_plan", "release"]);
    let log: Log = p.facts().into_iter().filter(|f| f.kind != "investigator_finding").collect();
    let plan = std::fs::read(p.path(".familiar/plan")).expect("the plan file");
    assert_eq!(plan, familiar_ai_plan::to_bytes(&familiar_ai_plan::claims_from_log(&log)), "the plan file is byte-identical with a finding");
    // told in the commit's reply
    assert!(told(&out).contains("a stronger model (opus) read why the tests of \"greet says hello\" never went red"), "{out}");
    // status --json: the card carries the finding
    let status = p.status_json();
    let card = status["cards"].as_array().and_then(|c| c.iter().find(|k| k["hold"] == "two_tries_not_red")).cloned().unwrap_or_default();
    assert_eq!(card["finding"]["category"], "wrong_test", "{card}");
    assert_eq!(card["finding"]["choice"], "try_again", "{card}");
    assert_eq!(card["finding"]["quotes"][0]["text"], "[ -n \"$out\" ]", "{card}");
    // a later hook owes nothing: no second diagnosis
    p.hook("Stop", json!({}));
    assert_eq!(p.file("calls").lines().count(), 1);
}

#[test]
fn the_card_quotes_the_proof_before_it_is_asked_and_the_findings_choice_comes_first() {
    let p = project("missed", false);
    let tf = p.tests_first();
    let said = said_at(&p);
    p.answer(None, json!({ "category": "requirement_missed", "reason": "the part never asks for hello", "quotes": [person_quote(said), test_quote(&tf, "[ -n \"$out\" ]")], "missed": "the word hello", "scope": "greet prints exactly hello" }));
    p.second_try();
    // without the proof the question is refused, naming it
    let refused = p.pre_ask(&p.question(false));
    assert_eq!(refused["hookSpecificOutput"]["permissionDecision"], "deny", "{refused}");
    let reason = refused["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.contains("ask again; your question must quote") && reason.contains("[ -n \"$out\" ]"), "{reason}");
    // with it, the card: the finding's choice first
    let asked = p.pre_ask(&p.question(true));
    assert_eq!(asked["hookSpecificOutput"]["permissionDecision"], "ask", "{asked}");
    let card = asked["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(labels(&card), ["have the agent plan part 1 again, from the code it picks", "let the agent write the tests for part 1 once more", "drop it", "something else"]);
    // the pick: re_plan, told once with the finding and the new reading it asks
    let out = p.picked(&card, "have the agent plan part 1 again, from the code it picks");
    let cta = told(&out);
    assert!(cta.contains("A stronger model found a requirement missed: the word hello.") && cta.contains("claim it again"), "{cta}");
    let answered = p.facts_of("hold_answered");
    assert_eq!(answered.last().map(|a| (a["hold"].clone(), a["choice"].clone())), Some((json!("two_tries_not_red"), json!("re_plan"))));
    let again = p.hook("Stop", json!({}));
    assert!(!told(&again).contains("A stronger model found a requirement missed"), "told once: {again}");
    // the rescope gate: lines 2 and 3 wait for the new reading
    let log: Log = p.facts();
    let v = View::new(&log);
    assert!(!familiar_ai_rules::plan::line_2(&v) && !familiar_ai_rules::plan::line_3(&v));
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says exactly hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "nothing", "--gives_back", "the word hello", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    let log: Log = p.facts();
    let v = View::new(&log);
    assert!(familiar_ai_rules::plan::line_2(&v) && familiar_ai_rules::plan::line_3(&v));
}

#[test]
fn behavior_exists_also_runs_the_planned_test_at_the_planning_commit() {
    let p = project("exists", false);
    // review 2a: a quote names a commit the brief named, here the planning commit
    let planning = p.head();
    p.tests_first();
    let said = said_at(&p);
    p.answer(None, json!({ "category": "behavior_exists", "reason": "greet already prints a greeting", "quotes": [{ "role": "code", "file_path": "src/app.sh", "line_start": 2, "line_end": 2, "commit": planning, "text": "greet() { echo hi; }" }, person_quote(said)] }));
    p.second_try();
    let found = p.facts_of("investigator_finding");
    assert_eq!(found.len(), 1, "{:?}", p.facts_of("investigator_unfinished"));
    let runs = found[0]["runs"].as_array().cloned().unwrap_or_default();
    let planning = runs.iter().find(|r| r["run_id"] == "at_planning").cloned().unwrap_or_default();
    assert_eq!(planning["label"], "passed", "the planned test passes on the code as planned: {runs:?}");
    assert!(found[0].get("guidance").is_none() && found[0].get("missed").is_none());
    let asked = p.pre_ask(&p.question(true));
    let card = asked["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(labels(&card).first().map(String::as_str), Some("drop it"), "{card}");
}

#[test]
fn a_quote_not_at_its_commit_is_unverified_and_two_misses_give_a_bare_card() {
    let p = project("unverified", false);
    let tf = p.tests_first();
    let said = said_at(&p);
    p.answer(None, json!({ "category": "wrong_test", "reason": "x", "quotes": [test_quote(&tf, "[ \"$out\" = hello ]"), person_quote(said)], "guidance": "check hello" }));
    let out = p.second_try();
    assert_eq!(p.file("calls").lines().count(), 2, "two diagnoses in the call");
    let misses = p.facts_of("investigator_unfinished");
    assert_eq!(misses.iter().map(|m| m["why"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["unverified", "unverified"]);
    assert!(p.facts_of("investigator_finding").is_empty());
    assert!(told(&out).contains("gave no finding"), "{out}");
    // the bare card: asked without any proof
    let asked = p.pre_ask(&p.question(false));
    assert_eq!(asked["hookSpecificOutput"]["permissionDecision"], "ask", "{asked}");
    let card = asked["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(labels(&card).first().map(String::as_str), Some("let the agent write the tests for part 1 once more"));
    let status = p.status_json();
    let card = status["cards"].as_array().and_then(|c| c.iter().find(|k| k["hold"] == "two_tries_not_red")).cloned().unwrap_or_default();
    assert!(card["finding"].is_null(), "{card}");
    p.hook("Stop", json!({}));
    assert_eq!(p.file("calls").lines().count(), 2, "nothing more is owed");
}

#[test]
fn an_answer_not_in_the_form_is_unparseable_and_a_second_call_can_still_find() {
    let p = project("unparseable", false);
    let tf = p.tests_first();
    let said = said_at(&p);
    std::fs::write(p.dir.join("answer.1"), "Looking at it, the test is weak.\n").expect("answer");
    p.answer(None, json!({ "category": "wrong_test", "reason": "weak", "quotes": [test_quote(&tf, "[ -n \"$out\" ]"), person_quote(said)], "guidance": "check hello" }));
    p.second_try();
    let misses = p.facts_of("investigator_unfinished");
    assert_eq!(misses.iter().map(|m| m["why"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["unparseable"]);
    assert_eq!(p.facts_of("investigator_finding").len(), 1, "the second diagnosis found");
}

#[test]
fn with_the_investigator_off_the_card_comes_at_once() {
    let p = project("off", true);
    p.tests_first();
    p.second_try();
    assert!(p.file("calls").is_empty(), "nothing launched");
    assert!(p.facts_of("investigator_finding").is_empty() && p.facts_of("investigator_unfinished").is_empty());
    let asked = p.pre_ask(&p.question(false));
    assert_eq!(asked["hookSpecificOutput"]["permissionDecision"], "ask", "{asked}");
}

#[test]
fn the_judge_reads_the_finding_at_the_next_seal() {
    let p = project("judge", false);
    let tf = p.tests_first();
    let said = said_at(&p);
    p.answer(None, json!({ "category": "wrong_test", "reason": "the test passes for any output", "quotes": [test_quote(&tf, "[ -n \"$out\" ]"), person_quote(said)], "guidance": "check that greet prints hello" }));
    p.second_try();
    let asked = p.pre_ask(&p.question(true));
    let card = asked["hookSpecificOutput"]["updatedInput"].clone();
    let out = p.picked(&card, "let the agent write the tests for part 1 once more");
    assert!(told(&out).contains("A stronger model found the test checks the wrong thing: check that greet prints hello."), "{out}");
    // a test that fails on the code as it is, committed alone: sealed
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ \"$out\" = hello ] || { echo \"expected hello, got: $out\"; exit 1; }\n");
    p.agent_commit("c1: a test that fails first");
    let (code, prompt, err) = p.run(&["judge-prompt", "c1"], "");
    assert_eq!(code, 0, "{err}");
    assert!(prompt.contains("AN INVESTIGATOR FOUND, before this test was written: the test checked the wrong thing (the test passes for any output); it should check that greet prints hello"), "{prompt}");
}

/// Review 2a and 2b: a quote at a commit the brief did not name, or a person's quote too short to
/// be proof, is unverified; a person's quote is kept as the bytes the person typed.
#[test]
fn a_quote_must_name_a_commit_of_the_brief_and_a_persons_quote_is_their_own_bytes() {
    for (name, quotes) in [
        ("unnamed", json!([{ "role": "test", "file_path": "tests/test_c1.sh", "line_start": 2, "line_end": 2, "commit": "HEAD~1", "text": "[ -n \"$out\" ]" }])),
        ("short", json!([{ "role": "person", "said": -1, "text": "hello" }])),
    ] {
        let p = project(name, false);
        p.tests_first();
        let said = said_at(&p);
        let quotes: Value = serde_json::from_str(&quotes.to_string().replace("-1", &said.to_string())).expect("json");
        p.answer(None, json!({ "category": "wrong_test", "reason": "x", "quotes": quotes, "guidance": "check hello" }));
        p.second_try();
        let whys: Vec<String> = p.facts_of("investigator_unfinished").iter().map(|m| m["why"].as_str().unwrap_or("").to_string()).collect();
        assert_eq!(whys, ["unverified", "unverified"], "{name}");
    }
    let p = project("bytes", false);
    let tf = p.tests_first();
    let said = said_at(&p);
    p.answer(None, json!({ "category": "wrong_test", "reason": "x", "quotes": [test_quote(&tf, "[ -n \"$out\" ]"), { "role": "person", "said": said, "text": "greet  the\nperson with" }], "guidance": "check hello" }));
    p.second_try();
    let found = p.facts_of("investigator_finding");
    assert_eq!(found.first().map(|f| f["quotes"][1]["text"].clone()), Some(json!("greet the person with")), "the person's own bytes");
}
