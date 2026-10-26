//! Only checkouts with a live or resumable agent have their cards relayed; temporary review
//! worktrees never do (the owner, 2026-10-14; decided 2026-10-15, plan §6.8). Main's cards
//! always reach another checkout. Another checkout's reach main only while it is alive: its lane
//! session file exists (`--resume` can continue it), or its worktree lock names a `claude`
//! process still running; and then only the cards of parts touched there. Every set-up checkout
//! stays listed; only its cards are filtered.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "4c3b2a19-8d7e-4f6a-9b5c-1d2e3f4a5b6c";
const LANE: &str = "9b2c4d6e-1f3a-4b5c-8d7e-0a1b2c3d4e5f";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").env("FAMILIAR_AI_BENCH", "0").create()
}

impl Project {
    /// At `at`, a confirmed plan whose anchor was edited outside the work before its start
    /// check: c1 held on anchor_moved, its card open, held by `holder`.
    fn a_card_at(&self, at: &Path, holder: &str) {
        let claim = |args: &[&str]| {
            let mut argv = vec!["claim"];
            argv.extend_from_slice(args);
            let (code, out, err) = self.run_in(at, &argv, "");
            assert_eq!(code, 0, "claim {args:?}: {out}{err}");
            out
        };
        claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        common::cases_kept_at(at, "c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        claim(&["criterion_head", "--criterion_id", "c1"]);
        std::fs::write(at.join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("outside edit");
        claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", holder]);
        let out = claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
    }

    /// A worktree beside the repository, on its own branch or detached, set up there with a card.
    fn worktree_with_a_card(&self, name: &str, detached: bool) -> PathBuf {
        let wt = self.dir.join(name);
        let path = wt.to_string_lossy().to_string();
        if detached {
            self.git(&["worktree", "add", "-q", "--detach", &path]);
        } else {
            self.git(&["worktree", "add", "-q", "-b", name, &path]);
        }
        assert_eq!(self.run_in(&wt, &["setup"], "").0, 0);
        self.a_card_at(&wt, LANE);
        wt
    }

    /// Another checkout's entry in `status --json` at `at`.
    fn entry(&self, at: &Path, folder: &str) -> Value {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        let j: Value = serde_json::from_str(&out).expect("status --json");
        j["checkouts"].as_array().and_then(|a| a.iter().find(|c| c["folder"] == folder).cloned()).unwrap_or_else(|| panic!("{folder} is not listed: {j}"))
    }

    /// The criterion ids of the cards `at` relays from the checkout `folder`.
    fn relayed(&self, at: &Path, folder: &str) -> Vec<String> {
        let e = self.entry(at, folder);
        let cards: Vec<String> = e["cards"].as_array().map(|a| a.iter().filter_map(|c| c["criterion_id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
        assert_eq!(e["counts"]["needs_you"], json!(cards.len()), "{e}");
        cards
    }
}

/// The lane's session file, as `familiar-ai launch_lane` writes it.
fn write_session(wt: &Path) {
    let kept = wt.join(".familiar/runs/lane");
    std::fs::create_dir_all(&kept).expect("runs folder");
    std::fs::write(kept.join("session"), LANE).expect("session file");
}

/// A running process and its start time, field 22 of `/proc/<pid>/stat`.
fn a_live_process() -> (Child, u32, String) {
    let child = Command::new("sleep").arg("60").spawn().expect("sleep");
    let pid = child.id();
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("stat");
    let start = stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(19)).expect("field 22").to_string();
    (child, pid, start)
}

#[test]
fn mains_cards_are_always_relayed() {
    let p = project("main");
    p.a_card_at(&p.repo(), SESSION);
    let wt = p.dir.join("other");
    p.git(&["worktree", "add", "-q", "--detach", &wt.to_string_lossy()]);
    assert_eq!(p.run_in(&wt, &["setup"], "").0, 0);
    assert_eq!(p.relayed(&wt, "repo"), ["c1"]);
}

#[test]
fn a_lane_is_relayed_while_its_session_file_exists() {
    let p = project("session");
    let wt = p.worktree_with_a_card("lane", false);
    assert_eq!(p.relayed(&p.repo(), "lane"), Vec::<String>::new(), "no agent: listed, its card not relayed");
    assert_eq!(p.run(&["statusline"], "").1.matches("needs you").count(), 0);
    write_session(&wt);
    assert_eq!(p.relayed(&p.repo(), "lane"), ["c1"]);
    assert!(p.run(&["statusline"], "").1.contains("lane: 1 needs you"));
}

#[test]
fn a_worktree_is_relayed_while_its_lock_names_a_live_claude_and_not_once_it_is_gone() {
    let p = project("lock");
    let wt = p.worktree_with_a_card("workflow", false);
    let (mut child, pid, start) = a_live_process();
    p.git(&["worktree", "lock", "--reason", &format!("claude agent agent-x (pid {pid} start {start})"), &wt.to_string_lossy()]);
    assert_eq!(p.relayed(&p.repo(), "workflow"), ["c1"], "a live agent");
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(p.relayed(&p.repo(), "workflow"), Vec::<String>::new(), "its process is gone");
    p.git(&["worktree", "unlock", &wt.to_string_lossy()]);
    let (mut other, pid, _) = a_live_process();
    p.git(&["worktree", "lock", "--reason", &format!("claude agent agent-x (pid {pid} start 1)"), &wt.to_string_lossy()]);
    assert_eq!(p.relayed(&p.repo(), "workflow"), Vec::<String>::new(), "another process under its pid");
    let _ = other.kill();
    let _ = other.wait();
    p.git(&["worktree", "unlock", &wt.to_string_lossy()]);
    assert_eq!(p.relayed(&p.repo(), "workflow"), Vec::<String>::new(), "a finished workflow worktree: its lock gone");
}

#[test]
fn a_detached_review_worktree_is_never_relayed() {
    let p = project("review");
    p.worktree_with_a_card("review", true);
    assert_eq!(p.entry(&p.repo(), "review")["folder"], "review", "listed");
    assert_eq!(p.relayed(&p.repo(), "review"), Vec::<String>::new());
}

#[test]
fn an_alive_lane_relays_only_the_cards_of_parts_touched_there() {
    let p = project("touched");
    // main: c1 planned and committed, its test not yet written
    p.a_card_at(&p.repo(), SESSION);
    p.write("src/app.sh", common::project::APP);
    p.land("the plan");
    // the lane is seeded from that plan; an outside edit of c1's lines there opens its card
    let wt = p.dir.join("lane");
    p.git(&["worktree", "add", "-q", "-b", "lane", &wt.to_string_lossy()]);
    let (code, out, err) = p.run_in(&wt, &["setup"], "");
    assert_eq!(code, 0, "{out}{err}");
    write_session(&wt);
    std::fs::write(wt.join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("outside edit");
    let read = json!({ "hook_event_name": "PreToolUse", "session_id": LANE, "cwd": wt.to_string_lossy(), "tool_name": "Read", "tool_input": { "file_path": wt.join("src/app.sh").to_string_lossy() } });
    assert_eq!(p.run_in(&wt, &["hook", "PreToolUse"], &read.to_string()).0, 0);
    let (code, out, _) = p.run_in(&wt, &["status", "--json"], "");
    assert_eq!(code, 0);
    let own: Value = serde_json::from_str(&out).expect("status --json");
    assert_eq!(own["cards"][0]["criterion_id"], "c1", "the lane holds c1's card: {own}");
    assert_eq!(p.relayed(&p.repo(), "lane"), Vec::<String>::new(), "inherited, untouched there");
    let (code, out, err) = p.run_in(&wt, &["claim", "criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", LANE], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.relayed(&p.repo(), "lane"), ["c1"], "claimed there");
}
