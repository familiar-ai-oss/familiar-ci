//! The second failed try's PostToolUse launches an investigator that holds
//! the criterion until green; a fake `claude` on PATH stands in for the model.
//! See docs/decisions.md#two-failures-send-an-investigator

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// The fake `claude` records how it was called, then plays the investigator's
/// session through the real binary in the launch's mode: `fix` passes, `fail`
/// makes two failing attempts.
const FAKE: &str = r#"#!/bin/sh
D="$(cd "$(dirname "$0")/.." && pwd)"
n=$(( $(cat "$D/n" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$D/n"
printf '%s\n' "$*" >> "$D/calls"
pwd -P >> "$D/cwds"
printf '%s|%s|%s|%s|%s\n' "${FAMILIAR_AI_CONVERSATION_ID-unset}" "${FAMILIAR_AI_INVESTIGATOR-unset}" "${FAMILIAR_AI_JUDGE-unset}" "${CLAUDE_PROJECT_DIR-unset}" "${CLAUDECODE-unset}" >> "$D/envs"
cat > "$D/prompt.$n"
U=""; M=""; prev=""
for a in "$@"; do
    [ "$prev" = "--session-id" ] && U="$a"
    [ "$prev" = "--model" ] && M="$a"
    prev="$a"
done
P="$(pwd)"
"$FA_BIN" status > "$D/status-at-start.$n" 2>&1
hook() { printf '%s' "$2" | "$FA_BIN" hook "$1" > "$D/hook.$n.$3" 2>&1; }
g() { git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@" >/dev/null 2>&1; }
mode=$(cat "$D/mode.$n" 2>/dev/null || cat "$D/mode")
# headless Claude Code leaves `model` out of SessionStart: the `fail` mode does too
if [ "$mode" = fix ]; then named=",\"model\":\"claude-$M-5-5\""; else named=""; fi
hook SessionStart "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"source\":\"startup\"$named}" start
hook PreToolUse "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$P/tests/test_c1.sh\"}}" edit-sealed
hook PreToolUse "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$P/src/app.sh\"}}" edit-code
hook PreToolUse "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"__HOLDER__\",\"cwd\":\"$P\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$P/src/app.sh\"}}" holder-edit
commit() {
    printf '#!/bin/sh\ngreet() { echo %s; }\ngreet\n' "$1" > src/app.sh
    g commit -q -am "c1: $1"
    hook PostToolUse "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git commit -am c1\"},\"tool_response\":{}}" "commit-$1"
}
if [ "$mode" = fix ]; then commit hello; else commit nope; commit still; fi
hook Stop "{\"hook_event_name\":\"Stop\",\"session_id\":\"$U\",\"cwd\":\"$P\"}" stop
mkdir -p "$HOME/.claude/projects/-p"
printf '{"type":"user","sessionId":"%s"}\n' "$U" > "$HOME/.claude/projects/-p/$U.jsonl"
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","total_cost_usd":0.12,"modelUsage":{"claude-%s-5-5":{}},"result":"done"}\n' "$U" "$M"
"#;

/// The sealed test, which prints what it got when it fails.
const TEST: &str = "out=$(sh src/app.sh)\n[ \"$out\" = hello ] || { echo \"expected hello, got: $out\"; exit 1; }\n";

/// Plans c1 held by SESSION on haiku, seals it, then makes one failing code
/// commit. The fake `claude` is first on PATH; with `off` no investigator runs.
fn one_failed(name: &str, off: bool) -> Project {
    let b = Project::builder(name).session(SESSION).env_remove("CLAUDE_CODE_SESSION_ID");
    std::fs::create_dir_all(b.dir().join("plugin")).expect("plugin folder");
    let (user, plugin) = (b.dir().join("user"), b.dir().join("plugin"));
    let b = b
        .env_remove("FAMILIAR_AI_INVESTIGATOR_BUDGET")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env("HOME", user)
        .env("CLAUDE_PLUGIN_ROOT", plugin)
        .env("FA_BIN", env!("CARGO_BIN_EXE_familiar-ai"))
        .env("CLAUDECODE", "1")
        .bin("claude", &FAKE.replace("__HOLDER__", SESSION));
    let b = if off { b } else { b.env_remove("FAMILIAR_AI_INVESTIGATOR") };
    std::fs::write(b.dir().join("mode"), "fix").expect("mode");
    let p = b.create();
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
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.hook("PostModelSwitch", json!({ "from_model": "claude-sonnet-5-5", "to_model": "claude-haiku-4-5-20251001" }));
    p.write("tests/test_c1.sh", TEST);
    assert!(p.bypass_commit("c1: tests first").to_string().contains(&esc("familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped")));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    p.bypass_commit("c1: first try");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo howdy; }\ngreet\n");
    p
}

impl Project {
    /// Makes the agent's second failing commit and returns its PostToolUse answer.
    fn second_try(&self) -> Value {
        self.bypass_commit("c1: second try")
    }

    /// The agent's commit in bypass mode; what the hook told.
    fn bypass_commit(&self, message: &str) -> Value {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {}, "permission_mode": "bypassPermissions" }))
    }

    /// Sends a PostToolUse of the holder that records nothing new.
    fn looked(&self) -> Value {
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "ls" }, "tool_response": {}, "permission_mode": "bypassPermissions" }))
    }

    /// Sends the holder's Stop in bypass mode.
    fn stop(&self) -> Value {
        self.hook("Stop", json!({ "permission_mode": "bypassPermissions" }))
    }

    fn of_kind(&self, kind: &str) -> Vec<Fact> {
        self.facts().into_iter().filter(|f| f.kind == kind).collect()
    }

    /// A file the fake `claude` wrote in the scratch folder.
    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }

    fn short(&self, subject: &str) -> String {
        let log = self.git(&["log", "--format=%h %s"]);
        log.lines().find_map(|l| l.split_once(' ').filter(|(_, s)| *s == subject).map(|(h, _)| h.to_string())).unwrap_or_else(|| panic!("{subject} in {log}"))
    }

    fn lines(&self, name: &str) -> Vec<String> {
        self.file(name).lines().map(str::to_string).collect()
    }
}

/// Escapes the quotes in `s` as they read inside the hook's JSON text.
fn esc(s: &str) -> String {
    s.replace('"', "\\\"")
}

fn field<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Returns the transfers as (from, to).
fn transfers(p: &Project) -> Vec<(String, String)> {
    p.of_kind("criterion_transfer").iter().map(|f| (field(f, "from_conversation").to_string(), field(f, "to_conversation").to_string())).collect()
}

/// The call to action an allowed stop tells the agent.
const CTA: &str = "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts.";

/// The stop reason of the holder's PostToolUse when the investigator proved it.
const PROVED: &str = "familiar-ai: escalated to the investigator — a stronger model (sonnet) proved \"greet says hello\"";

/// The answer that ends the holder's turn, with the same words as stop reason and context.
fn ends_the_turn(reason: &str) -> Value {
    json!({ "continue": false, "stopReason": reason, "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": reason } })
}

#[test]
fn the_second_failed_try_runs_a_sonnet_investigator_from_its_post_tool_use_and_ends_the_turn() {
    let p = one_failed("green", false);
    assert_eq!(p.second_try(), ends_the_turn(PROVED), "the turn ends here, with what the investigator came to");

    // the transfer and its model_seen, before the launch
    let tr = transfers(&p);
    assert_eq!(tr.len(), 2, "{tr:?}");
    let (from, me) = tr[0].clone();
    assert_eq!(from, SESSION);
    let u = me.strip_prefix(&format!("{SESSION}/")).unwrap_or_else(|| panic!("a fresh subagent of the holder's session: {me}")).to_string();
    assert_eq!(tr[1], (me.clone(), SESSION.to_string()), "the transfer back");
    let seen: Vec<(String, String)> = p.of_kind("model_seen").iter().map(|f| (field(f, "agent_conversation_id").to_string(), field(f, "model").to_string())).collect();
    assert_eq!(seen[1], (me.clone(), "sonnet".to_string()), "the rung, written with the transfer: {seen:?}");
    assert_eq!(seen[2], (me.clone(), "claude-sonnet-5-5".to_string()), "its own SessionStart names the model: {seen:?}");
    let first_seen = p.of_kind("model_seen")[1].t;
    assert!(first_seen < p.of_kind("criterion_transfer")[0].t, "model_seen lands first");
    assert!(
        p.file("status-at-start.1").contains("familiar-ai: \"greet says hello\" · the test fails, as it should before the fix · the code comes next · with a subagent"),
        "{}",
        p.file("status-at-start.1")
    );

    // the exact argv, the project as its folder, its environment
    let plugin = p.dir.join("plugin");
    assert_eq!(
        p.lines("calls"),
        [format!("-p --model sonnet --plugin-dir {} --output-format json --setting-sources  --strict-mcp-config --max-budget-usd 3.00 --session-id {u} --permission-mode bypassPermissions", plugin.display())],
        "lean: none of the person's settings or MCP servers; our plugin; tools kept; $3.00"
    );
    assert_eq!(p.lines("cwds"), [std::fs::canonicalize(p.repo()).expect("canon").display().to_string()]);
    assert_eq!(p.lines("envs"), [format!("{me}|off|off|unset|unset")], "named for its hooks; no judge, no nested launch; the two names stripped");
    let (one, two) = (p.short("c1: first try"), p.short("c1: second try"));
    // each try as git diff --stat shows it: the plan file is committed with the work
    let tried = |n: usize, short: &str, plan: u64, got: &str| {
        format!(
            "  try {n} ({short}): 2 files changed, {} insertions(+), 1 deletion(-)\n     .familiar/plan | {plan} {}\n     src/app.sh     | 2 +-\n    the test's output:\n      expected hello, got: {got}",
            plan + 1,
            "+".repeat(plan as usize)
        )
    };
    assert_eq!(
        p.file("prompt.1"),
        [
            "familiar-ai: you are taking over one piece of work that a cheaper model could not finish.".to_string(),
            "THE PERSON ASKED (their words):\ngreet with hello".to_string(),
            "THE CRITERION (the cheaper model's reading): greet says hello".to_string(),
            "THE JUDGE said of its sealed test: not judged".to_string(),
            "THE PLACE IN THE CODE: src/app.sh · greet · lines 1–3, and those lines as they are now:\n#!/bin/sh\ngreet() { echo howdy; }\ngreet".to_string(),
            "WHERE THE FIX LIVES (the cheaper model's claim): this_repository. YOUR FIRST ACT, before any commit, is your own claim: familiar-ai claim fix_location_category --criterion_id c1 --category this_repository --where '' when the fix is made in this repository; when it cannot be: --category dependency, environment or request, --where names where it lives, --needs what it needs; then the person is asked, and no test and no code are written.".to_string(),
            format!("THE SEALED TEST: tests/test_c1.sh (new in the seal commit)\n{}", TEST.trim_end()),
            "THE TEST COMMAND: sh tests/test_c1.sh; familiar-ai runs it after each commit; not passed = failed".to_string(),
            format!(
                "WHAT WAS TRIED (the failed tries since the seal, oldest first, at most the last two):\n{}\n{}",
                tried(1, &one, 2, "hey"),
                tried(2, &two, 1, "howdy")
            ),
            "WHAT FINISHES IT: the sealed test passes with the code changed at that place; the code is committed; familiar-ai runs the test and records it. If the sealed test does not answer what the person asked, it may be changed: familiar-ai claim test_change_announced --criterion_id c1 --reason '<why>', then the new test is committed alone and must fail first, then the code.".to_string(),
        ]
        .join("\n\n"),
        "the brief, whole"
    );
    // each run's end is kept by its commit, out of git
    let full = |subject: &str| {
        p.git(&["log", "--format=%H %s"]).lines().find_map(|l| l.split_once(' ').filter(|(_, s)| *s == subject).map(|(h, _)| h.to_string())).unwrap_or_default()
    };
    let kept = std::fs::read_to_string(p.path(&format!(".familiar/runs/{}.log", full("c1: first try")))).unwrap_or_default();
    assert_eq!(kept, "expected hello, got: hey\n");

    // its own hook calls were governed as the investigator
    assert!(p.file("hook.1.edit-sealed").contains("\"permissionDecision\":\"deny\""), "the sealed test: {}", p.file("hook.1.edit-sealed"));
    assert!(!p.file("hook.1.edit-code").contains("deny"), "its fix: {}", p.file("hook.1.edit-code"));
    assert!(p.file("hook.1.holder-edit").contains("wait for it to come back"), "the holder waits: {}", p.file("hook.1.holder-edit"));
    assert!(p.file("hook.1.commit-hello").contains(&esc("familiar-ai: ran the tests of \"greet says hello\": they pass · stamped")), "{}", p.file("hook.1.commit-hello"));
    assert_eq!(p.file("hook.1.stop").trim(), "{}", "green: its stop goes, and the call to action is the person's session's only");
    // the run, logged beside the judge's: argv, how it ended, cost and session from its result
    let log = std::fs::read_to_string(p.path(".familiar/investigator.log")).unwrap_or_default();
    let head = format!("criterion c1 · conversation {me} · {} -p --model sonnet --plugin-dir {} ", p.dir.join("bin/claude").display(), plugin.display());
    assert!(log.starts_with("--- at ") && log.contains(&head), "{log}");
    assert!(
        log.contains("--permission-mode bypassPermissions · exit 0 · ")
            && log.contains(&format!(" s · cost $0.1200 · session {u} · transcript .familiar/investigator/{u}.jsonl\nstdout (first 2000 characters):\n{{\"type\":\"result\"")),
        "{log}"
    );
    let transcript = std::fs::read_to_string(p.path(&format!(".familiar/investigator/{u}.jsonl"))).unwrap_or_default();
    assert_eq!(transcript, format!("{{\"type\":\"user\",\"sessionId\":\"{u}\"}}\n"), "its transcript, kept");
    for kept in [".familiar/investigator.log".to_string(), format!(".familiar/investigator/{u}.jsonl"), ".familiar/runs/x.log".to_string()] {
        let ignored = Command::new("git").args(["check-ignore", "-q", &kept]).current_dir(p.repo()).status().expect("git");
        assert!(ignored.success(), "{kept} stays on the laptop");
    }
    assert_eq!(p.looked(), json!({}), "a later PostToolUse launches nothing and ends nothing");
    assert_eq!(
        p.stop(),
        json!({ "hookSpecificOutput": { "hookEventName": "Stop", "additionalContext": CTA } }),
        "the holder may stop: every criterion green; the agent is told the call to action"
    );
    assert_eq!(p.lines("calls").len(), 1, "nothing more launched");
}

#[test]
fn not_green_ends_the_turn_with_the_state_and_the_holders_next_stop_runs_the_next_rung() {
    let p = one_failed("ladder", false);
    std::fs::write(p.dir.join("mode.1"), "fail").expect("mode");
    assert_eq!(
        p.second_try(),
        ends_the_turn(
            "familiar-ai: escalated to the investigator — a stronger model (sonnet) did not prove \"greet says hello\": two tries failed; a stronger model is taking over; the next stop hands it to a stronger model still (opus), or the person answers its card"
        )
    );
    let me = transfers(&p)[0].1.clone();
    assert!(p.file("hook.1.stop").contains("\"decision\":\"block\""), "its own stop was held: {}", p.file("hook.1.stop"));
    assert_eq!(transfers(&p).len(), 1, "it stays with the investigator");
    // its SessionStart named no model, so the binary records its result's model after the transfer
    let seen = p.of_kind("model_seen");
    let last = seen.last().unwrap_or_else(|| panic!("model_seen"));
    assert_eq!((field(last, "agent_conversation_id"), field(last, "model")), (me.as_str(), "claude-sonnet-5-5"));
    assert!(last.at >= p.of_kind("criterion_transfer")[0].at, "{} < the transfer's", last.at);

    // no later PostToolUse launches for that step: nothing new was recorded
    let out = p.looked();
    assert!(out.get("continue").is_none() && p.lines("calls").len() == 1, "{out}");

    // the holder's next stop launches opus, whose green returns the criterion to the holder
    let out = p.stop();
    assert_eq!(out["systemMessage"], "familiar-ai: a stronger model (opus) is taking \"greet says hello\" over\nfamiliar-ai: a stronger model (opus) proved \"greet says hello\"");
    assert_eq!(out["hookSpecificOutput"]["additionalContext"], CTA);
    // its brief shows the last two of the four failed tries since the seal
    let second = p.file("prompt.2");
    assert!(second.contains(&format!("  try 3 ({}): ", p.short("c1: nope"))) && second.contains("      expected hello, got: nope"), "{second}");
    assert!(second.contains(&format!("  try 4 ({}): ", p.short("c1: still"))) && !second.contains("  try 2 ("), "{second}");
    let calls = p.lines("calls");
    assert_eq!(calls.len(), 2);
    assert!(calls[1].starts_with("-p --model opus --plugin-dir "), "{}", calls[1]);
    let tr = transfers(&p);
    assert_eq!(tr.len(), 3, "{tr:?}");
    assert_eq!(tr[1].0, me, "the next rung takes it from the investigator that failed");
    assert!(tr[1].1.starts_with(&format!("{SESSION}/")) && tr[1].1 != me);
    assert_eq!(tr[2], (tr[1].1.clone(), SESSION.to_string()), "back to where the chain started");
}

#[test]
fn a_second_failure_recorded_outside_a_post_tool_use_launches_at_the_stop() {
    let p = one_failed("fallback", false);
    // the commit lands outside the agent: no PostToolUse records it
    p.git(&["commit", "-q", "-am", "c1: second try"]);
    assert!(p.lines("calls").is_empty());
    assert_eq!(
        p.stop(),
        json!({
            "systemMessage": "familiar-ai: a stronger model (sonnet) is taking \"greet says hello\" over\nfamiliar-ai: a stronger model (sonnet) proved \"greet says hello\"",
            "hookSpecificOutput": { "hookEventName": "Stop", "additionalContext": CTA }
        }),
        "the stop goes: the person sees it start and end; the agent is told the call to action"
    );
    let calls = p.lines("calls");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with("-p --model sonnet ") && calls[0].ends_with(" --permission-mode bypassPermissions"), "{}", calls[0]);
    assert_eq!(transfers(&p).len(), 2, "there and back");
}

#[test]
fn off_launches_nothing_and_the_stop_is_as_before() {
    let p = one_failed("off", true);
    let out = p.second_try();
    assert!(out.get("continue").is_none(), "{out}");
    assert!(out.to_string().contains(&esc("familiar-ai: \"greet says hello\" · two tries failed; a stronger model is taking over")), "{out}");
    let out = p.stop();
    assert_eq!(out["decision"], "block");
    assert!(
        out["reason"].as_str().unwrap_or("").contains("; or a stronger model (sonnet) takes it over"),
        "{out}"
    );
    assert!(p.lines("calls").is_empty() && transfers(&p).is_empty());
}

/// A captured live run (Claude Code 2.1.289) shows `continue: false` from
/// PostToolUse ends the turn after one Bash call and shows the reason under it.
#[test]
fn the_live_capture_ends_the_turn_on_continue_false() {
    let fixture = |name: &str| std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap_or_default();
    let result: Value = serde_json::from_str(&fixture("post_tool_use_continue_false_result.json")).unwrap_or_default();
    let fields = ["type", "subtype", "is_error", "stop_reason", "terminal_reason", "result", "num_turns"].map(|k| result[k].clone());
    assert_eq!(fields, [json!("result"), json!("success"), json!(false), json!("tool_use"), json!("hook_stopped"), json!(""), json!(2)]);
    let line: Value = serde_json::from_str(&fixture("post_tool_use_continue_false_transcript_line.json")).unwrap_or_default();
    assert_eq!(line["attachment"]["type"], "hook_stopped_continuation");
    assert_eq!(line["rendered"][0]["content"], "<system-reminder>\nPostToolUse:Bash hook stopped continuation: escalated to the investigator\n</system-reminder>");
    // what the person saw (Claude Code puts a no-break space after ⎿)
    let screen = fixture("post_tool_use_continue_false_screen.txt");
    assert!(screen.contains("● Bash(echo hi)\n  ⎿ \u{a0}hi\n  ⎿ \u{a0}PostToolUse:Bash hook stopped continuation: escalated to the investigator\n") && !screen.contains("echo there)"), "{screen}");
}
