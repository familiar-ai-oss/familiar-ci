//! `familiar-ai tidy` drops the agent's failed tries among its unpushed
//! commits and keeps one seal and one fix. Only commits landed during the
//! agent's own Bash call (`by: agent`) are rewritten.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5d1c0b8e-7f0a-4c55-9d43-2f1e6a7b8c90";

impl Project {
    /// main holds `base`; the branch `work` holds setup's commit and a
    /// hand-made commit, both before the intent.
    fn on_work(name: &str) -> Project {
        let p = Project::builder(name).session(SESSION).setup_on_branch("work").no_setup().create();
        let (code, out, err) = p.run(&["setup"], "");
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("as one commit \"familiar-ai setup\""), "{out}");
        std::fs::write(p.repo().join("README"), "made by hand\n").expect("readme");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "hand: readme"]);
        p
    }

    /// A commit of `path` made in the person's terminal, noticed at the
    /// agent's next tool call: its PreToolUse.
    fn hand_commit(&self, path: &str, message: &str) {
        self.git(&["add", "--", path]);
        self.git(&["commit", "-q", "-m", message]);
        self.bash("PreToolUse", "ls");
    }

    /// Each `commit_seen` in the record: its commit's subject and its `by`.
    fn seen_by(&self) -> Vec<(String, Option<String>)> {
        self.facts_of("commit_seen")
            .iter()
            .map(|f| {
                let id = f["commit"].as_str().expect("commit");
                (self.git(&["log", "-1", "--format=%s", id]), f.get("by").and_then(Value::as_str).map(str::to_string))
            })
            .collect()
    }

    fn statusline(&self) -> String {
        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "workspace": { "project_dir": self.repo().to_string_lossy() } });
        self.run(&["statusline"], &input.to_string()).1
    }

    /// The plan claimed; the test sealed red; two failed attempts; green.
    /// `test` is the test file's text.
    fn walked(name: &str, test: &str, push_after: Option<&str>) -> Project {
        Project::walked_as(name, test, push_after, false, true, &|_| {})
    }

    /// As [`walked`], with a first tests commit that passes (not red) and
    /// with or without the two failed attempts; `after_try_1` runs between
    /// them (the person's own commits).
    fn walked_as(name: &str, test: &str, push_after: Option<&str>, not_red_first: bool, tries: bool, after_try_1: &dyn Fn(&Project)) -> Project {
        let p = Project::on_work(name);
        let remote = p.dir.join("remote.git");
        if push_after.is_some() {
            p.git(&["init", "-q", "--bare", remote.to_str().expect("path")]);
            p.git(&["remote", "add", "origin", remote.to_str().expect("path")]);
        }
        let push = |at: &str| {
            if push_after == Some(at) {
                p.git(&["push", "-q", "-u", "origin", "work"]);
            }
        };
        push("hand");
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
        std::fs::create_dir_all(p.repo().join("tests")).expect("tests");
        if not_red_first {
            std::fs::write(p.repo().join("tests/test_c1.sh"), "sh src/app.sh | grep -q h\n").expect("test");
            assert!(p.commit("c1 tests, not red").to_string().contains("the test must fail on the code as it is, committed alone before any fix"));
        }
        std::fs::write(p.repo().join("tests/test_c1.sh"), test).expect("test");
        assert!(p.commit("c1 tests").to_string().contains("they fail, as they should before the fix"));
        if tries {
            std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hullo; }\ngreet\n").expect("try 1");
            p.commit("c1 try 1");
            push("try 1");
            after_try_1(&p);
            std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hell; }\ngreet\n").expect("try 2");
            std::fs::write(p.repo().join("src/extra.sh"), "# a helper the agent tried\n").expect("try 2");
            p.commit("c1 try 2");
            std::fs::remove_file(p.repo().join("src/extra.sh")).expect("rm");
        }
        std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hello; }\ngreet\n").expect("fix");
        assert!(p.commit("c1 the fix").to_string().contains("they pass"));
        assert!(p.statusline().contains(" · 1 green"), "{}", p.statusline());
        p
    }
}

const TEST: &str = "sh src/app.sh | grep -qx hello\n";

#[test]
fn the_failed_attempts_are_dropped_the_setup_and_hand_made_commits_kept_and_the_proof_survives() {
    let p = Project::walked("drop", TEST, None);
    let before_ids = p.git(&["log", "--format=%H", "--reverse"]);
    let before_tree = p.git(&["rev-parse", "HEAD^{tree}"]);
    let old_head = p.git(&["rev-parse", "HEAD"]);
    let status = p.git(&["status", "--porcelain", "--untracked-files=no"]);
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 try 1", "c1 try 2", "c1 the fix"]);

    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 the fix"]);
    let after_ids: Vec<String> = p.git(&["log", "--format=%H", "--reverse"]).lines().map(str::to_string).collect();
    let before_ids: Vec<&str> = before_ids.lines().collect();
    assert_eq!(&after_ids[..4], &before_ids[..4], "base, setup, the hand-made commit and the seal keep their ids: nothing before the first dropped attempt changed");
    assert_eq!(p.git(&["rev-parse", "HEAD^{tree}"]), before_tree, "the same files at the end");
    assert_eq!(p.git(&["diff", "--name-only", "HEAD~1", "HEAD"]), ".familiar/plan\nsrc/app.sh", "the fix holds the code (and the plan) as at green, and nothing of the dropped try");
    assert_eq!(p.git(&["status", "--porcelain", "--untracked-files=no"]), status, "the plan is projected from the record again; nothing else changed");
    assert_eq!(p.git(&["symbolic-ref", "--short", "HEAD"]), "work");
    let kept = p.git(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/familiar-ai/pre-tidy"]);
    assert!(kept.starts_with("refs/familiar-ai/pre-tidy/") && kept.ends_with(&old_head), "{kept}");
    assert!(out.contains("re-checked \"greet says hello\": the seal ") && out.contains("its tests fail there; they pass at the fix"), "{out}");
    assert!(out.contains("tidied work: 6 commits became 4 (2 failed attempts dropped, 2 commits kept as they were)"), "{out}");

    // the record took the new commits: the state is as it was, and the next
    // hook call has nothing new to read
    assert!(p.statusline().contains(" · 1 green"), "{}", p.statusline());
    let told = p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "familiar-ai tidy" }, "tool_response": {} }));
    assert!(!told.to_string().contains("ran the tests"), "{told}");
    let (_, status, _) = p.run(&["status"], "");
    assert!(status.contains("\"greet says hello\" · locally green"), "{status}");

    // a second tidy changes nothing
    let head = p.git(&["rev-parse", "HEAD"]);
    let (code, out, _) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nothing to tidy: "), "{out}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn a_seal_that_was_not_red_is_dropped_and_a_tidy_branch_is_left_as_it_is() {
    let p = Project::walked_as("notred", TEST, None, true, false, &|_| {});
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests, not red", "c1 tests", "c1 the fix"]);
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 the fix"]);
    assert!(out.contains("(1 failed attempt dropped, 2 commits kept as they were)"), "{out}");
    assert!(p.statusline().contains(" · 1 green"), "{}", p.statusline());

    let p = Project::walked_as("straight", TEST, None, false, false, &|_| {});
    let head = p.git(&["rev-parse", "HEAD"]);
    let (code, out, _) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nothing to tidy: work already has one seal commit and one fix commit per criterion"), "{out}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(p.git(&["for-each-ref", "refs/familiar-ai"]), "", "nothing changed, nothing kept");
}

#[test]
fn pushed_commits_are_never_touched() {
    // setup and the hand-made commit pushed: only the attempts are rewritten
    let p = Project::walked("pushed", TEST, Some("hand"));
    let pushed = p.git(&["rev-parse", "origin/work"]);
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 the fix"]);
    assert_eq!(p.git(&["rev-parse", "origin/work"]), pushed, "the remote's branch is where it was");
    assert_eq!(p.git(&["rev-parse", "HEAD~2"]), pushed, "the pushed commits are the same commits");

    // the seal and a failed attempt pushed: the criterion's seal is not
    // among the unpushed commits, so nothing is rewritten
    let p = Project::walked("sealpushed", TEST, Some("try 1"));
    let head = p.git(&["rev-parse", "HEAD"]);
    let (code, out, _) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nothing to tidy: no criterion has both its seal and its green among the unpushed commits"), "{out}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn a_mismatch_in_the_recheck_leaves_the_branch_at_its_original_head() {
    // the test passes whenever the file `override` is there: put there after
    // green, the tests no longer fail at the seal
    let p = Project::walked("mismatch", "[ -f override ] || sh src/app.sh | grep -qx hello\n", None);
    let head = p.git(&["rev-parse", "HEAD"]);
    let status = p.git(&["status", "--porcelain", "--untracked-files=no"]);
    let plan = p.read(".familiar/plan");
    std::fs::write(p.repo().join("override"), "").expect("override");
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("\"greet says hello\": the tests should fail at the seal") && err.contains("nothing was changed: work is at"), "{err}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head, "the branch is at its original head");
    assert_eq!(p.git(&["symbolic-ref", "--short", "HEAD"]), "work", "the work tree is back on the branch");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 try 1", "c1 try 2", "c1 the fix"]);
    let kept = p.git(&["for-each-ref", "--format=%(objectname)", "refs/familiar-ai/pre-tidy"]);
    assert_eq!(kept, head, "the original head is kept as a ref too");
    assert_eq!(p.git(&["status", "--porcelain", "--untracked-files=no"]), status);
    assert_eq!(p.read(".familiar/plan"), plan, "the plan file as it was");
    assert!(p.statusline().contains(" · 1 green"));
}

#[test]
fn uncommitted_changes_refuse_and_change_nothing() {
    let p = Project::walked("dirty", TEST, None);
    let head = p.git(&["rev-parse", "HEAD"]);
    std::fs::write(p.repo().join("README"), "changed\n").expect("edit");
    let (code, _, err) = p.run(&["tidy"], "");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("the work tree has uncommitted changes (README), so nothing was tidied"), "{err}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(p.read("README"), "changed\n");
}

#[test]
fn a_commit_is_the_agents_only_when_it_landed_during_its_own_bash_call() {
    let p = Project::walked_as("by", TEST, None, false, false, &|_| {});
    let seen = p.seen_by();
    let agent = Some("agent".to_string());
    assert_eq!(seen, [("c1 tests".to_string(), agent.clone()), ("c1 the fix".to_string(), agent.clone())], "made inside the PostToolUse of a Bash call");
    // the same kind of commit, made in the person's terminal: noticed at the
    // next PreToolUse, at the next SessionStart, at the PostToolUse of an edit
    std::fs::write(p.repo().join("NOTES"), "1\n").expect("notes");
    p.hand_commit("NOTES", "hand: notes 1");
    std::fs::write(p.repo().join("NOTES"), "2\n").expect("notes");
    p.git(&["commit", "-qm", "hand: notes 2", "--", "NOTES"]);
    p.hook("SessionStart", json!({ "source": "resume" }));
    std::fs::write(p.repo().join("NOTES"), "3\n").expect("notes");
    p.git(&["commit", "-qm", "hand: notes 3", "--", "NOTES"]);
    p.hook("PostToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": "README" }, "tool_response": {} }));
    let seen = p.seen_by();
    assert_eq!(seen[2..], [("hand: notes 1".to_string(), None), ("hand: notes 2".to_string(), None), ("hand: notes 3".to_string(), None)], "{seen:?}");
    // and the agent's commit after them is its own again
    std::fs::write(p.repo().join("NOTES"), "4\n").expect("notes");
    p.commit("agent: notes 4");
    assert_eq!(p.seen_by().last(), Some(&("agent: notes 4".to_string(), agent)));
}

#[test]
fn a_hand_commit_mid_session_between_the_agents_attempts_is_kept() {
    let p = Project::walked_as("handmid", TEST, None, false, true, &|p| {
        std::fs::write(p.repo().join("NOTES"), "the person's own notes\n").expect("notes");
        p.hand_commit("NOTES", "hand: notes");
    });
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 try 1", "hand: notes", "c1 try 2", "c1 the fix"]);
    let before_tree = p.git(&["rev-parse", "HEAD^{tree}"]);
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "hand: notes", "c1 the fix"]);
    assert_eq!(p.git(&["rev-parse", "HEAD^{tree}"]), before_tree, "the same files at the end");
    assert_eq!(p.git(&["diff", "--name-only", "HEAD~2", "HEAD~1"]), "NOTES", "the person's commit as it was");
    assert_eq!(p.git(&["diff", "--name-only", "HEAD~1", "HEAD"]), ".familiar/plan\nsrc/app.sh", "the fix holds none of the person's files");
    assert!(out.contains("tidied work: 7 commits became 5 (2 failed attempts dropped, 3 commits kept as they were)"), "{out}");
    // the new commits' facts: the seal is reused; the person's commit, put
    // on the seal, carries no by; the fix carries the agent's
    let seen = p.seen_by();
    let agent = Some("agent".to_string());
    assert_eq!(seen[seen.len() - 2..], [("hand: notes".to_string(), None), ("c1 the fix".to_string(), agent)], "{seen:?}");
    assert!(p.statusline().contains(" · 1 green"), "{}", p.statusline());
    let head = p.git(&["rev-parse", "HEAD"]);
    let (code, out, _) = p.run(&["tidy"], "");
    assert!(code == 0 && out.contains("nothing to tidy: "), "{out}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head, "a second tidy changes nothing");
}

#[test]
fn a_hand_commit_on_the_criterions_files_is_kept_and_nothing_is_tidied() {
    let p = Project::walked_as("handsame", TEST, None, false, true, &|p| {
        std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("app");
        p.hand_commit("src/app.sh", "hand: app");
    });
    let head = p.git(&["rev-parse", "HEAD"]);
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("nothing to tidy: ") && out.contains("(\"hand: app\") changes files the agent's dropped commits also changed"), "{out}");
    assert_eq!(p.git(&["rev-parse", "HEAD"]), head, "nothing rewritten");
    assert_eq!(p.log(), ["base", "familiar-ai setup", "hand: readme", "c1 tests", "c1 try 1", "hand: app", "c1 try 2", "c1 the fix"]);
    assert_eq!(p.git(&["for-each-ref", "refs/familiar-ai"]), "", "nothing changed, nothing kept");
}
