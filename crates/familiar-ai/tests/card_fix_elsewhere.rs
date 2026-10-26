//! A `fix_location_category` claim other than `this_repository` becomes the
//! person's `fix_elsewhere` card with no test or code written; the standing
//! claim is the latest one made under the current holder, or its first act.

mod common;

use common::Project;
use familiar_ai::door::investigator_run;
use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::criterion::{sealed_at, Place};
use familiar_ai_rules::{lifecycle, moves, View};
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const ELSEWHERE: [&str; 6] = ["--category", "dependency", "--where", "the greeting library", "--needs", "a release that says hello"];
const HERE: [&str; 4] = ["--category", "this_repository", "--where", ""];

/// The fake `claude`: the judge answers "covers it"; the investigator, through the
/// real binary, commits before claiming, claims the category in `play`, then
/// commits the fix and stops. D5 (the owner's announced definition change,
/// 2026-10-16): an investigator is sent for two_failures only, so it fixes.
const FAKE: &str = r#"#!/bin/sh
D="$(cd "$(dirname "$0")/.." && pwd)"
case "$*" in
*--plugin-dir*) ;;
*)
    cat > /dev/null
    printf '%s\n' '{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}'
    exit 0 ;;
esac
printf '%s\n' "$*" >> "$D/calls"
cat > "$D/prompt"
U=""; M=""; prev=""
for a in "$@"; do
    [ "$prev" = "--session-id" ] && U="$a"
    [ "$prev" = "--model" ] && M="$a"
    prev="$a"
done
P="$(pwd)"
hook() { printf '%s' "$2" | "$FA_BIN" hook "$1" > "$D/hook.$3" 2>&1; }
g() { git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@" >/dev/null 2>&1; }
hook SessionStart "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"source\":\"startup\",\"model\":\"claude-$M-5-5\"}" start
pre() { hook PreToolUse "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git commit -m c1\"}}" "$1"; }
pre commit-before-claim
if [ "$(cat "$FA_PLAY")" = here ]; then
    "$FA_BIN" claim fix_location_category --criterion_id c1 --category this_repository --where '' > "$D/claim" 2>&1
    pre commit-after-claim
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > src/app.sh
    g add -A
    g commit -q -m "c1: the fix"
    hook PostToolUse "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git commit -m c1\"},\"tool_response\":{}}" commit
else
    "$FA_BIN" claim fix_location_category --criterion_id c1 --category dependency --where 'the greeting library' --needs 'a release that says hello' > "$D/claim" 2>&1
fi
hook Stop "{\"hook_event_name\":\"Stop\",\"session_id\":\"$U\",\"cwd\":\"$P\"}" stop
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","total_cost_usd":0.05,"modelUsage":{"claude-%s-5-5":{}},"result":"done"}\n' "$U" "$M"
"#;

/// c1 planned on greet and held by SESSION, with `claim` its fix location;
/// the binary runs from no session, the fake `claude` first on its PATH.
fn project(name: &str, claim: &[&str]) -> Project {
    let b = Project::builder(name).session(SESSION);
    std::fs::create_dir_all(b.dir().join("plugin")).expect("plugin folder");
    let (user, plugin, play) = (b.dir().join("user"), b.dir().join("plugin"), b.dir().join("play"));
    let p = b
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("FAMILIAR_AI_INVESTIGATOR_BUDGET")
        .env_remove("FAMILIAR_AI_INVESTIGATOR")
        .env_remove("FAMILIAR_AI_JUDGE")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env("HOME", user)
        .env("CLAUDE_PLUGIN_ROOT", plugin)
        .env("FA_BIN", env!("CARGO_BIN_EXE_familiar-ai"))
        .env("FA_PLAY", play)
        .env("CLAUDECODE", "1")
        .bin("claude", FAKE)
        .create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#],
        vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
        vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
        vec!["criterion_head", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    let mut argv = vec!["fix_location_category", "--criterion_id", "c1"];
    argv.extend(claim.iter().copied());
    let out = p.claim(&argv);
    std::fs::write(p.dir.join("planning-claim"), out).expect("keep");
    p.hook("PostModelSwitch", json!({ "from_model": "claude-sonnet-5-5", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
    /// The agent's commit, its hook in bypass-permissions mode; what it told.
    fn agent_commit(&self, message: &str) -> Value {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {}, "permission_mode": "bypassPermissions" }))
    }

    /// A file the fake `claude` left in the scratch folder.
    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }
}

fn field<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn kinds(log: &Log, kind: &str) -> usize {
    log.iter().filter(|f| f.kind == kind).count()
}

fn denied(out: &Value) -> bool {
    out["hookSpecificOutput"]["permissionDecision"] == "deny"
}

fn commit_asked(p: &Project) -> Value {
    p.hook("PreToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m c1" } }))
}

fn answer(p: &Project, choice: &str) -> String {
    let (code, out, err) = p.run(&["claim", "hold_answered", "--criterion_id", "c1", "--hold", "fix_elsewhere", "--choice", choice, "--typed", ""], "");
    assert_eq!(code, 0, "{choice}: {out}{err}");
    out
}

/// A claim elsewhere pauses c1 on the fix_elsewhere card; no test, code or launch follows.
#[test]
fn a_dependency_claim_is_the_persons_card_with_where_and_needs_and_no_test_and_no_code() {
    let p = project("card", &ELSEWHERE);
    let out = p.file("planning-claim");
    assert!(out.contains("familiar-ai: the plan is confirmed"), "line 11 with any category confirms the plan: {out}");
    assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
    assert!(out.contains("\n  where: the greeting library\n  needs: a release that says hello\n"), "{out}");
    // what happened and the choices, in the person's terms
    assert!(out.contains("\n  what happened: the fix for part 1 is not in this repository: it lives in the greeting library. It needs a release that says hello.\n"), "{out}");
    for choice in ["    1) drop it\n", "    2) have the agent look again at where the fix lives\n", "    3) you make the fix where it lives; the agent carries on here\n"] {
        assert!(out.contains(choice), "{choice}: {out}");
    }

    let log = p.facts();
    let v = View::new(&log);
    assert!(lifecycle::whys(&v, "c1").iter().any(|w| w == "fix_elsewhere"));
    assert!(lifecycle::paused(&v, "c1"));
    assert!(!lifecycle::first_act(&v, "c1"), "the holder's own claim stands");
    assert!(!moves::criterion_admissible(&v).iter().any(|m| m.kind == "commit_seen"), "no commit is offered");

    // the agent's stop is the person's card: it asks and writes nothing
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let reason = stop["reason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\nfamiliar-ai: \"greet says hello\" · held on fix elsewhere · next: the person answers its card: release it, plan it again or take it over yourself\n"), "{reason}");
    assert!(!reason.contains("familiar-ai claim hold_answered"), "the card is the person's; no claim of theirs: {reason}");
    assert!(reason.contains("Ask the person with AskUserQuestion: the hook turns that call into the question card"), "{reason}");
    let log = p.facts();
    assert_eq!(kinds(&log, "commit_seen"), 0);
    assert_eq!(investigator_run::due(&log, SESSION, None), None, "never another rung's try");
    assert!(p.file("calls").is_empty());
}

#[test]
fn release_lets_the_criterion_go() {
    let p = project("release", &ELSEWHERE);
    answer(&p, "release");
    let log = p.facts();
    let v = View::new(&log);
    assert!(lifecycle::released(&v, "c1"));
    assert!(!lifecycle::whys(&v, "c1").iter().any(|w| w == "fix_elsewhere"), "answered");
    assert!(!lifecycle::first_act(&v, "c1"), "released: no claim is owed");
}

#[test]
fn re_plan_offers_the_claim_again_and_a_claim_here_puts_the_test_first() {
    let p = project("re-plan", &ELSEWHERE);
    answer(&p, "re_plan");
    let log = p.facts();
    let v = View::new(&log);
    assert!(!lifecycle::whys(&v, "c1").iter().any(|w| w == "fix_elsewhere"), "answered");
    assert!(lifecycle::replanned(&v, "c1"));
    assert!(moves::criterion_admissible(&v).iter().any(|m| m.kind == "fix_location_category"), "the claim is offered again");
    let (code, out, err) = p.run(&["claim", "fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("familiar-ai: \"greet says hello\" · the test comes first"), "{out}");
    let log = p.facts();
    assert!(!lifecycle::replanned(&View::new(&log), "c1"), "claimed since");
    let c = commit_asked(&p);
    assert!(!denied(&c), "the test may be committed: {c}");
}

#[test]
fn take_over_clears_the_hold_and_commits_go_ahead() {
    let p = project("take-over", &ELSEWHERE);
    answer(&p, "take_over");
    let log = p.facts();
    let v = View::new(&log);
    assert!(!lifecycle::whys(&v, "c1").iter().any(|w| w == "fix_elsewhere"), "answered");
    assert!(!lifecycle::paused(&v, "c1"));
    let c = commit_asked(&p);
    assert!(!denied(&c), "{c}");
}

/// After two failed tries since the seal (two_failures; D5: the ladder is for a
/// valid red only), the investigator is launched from the PostToolUse of the
/// commit that made the second.
fn escalated(p: &Project, play: &str) -> Value {
    std::fs::write(p.dir.join("play"), play).expect("play");
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ \"$out\" = hello ] || { echo \"expected hello, got: $out\"; exit 1; }\n");
    p.agent_commit("c1: tests first");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    p.agent_commit("c1: a first fix");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hullo; }\ngreet\n");
    let second = p.agent_commit("c1: a second fix");
    assert_eq!(p.file("calls").lines().count(), 1, "{second}");
    let prompt = p.file("prompt");
    assert!(prompt.contains("WHERE THE FIX LIVES (the cheaper model's claim): this_repository. YOUR FIRST ACT, before any commit, is your own claim: familiar-ai claim fix_location_category --criterion_id c1"), "{prompt}");
    let refused = p.file("hook.commit-before-claim");
    assert!(refused.contains("\"permissionDecision\":\"deny\""), "the investigator's commit before its claim: {refused}");
    assert!(refused.contains("claim it: familiar-ai claim fix_location_category --criterion_id c1"), "the claim named: {refused}");
    second
}

#[test]
fn the_investigators_first_act_is_its_claim_and_elsewhere_is_the_card_with_no_tries() {
    let p = project("escalated-elsewhere", &HERE);
    escalated(&p, "elsewhere");
    let claim = p.file("claim");
    assert!(claim.contains("familiar-ai: a question for you · \"greet says hello\"") && claim.contains("  where: the greeting library"), "{claim}");

    let log = p.facts();
    let v = View::new(&log);
    let claims: Vec<&Fact> = log.iter().filter(|f| f.kind == "fix_location_category").collect();
    assert_eq!(claims.iter().map(|f| field(f, "category")).collect::<Vec<_>>(), ["this_repository", "dependency"]);
    let sent = log.iter().find(|f| f.kind == "criterion_transfer").expect("the transfer");
    assert!(claims[1].t > sent.t, "the investigator's claim, after the transfer");
    assert!(!log.iter().any(|f| f.kind == "commit_seen" && f.t > claims[1].t), "no tries after the claim");
    assert!(lifecycle::whys(&v, "c1").iter().any(|w| w == "fix_elsewhere"), "the card again");
    assert!(sealed_at(&v, "c1", Place::Laptop).is_some(), "the holder's sealed test stands");
    assert_eq!(p.file("calls").lines().count(), 1, "no other rung");
}

#[test]
fn after_the_hand_back_the_earlier_claim_stands_and_the_holder_commits_with_no_new_claim() {
    let p = project("escalated-here", &HERE);
    escalated(&p, "here");
    let allowed = p.file("hook.commit-after-claim");
    assert!(!allowed.contains("\"permissionDecision\":\"deny\""), "claimed: its commit goes ahead: {allowed}");

    let log = p.facts();
    let v = View::new(&log);
    let transfers: Vec<&Fact> = log.iter().filter(|f| f.kind == "criterion_transfer").collect();
    assert_eq!(transfers.len(), 2, "there and back");
    assert_eq!(field(transfers[1], "to_conversation"), SESSION);
    assert!(matches!(lifecycle::state(&v, "c1"), "green" | "proven"), "the investigator's fix: {}", lifecycle::state(&v, "c1"));
    let claims: Vec<&Fact> = log.iter().filter(|f| f.kind == "fix_location_category").collect();
    assert_eq!(claims.len(), 2, "planning's and the investigator's");
    assert!(!lifecycle::first_act(&v, "c1"), "back with the holder: its planning claim stands again");
    assert_eq!(lifecycle::standing_claim(&v, "c1").map(|f| f.t), Some(claims[0].t), "derived: the holder's own, never re-appended");

    let c = commit_asked(&p);
    assert!(!denied(&c), "the holder's commit, no new claim: {c}");
    let log = p.facts();
    assert_eq!(kinds(&log, "fix_location_category"), 2, "nothing appended");
}
