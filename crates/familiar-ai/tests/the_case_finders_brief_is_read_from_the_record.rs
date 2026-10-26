//! The case-finder's brief (the owner's plan of 2026-10-25, §2) [draft words, draft names]: the
//! prompt the binary hands the subagent in place of the main agent's, word for word, every variable
//! from a recorded fact: the person's words (`ask::words_of`), their typed answers on cards
//! (`trace::answers`; the line left out when there are none), the part's text, the promise's
//! accepts / gives back / refuses, its cases one line each, the intent's sha and the most cases (8).
//! The case-finder pushes back (the owner's plan of 2026-10-25, approved as written) [draft words]:
//! the plan's place when the record has one before the finder runs (the part's latest
//! `fix_location_category` where, and a standing `place_chosen`'s places), and an ask of four steps
//! in order: the person's situation traced to where it really fails, every clause of their words
//! covered, the case that defeats the planned fix (only when a place is recorded), then the variants.
//! With no place recorded the brief says nothing about one. The answer's JSON is as before.

mod cases_fixture;

use cases_fixture::*;
use familiar_ai::teach::case_finder::{after_finding, brief, MAX_CASES};
use familiar_ai_rules::View;
use serde_json::json;

/// The ask with no place recorded: three steps, nothing said about a place. Updated with the
/// definition (the owner's plan of 2026-10-25, the case-finder pushes back) [draft words]: it was
/// "Read where this behaviour lives, which inputs and paths reach it, and what nearby behaviour
/// must keep working." and the variants alone.
const ASK: &str = "The code as it stands is in the current directory (the commit 4f1c2ab). Work in this order:
1. Trace the person's own situation, as they describe it above, through the code to where it really fails. If that
   is not what the part promises, the first case is the person's situation
   where it really fails, and its why says what the plan narrowed.
2. Check that each clause of the person's words is covered by the part or its cases. A clause left out is a case,
   and its why names the clause.
3. Then the variants of what the person described that the code makes reachable (other options, styles, types,
   call paths), and behaviour that must stay as it is.

List up to 8 more cases a complete change must also satisfy, in the order of the steps, only ones the planned
cases do not already cover. Leave out details the person's words do not imply (exact wording of a message, an exact
value nobody asked for). If the planned cases already cover it, say so and list none.

";

/// The ask with a place recorded: four steps, the third the case that defeats the planned fix.
const ASK_AT_PLACE: &str = "The code as it stands is in the current directory (the commit 4f1c2ab). Work in this order:
1. Trace the person's own situation, as they describe it above, through the code to where it really fails. If that
   is not what the part promises, or not where the agent plans to fix it, the first case is the person's situation
   where it really fails, and its why says what the plan narrowed.
2. Check that each clause of the person's words is covered by the part or its cases. A clause left out is a case,
   and its why names the clause.
3. Name the case that would defeat a fix where the agent plans it: a situation that fix would still get wrong.
4. Then the variants of what the person described that the code makes reachable (other options, styles, types,
   call paths), and behaviour that must stay as it is.

List up to 8 more cases a complete change must also satisfy, in the order of the steps, only ones the planned
cases do not already cover. Leave out details the person's words do not imply (exact wording of a message, an exact
value nobody asked for). If the planned cases already cover it, say so and list none.

";

/// The answer's shape, word for word as before the push back.
const ANSWER: &str = r#"Answer with JSON only:
{"cases": [{"short": "<one line, at most 70 characters, for the person>",
            "given": "<the situation, concretely>",
            "expect": "<what a correct change gives>",
            "why": "<what in the person's words or the code makes it necessary>"}],
 "none_because": "<when the list is empty: why the planned cases cover it>"}"#;

const HEAD: &str = "You find the test cases a change must satisfy, before any test or fix is written. You only read: you may not edit,
write or run anything.

The person asked:
error groups with every frame hidden crash with an IndexError
";

const MIDDLE: &str = "
The agent split that into this part:
error groups with every frame hidden show a clear message

What it promises: given an error group whose frames are all hidden, it gives back a clear message in place of the traceback; it refuses nothing.

The cases the agent already plans to test:
n1 (normal): given an ExceptionGroup whose every frame is hidden → expect the hidden-entries message
o1 (opposite): given a group with visible frames → expect the traceback as before

";

#[test]
fn the_brief_is_the_plans_words_with_every_variable_from_its_fact() {
    assert_eq!(MAX_CASES, 8);
    // planned() records its fix_location_category with where "": no place
    let log = planned();
    assert_eq!(brief(&View::new(&log), "c1"), format!("{HEAD}{MIDDLE}{ASK}{ANSWER}"));
}

#[test]
fn the_persons_typed_answers_follow_their_words() {
    let log = raw(&planned(), "hold_answered", json!({ "criterion_id": "c1", "hold": "repro_unknown", "choice": "typed", "typed": "python -m pytest --tb=short shows the IndexError" }));
    let want = format!("{HEAD}python -m pytest --tb=short shows the IndexError\n{MIDDLE}{ASK}{ANSWER}");
    assert_eq!(brief(&View::new(&log), "c1"), want);
}

#[test]
fn the_brief_reads_the_latest_promise_and_nothing_the_agent_said() {
    // the promise claimed again: its new case is in the brief
    let log = raw(
        &planned(),
        "promise",
        json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "an error group", "gives_back": "a message", "refuses": "a group with no frames", "contract_version": 1, "cases": [
            { "case_id": "n1", "kind": "normal", "given": "g1", "expect": "e1" },
            { "case_id": "e1", "kind": "edge", "given": "g2", "expect": "e2" }
        ] }),
    );
    // the agent's own words in the record go nowhere near the brief
    let log = raw(&log, "off_list_reason", json!({ "criterion_id": "c1", "reason": "AGENTS OWN REASONING" }));
    let b = brief(&View::new(&log), "c1");
    assert!(b.contains("What it promises: given an error group, it gives back a message; it refuses a group with no frames.\n"), "{b}");
    assert!(b.contains("\nn1 (normal): given g1 → expect e1\ne1 (edge): given g2 → expect e2\n\n"), "{b}");
    assert!(!b.contains("AGENTS OWN REASONING"), "{b}");
}

#[test]
fn after_the_finding_the_agent_is_told_the_card_is_open_and_to_ask_the_person() {
    let log = found(&planned(), "found", 3);
    let told = after_finding(&View::new(&log), "c1");
    assert!(told.starts_with(&format!("familiar-ai: the case-finder answered for \"{PART}\": it found three more checks worth adding.\n")), "{told}");
    assert!(told.contains("Ask the person with AskUserQuestion"), "{told}");
    assert!(told.contains("  1. the same with the other --tb styles (short, line, native)"), "{told}");
    let none = after_finding(&View::new(&found(&planned(), "none", 0)), "c1");
    assert!(none.starts_with(&format!("familiar-ai: the case-finder answered for \"{PART}\": it found nothing more worth checking.\n")), "{none}");
    let failed = after_finding(&View::new(&found(&planned(), "failed", 0)), "c1");
    assert!(failed.starts_with(&format!("familiar-ai: the case-finder answered for \"{PART}\": it could not finish looking.\n")), "{failed}");
}

const WHERE: &str = "src/_pytest/_code/code.py, where the traceback is cut to its visible entries";

fn at_place(log: &familiar_ai_facts::Log) -> familiar_ai_facts::Log {
    raw(log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": WHERE }))
}

fn chosen(log: &familiar_ai_facts::Log) -> familiar_ai_facts::Log {
    raw(log, "place_chosen", json!({ "criterion_id": "c1", "places": [{ "file_path": "src/_pytest/_code/code.py", "symbol": "FormattedExcinfo.repr_traceback", "line_start": 1100, "line_end": 1180 }] }))
}

const CHOSEN: &str = "src/_pytest/_code/code.py FormattedExcinfo.repr_traceback lines 1100–1180";

/// The place a brief carries: the lines under "Where the agent plans to fix it:", or None.
fn carried(b: &str) -> Option<&str> {
    let (_, rest) = b.split_once("\n\nWhere the agent plans to fix it:\n")?;
    rest.split_once("\n\nThe code as it stands").map(|(p, _)| p)
}

#[test]
fn the_plans_place_is_carried_when_the_record_has_one_and_none_is_named_without_one() {
    let at = |log: &familiar_ai_facts::Log| brief(&View::new(log), "c1");
    let none = at(&planned());
    assert_eq!(carried(&none), None, "a where of \"\" is no place: {none}");
    assert!(!none.contains("where the agent plans") && !none.contains("would defeat"), "{none}");

    // the latest fix_location_category's where
    let log = at_place(&planned());
    let block = format!("Where the agent plans to fix it:\n{WHERE}\n\n");
    assert_eq!(at(&log), format!("{HEAD}{MIDDLE}{block}{ASK_AT_PLACE}{ANSWER}"));

    // a standing place_chosen's places follow, one line each
    let log = chosen(&log);
    assert_eq!(carried(&at(&log)), Some(format!("{WHERE}\n{CHOSEN}").as_str()));
    // the places alone, the where blank
    assert_eq!(carried(&at(&chosen(&planned()))), Some(CHOSEN));

    // a re_plan unchooses the places; a later where of "" is the latest
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "re_plan", "typed": "" }));
    assert_eq!(carried(&at(&log)), Some(WHERE), "the re_plan unchose the places");
    let log = raw(&log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "" }));
    assert_eq!(at(&log), format!("{HEAD}{MIDDLE}{ASK}{ANSWER}"));
}

#[test]
fn the_ask_is_four_steps_in_order_and_the_answers_shape_is_as_before() {
    let steps = ["1. Trace the person's own situation", "2. Check that each clause of the person's words", "3. Name the case that would defeat a fix", "4. Then the variants", "List up to 8 more cases"];
    let b = brief(&View::new(&at_place(&planned())), "c1");
    let at: Vec<usize> = steps.iter().map(|s| b.find(s).unwrap_or_else(|| panic!("{s}: {b}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "in order: {at:?}\n{b}");
    assert!(b.contains("in the order of the steps") && b.ends_with(ANSWER), "{b}");
    // with no place, the third step is left out and the variants are the third
    let b = brief(&View::new(&planned()), "c1");
    let steps = ["1. Trace the person's own situation", "2. Check that each clause of the person's words", "3. Then the variants", "List up to 8 more cases"];
    let at: Vec<usize> = steps.iter().map(|s| b.find(s).unwrap_or_else(|| panic!("{s}: {b}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]) && !b.contains("4. "), "in order: {at:?}\n{b}");
    assert!(b.ends_with(ANSWER), "{b}");
}

/// The brief replayed on kept bench runs, no model asked [draft names]:
/// `FAMILIAR_AI_CASE_FINDER_REPLAY` names a file of `<case>\t<record>` lines. Each record is read
/// from a copy in the temporary folder (opening a record writes to it); for each part, on the
/// record as it stood just before the part's first `cases_found`, its brief is written to
/// `FAMILIAR_AI_CASE_FINDER_OUT`, one JSON line each (case, criterion_id, at, the place the brief
/// carried or null, the brief); a run with no `cases_found` gets one line, brief null.
#[test]
#[ignore = "reads kept bench runs: FAMILIAR_AI_CASE_FINDER_REPLAY=<list> FAMILIAR_AI_CASE_FINDER_OUT=<file> cargo test -p familiar-ai --test the_case_finders_brief_is_read_from_the_record -- --ignored"]
fn the_kept_runs_replayed_through_the_brief() {
    let list = std::env::var("FAMILIAR_AI_CASE_FINDER_REPLAY").expect("FAMILIAR_AI_CASE_FINDER_REPLAY names the list of runs");
    let out = std::env::var("FAMILIAR_AI_CASE_FINDER_OUT").expect("FAMILIAR_AI_CASE_FINDER_OUT names the file written");
    let scratch = std::env::temp_dir().join(format!("familiar-ai-case-finder-{}", std::process::id()));
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
            let b = brief(&View::new(&log[..j]), c);
            lines.push_str(&format!("{}\n", json!({ "case": case, "criterion_id": c, "at": j, "place": carried(&b), "brief": b })));
        }
        if seen.is_empty() {
            lines.push_str(&format!("{}\n", json!({ "case": case, "brief": null })));
        }
        println!("{case}: {} parts briefed, {} facts", seen.len(), log.len());
    }
    std::fs::write(&out, lines).expect("the replay written");
    let _ = std::fs::remove_dir_all(&scratch);
}
