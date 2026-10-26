//! c_dep (plan §7.3, §7.4; D13–D18; lane S9c): the person's `a_dependency` is drafted by the
//! binary in the picker's own PostToolUse. The pick is recorded, the record let go while the
//! holder's model is asked once, then the draft and the dependency's lines land as one batch when
//! the whole plan holds with them, and the holder is handed the dependency. A plan that does not
//! hold writes no line and opens the environment_error card. The backstop heals a draft whose
//! lines are missing and drafts an approval again; two misses reopen the card. While a draft is
//! being written, another conversation's edit waits.

mod common;
mod run_cards;

use common::Project;
use familiar_ai_rules::{lifecycle, plan, View};
use run_cards::*;
use serde_json::{json, Value};

const DEP_TEXT: &str = "the greeting word is read from one place";
const PICK: &str = "the crash is in something part 1 depends on; plan that first";

/// The one-shot's answer, as Claude Code prints it with `--output-format json`.
fn answer(place: Value) -> String {
    json!({ "type": "result", "subtype": "success", "is_error": false, "structured_output": { "text": DEP_TEXT, "test_file": "tests/test_c2.sh", "place": place } }).to_string()
}

/// A fake `claude`: the dependency's one-shot checks the record is free (a `status` from the
/// project answers) and answers `dep`; any other prompt is the judge's, which says yes.
fn fake(dir: &std::path::Path, dep: &str) -> String {
    let repo = dir.join("repo");
    let mark = dir.join("record-during-one-shot");
    let judge = r#"{"type":"result","subtype":"success","is_error":false,"structured_output":{"answers_intent":true,"gap":"","reason":"it exercises the change"}}"#;
    format!(
        "#!/bin/sh\nin=$(cat)\ncase \"$in\" in\n*'must be built first'*)\n  if (cd '{}' && FAMILIAR_AI_JUDGE=off '{}' status >/dev/null 2>&1); then echo free >> '{}'; else echo held >> '{}'; fi\n  if [ -f '{}' ]; then (cd '{}' && FAMILIAR_AI_JUDGE=off FAMILIAR_AI_BIN='{}' sh '{}'); fi\n  printf '%s' '{}'\n  ;;\n*) printf '%s' '{}' ;;\nesac\n",
        repo.display(),
        env!("CARGO_BIN_EXE_familiar-ai"),
        mark.display(),
        mark.display(),
        dir.join("during.sh").display(),
        repo.display(),
        env!("CARGO_BIN_EXE_familiar-ai"),
        dir.join("during.sh").display(),
        dep,
        judge
    )
}

/// c1 planned on greet, held by SESSION, with a fake `claude` answering `dep` (or none); a second
/// file, src/word.sh, holds the word the dependency's place may name.
fn planned_with(name: &str, dep: Option<&str>) -> Project {
    let b = Project::builder(name).session(SESSION).env("FAMILIAR_AI_INVESTIGATOR", "off").file("src/word.sh", "#!/bin/sh\nword() { echo hi; }\n");
    let b = match dep {
        Some(dep) => {
            let script = fake(b.dir(), dep);
            b.env_remove("FAMILIAR_AI_JUDGE").bin("claude", &script)
        }
        None => b.env("FAMILIAR_AI_JUDGE", "off"),
    };
    let p = b.create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", TEST_FILE, "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION],
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p
}

/// c1's test committed alone and its run read as a crash at greet.
fn crashed_with(name: &str, dep: Option<&str>, located: bool) -> Project {
    let p = planned_with(name, dep);
    p.write(TEST_FILE, RED);
    commit(&p, "c1: tests first");
    // not located: the crash names no place (the run's own reading points at the test's check, H-2)
    let at = if located { json!({ "location": { "file_path": "src/app.sh", "line": 2, "function": "greet() {" } }) } else { json!({ "location": null }) };
    relabel(&p, "crashed", at);
    p
}

fn kinds_of(p: &Project, c: &str) -> Vec<String> {
    p.facts().iter().filter(|f| f.criterion_id() == Some(c)).map(|f| f.kind.to_string()).collect()
}

/// A fact appended as if long ago, so no call is still drafting it.
fn old(p: &Project, kind: &str, fields: Value) -> u64 {
    let Value::Object(m) = fields else { unreachable!() };
    record(p).append_raw(kind, m, 0.0, familiar_ai::TENANT).expect("appended").t
}

fn any_hook(p: &Project) -> Value {
    p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "ls" }, "tool_response": {} }))
}

#[test]
fn the_approval_is_drafted_with_the_record_free_and_lands_whole_and_the_holder_is_handed_it() {
    let place = json!({ "file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3 });
    let p = crashed_with("dep-drafted", Some(&answer(place)), true);
    assert_eq!(cards(&p), ["crashed"]);
    let out = pick(&p, &ask(&p), PICK);
    assert_eq!(std::fs::read_to_string(p.dir.join("record-during-one-shot")).unwrap_or_default(), "free\n", "the record is let go during the one-shot");
    let drafted = p.facts_of("dependency_drafted");
    assert_eq!(drafted.len(), 1, "{drafted:?}");
    assert_eq!(drafted[0]["criterion_id"], "c2");
    assert_eq!(drafted[0]["text"], DEP_TEXT);
    assert_eq!(drafted[0]["test_file"], "tests/test_c2.sh");
    let kinds = kinds_of(&p, "c2");
    for k in ["criterion", "place_chosen", "anchor_sha_matched", "test_planned", "criterion_head", "criterion_assigned", "fix_location_category"] {
        assert!(kinds.iter().any(|x| x == k), "{k} in {kinds:?}");
    }
    let log = p.facts();
    let v = View::new(&log);
    assert!(plan::planning_confirm_complete(&v), "the plan holds");
    assert_eq!(plan::string_before(&v, "c1").as_deref(), Some("c2"), "inserted before c1");
    assert_eq!(plan::assigned_to(&v, "c2"), Some(SESSION), "c1's holder");
    assert_eq!(lifecycle::state(&v, "c1"), "waiting");
    assert_eq!(lifecycle::state(&v, "c2"), "testing");
    let line = format!("\"{DEP_TEXT}\" was added for \"greet says hello\": {DEP_TEXT}. Its plan is complete. Do it yourself, or send it to a subagent.");
    assert_eq!(context(&out).matches(&line).count(), 1, "{out}");
    // the plan file mirrors the batch
    assert!(p.read(".familiar/plan").contains("c2"), "{}", p.read(".familiar/plan"));
}

#[test]
fn a_plan_that_does_not_hold_writes_no_line_and_opens_the_environment_error_card() {
    let place = json!({ "file_path": ".venv/lib/greeting.py", "symbol": "word", "line_start": 1, "line_end": 4 });
    let p = crashed_with("dep-venv", Some(&answer(place)), false);
    pick(&p, &ask(&p), PICK);
    assert_eq!(p.facts_of("dependency_drafted").len(), 1);
    assert!(kinds_of(&p, "c2").iter().all(|k| k == "dependency_drafted"), "no line: {:?}", kinds_of(&p, "c2"));
    let missed = p.facts_of("dependency_unanswered");
    assert_eq!(missed.len(), 1, "{missed:?}");
    assert!(missed[0]["why"].as_str().unwrap_or("").starts_with("the plan with the dependency is incomplete"), "{missed:?}");
    assert_eq!(cards(&p), ["environment_error"]);
}

#[test]
fn a_crash_between_the_draft_and_its_lines_is_healed_by_the_backstop() {
    let p = crashed_with("dep-heal", None, true);
    let t = old(&p, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    old(&p, "dependency_drafted", json!({ "approval": t, "criterion_id": "c2", "text": DEP_TEXT, "test_file": "tests/test_c2.sh" }));
    any_hook(&p);
    let log = p.facts();
    let v = View::new(&log);
    assert!(plan::criteria(&v).contains(&"c2".to_string()), "the lines are written again");
    assert!(plan::planning_confirm_complete(&v));
    assert_eq!(lifecycle::state(&v, "c1"), "waiting");
    assert_eq!(p.facts_of("dependency_drafted").len(), 1, "drafted once");
}

#[test]
fn without_claude_each_try_is_a_miss_and_two_misses_reopen_the_card() {
    let p = crashed_with("dep-miss", None, true);
    old(&p, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    assert!(cards(&p).is_empty(), "answered");
    any_hook(&p);
    assert_eq!(p.facts_of("dependency_unanswered").len(), 1);
    assert!(cards(&p).is_empty(), "one miss: drafting again");
    any_hook(&p);
    assert_eq!(p.facts_of("dependency_unanswered").len(), 2);
    assert_eq!(cards(&p), ["crashed"], "two misses end the approval");
    any_hook(&p);
    assert_eq!(p.facts_of("dependency_unanswered").len(), 2, "an ended approval is not drafted again");
}

#[test]
fn a_dependency_on_two_failures_and_on_command_broke_drafts() {
    for hold in ["two_failures", "command_broke"] {
        let place = json!({ "file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3 });
        let p = planned_with(&format!("dep-{hold}"), Some(&answer(place)));
        p.write(TEST_FILE, RED);
        commit(&p, "c1: tests first");
        old(&p, "hold_answered", json!({ "criterion_id": "c1", "hold": hold, "choice": "a_dependency", "typed": "" }));
        any_hook(&p);
        let log = p.facts();
        let v = View::new(&log);
        assert_eq!(p.facts_of("dependency_drafted").len(), 1, "{hold}");
        assert_eq!(plan::string_before(&v, "c1").as_deref(), Some("c2"), "{hold}");
        assert!(plan::planning_confirm_complete(&v), "{hold}");
    }
}

/// D16: while another call drafts the dependency, an edit and a stop wait for it.
#[test]
fn while_a_dependency_is_drafted_another_conversations_edit_waits() {
    let p = crashed_with("dep-wait", None, true);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let Value::Object(m) = json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" }) else { unreachable!() };
    record(&p).append_raw("hold_answered", m, now, familiar_ai::TENANT).expect("appended");
    let out = p.tool("PreToolUse", "Write", json!({ "file_path": p.path("src/app.sh").to_string_lossy(), "content": "x\n" }));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let why = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(why.contains("wait") && why.contains("being drafted"), "{why}");
    assert!(p.facts_of("dependency_unanswered").is_empty(), "the drafting call is left to finish");
}

/// Assumption 8: an empty commit B is seen as a commit with no touches.
#[test]
fn an_empty_commit_is_seen_with_no_touches() {
    let p = planned_with("dep-empty-b", None);
    p.git(&["commit", "-q", "--allow-empty", "-m", "B"]);
    any_hook(&p);
    let seen = p.facts_of("commit_seen");
    let b = seen.last().expect("seen");
    assert_eq!((b["touches_tests"].clone(), b["touches_code"].clone()), (json!(false), json!(false)), "{b}");
    assert_eq!(b["anchor_overlap"], json!([]), "{b}");
}

fn word_place() -> Value {
    json!({ "file_path": "src/word.sh", "symbol": "word", "line_start": 2, "line_end": 2 })
}

fn write_edit(p: &Project, file: &str) -> Value {
    p.tool("PreToolUse", "Write", json!({ "file_path": p.path(file).to_string_lossy(), "content": "x\n" }))
}

/// Review 1 (S9c): the same holder holds c1 (waiting) and its dependency c2. An edit of c1's code
/// is refused as c1's commit is; an edit of c2's place is allowed.
#[test]
fn the_holder_of_both_is_refused_the_waiting_parts_code_and_may_edit_the_dependencys() {
    let p = crashed_with("dep-review1", Some(&answer(word_place())), false);
    pick(&p, &ask(&p), PICK);
    let log = p.facts();
    let v = View::new(&log);
    assert_eq!((plan::assigned_to(&v, "c1"), plan::assigned_to(&v, "c2")), (Some(SESSION), Some(SESSION)));
    assert_eq!(lifecycle::state(&v, "c1"), "waiting");
    let out = write_edit(&p, "src/app.sh");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    let why = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(why.contains("\"greet says hello\""), "{why}");
    let ok = write_edit(&p, "src/word.sh");
    assert_ne!(ok["hookSpecificOutput"]["permissionDecision"], "deny", "{ok}");
}

/// Review 2 (S9c): a part claimed by another conversation during the one-shot keeps its id; the
/// batch takes the next free one.
#[test]
fn a_part_claimed_during_the_one_shot_keeps_its_id_and_the_dependency_takes_the_next() {
    let p = crashed_with("dep-review2", Some(&answer(word_place())), false);
    let claim = "\"$FAMILIAR_AI_BIN\" claim criterion --criterion_id c2 --text 'greet waves' --path full --behavior_id b1 --promise_id p1 >/dev/null 2>&1\n";
    std::fs::write(p.dir.join("during.sh"), claim).expect("during.sh");
    pick(&p, &ask(&p), PICK);
    let crits: Vec<Value> = p.facts_of("criterion").into_iter().filter(|c| c["criterion_id"] == "c2").collect();
    assert_eq!(crits.len(), 1, "c2 is the other conversation's: {crits:?}");
    assert_eq!(crits[0]["text"], "greet waves");
    let drafted = p.facts_of("dependency_drafted");
    assert_eq!(drafted.len(), 1, "{drafted:?}");
    assert_eq!(drafted[0]["criterion_id"], "c3");
}

/// Review 3 (S9c): the one-shot's test file is a relative path inside the project, under no
/// .venv or node_modules, and no other part's planned test; otherwise the answer is a miss.
#[test]
fn a_test_file_outside_the_project_or_another_parts_is_a_miss() {
    for (i, f) in ["../outside/test_dep.sh", "/tmp/test_dep.sh", ".venv/tests/test_dep.sh", "web/node_modules/x/test_dep.sh", "tests/test_c1.sh"].iter().enumerate() {
        let a = json!({ "type": "result", "subtype": "success", "is_error": false, "structured_output": { "text": DEP_TEXT, "test_file": f, "place": word_place() } }).to_string();
        let p = crashed_with(&format!("dep-review3-{i}"), Some(&a), false);
        pick(&p, &ask(&p), PICK);
        assert!(p.facts_of("dependency_drafted").is_empty(), "{f}: no draft");
        let missed = p.facts_of("dependency_unanswered");
        assert_eq!(missed.len(), 1, "{f}: {missed:?}");
        assert!(missed[0]["why"].as_str().unwrap_or("").contains(f), "{f}: {missed:?}");
    }
}

/// Review 4 (S9c): after a plan that does not hold, only try_again on the environment_error card
/// writes the lines again; re_plan does not.
#[test]
fn only_try_again_on_the_refused_plan_runs_the_batch_again() {
    let venv = json!({ "file_path": ".venv/lib/greeting.py", "symbol": "word", "line_start": 1, "line_end": 4 });
    for (choice, misses) in [("re_plan", 1), ("try_again", 2)] {
        let p = crashed_with(&format!("dep-review4-{choice}"), Some(&answer(venv.clone())), false);
        pick(&p, &ask(&p), PICK);
        assert_eq!(cards(&p), ["environment_error"]);
        old(&p, "hold_answered", json!({ "criterion_id": "c1", "hold": "environment_error", "choice": choice, "typed": "" }));
        any_hook(&p);
        assert_eq!(p.facts_of("dependency_unanswered").len(), misses, "{choice}");
    }
}
