//! `familiar-ai setup` in a fresh worktree of a project whose committed plan
//! has claims starts the record from that plan and the history, as `verify`
//! does in a fresh clone: the checkout shows the states verify's facts give,
//! a part verify covered reads as locally verified (never proven by the
//! pipeline while no CI run was seen), and the plan file it mirrors never
//! shrinks.

mod common;

use std::path::{Path, PathBuf};

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, PLAN_FILE, RECORD_FILE};
use familiar_ai_facts::Record;
use serde_json::{json, Value};

const SESSION: &str = "3a7c9e1f-5b2d-4e8a-9c6f-1d2e3f4a5b6c";

/// A set-up project planned for one criterion, its test committed alone and red.
fn sealed(name: &str) -> Project {
    sealed_with(name, |p| p.cases_kept("c1")) // the cases card passed (plan cases-card, 2026-10-25) [draft names]
}

/// `sealed`, the cases card passed by `card` once the part's contract is whole.
fn sealed_with(name: &str, card: fn(&Project)) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let said = p.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded");
    p.claim(&["intent_submitted", "--for", &said.to_string(), "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    card(&p);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    p
}

/// `sealed`, then the code on its anchor (green here) and the plan the code's
/// commit left behind, committed so the history carries it.
fn green(name: &str) -> Project {
    let p = sealed(name);
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: code");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    p.commit("c1: the plan");
    p
}

impl Project {
    /// A fresh worktree of HEAD on a new branch, beside the repository.
    fn worktree(&self) -> PathBuf {
        let wt = self.dir.join("wt");
        self.git(&["worktree", "add", "-q", "-b", "lane", &wt.to_string_lossy()]);
        wt
    }

    /// `familiar-ai setup` at `at`, which must succeed; what it printed.
    fn setup_at(&self, at: &Path) -> String {
        let (code, out, err) = self.run_in(at, &["setup"], "");
        assert_eq!(code, 0, "setup: {out}{err}");
        out
    }

    /// Each criterion's id, text and state in `status --json` at `at`.
    fn criteria_at(&self, at: &Path) -> Vec<(String, String, String)> {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "status --json: {err}");
        let j: Value = serde_json::from_str(&out).expect("status --json prints JSON");
        let s = |c: &Value, k: &str| c[k].as_str().unwrap_or_default().to_string();
        j["criteria"].as_array().expect("criteria").iter().map(|c| (s(c, "id"), s(c, "text"), s(c, "state_name"))).collect()
    }
}

fn plan_at(at: &Path) -> String {
    std::fs::read_to_string(at.join(FAMILIAR_FOLDER).join(PLAN_FILE)).unwrap_or_default()
}

fn part(state: &str) -> Vec<(String, String, String)> {
    vec![("c1".to_string(), "greet says hello".to_string(), state.to_string())]
}

/// Setup in a fresh worktree of a sealed project, its record missing or, with
/// `empty_record`, there and holding no fact: the record starts from the
/// committed plan and the history. At the tests-first commit the plan in git
/// does not yet say the test was written, so verify's facts give no run and
/// the part's test comes first.
fn starts_from_the_committed_plan(name: &str, empty_record: bool) {
    let p = sealed(name);
    let wt = p.worktree();
    // the plan as committed at HEAD, which the worktree checked out
    let committed = plan_at(&wt);
    assert!(!committed.is_empty());
    let record = wt.join(FAMILIAR_FOLDER).join(RECORD_FILE);
    assert!(!record.exists());
    if empty_record {
        let r = Record::open(&record).expect("an empty record");
        assert!(r.is_empty().expect("read"));
    }
    let out = p.setup_at(&wt);
    assert!(
        out.lines().any(|l| l == "familiar-ai: .familiar/record starts from the committed plan: 1 part, 1 commit rechecked, 0 runs"),
        "{out}"
    );
    assert!(!out.contains("starts empty"), "{out}");
    assert_eq!(p.criteria_at(&wt), part("testing"));
    assert_eq!(plan_at(&wt), committed, "setup leaves the plan file as committed");

    // a further claim appends to the plan file; the committed lines stay above it
    let _ = p.run_in(&wt, &["claim", "behavior", "--behavior_id", "b1", "--text", "the greeting"], "");
    let _ = p.run_in(&wt, &["claim", "promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#], "");
    let (code, out, err) = p.run_in(&wt, &["claim", "criterion", "--criterion_id", "c2", "--text", "bye says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"], "");
    assert_eq!(code, 0, "claim: {out}{err}");
    let now = plan_at(&wt);
    assert!(now.starts_with(&committed) && now.len() > committed.len(), "committed:\n{committed}\nnow:\n{now}");
}

#[test]
fn a_fresh_worktree_starts_its_record_from_the_committed_plan_and_the_history() {
    starts_from_the_committed_plan("seeded", false);
}

#[test]
fn an_empty_record_starts_from_the_committed_plan_too() {
    starts_from_the_committed_plan("seeded-empty", true);
}

#[test]
fn a_part_verify_covered_reads_locally_verified_never_proven_by_the_pipeline() {
    let p = green("verified");
    let wt = p.worktree();
    let out = p.setup_at(&wt);
    assert!(
        out.lines().any(|l| l == "familiar-ai: .familiar/record starts from the committed plan: 1 part, 3 commits rechecked, 3 runs"),
        "{out}"
    );
    assert_eq!(p.criteria_at(&wt), part("proven"));
    let (_, status, _) = p.run_in(&wt, &["status"], "");
    assert!(status.contains("familiar-ai: \"greet says hello\" · locally verified"), "{status}");
    assert!(!status.contains("pipeline"), "no CI run was seen: {status}");
    assert!(status.contains("familiar-ai: CI: no run for "), "{status}");
}

/// The seed (the owner, 2026-10-15): setup writes its reruns as this checkout's laptop evidence
/// and record_seeded last, so the part reads sealed and green on the laptop too (its journey:
/// red at the tests-first commit, green at the head), nothing is owed, and the plan file stays
/// the committed one until a claim made here adds its one line.
#[test]
fn the_seed_reads_sealed_and_green_on_the_laptop_owes_nothing_and_keeps_the_committed_plan() {
    let p = green("laptop");
    let short = |rev: &str| p.git(&["rev-parse", "--short=7", rev]).trim().to_string();
    let (tests_first, head) = (short("HEAD~2"), short("HEAD"));
    let wt = p.worktree();
    let committed = plan_at(&wt);
    p.setup_at(&wt);
    assert_eq!(plan_at(&wt), committed, "setup leaves the plan file as committed");
    let (code, out, err) = p.run_in(&wt, &["status", "--json"], "");
    assert_eq!(code, 0, "{err}");
    let j: Value = serde_json::from_str(&out).expect("status --json");
    let step = |name: &str| j["criteria"][0]["journey"]["steps"].as_array().and_then(|s| s.iter().find(|x| x["name"] == name).cloned()).unwrap_or(Value::Null);
    assert_eq!(step("red"), json!({ "name": "red", "done": true, "at": tests_first }), "{j}");
    assert_eq!(step("green"), json!({ "name": "green", "done": true, "at": head }), "{j}");
    assert_eq!(common::project::facts_at(&wt).last().map(|f| f.kind), Some("record_seeded"), "the mark is written last");

    let (code, out, err) = p.run_in(&wt, &["claim", "behavior", "--behavior_id", "b2", "--text", "the farewell"], "");
    assert_eq!(code, 0, "claim: {out}{err}");
    let line = json!({ "kind": "behavior", "behavior_id": "b2", "text": "the farewell" });
    assert_eq!(plan_at(&wt), format!("{committed}{line}\n"), "the committed plan, then the one claim made here");
    assert!(common::project::facts_at(&wt).iter().all(|f| f.kind != "run_reserved"), "nothing is owed: no run after setup");
}

/// The case-finder found one case and the person added it (add_all) [draft names]: the binary's
/// finding and the person's answer appended to the record as the hook and the picker write them,
/// the pick told as the picker's reply tells it (a fixture, as `common::cases_kept_at` is), then
/// the agent's promise claimed again holding the found case.
fn added_a_found_case(p: &Project) {
    let obj = |v: Value| v.as_object().cloned().unwrap_or_default();
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(1.0);
    {
        let record = Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("the record");
        let found = obj(json!({ "criterion_id": "c1", "outcome": "found", "cases": [{ "case_id": "a1", "short": "a name with a space is greeted in full", "given": "the name Ada Lovelace", "expect": "hello Ada Lovelace", "why": "greet reads its whole argument" }], "why": "", "model": "sonnet", "tokens": 1500 }));
        let added = obj(json!({ "criterion_id": "c1", "hold": "cases_unconfirmed", "choice": "add_all", "typed": "" }));
        let facts = record.append_all(&[("cases_found", found, at), ("hold_answered", added, at)], familiar_ai::TENANT).expect("the finding and the person's add_all");
        record.append_raw("told", obj(json!({ "about": facts[1].t.to_string() })), at, familiar_ai::TENANT).expect("the pick told");
    }
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"},{"case_id":"a1","kind":"edge","short":"a name with a space is greeted in full","given":"the name Ada Lovelace","expect":"hello Ada Lovelace"}]"#]);
}

/// The found path (the owner, 2026-10-25: "Mirror them") [draft names]: the finder found a case,
/// the person added it, and the promise holds it. A fresh worktree's record carries the finding
/// and the answer as recorded, from the committed plan, so nothing is looked for or asked again
/// and the part's test comes first.
#[test]
fn a_found_case_the_person_added_is_neither_looked_for_nor_asked_again_in_a_fresh_worktree() {
    let p = sealed_with("found", added_a_found_case);
    let wt = p.worktree();
    p.setup_at(&wt);
    assert_eq!(p.criteria_at(&wt), part("testing"));
    let (code, out, err) = p.run_in(&wt, &["status", "--json"], "");
    assert_eq!(code, 0, "{err}");
    let j: Value = serde_json::from_str(&out).expect("status --json");
    let holds = j["criteria"][0]["holds"].to_string();
    assert!(!holds.contains("cases_"), "{holds}");
    let seeded = common::project::facts_at(&wt);
    let of = |kind: &str| seeded.iter().filter(|f| f.kind == kind).map(|f| Value::Object(f.fields.clone())).collect::<Vec<_>>();
    assert_eq!(of("cases_found"), p.facts_of("cases_found"), "the finding, as recorded");
    assert_eq!(of("hold_answered"), p.facts_of("hold_answered"), "the person's answer, as recorded");
}

/// The plan reads the cases card as the rules name it [draft names]: the finding is a plan kind,
/// every choice the rules give the cases card and words typed are plan lines, and no answer on
/// any other card is. The plan crate cannot read the rules, so this pins its names to theirs.
#[test]
fn the_plan_carries_the_cases_cards_answers_as_the_rules_name_them_and_no_other_cards() {
    use familiar_ai_plan::{Claim, PLAN_KINDS};
    use familiar_ai_rules::{cases, criterion::ANSWERS};
    assert!(PLAN_KINDS.contains(&cases::FOUND), "{PLAN_KINDS:?}");
    let answer = |hold: &str, choice: &str| json!({ "criterion_id": "c1", "hold": hold, "choice": choice, "typed": "" }).as_object().cloned().unwrap_or_default();
    let mut cards = 0;
    for (hold, choices) in ANSWERS {
        for choice in choices.iter().chain([cases::TYPED].iter()) {
            assert_eq!(Claim::new("hold_answered", answer(hold, choice)).is_ok(), hold == cases::CARD, "{hold} {choice}");
        }
        cards += usize::from(hold == cases::CARD);
    }
    assert_eq!(cards, 1, "the rules name the cases card once");
    for hold in [familiar_ai_rules::ask::HOLD, familiar_ai_rules::trace::REPRO] {
        assert!(Claim::new("hold_answered", answer(hold, cases::TYPED)).is_err(), "{hold}");
    }
}

#[test]
fn a_fresh_worktree_of_an_empty_committed_plan_starts_empty_as_before() {
    let p = Project::builder("empty").session(SESSION).create();
    if !p.path(".familiar/plan").exists() {
        p.write(".familiar/plan", "");
    }
    p.land("the empty plan");
    assert_eq!(plan_at(&p.repo()), "");

    let wt = p.worktree();
    let out = p.setup_at(&wt);
    assert!(out.lines().any(|l| l == "familiar-ai: .familiar/record starts empty"), "{out}");
    assert!(!out.contains("starts from the committed plan"), "{out}");
    assert_eq!(p.criteria_at(&wt), Vec::new());
}

// A record rebuilt from the committed plan carries the cases card's mirrored answer (the owner's ruling of
// 2026-10-25, "Mirror them") [draft names]; the answer was told where it was given, so the rebuilt record
// does not tell it again on its first hook (as verify's runs in a seeded record are not told either).
#[test]
fn a_mirrored_cases_card_answer_is_not_told_again_after_the_record_is_rebuilt() {
    let p = sealed("seed-no-retell");
    std::fs::remove_file(p.record_path()).expect("the record removed");
    p.setup();
    let out = p.tool("PreToolUse", "Edit", serde_json::json!({ "file_path": p.path("src/app.sh").to_string_lossy(), "old_string": "hi", "new_string": "hello", "replace_all": false }));
    assert!(!out.to_string().contains("the person answered"), "{out}");
}
