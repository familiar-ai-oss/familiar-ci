//! The person's keep_going on `anchor_untouched` appends the place where the
//! change landed to the plan, with the answer's stamp, and keeps the proof.
//! The owner (2026-10-13): "at keep_going where there is actually a change
//! outside the original plan we have to append an anchor to the plan and use
//! the existing infrastructure. so we update the plan, with a stamp that the
//! user basically overrode it."
//!
//! Right after the pick the binary writes `place_chosen` (the files the code
//! landed in since the seal; the symbol from git's hunk header, the enclosing
//! line git prints after `@@`, with the lines from it to the hunk's end; a
//! file-level place when git gives none) and its planning check at the seal's
//! commit. The plan mirror carries both, and verify in a fresh clone proves
//! the part at the new place, its teeth reverting the change there.
//! See docs/decisions.md#holds-are-the-persons

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5b0f7a3e-2d61-4c1e-9a8f-6e2b1c0d4f17";
const KEEP: &str = "keep the change for part 1 where it landed";
const HAPPENED: &str = "the tests for part 1 pass, but none of the changes touched the code the agent had picked for it.";

impl Project {
    /// c1 planned (full path, b1/p1) on src/app.sh greet 2–2, sealed red, then
    /// fixed beside its anchor (line 3 of src/app.sh, and src/other.sh), its
    /// tests passing: held on anchor_untouched. Returns the tests-first sha.
    fn untouched_here(name: &str) -> (Project, String) {
        let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").env("PATH", "/usr/bin:/bin").create();
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
        p.write("src/other.sh", "echo other\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "a second file"]);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
        p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "c1: tests first"]);
        let tests_first = p.head();
        p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git add -A && git commit -m 'c1: tests first'" }, "tool_response": {} }));
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet; echo hello\n");
        p.write("src/other.sh", "echo other, too\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "c1: the fix, beside the anchor"]);
        p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am 'c1: the fix'" }, "tool_response": {} }));
        assert!(p.status().contains("a question for you"), "held on anchor_untouched: {}", p.status());
        (p, tests_first)
    }

    /// The person's pick of keep_going in Claude Code's picker.
    fn pick_keep_going(&self) {
        let prose = format!(
            "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: {HAPPENED} In the agent's words: \"the hello went on the line after greet, not inside it\"\n\nwhat now?"
        );
        let ask = json!({ "tool_input": { "questions": [{ "question": prose, "header": "x", "options": [
            { "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }
        ], "multiSelect": false }] } });
        let pre = self.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", ask);
        let card = pre["hookSpecificOutput"]["updatedInput"].clone();
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let response = json!({ "questions": card["questions"], "answers": { q: KEEP }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        input["annotations"] = json!({});
        self.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }));
    }
}

/// The two places where the change landed: the hunk under greet's line (git's
/// hunk header), and src/other.sh's first line, which no line above names.
fn landed() -> Value {
    json!([
        { "file_path": "src/app.sh", "symbol": "greet() { echo hi; }", "line_start": 2, "line_end": 3 },
        { "file_path": "src/other.sh", "symbol": "", "line_start": 1, "line_end": 1 }
    ])
}

/// What the pick appended: the answer, the stamped place right after it, its planning checks.
fn assert_appended(p: &Project, tests_first: &str) {
    let facts = p.facts();
    // the answer on this card, not the cases card's before the test (plan cases-card, 2026-10-25) [draft names]
    let i = facts.iter().position(|f| f.kind == "hold_answered" && f.fields.get("hold").and_then(Value::as_str) != Some(common::project::CASES_CARD)).expect("the answer is recorded");
    assert_eq!(Value::Object(facts[i].fields.clone()), json!({ "criterion_id": "c1", "hold": "anchor_untouched", "choice": "keep_going", "typed": "" }));
    let next = &facts[i + 1];
    assert_eq!(next.kind, "place_chosen", "right after the answer: {:?}", next);
    assert_eq!(Value::Object(next.fields.clone()), json!({ "criterion_id": "c1", "places": landed(), "hold": "anchor_untouched", "choice": "keep_going" }));
    let checks: Vec<Value> = facts[i + 1..]
        .iter()
        .filter(|f| f.kind == "anchor_sha_matched" && f.fields.get("checked_at") == Some(&json!("planning")))
        .map(|f| json!([f.fields["file_path"], f.fields["symbol"], f.fields["line_start"], f.fields["line_end"], f.fields["sha"], f.fields["matched"]]))
        .collect();
    assert_eq!(
        checks,
        [json!(["src/app.sh", "greet() { echo hi; }", 2, 3, tests_first, true]), json!(["src/other.sh", "", 1, 1, tests_first, true])],
        "each place's planning check, at the seal's commit"
    );
    let plan = p.read(".familiar/plan");
    let lines: Vec<Value> = plan.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let kept = lines.iter().position(|l| l["kind"] == "place_chosen" && l["hold"] == "anchor_untouched").expect("the plan carries the kept place");
    assert_eq!(lines[kept]["places"], landed());
    assert_eq!(lines[kept]["choice"], "keep_going");
    assert!(lines[kept + 1..].iter().any(|l| l["kind"] == "anchor_sha_matched" && l["checked_at"] == "planning" && l["file_path"] == "src/app.sh"), "and its planning check: {plan}");
}

#[test]
fn the_persons_pick_appends_the_place_where_the_change_landed_with_its_stamp() {
    let (p, tests_first) = Project::untouched_here("kept-picked");
    p.pick_keep_going();
    assert_appended(&p, &tests_first);
    let status = p.status();
    assert!(!status.contains("a question for you"), "the card is closed: {status}");
    assert!(status.contains("familiar-ai: \"greet says hello\" · locally green"), "the proof is kept: {status}");
}

#[test]
fn the_persons_terminal_answer_appends_it_too_and_verify_proves_the_part_at_the_new_place() {
    let (p, tests_first) = Project::untouched_here("kept-terminal");
    let (code, out, err) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "anchor_untouched", "--choice", "keep_going", "--typed", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_appended(&p, &tests_first);
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "the plan"]);
    let clone = p.dir.join("clone-kept");
    p.git_in(&p.dir, &["clone", "-q", "--branch", "main", p.repo().to_str().expect("path"), clone.to_str().expect("path")]);
    let (code, out, err) = p.run_in(&clone, &["verify"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.starts_with("familiar-ai: \"greet says hello\" · proven · red at "), "{out}");
    assert!(out.contains("fails with the change taken back out (exit 1)"), "the teeth revert the change at the new place: {out}");
}

#[test]
fn an_agents_place_chosen_carrying_the_persons_answer_is_refused() {
    let (p, _) = Project::untouched_here("kept-agent");
    let (code, out, err) = p.try_claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":3}]"#, "--hold", "anchor_untouched", "--choice", "keep_going"]);
    assert_ne!(code, 0, "only the binary writes the person's answer on a place: {out}{err}");
    assert!(p.facts_of("place_chosen").iter().all(|f| f.get("hold").is_none()), "nothing written");
}
