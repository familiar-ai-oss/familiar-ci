//! The lens panel's briefs (the owner's plan of 2026-10-26, lens-panel.md, approved; they replace
//! the case-finder's brief of 2026-10-25) [draft words]: the prompt the binary hands each lens and
//! the synthesis in place of the main agent's, word for word the offline trial's
//! (scratchpad/lens/run_lens.py: PANEL, each lens's body, RULES, FINDINGS_OUT; the synthesis its
//! SYNTH, its output the cases the person's card offers and the questions for the person), every
//! variable from a recorded fact: the person's words (`ask::words_of`), their typed answers on how
//! they make it happen (`trace::answers`, "(nothing typed)" when there are none), and the part's
//! plan lines as the record holds them (its intent, behavior, latest promise with its cases,
//! criterion, planned tests, latest fix location and a standing place, in record order, printed as
//! the trial printed them); the synthesis also reads the five lenses' findings.

mod cases_fixture;

use cases_fixture::*;
use familiar_ai::teach::lens::{after_answer, brief, draft, lens_brief, synthesis_brief, MAX_CASES, MAX_QUESTIONS};
use familiar_ai_rules::View;
use serde_json::{json, Value};

/// The trial's PANEL, word for word.
const PANEL: &str = "You are one advisory lens on a panel reviewing a DRAFT plan a coding agent made from a person's change request:
its parts, the promise each part makes with the test cases it will check, and where it plans to fix it. ";

/// The trial's ambiguity lens, word for word.
const AMBIGUITY: &str = "Your lens: AMBIGUITY — places where the request is underspecified and the draft silently guessed.

Hunt for:
- vague qualifiers in the request (\"gracefully\", \"should handle\", \"correctly\") that the draft resolved to a specific
  reading without evidence;
- unstated scope boundaries (which components/paths are in or out) the draft assumed;
- missing constraints (compatibility, ordering, error behavior) where two reasonable readings of the request diverge
  and the draft picked one.";

/// The trial's RULES, word for word.
const RULES: &str = "RULES:
- You NEVER answer a question about what the person wants — you surface what should be ASKED. Humans answer.
- Only surface findings whose answer would plausibly CHANGE the implementation or the tests. Quality over
  quantity; an empty findings list is a valid, good answer.
- You only read: Read, Grep and Glob on the repository in the current directory, at the commit the person is on.
  Gather your own context from the code; cite file:line for what you rely on.";

/// The trial's FINDINGS_OUT, word for word.
const FINDINGS_OUT: &str = r#"Output ONLY the JSON object, no prose, no markdown:
{"findings": [{"kind": "<your lens>", "note": "<what the draft guessed or missed and why it matters>",
  "target_symbol": "<qualified name or null>", "file_path": "<path or null>",
  "candidate_question": "<the question a human should be asked, or null>"}]}"#;

/// The first line of each of the other lenses' bodies, the trial's words.
const FIRST_LINES: [(&str, &str); 5] = [
    ("ambiguity", "Your lens: AMBIGUITY — places where the request is underspecified and the draft silently guessed."),
    ("completeness", "Your lens: COMPLETENESS — what the request implies that no draft case covers, and what the draft"),
    ("happy_path", "Your lens: the HAPPY PATH SEAM (founder-center's planning stage S2, which reads the code as"),
    ("edge_cases", "Your lens: EDGE CASES (founder-center's planning stage S2-L, which reads the code as"),
    ("order_and_coupling", "Your lens: GAPS IN ORDER AND COUPLING (founder-center's planning stage S3, which reads the code as"),
];

const TYPED_HEAD: &str = "THEIR ANSWER TO \"How do you make it happen? What do you run, and what do you see?\":";

/// What every brief reads, with `typed` the person's typed answers.
fn read(log: &familiar_ai_facts::Log, typed: &str) -> String {
    format!("THE PERSON ASKED:\n{SAID}\n\n{TYPED_HEAD}\n{typed}\n\nTHE AGENT'S DRAFT PLAN (plan lines as it submitted them):\n{}", draft(&View::new(log), "c1"))
}

#[test]
fn the_ambiguity_lens_brief_is_the_trials_words_with_every_variable_from_its_fact() {
    assert_eq!((MAX_CASES, MAX_QUESTIONS), (8, 3));
    let log = planned();
    let want = format!("{PANEL}{AMBIGUITY}\n\n{RULES}\n\n{}\n\n{FINDINGS_OUT}", read(&log, "(nothing typed)"));
    assert_eq!(lens_brief(&View::new(&log), "c1", "ambiguity").as_deref(), Some(want.as_str()));
    assert_eq!(brief(&View::new(&log), "c1", "ambiguity"), want);
}

#[test]
fn each_lens_brief_carries_its_own_body_and_the_rest_as_the_ambiguity_lens() {
    let log = planned();
    let v = View::new(&log);
    for (lens, first) in FIRST_LINES {
        let b = lens_brief(&v, "c1", lens).unwrap_or_else(|| panic!("{lens}: no brief"));
        assert!(b.starts_with(&format!("{PANEL}{first}\n")), "{lens}: {b}");
        assert!(b.ends_with(&format!("\n\n{RULES}\n\n{}\n\n{FINDINGS_OUT}", read(&log, "(nothing typed)"))), "{lens}: {b}");
        for (other, line) in FIRST_LINES.iter().filter(|(o, _)| *o != lens) {
            assert!(!b.contains(line), "{lens} carries {other}'s body");
        }
    }
    assert!(lens_brief(&v, "c1", "synthesis").is_none() && lens_brief(&v, "c1", "case-finder").is_none());
}

/// The draft is the part's plan lines in record order, each its kind first then its fields, as the
/// trial printed them (`json.dumps(..., indent=1)`).
#[test]
fn the_draft_is_the_parts_plan_lines_in_record_order() {
    let log = planned();
    let d = draft(&View::new(&log), "c1");
    assert!(d.starts_with("[\n {\n  \"kind\": \"intent_submitted\",\n"), "one space per level: {d}");
    let lines: Vec<Value> = serde_json::from_str(&d).expect("the draft is JSON");
    let kinds: Vec<&str> = lines.iter().filter_map(|l| l["kind"].as_str()).collect();
    assert_eq!(kinds, ["intent_submitted", "behavior", "promise", "criterion", "test_planned", "fix_location_category"]);
    assert_eq!(lines[0], json!({ "kind": "intent_submitted", "text": PART, "sha": SHA, "for": 0 }));
    assert_eq!(lines[2]["cases"].as_array().map(Vec::len), Some(2), "the promise with its cases");
    assert_eq!(lines[5], json!({ "kind": "fix_location_category", "criterion_id": "c1", "category": "this_repository", "where": "" }));
    // each line's fields in the form's order, as the plan file writes them (the trial read the plan
    // file), whatever order the record holds them in
    let log = raw(&log, "behavior", json!({ "text": "a hidden traceback reads plainly", "behavior_id": "b1" }));
    let d = draft(&View::new(&log), "c1");
    assert!(d.contains(&format!("\n {{\n  \"kind\": \"intent_submitted\",\n  \"text\": \"{PART}\",\n  \"sha\": \"{SHA}\",\n  \"for\": 0\n }}")), "{d}");
    assert!(d.contains("\n {\n  \"kind\": \"behavior\",\n  \"behavior_id\": \"b1\",\n  \"text\": \"a hidden traceback reads plainly\"\n }"), "{d}");
    let log = planned();

    // the latest promise, the latest fix location and a standing place; nothing the agent said, nothing of another part's
    let log = raw(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "an error group", "gives_back": "a message", "refuses": "nothing", "contract_version": 1, "cases": [{ "case_id": "n1", "kind": "normal", "given": "g1", "expect": "e1" }, { "case_id": "o1", "kind": "opposite", "given": "g2", "expect": "e2" }] }));
    let log = raw(&log, "place_chosen", json!({ "criterion_id": "c1", "places": [{ "file_path": "src/_pytest/_code/code.py", "symbol": "FormattedExcinfo.repr_traceback", "line_start": 1100, "line_end": 1180 }] }));
    let log = raw(&log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "src/_pytest/_code/code.py" }));
    let log = raw(&log, "off_list_reason", json!({ "criterion_id": "c1", "reason": "AGENTS OWN REASONING" }));
    let log = raw(&log, "test_planned", json!({ "criterion_id": "c2", "file_path": "testing/test_other.py", "will": "create" }));
    let d = draft(&View::new(&log), "c1");
    assert!(!d.contains("AGENTS OWN REASONING") && !d.contains("test_other.py"), "{d}");
    let lines: Vec<Value> = serde_json::from_str(&d).expect("the draft is JSON");
    let kinds: Vec<&str> = lines.iter().filter_map(|l| l["kind"].as_str()).collect();
    assert_eq!(kinds, ["intent_submitted", "behavior", "criterion", "test_planned", "promise", "place_chosen", "fix_location_category"]);
    assert_eq!(lines[4]["accepts"], json!("an error group"), "the latest promise");
    assert_eq!(lines[6]["where"], json!("src/_pytest/_code/code.py"), "the latest fix location");

    // a re_plan unchooses the place
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "re_plan", "typed": "" }));
    assert!(!draft(&View::new(&log), "c1").contains("place_chosen"));
}

#[test]
fn the_persons_typed_answers_follow_their_words() {
    let log = raw(&planned(), "hold_answered", json!({ "criterion_id": "c1", "hold": "repro_unknown", "choice": "typed", "typed": "python -m pytest --tb=short shows the IndexError" }));
    let b = brief(&View::new(&log), "c1", "edge_cases");
    assert!(b.contains(&format!("THE PERSON ASKED:\n{SAID}\n\n{TYPED_HEAD}\npython -m pytest --tb=short shows the IndexError\n\nTHE AGENT'S DRAFT PLAN")), "{b}");
    assert!(!b.contains("(nothing typed)"), "{b}");
}

/// The synthesis's ask [draft words]: the trial's SYNTH, its output the cases the person's card
/// offers and the questions for the person.
const SYNTHESIS_HEAD: &str = "You merge the findings of an advisory lens panel into the test cases a coding agent's change must also satisfy,
before any test or fix is written, plus at most 3 questions for the person.";

const SYNTHESIS_ASK: &str = r#"Produce the cases a complete change must also satisfy, beyond the draft's own:
- Keep the draft where no finding changes it. Where a finding shows the draft guessed, narrowed or missed something
  the person's words imply, write a case: the situation, concretely, and what a correct change gives (normal: the
  behavior asked for; opposite: the same call with the trigger absent; edge: a boundary). These are the cases the
  person's card offers to add to the plan; list up to 8, the ones that matter most first, and leave out details the
  person's words do not imply.
- DEDUPLICATE: findings that describe the same unknown become one case or one question.
- Never answer a question about what the person wants: when an unknown would change the plan and the person's words
  do not settle it, ask it (text: one direct question a non-engineer can parse; options: 2 to 4 answers, not your
  recommendation; why: a short grounding note), and keep the draft's reading for that point. At most 3 questions.
- An empty question list is valid. If the draft's cases already cover it, list no case and say why.

You have no tools: answer from what is here.

Output ONLY the JSON object, no prose, no markdown:
{"cases": [{"short": "<one line, at most 70 characters, for the person>",
            "given": "<the situation, concretely>",
            "expect": "<what a correct change gives>",
            "why": "<the finding or the person's words that make it necessary>"}],
 "questions": [{"text": "...", "options": ["...", "..."], "why": "<short grounding note>"}],
 "none_because": "<when the case list is empty: why the draft's cases cover it>"}"#;

#[test]
fn the_synthesis_brief_carries_the_five_answers_and_asks_for_cases_and_questions() {
    let mut log = planned();
    for (lens, _) in FIRST_LINES {
        let findings = if lens == "edge_cases" {
            json!([{ "kind": "edge_cases", "note": "a nested group hides its frames too", "file_path": "src/_pytest/_code/code.py", "target_symbol": "", "candidate_question": "Should a nested group show the message?" }])
        } else {
            json!([])
        };
        let outcome = if lens == "edge_cases" { "found" } else { "none" };
        log = raw(&log, "lens_found", json!({ "criterion_id": "c1", "lens": lens, "outcome": outcome, "findings": findings, "model": "claude-opus-5-5", "tokens": 3000 }));
    }
    let b = synthesis_brief(&View::new(&log), "c1");
    let head = format!("{SYNTHESIS_HEAD}\n\n{}\n\nTHE PANEL'S FINDINGS (five lenses: ambiguity, completeness, the happy path from the person's entry, edge cases, order and coupling):\n", read(&log, "(nothing typed)"));
    assert!(b.starts_with(&head), "{b}");
    assert!(b.ends_with(&format!("\n\n{SYNTHESIS_ASK}")), "{b}");
    assert_eq!(brief(&View::new(&log), "c1", "synthesis"), b);
    let findings = &b[head.len()..b.len() - SYNTHESIS_ASK.len() - 2];
    let found: Value = serde_json::from_str(findings).unwrap_or_else(|e| panic!("{e}: {findings}"));
    assert_eq!(
        found,
        json!({
            "ambiguity": [], "completeness": [], "happy_path": [],
            "edge_cases": [{ "kind": "edge_cases", "note": "a nested group hides its frames too", "target_symbol": null, "file_path": "src/_pytest/_code/code.py", "candidate_question": "Should a nested group show the message?" }],
            "order_and_coupling": []
        })
    );
    let order: Vec<usize> = FIRST_LINES.iter().map(|(l, _)| findings.find(&format!("\"{l}\"")).unwrap_or(usize::MAX)).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the lenses in release order: {findings}");
}

#[test]
fn after_the_synthesis_the_agent_is_told_the_card_is_open_and_to_ask_the_person() {
    let log = found(&planned(), "found", 3);
    let told = after_answer(&View::new(&log), "c1");
    assert!(told.starts_with(&format!("familiar-ai: the lens panel answered for \"{PART}\": it found three more checks worth adding.\n")), "{told}");
    assert!(told.contains("Ask the person with AskUserQuestion"), "{told}");
    assert!(told.contains("  1. the same with the other --tb styles (short, line, native)"), "{told}");
    let none = after_answer(&View::new(&found(&planned(), "none", 0)), "c1");
    assert!(none.starts_with(&format!("familiar-ai: the lens panel answered for \"{PART}\": it found nothing more worth checking.\n")), "{none}");
    let failed = after_answer(&View::new(&found(&planned(), "failed", 0)), "c1");
    assert!(failed.starts_with(&format!("familiar-ai: the lens panel answered for \"{PART}\": it could not finish.\n")), "{failed}");
}

/// The briefs replayed on kept bench runs, no model asked [draft names] (after lane l's replay of
/// the case-finder's brief): `FAMILIAR_AI_LENS_REPLAY` names a file of `<case>\t<record>` lines.
/// Each record is read from a copy in the temporary folder (opening a record writes to it); for
/// each part, on the record as it stood just before the part's first `cases_found`, the five lens
/// briefs are written to `FAMILIAR_AI_LENS_OUT`, one JSON line per lens (case, criterion_id, lens,
/// brief); a run with no `cases_found` gets one line, brief null.
#[test]
#[ignore = "reads kept bench runs: FAMILIAR_AI_LENS_REPLAY=<list> FAMILIAR_AI_LENS_OUT=<file> cargo test -p familiar-ai --test the_lens_briefs_are_read_from_the_record -- --ignored"]
fn the_kept_runs_replayed_through_the_lens_briefs() {
    let list = std::env::var("FAMILIAR_AI_LENS_REPLAY").expect("FAMILIAR_AI_LENS_REPLAY names the list of runs");
    let out = std::env::var("FAMILIAR_AI_LENS_OUT").expect("FAMILIAR_AI_LENS_OUT names the file written");
    let scratch = std::env::temp_dir().join(format!("familiar-ai-lens-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("the scratch folder");
    let mut lines = String::new();
    for row in std::fs::read_to_string(&list).expect("the list").lines().filter(|l| !l.trim().is_empty()) {
        let Some((case, record)) = row.split_once('\t') else { panic!("not <case>\\t<record>: {row}") };
        let copy = scratch.join(case);
        std::fs::copy(record, &copy).unwrap_or_else(|e| panic!("{record}: {e}"));
        let log = familiar_ai_facts::Record::open(&copy).and_then(|r| r.read_all()).unwrap_or_else(|e| panic!("{case}: {e:?}"));
        std::fs::remove_file(&copy).expect("the copy removed");
        let mut seen: Vec<&str> = Vec::new();
        for (j, f) in log.iter().enumerate() {
            let Some(c) = f.criterion_id().filter(|c| f.kind == "cases_found" && !seen.contains(c)) else { continue };
            seen.push(c);
            let v = View::new(&log[..j]);
            for (lens, _) in FIRST_LINES {
                lines.push_str(&format!("{}\n", json!({ "case": case, "criterion_id": c, "lens": lens, "brief": brief(&v, c, lens) })));
            }
        }
        if seen.is_empty() {
            lines.push_str(&format!("{}\n", json!({ "case": case, "brief": null })));
        }
        println!("{case}: {} parts briefed, {} facts", seen.len(), log.len());
    }
    std::fs::write(&out, lines).expect("the replay written");
    let _ = std::fs::remove_dir_all(&scratch);
}
