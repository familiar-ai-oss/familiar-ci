//! On the bench (a headless `claude -p`), a stop with only the person's cards left goes, and the
//! card waits in the record; see docs/decisions.md#holds-are-the-persons.
//!
//! "Goes" is what Claude Code does with the answer, not only the absence of `"decision": "block"`:
//! Claude Code (2.1.29x) treats a Stop hook's `hookSpecificOutput.additionalContext` as feedback that
//! continues the turn, and counts it toward its stop-hook block cap. The 44 benchmark at f51f5f6
//! (pydantic-13506, -13535: `fix_elsewhere`; -13149: `two_tries_not_red`) ended each such session by
//! that cap, nine "Stop says: … a question for you" in a row. The card's words go to the person
//! (`systemMessage`) and nothing goes back to the agent.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").create()
}

impl Project {
    /// The hook, on the bench (`FAMILIAR_AI_BENCH=1`) or not.
    fn hook_on(&self, event: &str, rest: Value, bench: bool) -> Value {
        self.set_env("FAMILIAR_AI_BENCH", Some(if bench { "1" } else { "0" }));
        let out = self.hook(event, rest);
        self.set_env("FAMILIAR_AI_BENCH", Some("0"));
        out
    }

    /// c1 on src/app.sh greet 1–3, tested by tests/test_c1.sh, the fix in this repository.
    fn plan(&self) {
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    }

    /// The open card's first line, as `status` shows it.
    fn card_line(&self) -> String {
        let status = self.status();
        status.lines().find(|l| l.starts_with("familiar-ai: a question for you · ")).map(str::to_string).unwrap_or_else(|| panic!("no card open: {status}"))
    }

    /// The same Stop, on the bench and off it, with the card open.
    fn stops_with_the_card(&self, hold: &str) {
        let card = self.card_line();
        let open = |j: &Value| j["cards"].as_array().cloned().unwrap_or_default().iter().map(|c| (c["criterion_id"].clone(), c["hold"].clone())).collect::<Vec<_>>();
        assert_eq!(open(&self.status_json()), [(json!("c1"), json!(hold))], "c1's card is {hold}");

        let off = self.hook_on("Stop", json!({}), false);
        assert_eq!(off["decision"], "block", "off the bench the stop is refused: {off}");
        assert!(off["reason"].as_str().unwrap_or("").contains("next: the person answers its card"), "the refusal names the card: {off}");

        for event in ["Stop", "SubagentStop"] {
            for stop_hook_active in [false, true] {
                let on = self.hook_on(event, json!({ "stop_hook_active": stop_hook_active }), true);
                assert!(on.get("decision").is_none(), "{event} on the bench goes: {on}");
                assert!(on.get("hookSpecificOutput").is_none(), "{event} on the bench sends nothing back to the agent, so Claude Code ends the turn: {on}");
                assert!(on.get("continue").is_none(), "{event}: {on}");
                assert!(on["systemMessage"].as_str().unwrap_or("").contains(&card), "{event}: the person is shown the card: {on}");
            }
        }
        // the card waits in the record
        assert_eq!(self.card_line(), card);
        assert_eq!(open(&self.status_json()), [(json!("c1"), json!(hold))]);
    }
}

/// pydantic-13506 and -13535: the agent named the fix's place outside this repository
/// (the Rust in pydantic-core, a prebuilt wheel); the `fix_elsewhere` card is the person's.
#[test]
fn fix_elsewhere_on_the_bench_the_stop_goes_and_the_card_waits() {
    let p = project("bench-fix-elsewhere");
    p.plan();
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "dependency", "--where", "the greeting library", "--needs", "a release that says hello"]);
    assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
    p.stops_with_the_card("fix_elsewhere");
}

/// pydantic-13506 as it went: the test sealed red, the code committed, and only then the fix
/// named as elsewhere (no toolchain to build it).
#[test]
fn fix_elsewhere_after_the_seal_on_the_bench_the_stop_goes_and_the_card_waits() {
    let p = project("bench-fix-elsewhere-sealed");
    p.plan();
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ \"$out\" = hello ] || { echo \"expected hello, got: $out\"; exit 1; }\n");
    p.commit("c1: a test that fails first");
    let refused = p.hook_on("Stop", json!({}), true);
    assert_eq!(refused["decision"], "block", "sealed, the code is the agent's: refused on the bench too: {refused}");
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "environment", "--where", "no toolchain here to build the fix"]);
    assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
    p.stops_with_the_card("fix_elsewhere");
}

/// pydantic-13149: two tries to go red failed (a passing tests-first run, then a commit with
/// the tests still passing); the `two_tries_not_red` card is the person's.
#[test]
fn two_tries_not_red_on_the_bench_the_stop_goes_and_the_card_waits() {
    let p = project("bench-two-tries-not-red");
    p.plan();
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ -n \"$out\" ] || exit 1\n");
    p.commit("c1: a test that passes");
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ -n \"$out\" ] || { echo empty; exit 1; }\n");
    p.commit("c1: a test that still passes");
    let card = p.card_line();
    assert!(card.contains("two tries to go red failed"), "{card}");
    p.stops_with_the_card("two_tries_not_red");
}
