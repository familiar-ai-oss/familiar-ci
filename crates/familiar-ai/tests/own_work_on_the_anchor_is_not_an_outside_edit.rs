//! Changes made by this plan's own earlier criteria are not outside edits
//! (the owner, 2026-10-11): "The anchor is moved when its lines at the start
//! differ from its lines at planning, unless the working tree matches the
//! last commit there and every commit that touched those lines is in the
//! record as another criterion's work in this plan. Then the criterion goes
//! on at the lines' new place, and the agent is told which commit and
//! criterion changed them." The start check carries `tip_matched`.
//!
//! ```text
//!   src/app.sh  greet (c1's place)     src/bye.sh  farewell (c2's place, line 3)
//!   c1's fix, committed and stamped, also changes farewell; tree clean
//!                                       → c2 goes on, told: the work on c1
//!   the same, and farewell edited again, not committed
//!                                       → anchor_moved: the card
//!   a commit no criterion's stamp claims changes farewell
//!                                       → anchor_moved: the card
//! ```
//!
//! The brief's c1/c2 are mirrored here: c1 runs first on the string, so its
//! work is what lands on c2's lines before c2's start.

mod common;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\n";
const BYE: &str = "#!/bin/sh\n\nfarewell() { echo bye; }\n";
const NIGHT: &str = "#!/bin/sh\n\nnight() { echo night; }\n";
const GREET: &str = "#!/bin/sh\ngreet() { echo hello; }\n";
// a line added above farewell and farewell itself changed: two hunks
const BYE_CHANGED: &str = "#!/bin/sh\n# says goodbye\n\nfarewell() { echo goodbye; }\n";

const SESSION: &str = "5c1d2e3f-4a5b-4c6d-8e7f-90a1b2c3d4e5";
const MOVED: &str = "the code at the plan's place changed after the plan was made";
const TOLD: &str = "its lines were changed by the work on \"greet says hello\"; they now sit at src/bye.sh";

struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(name: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("familiar-ai-own-work-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("repo/src")).expect("temp project");
        let p = Project { dir };
        p.git(&["init", "-q", "-b", "main"]);
        std::fs::write(p.repo().join("src/app.sh"), APP).expect("write");
        std::fs::write(p.repo().join("src/bye.sh"), BYE).expect("write");
        std::fs::write(p.repo().join("src/night.sh"), NIGHT).expect("write");
        assert_eq!(p.run(&["setup"], "").0, 0);
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "base"]);
        p
    }

    fn repo(&self) -> PathBuf {
        self.dir.join("repo")
    }

    /// The cases card passed for `c` (plan cases-card, 2026-10-25) [draft names].
    fn cases_kept(&self, c: &str) {
        common::cases_kept_at(&self.repo(), c);
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(self.repo())
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn run(&self, args: &[&str], stdin: &str) -> (i32, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
            .env("FAMILIAR_AI_JUDGE", "off")
            .env("FAMILIAR_AI_INVESTIGATOR", "off")
            .env("FAMILIAR_AI_HOME", self.dir.join("home"))
            .env("CLAUDE_CODE_SESSION_ID", SESSION)
            .env_remove("CLAUDE_PROJECT_DIR")
            .current_dir(self.repo())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child.stdin.take().expect("stdin").write_all(stdin.as_bytes()).expect("write");
        let out = child.wait_with_output().expect("wait");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }

    fn claim(&self, args: &[&str]) {
        let mut argv = vec!["claim"];
        argv.extend(args);
        let (code, out, err) = self.run(&argv, "");
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }

    /// Commit everything and tell the binary, as the PostToolUse hook does.
    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.commit_staged(message)
    }

    /// Commit what is staged, the rest of the working tree left as it is.
    fn commit_staged(&self, message: &str) -> String {
        self.git(&["commit", "-q", "-m", message]);
        let payload = json!({ "hook_event_name": "PostToolUse", "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "tool_name": "Bash", "tool_input": { "command": format!("git commit -m '{message}'") }, "tool_response": {} });
        let (code, out, err) = self.run(&["hook", "PostToolUse"], &payload.to_string());
        assert_eq!(code, 0, "{err}");
        println!("--- familiar-ai hook PostToolUse\n{out}");
        out
    }

    /// Any hook writes the checks owed.
    fn look(&self) {
        let payload = json!({ "hook_event_name": "PreToolUse", "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "tool_name": "Read", "tool_input": { "file_path": self.repo().join("src/app.sh").to_string_lossy() } });
        let (code, _, err) = self.run(&["hook", "PreToolUse"], &payload.to_string());
        assert_eq!(code, 0, "{err}");
    }

    /// c's latest start check, from the plan's own export of the record.
    fn start_check(&self, c: &str) -> Value {
        let plan = std::fs::read_to_string(self.repo().join(".familiar/plan")).expect("the plan's export");
        plan.lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .rfind(|f| f["kind"] == "anchor_sha_matched" && f["criterion_id"] == c && f["checked_at"] == "start")
            .unwrap_or_else(|| panic!("no start check for {c}: {plan}"))
    }

    fn status(&self) -> String {
        self.run(&["status"], "").1
    }

    /// c1 on greet (src/app.sh 2); c2 strung after it on farewell
    /// (src/bye.sh 3); c1's tests committed first, red, stamped.
    fn planned(name: &str) -> Project {
        let p = Project::plan_only(name, false);
        p.c1_tests_first();
        p
    }

    /// `planned` with c3 strung after c2.
    fn planned_three(name: &str) -> Project {
        let p = Project::plan_only(name, true);
        p.c1_tests_first();
        p
    }

    /// The plan alone, confirmed; nothing committed after the base.
    /// `third`: c3 on night (src/night.sh 3), strung after c2.
    fn plan_only(name: &str, third: bool) -> Project {
        let p = Project::new(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet says hello, farewell says goodbye"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["criterion", "--criterion_id", "c2", "--text", "farewell says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/bye.sh","symbol":"farewell","line_start":3,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        let mut crits = vec!["c1", "c2"];
        if third {
            p.claim(&["criterion", "--criterion_id", "c3", "--text", "good night is said", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
            p.cases_kept("c3"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
            p.claim(&["place_chosen", "--criterion_id", "c3", "--places", r#"[{"file_path":"src/night.sh","symbol":"night","line_start":3,"line_end":3}]"#]);
            p.claim(&["test_planned", "--criterion_id", "c3", "--file_path", "tests/test_c3.sh", "--will", "create"]);
            p.claim(&["criterion_strung_on_head", "--criterion_id", "c3", "--head", "c1"]);
            crits.push("c3");
        }
        for c in crits {
            p.claim(&["criterion_assigned", "--criterion_id", c]);
            p.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
        }
        p
    }

    /// c1's tests committed first, red, stamped.
    fn c1_tests_first(&self) {
        std::fs::create_dir_all(self.repo().join("tests")).expect("tests");
        std::fs::write(self.repo().join("tests/test_c1.sh"), ". src/app.sh; greet | grep -qx hello\n").expect("test");
        let sealed = self.commit("c1: tests first");
        assert!(sealed.contains("they fail, as they should before the fix · stamped"), "{sealed}");
    }

    /// c1's fix, which also changes farewell, committed and stamped for c1;
    /// `then`: farewell as the working tree has it after, not committed
    /// (the hook that sees the commit writes c2's start check).
    fn c1_fix_also_changes_farewell(&self, then: Option<&str>) -> String {
        std::fs::write(self.repo().join("src/app.sh"), GREET).expect("fix");
        std::fs::write(self.repo().join("src/bye.sh"), BYE_CHANGED).expect("fix");
        self.git(&["add", "-A"]);
        if let Some(after) = then {
            std::fs::write(self.repo().join("src/bye.sh"), after).expect("edit");
        }
        let green = self.commit_staged("c1: greet says hello");
        assert!(green.contains("they pass · stamped"), "{green}");
        self.git(&["rev-parse", "HEAD"])
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn another_criterions_committed_work_on_the_lines_goes_on_and_is_told() {
    let p = Project::planned("own");
    let sha = p.c1_fix_also_changes_farewell(None);
    p.look();
    let st = p.start_check("c2");
    assert_eq!(st["tip_matched"], json!(true), "the working tree is the commit: {st}");
    let status = p.status();
    assert!(!status.contains(MOVED), "c1's own recorded work is not an outside edit: {status}");
    let (a, b) = (st["line_start"].as_u64().unwrap_or(0), st["line_end"].as_u64().unwrap_or(0));
    let told = format!("{TOLD} {a}–{b} (commit {})", &sha[..7]);
    assert!(status.contains(&told), "the agent is told which commit and criterion changed them: {told}\n{status}");
}

#[test]
fn an_edit_not_committed_on_top_of_that_work_is_the_card() {
    let p = Project::planned("own-uncommitted");
    p.c1_fix_also_changes_farewell(Some("#!/bin/sh\n# says goodbye\n\nfarewell() { echo see you; }\n"));
    p.look();
    assert_eq!(p.start_check("c2")["tip_matched"], json!(false), "the working tree is not the commit");
    let status = p.status();
    assert!(status.contains(MOVED), "anchor_moved: the card: {status}");
    assert!(!status.contains(TOLD), "{status}");
}

// A commit that names c2's and c3's lines while c1 is at work: c1's run
// stamps it for c1, which it does not name; no stamp ties it to another
// criterion it names, so c2's start check finds an outside change.
#[test]
fn a_commit_no_criterions_stamp_claims_is_the_card() {
    let p = Project::planned_three("unstamped");
    std::fs::write(p.repo().join("src/bye.sh"), BYE_CHANGED).expect("edit");
    std::fs::write(p.repo().join("src/night.sh"), "#!/bin/sh\n\nnight() { echo good night; }\n").expect("edit");
    p.commit("tidy farewell and night");
    std::fs::write(p.repo().join("src/app.sh"), GREET).expect("fix");
    let green = p.commit("c1: greet says hello");
    assert!(green.contains("they pass · stamped"), "{green}");
    p.look();
    assert_eq!(p.start_check("c2")["tip_matched"], json!(true));
    let status = p.status();
    assert!(status.contains(MOVED), "a commit the record does not tie to another criterion: {status}");
    assert!(!status.contains(TOLD), "{status}");
}

// The start check is owed to a testing criterion only (the model, 2026-10-11,
// 18:52; sibling_before_start made the held case unneeded): a commit that
// names c2's lines alone, before c2's turn, is c2's code landed before its
// test; c2 is held on not_stamped and its start check is not written.
#[test]
fn a_commit_on_the_lines_alone_before_the_turn_is_its_code() {
    let p = Project::planned("alone");
    std::fs::write(p.repo().join("src/bye.sh"), BYE_CHANGED).expect("edit");
    p.commit("tidy farewell");
    std::fs::write(p.repo().join("src/app.sh"), GREET).expect("fix");
    let green = p.commit("c1: greet says hello");
    assert!(green.contains("they pass · stamped"), "{green}");
    p.look();
    let plan = std::fs::read_to_string(p.repo().join(".familiar/plan")).expect("the plan's export");
    assert!(!plan.lines().any(|l| l.contains("\"anchor_sha_matched\"") && l.contains("\"c2\"") && l.contains("\"start\"")), "no start check for a held c2: {plan}");
    let status = p.status();
    assert!(status.contains("\"farewell says goodbye\" · the code landed before its test was sealed"), "{status}");
    assert!(!status.contains(MOVED), "{status}");
}
