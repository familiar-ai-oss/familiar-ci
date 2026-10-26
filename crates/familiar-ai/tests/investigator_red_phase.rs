//! When the default model twice fails to go red, the hold is the person's card.
//! D5 (the owner's announced definition change, 2026-10-16): the investigator
//! ladder is for a valid red only; no stronger model is launched for
//! `two_tries_not_red`.

mod common;

use std::path::PathBuf;

use common::Project;
use familiar_ai::door::investigator_run;
use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::criterion::{sealed_at, Place};
use familiar_ai_rules::{lifecycle, View};
use serde_json::{json, Value};

fn fixture_13363() -> Log {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pydantic_13363_record.json")).expect("fixture");
    let facts: Vec<Value> = serde_json::from_str(&text).expect("json");
    facts.iter().map(|f| Fact::from_json(f).expect("a fact")).collect()
}

/// A benchmark record whose seal fell when an announced test change passed
/// (t20) never held `two_failures`; `tries_not_red` counts from that run and
/// reaches 2 at the first code commit (t21), which holds it for the person.
#[test]
fn pydantic_13363_never_held_on_two_failures_and_its_first_code_commit_is_the_persons_card() {
    let log = fixture_13363();
    let holder = "a1ca016e-4afd-4c62-a519-b6c20ee11e88";
    let prefix = |n: usize| log[..n].to_vec();
    for n in 1..=log.len() {
        let l = prefix(n);
        let v = View::new(&l);
        assert!(lifecycle::attempts(&v, "c1") <= 1, "at {n}");
        assert!(!lifecycle::whys(&v, "c1").iter().any(|w| w == "two_failures"), "at {n}");
        let held = lifecycle::whys(&v, "c1").iter().any(|w| w == "two_tries_not_red");
        // from the code commit's landing (t21): a code commit with no run yet is a try
        assert_eq!(held, n >= 22, "two_tries_not_red at {n}");
    }
    let full = View::new(&log);
    assert_eq!(lifecycle::attempts(&full, "c1"), 0);
    assert!(lifecycle::whys(&full, "c1").iter().any(|w| w == "tests_not_red"));
    assert!(sealed_at(&full, "c1", Place::Laptop).is_none());
    // the seal stood once: the red run at t14, before the test was changed
    assert!(sealed_at(&View::new(&prefix(15)), "c1", Place::Laptop).is_some());

    let count = |n: usize| lifecycle::tries_not_red(&View::new(&prefix(n)), "c1");
    assert_eq!((count(15), count(21), count(24), count(27)), (0, 1, 2, 3), "the red run, the passing tests-first run, then each code commit");
    assert_eq!(count(log.len()), 15, "the whole record: one passing tests-first run and fourteen code commits");
    // commit 8791525's PostToolUse: the log before it ended at t20
    assert_eq!(investigator_run::due(&prefix(24), holder, Some(&prefix(21))), None, "no investigator for two_tries_not_red");
    assert_eq!(investigator_run::due(&prefix(27), holder, Some(&prefix(24))), None);
    let cards: Vec<String> = familiar_ai_rules::terminal::cards(&View::new(&prefix(24))).into_iter().map(|c| c.hold).collect();
    assert_eq!(cards, ["two_tries_not_red"], "the person's card");
}

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// The fake `claude`: the judge's call (no `--plugin-dir`) answers "covers
/// it"; the investigator's plays its session through the real binary: the
/// sealed test written and committed alone, then its stop.
const FAKE: &str = r#"#!/bin/sh
D="$(cd "$(dirname "$0")/.." && pwd)"
case "$*" in
*--plugin-dir*) ;;
*)
    printf '%s\n' "$*" >> "$D/judge-calls"
    cat > /dev/null
    printf '%s\n' '{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}'
    exit 0 ;;
esac
printf '%s\n' "$*" >> "$D/calls"
cat > "$D/prompt"
if [ -e "$D/no-seal" ]; then
    printf '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"result":"no test"}\n'
    exit 0
fi
U=""; M=""; prev=""
for a in "$@"; do
    [ "$prev" = "--session-id" ] && U="$a"
    [ "$prev" = "--model" ] && M="$a"
    prev="$a"
done
P="$(pwd)"
"$FA_BIN" status > "$D/status-at-start" 2>&1
hook() { printf '%s' "$2" | "$FA_BIN" hook "$1" > "$D/hook.$3" 2>&1; }
g() { git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@" >/dev/null 2>&1; }
hook SessionStart "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"source\":\"startup\",\"model\":\"claude-$M-5-5\"}" start
printf 'out=$(sh src/app.sh)\n[ "$out" = hello ] || { echo "expected hello, got: $out"; exit 1; }\n' > tests/test_c1.sh
g add -A
g commit -q -m "c1: a test that fails first"
hook PostToolUse "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"$U\",\"cwd\":\"$P\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git commit -m c1\"},\"tool_response\":{}}" commit
hook Stop "{\"hook_event_name\":\"Stop\",\"session_id\":\"$U\",\"cwd\":\"$P\"}" stop
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","total_cost_usd":0.05,"modelUsage":{"claude-%s-5-5":{}},"result":"done"}\n' "$U" "$M"
"#;

/// The project planned to c1, held by SESSION, with no model seen for the
/// holder. The fake `claude` is first on PATH; the judge and the investigator run.
fn bare(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION).env_remove("CLAUDE_CODE_SESSION_ID");
    std::fs::create_dir_all(b.dir().join("plugin")).expect("plugin folder");
    let (user, plugin) = (b.dir().join("user"), b.dir().join("plugin"));
    let p = b
        .env_remove("FAMILIAR_AI_INVESTIGATOR_BUDGET")
        .env_remove("FAMILIAR_AI_INVESTIGATOR")
        .env_remove("FAMILIAR_AI_JUDGE")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env("HOME", user)
        .env("CLAUDE_PLUGIN_ROOT", plugin)
        .env("FA_BIN", env!("CARGO_BIN_EXE_familiar-ai"))
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
        vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
    ] {
        p.claim(&args);
    }
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p
}

/// [`bare`], its holder on haiku.
fn project(name: &str) -> Project {
    let p = bare(name);
    p.hook("PostModelSwitch", json!({ "from_model": "claude-sonnet-5-5", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
    /// The holder's transcript, carried on its commits when written.
    fn transcript(&self) -> PathBuf {
        self.dir.join("holder.jsonl")
    }

    /// The agent's commit in bypass mode; what the hook told.
    fn bypass_commit(&self, message: &str) -> Value {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        let mut payload = self.payload("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {}, "permission_mode": "bypassPermissions" }));
        if self.transcript().exists() {
            payload["transcript_path"] = Value::from(self.transcript().to_string_lossy().into_owned());
        }
        self.hook_with("PostToolUse", &payload)
    }

    /// A file the fake `claude` wrote in the scratch folder.
    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }
}

#[test]
fn two_tries_to_go_red_launch_nothing_and_the_hold_is_the_persons_card() {
    let p = project("walk");
    // try 1: a tests-first run that passed (tests_not_red)
    p.write("tests/test_c1.sh", "out=$(sh src/app.sh)\n[ -n \"$out\" ]\n");
    let first = p.bypass_commit("c1: tests first");
    assert!(first.get("continue").is_none(), "{first}");
    // try 2: a code commit before any seal: held, nothing launched
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    let second = p.bypass_commit("c1: a fix before a seal");
    assert!(p.file("calls").is_empty(), "no investigator: {second}");
    let log = p.facts();
    assert!(!log.iter().any(|f| f.kind == "criterion_transfer"), "no transfer");
    let v = View::new(&log);
    assert!(lifecycle::whys(&v, "c1").iter().any(|w| w == "two_tries_not_red"));
    assert!(sealed_at(&v, "c1", Place::Laptop).is_none());
    // the person's card, said plainly; no stronger model offered
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let reason = stop["reason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\nfamiliar-ai: \"greet says hello\" · two tries to go red failed; this needs you · next: the person answers its card: try again, plan it again or release it\n"), "{reason}");
    assert!(!reason.contains("stronger model") && !reason.contains("criterion_transfer"), "{reason}");
    assert!(p.file("calls").is_empty(), "the stop launches nothing either");
}
