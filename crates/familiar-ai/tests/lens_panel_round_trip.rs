//! The lens panel released through the orchestrator (the owner's plan of 2026-10-26, lens-panel.md,
//! approved with its names; it replaces the case-finder's round trip of 2026-10-25): once a
//! full-path part's contract is whole, each Agent call of the main agent is released to the next
//! lens (`lens_released`, with the call's tool_use_id, so five calls in one message each get their
//! own) and rewritten to `familiar-ai:lens` with that lens's brief, on the model the environment
//! names (opus by default); once the five answered, the next call is the synthesis. A lens only
//! reads, the synthesis has no tools. Each answer is read at the main agent's PostToolUse on its
//! call: a lens's as `lens_found`, the synthesis's as `cases_found` with its questions, and the
//! person's card opens. Payloads are the spike's captures (session 6b42509f, spike0), the agent
//! renamed to the lens.

mod common;

use common::project::{LENS, LENSES};
use common::Project;
use familiar_ai_facts::{Fact, Record};
use familiar_ai_rules::View;
use serde_json::{json, Value};

/// The lens's agent id in the captures.
const AGENT: &str = "a09dfaf2a8482404c";

/// A full-path project planned to c1's whole contract: the lens panel is owed.
fn project(name: &str) -> Project {
    let p = Project::builder(name).env_remove("FAMILIAR_AI_LENS_MODEL").env_remove("FAMILIAR_AI_CASE_FINDER_MODEL").create();
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

    /// The brief the binary makes for `lens` on c1 from the record as it stands.
    fn brief(&self, lens: &str) -> String {
        let log = self.facts();
        familiar_ai::teach::lens::brief(&View::new(&log), "c1", lens)
    }

    /// A call of the lens agent's own (inside the subagent): the hook's answer.
    fn lens_calls(&self, tool: &str, input: Value) -> Value {
        self.captured("PreToolUse", "pre_tool_use_write_in_case_finder", json!({ "agent_id": AGENT, "agent_type": LENS, "tool_name": tool, "tool_input": input }))
    }
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn a_case(short: &str) -> Value {
    json!({ "short": short, "given": "the name Ada", "expect": "hello Ada", "why": "the person's words" })
}

fn context(out: &Value) -> String {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

fn id(i: usize) -> String {
    format!("toolu_lens_{i}")
}

fn cases_owed(p: &Project) -> bool {
    p.criterion_json("c1")["holds"].to_string().contains("cases_owed")
}

#[test]
fn five_spawns_in_one_message_each_get_their_own_lens_and_its_brief() {
    let p = project("spawn");
    for (i, lens) in LENSES.iter().enumerate() {
        let out = p.spawn(&id(i));
        let h = &out["hookSpecificOutput"];
        assert_eq!(h["permissionDecision"], json!("allow"), "{lens}: {out}");
        let want = json!({ "description": "Say hi", "prompt": p.brief(lens), "subagent_type": LENS, "model": "opus", "run_in_background": false });
        assert_eq!(h["updatedInput"], want, "{lens}: the exact rewrite: {out}");
        let reason = h["permissionDecisionReason"].as_str().unwrap_or("");
        assert!(reason.starts_with("familiar-ai: \"greet says hello\": ") && reason.contains(LENS) && reason.contains("opus"), "{reason}");
        assert!(reason.contains(&familiar_ai::teach::lens::named(lens)), "{lens}: {reason}");
        assert!(!reason.contains("you must") && !reason.contains("say hi"), "a fact, not a command: {reason}");
    }
    let released: Vec<(String, String, String)> = p.of_kind("lens_released").iter().map(|f| (text(f, "criterion_id").into(), text(f, "lens").into(), text(f, "tool_use_id").into())).collect();
    let want: Vec<(String, String, String)> = LENSES.iter().enumerate().map(|(i, l)| ("c1".into(), l.to_string(), id(i))).collect();
    assert_eq!(released, want, "each call its own lens, in release order");

    // the same call again is the same lens, released once
    let again = p.spawn(&id(2));
    assert_eq!(again["hookSpecificOutput"]["updatedInput"]["prompt"], json!(p.brief("happy_path")), "{again}");
    assert_eq!(p.of_kind("lens_released").len(), 5);

    // a sixth call while the five run is none of them: refused, naming what runs
    let sixth = p.spawn("toolu_sixth");
    assert_eq!(sixth["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{sixth}");
    let reason = sixth["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.contains("running") && reason.contains("lens panel"), "{reason}");
    assert_eq!(p.of_kind("lens_released").len(), 5, "nothing released for it");
}

#[test]
fn the_panels_model_is_the_environments_and_opus_when_it_names_none() {
    let p = project("model");
    p.set_env("FAMILIAR_AI_LENS_MODEL", Some("haiku"));
    assert_eq!(p.spawn(&id(0))["hookSpecificOutput"]["updatedInput"]["model"], json!("haiku"));
    p.set_env("FAMILIAR_AI_LENS_MODEL", Some("  "));
    assert_eq!(p.spawn(&id(1))["hookSpecificOutput"]["updatedInput"]["model"], json!("opus"), "a blank value");
    // the case-finder's variable is gone
    p.set_env("FAMILIAR_AI_LENS_MODEL", None);
    p.set_env("FAMILIAR_AI_CASE_FINDER_MODEL", Some("haiku"));
    assert_eq!(p.spawn(&id(2))["hookSpecificOutput"]["updatedInput"]["model"], json!("opus"));
}

#[test]
fn each_answer_is_recorded_by_its_call_and_the_agent_is_told_what_comes_next() {
    let p = project("answers");
    for i in 0..5 {
        p.spawn(&id(i));
    }
    // the happy path lens answers first, with one finding
    let finding = json!({ "findings": [{ "kind": "happy_path", "note": "greet is reached through main, which lowercases the name first", "target_symbol": "main", "file_path": "src/app.sh", "candidate_question": null }] });
    let out = p.returned(&id(2), &format!("```json\n{finding}\n```"));
    let found = p.of_kind("lens_found");
    assert_eq!(found.len(), 1, "{out}");
    assert_eq!(
        Value::Object(found[0].fields.clone()),
        json!({ "criterion_id": "c1", "lens": "happy_path", "outcome": "found", "findings": [{ "kind": "happy_path", "note": "greet is reached through main, which lowercases the name first", "file_path": "src/app.sh", "target_symbol": "main", "candidate_question": "" }], "model": "claude-sonnet-5-5", "tokens": 31250 })
    );
    let told = context(&out);
    assert!(told.starts_with("familiar-ai: the happy path lens answered for \"greet says hello\": it found one thing worth asking about."), "{told}");
    assert!(told.contains("the ambiguity, completeness, edge cases and order and coupling lenses running"), "{told}");
    // the same call's answer again records nothing
    p.returned(&id(2), r#"{"findings": []}"#);
    assert_eq!(p.of_kind("lens_found").len(), 1);

    for i in [0, 1, 3] {
        p.returned(&id(i), r#"{"findings": []}"#);
    }
    let out = p.returned(&id(4), r#"{"findings": []}"#);
    assert!(context(&out).contains("the synthesis is next"), "{out}");
    assert_eq!(p.of_kind("lens_found").iter().map(|f| text(f, "lens").to_string()).collect::<Vec<_>>(), ["happy_path", "ambiguity", "completeness", "edge_cases", "order_and_coupling"]);
    assert!(p.of_kind("cases_found").is_empty());

    // the next call is the synthesis, its brief carrying the five answers
    let out = p.spawn("toolu_synthesis");
    let h = &out["hookSpecificOutput"];
    assert_eq!(h["updatedInput"]["prompt"], json!(p.brief("synthesis")), "{out}");
    assert!(p.brief("synthesis").contains("main, which lowercases the name first"));
    assert!(h["permissionDecisionReason"].as_str().unwrap_or("").contains("it has no tools"), "{out}");
    assert_eq!(p.of_kind("lens_released").last().map(|f| text(f, "lens").to_string()).as_deref(), Some("synthesis"));

    let answer = json!({
        "cases": [a_case("a name in capitals stays as typed"), a_case("a name with a space")],
        "questions": [
            { "text": "Should a name typed in capitals be greeted in capitals?", "options": ["as typed", "in lower case"], "why": "main lowercases it" },
            { "text": "Should an empty name be greeted?", "options": ["yes", "no"], "why": "the draft refuses it" }
        ],
        "none_because": ""
    });
    let out = p.returned("toolu_synthesis", &answer.to_string());
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 1, "{out}");
    let f = &found[0];
    assert_eq!((text(f, "outcome"), text(f, "model")), ("found", "claude-sonnet-5-5"));
    assert_eq!(f.fields["cases"].as_array().map(|cs| cs.iter().filter_map(|c| c["case_id"].as_str()).collect::<Vec<_>>()), Some(vec!["a1", "a2"]));
    assert_eq!(
        f.fields["questions"],
        json!([
            { "question_id": "q1", "text": "Should a name typed in capitals be greeted in capitals?", "options": ["as typed", "in lower case"], "why": "main lowercases it" },
            { "question_id": "q2", "text": "Should an empty name be greeted?", "options": ["yes", "no"], "why": "the draft refuses it" }
        ])
    );
    let log = p.facts();
    let told = familiar_ai::teach::lens::after_answer(&View::new(&log), "c1");
    assert!(told.starts_with("familiar-ai: the lens panel answered for \"greet says hello\": it found two more checks worth adding and asks the person two questions.\n"), "{told}");
    assert!(context(&out).contains(&told), "the agent is told the card is open: {out}");
    let status = p.status_json().to_string();
    assert!(status.contains("cases_unconfirmed") && !status.contains("\"cases_owed\""), "{status}");

    // nothing is owed any more: an answer again records nothing, and a spawn goes as asked
    p.returned("toolu_synthesis", &answer.to_string());
    assert_eq!(p.of_kind("cases_found").len(), 1);
    let out = p.spawn("toolu_later");
    assert!(out["hookSpecificOutput"]["updatedInput"].is_null(), "no rewrite once the panel answered: {out}");
    assert_ne!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{out}");
}

#[test]
fn a_lens_only_reads_and_the_synthesis_has_no_tools() {
    let p = project("reads");
    let started = p.captured("SubagentStart", "subagent_start_case_finder", json!({ "agent_type": LENS }));
    assert!(!started.to_string().contains("THE PERSON ASKED") && !started.to_string().contains("works for the person"), "no context beside the brief: {started}");
    p.spawn(&id(0));
    for (tool, input) in [
        ("Write", json!({ "file_path": p.path("notes.txt").to_string_lossy(), "content": "x" })),
        ("Edit", json!({ "file_path": p.path("src/app.sh").to_string_lossy(), "old_string": "hi", "new_string": "hello" })),
        ("Bash", json!({ "command": "ls" })),
        ("Agent", json!({ "description": "d", "prompt": "p", "subagent_type": "general-purpose" })),
    ] {
        let out = p.lens_calls(tool, input);
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{tool}: {out}");
    }
    let read = json!({ "file_path": p.path("src/app.sh").to_string_lossy() });
    let out = p.lens_calls("Read", read.clone());
    assert_ne!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "a lens's read goes ahead: {out}");
    let stop = p.captured("SubagentStop", "subagent_stop_case_finder", json!({ "agent_type": LENS }));
    assert_ne!(stop["decision"], json!("block"), "a lens's stop is not held: {stop}");

    // once only the synthesis runs, the lens agent has no tools
    p.returned(&id(0), r#"{"findings": []}"#);
    for i in 1..5 {
        p.spawn(&id(i));
        p.returned(&id(i), r#"{"findings": []}"#);
    }
    p.spawn("toolu_synthesis");
    let out = p.lens_calls("Read", read);
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], json!("deny"), "{out}");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").contains("the synthesis has no tools"), "{out}");
    assert!(p.of_kind("cases_found").is_empty(), "the answer is read at the PostToolUse, not the stop");
}

#[test]
fn every_lens_answer_is_checked_against_its_schema() {
    let finding = |more: Value| {
        let mut f = json!({ "kind": "ambiguity", "note": "the draft reads greet as main's", "target_symbol": "greet", "file_path": "src/app.sh", "candidate_question": "Which greet?" });
        if let (Some(o), Some(m)) = (f.as_object_mut(), more.as_object()) {
            o.extend(m.clone());
        }
        f
    };
    let bare = |fs: Vec<Value>| json!({ "findings": fs }).to_string();
    let mut no_kind = finding(json!({}));
    if let Some(o) = no_kind.as_object_mut() {
        o.remove("kind");
    }
    for (name, answer, outcome, n) in [
        ("none", bare(vec![]), "none", 0),
        ("found", bare(vec![finding(json!({}))]), "found", 1),
        ("fenced", format!("Here they are.\n```json\n{}\n```\n", bare(vec![finding(json!({})), finding(json!({ "note": "two" }))])), "found", 2),
        ("nulls", bare(vec![finding(json!({ "target_symbol": null, "file_path": null, "candidate_question": null }))]), "found", 1),
        ("no kind", bare(vec![no_kind.clone()]), "found", 1),
        ("no note", bare(vec![finding(json!({ "note": " " }))]), "failed", 1),
        ("extra", bare(vec![finding(json!({ "severity": "high" }))]), "failed", 1),
        ("other key", json!({ "findings": [], "more": 1 }).to_string(), "failed", 1),
        ("prose", "Everything reads one way.".to_string(), "failed", 1),
        ("cut", "NOTE: this agent stopped at its 25-turn limit before finishing. The text below is PARTIAL output.\n{\"findings\":[]}".to_string(), "failed", 1),
    ] {
        let p = project(&format!("lens-{}", name.replace(' ', "-")));
        p.spawn(&id(0));
        p.returned(&id(0), &answer);
        let found = p.of_kind("lens_found");
        assert_eq!(found.len(), 1, "{name}");
        let f = &found[0];
        assert_eq!((text(f, "lens"), text(f, "outcome")), ("ambiguity", outcome), "{name}: {f:?}");
        let fs = f.fields["findings"].as_array().cloned().unwrap_or_default();
        assert_eq!(fs.len(), n, "{name}: {fs:?}");
        if outcome == "failed" {
            assert_eq!(fs[0]["kind"], json!("failed"), "{name}");
            assert!(fs[0]["note"].as_str().is_some_and(|w| w.starts_with("the ambiguity lens")), "{name}: a failure says why: {fs:?}");
        }
        if name == "nulls" {
            assert_eq!((fs[0]["target_symbol"].as_str(), fs[0]["file_path"].as_str(), fs[0]["candidate_question"].as_str()), (Some(""), Some(""), Some("")));
        }
        if name == "no kind" {
            assert_eq!(fs[0]["kind"], json!("ambiguity"), "the lens's own");
        }
        assert!(cases_owed(&p), "{name}: the other lenses are still owed");
    }
}

#[test]
fn every_synthesis_answer_is_checked_against_its_schema() {
    let three = json!({ "cases": [a_case("one"), a_case("two"), a_case("three")] }).to_string();
    let none = json!({ "cases": [], "none_because": "n1 and o1 cover every path greet has" }).to_string();
    let nine = json!({ "cases": (1..=9).map(|i| a_case(&format!("case {i}"))).collect::<Vec<_>>() }).to_string();
    let long = json!({ "cases": [a_case(&"x".repeat(71))] }).to_string();
    let extra = json!({ "cases": [{ "short": "s", "given": "g", "expect": "e", "why": "w", "id": "z9" }] }).to_string();
    let missing = json!({ "cases": [{ "short": "s", "given": "g", "expect": "e" }] }).to_string();
    let blank = json!({ "cases": [{ "short": "s", "given": " ", "expect": "e", "why": "w" }] }).to_string();
    let unexplained = json!({ "cases": [] }).to_string();
    let q = |text: &str, options: &[&str]| json!({ "text": text, "options": options, "why": "it decides a case" });
    let asked = json!({ "cases": [], "questions": [
        q("Which style?", &["long", "short"]),
        q("One option only?", &["yes"]),
        q("Five options?", &["a", "b", "c", "d", "e"]),
        q("Which style?", &["long", "short"]),
        { "text": "  ", "options": ["a", "b"] },
        { "text": "No why?", "options": ["a", "b"] },
        q("Nested too?", &["yes", "no", "only the outer"]),
        q("A fourth good one?", &["yes", "no"])
    ] })
    .to_string();
    for (name, answer, outcome, n) in [
        ("bare", three.as_str(), "found", 3),
        ("fenced", &format!("Here is what I found.\n```json\n{three}\n```\n"), "found", 3),
        ("none", none.as_str(), "none", 0),
        ("nine", nine.as_str(), "found", 8),
        ("long", long.as_str(), "found", 1),
        ("asked", asked.as_str(), "none", 0),
        ("extra", extra.as_str(), "failed", 0),
        ("missing", missing.as_str(), "failed", 0),
        ("blank", blank.as_str(), "failed", 0),
        ("unexplained", unexplained.as_str(), "failed", 0),
        ("prose", "I looked at greet and everything is covered.", "failed", 0),
    ] {
        let p = project(&format!("schema-{name}"));
        p.lenses_answer_none();
        p.synthesis_answers(answer);
        let found = p.of_kind("cases_found");
        assert_eq!(found.len(), 1, "{name}");
        let f = &found[0];
        assert_eq!(text(f, "outcome"), outcome, "{name}: {f:?}");
        assert_eq!(f.fields.get("cases").and_then(Value::as_array).map(Vec::len), Some(n), "{name}");
        match name {
            "none" => assert_eq!(text(f, "why"), "n1 and o1 cover every path greet has"),
            "nine" => assert_eq!(text(f, "why"), "the lens panel listed 9, the first 8 are kept: 1 left out"),
            "long" => assert_eq!(f.fields["cases"][0]["short"], json!("x".repeat(71)), "the long line whole"),
            "asked" => {
                // the first three well-formed ones, each text once, as q1..q3; a why left out is blank
                let qs: Vec<(String, String, usize, String)> = f.fields["questions"]
                    .as_array()
                    .map(|qs| qs.iter().map(|q| (q["question_id"].as_str().unwrap_or("").into(), q["text"].as_str().unwrap_or("").into(), q["options"].as_array().map_or(0, Vec::len), q["why"].as_str().unwrap_or("").into())).collect())
                    .unwrap_or_default();
                assert_eq!(
                    qs,
                    [
                        ("q1".to_string(), "Which style?".to_string(), 2, "it decides a case".to_string()),
                        ("q2".to_string(), "No why?".to_string(), 2, String::new()),
                        ("q3".to_string(), "Nested too?".to_string(), 3, "it decides a case".to_string())
                    ]
                );
                assert_eq!(text(f, "why"), "", "questions stand without a none_because");
            }
            _ => {}
        }
        if outcome == "failed" {
            assert!(!text(f, "why").trim().is_empty(), "{name}: a failure says why");
        }
        if name != "asked" {
            assert!(f.fields.get("questions").is_none(), "{name}: no questions, no field: {f:?}");
        }
    }
}

/// "Keep what fits" (the owner's ruling, 2026-10-25) [draft words], kept for the synthesis: its
/// first eight cases as a1..a8, a short line over 70 characters whole, and the found finding's why
/// saying how many were left out; the person's card shows that line after the found checks.
#[test]
fn a_synthesis_past_the_briefs_limits_keeps_what_fits_and_the_card_says_what_was_left_out() {
    let p = project("fits");
    let long = "the same greeting when the name comes in capitals, lower case or mixed case, as typed";
    let mut listed: Vec<Value> = (1..=10).map(|i| a_case(&format!("case {i}"))).collect();
    listed[1] = a_case(long);
    p.lenses_answer_none();
    let out = p.synthesis_answers(&json!({ "cases": listed }).to_string());
    let f = &p.of_kind("cases_found")[0];
    let note = "the lens panel listed 10, the first 8 are kept: 2 left out";
    assert_eq!((text(f, "outcome"), text(f, "why")), ("found", note), "{f:?}");
    let status = p.status_json();
    let card = status["cards"].as_array().and_then(|cs| cs.iter().find(|c| c["hold"] == "cases_unconfirmed")).cloned().unwrap_or_else(|| panic!("the cases card: {status}"));
    let lines: Vec<String> = card["person"]["lines"].as_array().map(|ls| ls.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    assert!(lines.contains(&format!("  2. {long}")), "{lines:?}");
    assert_eq!(lines[lines.len() - 3..], ["  8. case 8".to_string(), String::new(), note.to_string()], "{lines:?}");
    assert!(context(&out).contains(&format!("  {note}\n")), "the agent is told the line with the card's: {out}");
}

/// The person's card in `status --json` (its `person`) reads as the picker shows it: each option's
/// label and its own line (the wording the owner approved, plan §3).
#[test]
fn the_persons_card_in_status_gives_each_option_its_own_line() {
    let p = project("person");
    p.lenses_answer_none();
    p.synthesis_answers(&json!({ "cases": [a_case("one"), a_case("two"), a_case("three")] }).to_string());
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

/// The synthesis cut off at its turn limit is failed; look again re-runs the whole panel, its
/// calls released again from the first lens.
#[test]
fn the_turn_limit_records_failed_and_look_again_reruns_the_whole_panel() {
    let p = project("cut");
    p.lenses_answer_none();
    p.spawn("toolu_synthesis");
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_cut", json!({ "tool_use_id": "toolu_synthesis" }));
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 1);
    let f = &found[0];
    assert_eq!((text(f, "outcome"), text(f, "model")), ("failed", "claude-haiku-4-5-20251001"));
    assert!(text(f, "why").contains("turn limit"), "{f:?}");
    assert_eq!(f.fields.get("tokens"), Some(&json!(5320)));
    assert!(p.status_json().to_string().contains("cases_unconfirmed"));

    p.persons_answer("look_again");
    assert!(cases_owed(&p), "a second panel is owed");
    let spawn = p.spawn(&id(0));
    assert_eq!(spawn["hookSpecificOutput"]["updatedInput"]["prompt"], json!(p.brief("ambiguity")), "the first lens again: {spawn}");
    p.returned(&id(0), r#"{"findings": []}"#);
    for i in 1..5 {
        p.spawn(&id(i));
        p.returned(&id(i), r#"{"findings": []}"#);
    }
    p.synthesis_answers(&json!({ "cases": [a_case("one")] }).to_string());
    let found = p.of_kind("cases_found");
    assert_eq!(found.len(), 2);
    assert_eq!(text(&found[1], "outcome"), "found");
    assert_eq!(p.of_kind("lens_found").len(), 10, "five lenses each panel");
}

#[test]
fn a_background_launch_or_a_call_never_released_records_nothing() {
    let p = project("async");
    p.spawn(&id(0));
    let launched = json!({ "tool_use_id": id(0), "tool_response": { "isAsync": true, "status": "async_launched", "agentId": AGENT, "agentType": LENS, "resolvedModel": "claude-opus-5-5", "prompt": "p", "outputFile": "/tmp/claude/tasks/output", "canReadOutputFile": true } });
    p.captured("PostToolUse", "post_tool_use_agent_case_finder_found", launched);
    p.returned("toolu_never_released", r#"{"findings": []}"#);
    assert!(p.of_kind("lens_found").is_empty() && p.of_kind("cases_found").is_empty());
}

/// The person's last answer comes through the picker: every other line of the plan held, the
/// panel found nothing more and asked nothing, and the person picks "Looks right" on the card in
/// Claude Code's picker. The plan is confirmed at that PostToolUse, and the places it owes are
/// listed there (the owner's plan of 2026-10-21, step 3), not first at a later claim.
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
    p.lenses_answer_none();
    p.synthesis_answers(r#"{"cases": [], "none_because": "n1 and o1 cover every path greet has"}"#);
    assert_eq!(p.status_json()["plan"], json!("planning"), "the card is open");
    assert!(p.of_kind("places_listed").is_empty(), "nothing is listed before the plan is confirmed");

    let log = p.facts();
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().find(|c| c.hold == "cases_unconfirmed").expect("the cases card");
    let question = familiar_ai::teach::cases_card::shape(&familiar_ai::teach::cases_card::shown(&v, &card));
    let asking = json!({ "tool_input": { "questions": [{ "question": question, "header": "x", "options": [{ "label": "yes", "description": "Continue" }], "multiSelect": false }] } });
    let pre = p.captured("PreToolUse", "pre_tool_use_ask_user_question", asking);
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], json!("ask"), "{pre}");
    let asked = pre["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(asked["questions"].as_array().map(Vec::len), Some(1), "no question of the panel's: {asked}");

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
