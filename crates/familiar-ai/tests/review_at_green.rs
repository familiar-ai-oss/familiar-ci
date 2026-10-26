//! The reviewer at green [draft names] (the owner's plan of 2026-10-20, step 3). At a stop, a part
//! whose seal the judge covered and whose tests pass at a commit after the tests-first one is
//! read once per green commit by the judge's model, with read-only tools in a scratch copy of
//! that commit: does the fix do what the person asked, everywhere they asked it? A "no" blocks
//! the stop only when the binary finds every quote at its file and line (±2) at the green commit;
//! any other "no" is recorded as not judged and never blocks. Two verified "no"s are the person's
//! card. Runs the real binary with a fake `claude` first on PATH; no model is called.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it runs greet and checks it says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

const GREEN: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
const GAP: &str = "greet saying hello to the name it is given";
const REAL: &str = "greet() { echo hello; }";

/// The reviewer's answer, as Claude Code prints it.
fn review(covers: bool, gap: &str, quotes: Value, reason: &str) -> String {
    let answer = json!({ "covers": covers, "gap": gap, "quotes": quotes, "reason": reason }).to_string();
    json!({ "type": "result", "is_error": false, "result": answer, "modelUsage": { "claude-opus-4-6": {} } }).to_string()
}

/// A project planned to c1 (anchor src/app.sh, test tests/test_c1.sh), its holder on haiku, the
/// judge on. The fake `claude` answers the judge from `bin/answer`; a call with read-only tools is
/// the reviewer's: it keeps its argv, its folder, what src/app.sh reads there and its prompt, and
/// answers from `bin/review`.
fn project(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION);
    let d = b.dir().join("bin");
    let fake = format!(
        "#!/bin/sh\nd='{d}'\ncase \"$*\" in\n*'--tools Read,Grep'*)\n  printf '%s\\n' \"$*\" >> \"$d/review_calls\"\n  pwd > \"$d/review_cwd\"\n  cat src/app.sh > \"$d/review_saw\" 2>/dev/null\n  cat > \"$d/review_prompt\"\n  cat \"$d/review\" ;;\n*)\n  printf '%s\\n' \"$*\" >> \"$d/calls\"\n  cat > /dev/null\n  cat \"$d/answer\" ;;\nesac\n",
        d = d.display()
    );
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), YES).expect("answer");
    let p = b.env_remove("FAMILIAR_AI_JUDGE").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
    fn reviewer_says(&self, answer: &str) {
        std::fs::write(self.dir.join("bin/review"), answer).expect("review");
    }

    /// The test committed alone, sealed red and covered; then the code that makes it pass.
    fn green(&self) {
        self.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c1: tests"]);
        let out = self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m tests" }, "tool_response": {} }));
        assert!(out.to_string().contains("sealed (red, covered)"), "{out}");
        self.code(GREEN);
    }

    /// A code commit, handed to the hook; the part is green after it. A second commit whose
    /// files and tests are as the first's is read as the same content, so it is not run again.
    fn code(&self, text: &str) {
        self.write("src/app.sh", text);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c1: code"]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am code" }, "tool_response": {} }));
        assert_eq!(self.state_name("c1"), "green", "{}", self.status());
    }

    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join("bin").join(name)).unwrap_or_default()
    }

    fn reviews(&self) -> usize {
        self.file("review_calls").lines().count()
    }

    fn verdicts(&self) -> Vec<Value> {
        self.facts_of("fix_verdict")
    }
}

fn reason(out: &Value) -> &str {
    out["reason"].as_str().unwrap_or("")
}

#[test]
fn a_no_whose_quotes_are_in_the_repo_refuses_the_stop_and_names_the_gap() {
    let p = project("real");
    p.green();
    assert_eq!(p.reviews(), 0, "the reviewer is asked at a stop, not at the green commit");
    p.reviewer_says(&review(false, GAP, json!([{ "file": "src/app.sh", "line": 2, "text": REAL }]), "greet never reads its argument."));
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let r = reason(&stop);
    assert!(
        r.starts_with(&format!(
            "familiar-ai: not done yet · \"greet says hello\" · the reviewer (opus) found the fix does not yet do {GAP}\n  the reviewer's reason: greet never reads its argument\n  its proof: src/app.sh:2 \"{REAL}\"\n  next: change the code so it does, and commit it; the new commit is reviewed again at your next stop"
        )),
        "{r}"
    );
    assert!(
        stop["systemMessage"].as_str().unwrap_or("").ends_with(&format!("\nfamiliar-ai: the reviewer (opus) read the fix against your request: it does not yet do {GAP}")),
        "{stop}"
    );
    // the judge's model, read-only tools in a scratch copy of the green commit, the judge's cap
    let schema = familiar_ai::door::review::SCHEMA;
    assert_eq!(p.file("review_calls"), format!("-p --model opus --output-format json --json-schema {schema} --setting-sources  --strict-mcp-config --tools Read,Grep --max-budget-usd 0.50\n"));
    let cwd = p.file("review_cwd").trim().to_string();
    assert_ne!(std::path::Path::new(&cwd), p.repo().as_path(), "never the checkout itself");
    assert_eq!(p.file("review_saw"), GREEN, "the copy is the green commit");
    assert!(!std::path::Path::new(&cwd).exists(), "the copy is removed after: {cwd}");
    // what it reads: the person's words, the criterion, the sealed test, the fix and the code around it
    let prompt = p.file("review_prompt");
    for part in [
        "THE INTENT (what the person asked for, in their own words)\ngreet with hello",
        "criterion c1: greet says hello",
        "THE SEALED TEST tests/test_c1.sh",
        "THE FIX (from the tests-first commit to the green commit)",
        "-    2| greet() { echo hi; }",
        "+    2| greet() { echo hello; }",
        "THE CODE AROUND THE CHANGE src/app.sh",
        "Against the person's words and the code around the change: does the fix do what was asked, everywhere it was asked? Does it change anything else the code around it relies on (documented precedence, a sibling path, a repr or other derived output, a hot path that bypasses the change)? For any gap, quote the exact repo lines that show it, with file:line.",
    ] {
        assert!(prompt.contains(part), "{part:?} in {prompt}");
    }
    // the fact: a verified no at the green commit, its quote as git holds it
    let v = p.verdicts();
    assert_eq!(v.len(), 1);
    assert_eq!((v[0]["covers"].as_str(), v[0]["commit"].as_str(), v[0]["gap"].as_str()), (Some("no"), Some(p.head().as_str()), Some(GAP)));
    assert_eq!(v[0]["quotes"], json!([{ "file_path": "src/app.sh", "line": 2, "text": REAL }]));
    assert!(p.status().contains(&format!("familiar-ai: \"greet says hello\" · the reviewer (opus) found the fix does not yet do {GAP}; the code may be changed")), "{}", p.status());
    // once per green commit; the no still stands, with the judge off too: it is a fact
    assert_eq!(p.hook("Stop", json!({}))["decision"], "block");
    assert_eq!(p.reviews(), 1);
    p.set_env("FAMILIAR_AI_JUDGE", Some("off"));
    assert_eq!(p.hook("Stop", json!({}))["decision"], "block");
}

#[test]
fn a_no_whose_quote_is_not_in_the_repo_is_not_judged_and_never_blocks() {
    let p = project("fabricated");
    p.green();
    p.reviewer_says(&review(false, GAP, json!([{ "file": "src/app.sh", "line": 2, "text": "greet() { echo \"hello $1\"; }" }]), "it ignores the name."));
    let stop = p.hook("Stop", json!({}));
    assert!(stop.get("decision").is_none(), "{stop}");
    assert!(
        stop["systemMessage"].as_str().unwrap_or("").starts_with("familiar-ai: the reviewer's no is not judged: quote 1: src/app.sh line 2 does not read as quoted; it never blocks; its call is logged in .familiar/judge.log"),
        "{stop}"
    );
    let v = p.verdicts();
    assert_eq!((v[0]["covers"].as_str(), v[0]["why"].as_str()), (Some("not_judged"), Some("unverified")), "{v:?}");
    assert_eq!(v[0]["gap"].as_str(), Some(GAP), "recorded as it was said");
    // a no with no quote is not judged either; the second miss at this commit ends the asking
    p.reviewer_says(&review(false, GAP, json!([]), "it ignores the name."));
    assert!(p.hook("Stop", json!({})).get("decision").is_none());
    assert_eq!(p.reviews(), 2);
    assert_eq!(p.verdicts()[1]["why"].as_str(), Some("unverified"));
    p.hook("Stop", json!({}));
    assert_eq!(p.reviews(), 2, "twice not judged at one commit: not asked again");
}

#[test]
fn covers_lets_the_stop_through_once_per_green_commit() {
    let p = project("covers");
    p.green();
    p.reviewer_says(&review(true, "", json!([]), "greet says hello on every path asked for."));
    let stop = p.hook("Stop", json!({}));
    assert!(stop.get("decision").is_none(), "{stop}");
    assert_eq!(stop["systemMessage"].as_str().unwrap_or(""), "familiar-ai: the reviewer (opus) read the fix against your request: it does what was asked");
    assert_eq!(p.verdicts()[0]["covers"].as_str(), Some("yes"));
    p.hook("Stop", json!({}));
    assert_eq!(p.reviews(), 1);
}

#[test]
fn a_quote_two_lines_off_is_found_and_kept_at_its_own_line() {
    let p = project("window");
    p.green();
    p.reviewer_says(&review(false, GAP, json!([{ "file": "./src/app.sh", "line": 4, "text": format!("  {REAL}  ") }]), "r"));
    assert_eq!(p.hook("Stop", json!({}))["decision"], "block");
    assert_eq!(p.verdicts()[0]["quotes"], json!([{ "file_path": "src/app.sh", "line": 2, "text": REAL }]));
}

#[test]
fn two_verified_nos_on_one_part_are_the_persons_card() {
    let p = project("twice");
    p.green();
    p.reviewer_says(&review(false, GAP, json!([{ "file": "src/app.sh", "line": 2, "text": REAL }]), "greet never reads its argument."));
    assert_eq!(p.hook("Stop", json!({}))["decision"], "block");
    // the agent may change the code while the no stands: the work is open again
    let edit = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": p.path("src/app.sh").to_string_lossy() } }));
    assert_ne!(edit["hookSpecificOutput"]["permissionDecision"], "deny", "{edit}");
    assert_ne!(p.bash("PreToolUse", "git commit -am code")["hookSpecificOutput"]["permissionDecision"], "deny");
    // another fix, green again, reviewed again: a second verified no
    p.code("#!/bin/sh\n# says hello\ngreet() { echo hello; }\ngreet\n");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(p.reviews(), 2, "a new green commit is a new review");
    assert_eq!(stop["decision"], "block", "{stop}");
    assert!(reason(&stop).starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion"), "{stop}");
    let status = p.status();
    assert!(status.contains(&format!("its tests pass, but a reviewer read the fix against the person's words twice and found it does not do {GAP}. What now?")), "{status}");
    assert_eq!(p.state_name("c1"), "held");
    // the person keeps the fix as it is: the stop goes through, nothing more is asked
    let (code, _, err) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "review_disputed", "--choice", "keep_going", "--typed", ""], "");
    assert_eq!(code, 0, "{err}");
    assert!(p.hook("Stop", json!({})).get("decision").is_none());
    assert_eq!(p.reviews(), 2);
}
