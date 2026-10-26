//! Plan §1.2, §6.7 (S8): the reply that launches the investigator still tells what
//! the call is owed. The second failed try's own run ends before the investigator is
//! launched; its result is told by that reply or by the next one, written once and
//! never written without being spoken. A fake `claude` on PATH stands in for the model.

mod common;

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
    assert!(p.bypass_commit("c1: tests first").to_string().contains("familiar-ai: ran the tests of"));
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
}


fn text(f: &Fact, k: &str) -> String {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

#[test]
fn the_launching_reply_carries_the_owed_result_or_the_next_reply_does() {
    let p = one_failed("owed", false);
    let reply = p.second_try();
    assert_eq!(p.of_kind("criterion_transfer").len(), 2, "the investigator was launched: {reply}");
    let second = p.short("c1: second try");
    let owed = p
        .of_kind("run_reserved")
        .iter()
        .find(|f| text(f, "commit").starts_with(&second))
        .map(|f| text(f, "run_id"))
        .expect("the second try's run");
    assert!(p.of_kind("test_log_translation").iter().any(|f| text(f, "run_id") == owed), "the run ended before the launch");
    let next = p.looked();
    // the result was spoken once: in the launching reply, or in the next one, and `told` agrees
    // (the investigator took the part when the run ended: its own hooks are what it was told in)
    let mut heard = vec![reply.to_string(), next.to_string()];
    for hook in ["start", "edit-sealed", "edit-code", "holder-edit", "commit-hello", "stop"] {
        heard.push(p.file(&format!("hook.1.{hook}")));
    }
    let said = heard.iter().filter(|r| r.contains(&format!("run {owed}"))).count();
    let told = p.facts_of("told").iter().filter(|t| t["about"] == owed.as_str()).count();
    assert_eq!(told, 1, "told exactly once: {:?}", p.facts_of("told"));
    assert_eq!(said, 1, "spoken exactly once, never written unspoken: {heard:?}");
    assert!(!p.looked().to_string().contains(&format!("run {owed}")), "and not again");
}
