//! The bench's real claim orders, replayed through the rules (ports 2026-10-24, piece 1) [draft
//! names]: a measurement, not a rule. The kept `.familiar/record` of every plugin case of three
//! benches (`runs-max/<case>__plugin/attempt-1/`: bench-c2v, run at 1df36806; bench-c2b-f1;
//! bench-44-bar) was read through the facts crate's reader and cut down to the facts line 4 reads,
//! in record order, each with its kind, time and fields as recorded (`tests/replayed/<bench>.jsonl`,
//! one case a line). The replay rebuilds each log fact by fact through `append_raw`, asks the rules
//! for every part's line 4 at every prefix, and looks for one signature: a part's line 4 held, and
//! the agent's re-claim of a fact already standing (the same kind, part and fields as the latest of
//! its kind) undid it. A case is flagged when it shows twice; once is a toll the agent paid and got
//! past. A record from before the try holds no `trace_listed` and no `off_list_reason`.
//!
//! The cut is checked when it is made (`the_kept_records_are_cut_to_what_line_4_reads`, run by hand
//! with the bench folder): after every fact of the whole record, every part's line 4 reads the same
//! as after the matching fact of the cut one.

mod traced;

use std::collections::BTreeMap;

use familiar_ai_facts::{append_raw, Fact, Log, Record};
use familiar_ai_rules::{plan, trace, View};
use serde_json::{json, Value};
use traced::*;

/// The agent's claims among the kinds line 4 reads [draft name]: the part and its path, its anchor
/// exits, its planned tests and the stacks they run under, and its reasons for a place off the list.
const AGENT_CLAIMS: [&str; 7] = ["criterion", "place_chosen", "new_work_spot_chosen", "new_work_no_spot", "test_planned", "test_stack_detected", trace::OFF_LIST];

/// The rest line 4 reads [draft name]: the person's answers (a `re_plan` unchooses the place) and the
/// binary's tests-first commit and try listings.
const ALSO_READ: [&str; 3] = ["hold_answered", "test_written", trace::LISTED];

/// The benches, by their folder's name, and how many plugin cases each kept.
const BENCHES: [(&str, usize); 3] = [("bench-c2v", 40), ("bench-c2b-f1", 40), ("bench-44-bar", 44)];

/// How many times the signature must show for a case to be flagged [draft name].
const FLAGGED_AT: usize = 2;

fn read_by_line_4(kind: &str) -> bool {
    AGENT_CLAIMS.contains(&kind) || ALSO_READ.contains(&kind)
}

/// One kept case: its name and its facts, each `[at, kind, fields]`.
struct Case {
    case: String,
    facts: Vec<Value>,
}

fn cases_of(jsonl: &str) -> Vec<Case> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("a kept case is one JSON line");
            Case { case: v["case"].as_str().expect("its case").to_string(), facts: v["facts"].as_array().expect("its facts").clone() }
        })
        .collect()
}

/// The log rebuilt fact by fact through the one door.
fn rebuild(facts: &[Value]) -> Log {
    facts.iter().fold(Vec::new(), |log, f| {
        let (at, kind, fields) = (f[0].as_f64().expect("at"), f[1].as_str().expect("kind"), f[2].as_object().cloned().expect("fields"));
        append_raw(&log, kind, fields, at, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
    })
}

/// The facts of a log that line 4 reads, as `[at, kind, fields]`.
fn cut(log: &[Fact]) -> Vec<Value> {
    log.iter().filter(|f| read_by_line_4(f.kind)).map(|f| json!([f.at, f.kind, Value::Object(f.fields.clone())])).collect()
}

/// One kept case as its line.
fn line_of(case: &str, log: &[Fact]) -> String {
    serde_json::to_string(&json!({ "case": case, "facts": cut(log) })).expect("json") + "\n"
}

/// Every part's line 4 at the prefix `log`.
fn line_4_of_each(log: &[Fact]) -> BTreeMap<String, bool> {
    let v = View::new(log);
    plan::criteria(&v)
        .into_iter()
        .map(|c| {
            let held = plan::line_4_of(&v, &c);
            (c, held)
        })
        .collect()
}

/// One sighting of the signature: the agent's fact at `t`, of `kind`, undid `part`'s line 4.
#[derive(Debug, Clone, PartialEq)]
struct Undone {
    t: u64,
    kind: &'static str,
    part: String,
}

/// Whether `f` is the agent's re-claim of a fact already standing: one of its claims, identical to
/// the latest earlier fact of its kind about the same part.
fn re_claimed(earlier: &[Fact], f: &Fact) -> bool {
    AGENT_CLAIMS.contains(&f.kind) && earlier.iter().rev().find(|g| g.kind == f.kind && g.criterion_id() == f.criterion_id()).is_some_and(|g| g.fields == f.fields)
}

/// The signature at every prefix of `log` [draft name]: a part's line 4 held, and the agent's
/// re-claim of a fact already standing undid it.
fn undone_by_a_re_claim(log: &[Fact]) -> Vec<Undone> {
    let mut out = Vec::new();
    let mut before = line_4_of_each(&[]);
    for (i, f) in log.iter().enumerate() {
        let after = line_4_of_each(&log[..=i]);
        if re_claimed(&log[..i], f) {
            for (c, held) in &before {
                if *held && after.get(c) == Some(&false) {
                    out.push(Undone { t: f.t, kind: f.kind, part: c.clone() });
                }
            }
        }
        before = after;
    }
    out
}

/// The flagged cases of a bench's kept lines, each with its sightings.
fn flagged(jsonl: &str) -> Vec<(String, Vec<Undone>)> {
    cases_of(jsonl)
        .into_iter()
        .map(|c| {
            let seen = undone_by_a_re_claim(&rebuild(&c.facts));
            (c.case, seen)
        })
        .filter(|(_, seen)| seen.len() >= FLAGGED_AT)
        .collect()
}

fn reason(log: &Log) -> Log {
    raw(log, trace::OFF_LIST, json!({ "criterion_id": "c1", "reason": "the header comes from a template the test renders through a cache" }))
}

fn off_list_place(log: &Log) -> Log {
    placed(log, "src/template.py", "render_header", (3, 9))
}

#[test]
fn the_replay_flags_line_4_undone_twice_by_the_same_tests_planned_again_not_by_the_same_place_again_nor_the_reason_last() {
    // definition change (ports 2026-10-24, piece 2): the same place claimed again after its reason
    // keeps line 4, so the deadlock of 1df36806 shows nothing
    let mut log = off_list_place(&reason(&off_list_place(&tried(&planned(), true, top()))));
    assert!(plan::line_4_of(&View::new(&log), "c1"), "the same place claimed again keeps its reason");
    assert_eq!(undone_by_a_re_claim(&log), Vec::new());
    // the signature by hand: line 4 holds, then the same tests planned again owe a new try and undo
    // it, twice
    for _ in 0..2 {
        assert!(plan::line_4_of(&View::new(&log), "c1"));
        log = raw(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": TEST, "will": "create" }));
        log = tried(&log, true, top());
    }
    let found = flagged(&line_of("by hand", &log));
    let seen: Vec<(&str, &str)> = found.iter().flat_map(|(case, seen)| seen.iter().map(move |u| (case.as_str(), u.kind))).collect();
    assert_eq!(seen, vec![("by hand", "test_planned"), ("by hand", "test_planned")], "{found:?}");
    // another place after the reason undoes line 4 too, but it is a new claim, not a re-claim
    let log = placed(&reason(&off_list_place(&tried(&planned(), true, top()))), "src/template.py", "render_footer", (12, 20));
    assert!(!plan::line_4_of(&View::new(&log), "c1"));
    assert_eq!(undone_by_a_re_claim(&log), Vec::new());
    // the same claims reordered, the place three times and the reason last: line 4 never held before
    let mut log = tried(&planned(), true, top());
    for _ in 0..3 {
        log = off_list_place(&log);
    }
    let log = reason(&log);
    assert!(plan::line_4_of(&View::new(&log), "c1"));
    assert_eq!(undone_by_a_re_claim(&rebuild(&cut(&log))), Vec::new());
    assert_eq!(flagged(&line_of("by hand, reordered", &log)), Vec::new());
}

#[test]
fn no_c2v_record_is_flagged() {
    let jsonl = include_str!("replayed/bench-c2v.jsonl");
    assert_eq!(cases_of(jsonl).len(), BENCHES[0].1, "bench-c2v: every kept case");
    let found = flagged(jsonl);
    let counts: Vec<(&str, usize)> = found.iter().map(|(case, seen)| (case.as_str(), seen.len())).collect();
    assert_eq!(counts, Vec::<(&str, usize)>::new(), "{found:?}");
}

#[test]
fn the_c2b_f1_and_44_bar_records_flag_none() {
    for (bench, jsonl, kept) in [("bench-c2b-f1", include_str!("replayed/bench-c2b-f1.jsonl"), BENCHES[1].1), ("bench-44-bar", include_str!("replayed/bench-44-bar.jsonl"), BENCHES[2].1)] {
        let cases = cases_of(jsonl);
        assert_eq!(cases.len(), kept, "{bench}: every kept case");
        // records from before the try: no listing, no reason
        assert!(cases.iter().all(|c| !c.facts.iter().any(|f| f[1] == json!(trace::LISTED) || f[1] == json!(trace::OFF_LIST))), "{bench}");
        assert_eq!(flagged(jsonl), Vec::new(), "{bench}");
    }
}

/// Reads every kept record of the benches under `FAMILIAR_AI_BENCH` (the folder holding
/// `bench-c2v`, `bench-c2b-f1` and `bench-44-bar`) through the facts crate's reader, from a copy in
/// the temporary folder (opening a record writes to it), and writes `tests/replayed/<bench>.jsonl`.
#[test]
#[ignore = "reads the bench folders: FAMILIAR_AI_BENCH=<folder> cargo test -p familiar-ai-rules --test replayed_bench_records -- --ignored"]
fn the_kept_records_are_cut_to_what_line_4_reads() {
    let root = std::path::PathBuf::from(std::env::var("FAMILIAR_AI_BENCH").expect("FAMILIAR_AI_BENCH names the bench folder"));
    let scratch = std::env::temp_dir().join(format!("familiar-ai-replayed-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("the scratch folder");
    for (bench, kept) in BENCHES {
        let runs = root.join(bench).join("runs-max");
        let mut names: Vec<String> = std::fs::read_dir(&runs).expect("runs-max").filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with("__plugin")).collect();
        names.sort();
        assert_eq!(names.len(), kept, "{bench}: every plugin case");
        let mut out = String::new();
        for name in names {
            let record = runs.join(&name).join("attempt-1/repo/.familiar/record");
            let copy = scratch.join(&name);
            std::fs::copy(&record, &copy).unwrap_or_else(|e| panic!("{}: {e}", record.display()));
            let whole = Record::open(&copy).and_then(|r| r.read_all()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            std::fs::remove_file(&copy).expect("the copy removed");
            let kept = rebuild(&cut(&whole));
            // after every fact of the whole record, line 4 reads as after the matching fact of the cut
            let mut k = 0;
            for i in 0..=whole.len() {
                k += usize::from(i > 0 && read_by_line_4(whole[i - 1].kind));
                assert_eq!(line_4_of_each(&whole[..i]), line_4_of_each(&kept[..k]), "{bench} {name}: line 4 after fact {i}");
            }
            let case = name.trim_end_matches("__plugin");
            out.push_str(&line_of(case, &whole));
            println!("{bench} {case}: {} facts of {}", kept.len(), whole.len());
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/replayed");
        std::fs::create_dir_all(&dir).expect("tests/replayed");
        std::fs::write(dir.join(format!("{bench}.jsonl")), out).expect("the kept cases written");
    }
    let _ = std::fs::remove_dir(&scratch);
}
