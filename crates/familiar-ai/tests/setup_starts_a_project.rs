//! `familiar-ai setup` makes the folder, its ignore lines and the marker, and
//! is safe to run again; `status` and `claim` outside a project refuse with
//! one line.

mod common;

use common::{run_binary, Scratch};

#[test]
fn setup_in_a_fresh_repository_makes_the_folder_and_the_ignore_line_then_says_already() {
    let p = Scratch::new("setup-fresh");
    p.git_init();
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "");
    let lines: Vec<&str> = out.lines().map(|l| l.strip_prefix("familiar-ai: ").expect("every line is familiar-ai's")).collect();
    assert_eq!(lines.len(), 11, "{out}");
    assert!(lines[0].starts_with("made .familiar at "), "{out}");
    assert!(lines[1].starts_with("marked this project as set up at "), "{out}");
    assert!(lines[2].starts_with("added .familiar/record to "), "{out}");
    assert!(lines[3].starts_with("added .familiar/judge.log to "), "{out}");
    assert!(lines[4].starts_with("added .familiar/investigator.log to "), "{out}");
    assert!(lines[5].starts_with("added .familiar/runs to "), "{out}");
    assert!(lines[6].starts_with("added .familiar/investigator to "), "{out}");
    assert!(lines[7].starts_with("added .familiar/verify-report.json to "), "{out}");
    assert_eq!(lines[8], format!("added the familiar-ai status line to {}", p.dir.join(".claude/settings.json").display()), "{out}");
    assert_eq!(lines[9], format!("added the familiar-ai lines to {}", p.dir.join("CLAUDE.md").display()), "{out}");
    // setup is its own commit
    assert!(lines[10].starts_with("committed .gitignore and .claude/settings.json and CLAUDE.md as one commit \"familiar-ai setup\" ("), "{out}");
    assert_eq!(p.git(&["log", "--format=%s"]), "familiar-ai setup");
    assert_eq!(p.git(&["show", "--format=", "--name-only", "HEAD"]), ".claude/settings.json\n.gitignore\nCLAUDE.md");
    assert_eq!(p.git(&["status", "--porcelain"]), "", "the work tree has no other change from setup");
    let settings: serde_json::Value = serde_json::from_str(&p.read(".claude/settings.json")).expect("settings");
    assert_eq!(settings, serde_json::json!({ "statusLine": { "type": "command", "command": "familiar-ai statusline" } }));
    let marker = std::path::PathBuf::from(lines[1].trim_start_matches("marked this project as set up at "));
    assert!(marker.starts_with(common::test_home().join("projects")), "{marker:?}");
    let body = std::fs::read_to_string(&marker).expect("the marker");
    assert!(body.starts_with(&format!("root: {}\nat: ", std::fs::canonicalize(&p.dir).expect("canon").display())), "{body}");
    assert!(p.dir.join(".familiar").is_dir());
    assert_eq!(std::fs::read_dir(p.dir.join(".familiar")).expect("dir").count(), 0, "nothing else is written");
    assert_eq!(p.read(".gitignore"), ".familiar/record\n.familiar/judge.log\n.familiar/investigator.log\n.familiar/runs\n.familiar/investigator\n.familiar/verify-report.json\n");
    assert!(p.git_ignores(".familiar/record") && p.git_ignores(".familiar/judge.log") && p.git_ignores(".familiar/investigator.log"));
    assert!(p.git_ignores(".familiar/runs/0123abc.log") && p.git_ignores(".familiar/investigator/u1.jsonl"), "what is kept stays on the laptop");
    assert!(p.git_ignores(".familiar/verify-report.json"), "the verify report stays on the laptop");
    assert!(!p.git_ignores(".familiar/plan"), "the plan stays committable");

    // the second run changes nothing and says so
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{err}");
    let lines: Vec<&str> = out.lines().map(|l| l.strip_prefix("familiar-ai: ").expect("every line is familiar-ai's")).collect();
    assert_eq!(lines.len(), 11, "{out}");
    assert_eq!(lines[8], format!("the familiar-ai status line is already set in {}", p.dir.join(".claude/settings.json").display()), "{out}");
    assert_eq!(lines[9], "CLAUDE.md already carries the familiar-ai lines", "{out}");
    assert_eq!(lines[10], "already set up: setup has nothing to commit", "{out}");
    assert_eq!(p.git(&["log", "--format=%s"]), "familiar-ai setup", "no second commit");
    assert!(lines[0].starts_with(".familiar is already at "), "{out}");
    assert!(lines[1].starts_with("this project is already marked as set up at "), "{out}");
    assert_eq!(lines[2], ".familiar/record is already ignored by git", "{out}");
    assert_eq!(lines[3], ".familiar/judge.log is already ignored by git", "{out}");
    assert_eq!(lines[4], ".familiar/investigator.log is already ignored by git", "{out}");
    assert_eq!(lines[5], ".familiar/runs is already ignored by git", "{out}");
    assert_eq!(lines[6], ".familiar/investigator is already ignored by git", "{out}");
    assert_eq!(lines[7], ".familiar/verify-report.json is already ignored by git", "{out}");
    assert_eq!(p.read(".gitignore"), ".familiar/record\n.familiar/judge.log\n.familiar/investigator.log\n.familiar/runs\n.familiar/investigator\n.familiar/verify-report.json\n");
    assert_eq!(std::fs::read_to_string(&marker).expect("the marker"), body, "the marker is left as it was");

    // a session started here now shows the plan's picture, not the setup line
    let input = format!("{{\"hook_event_name\":\"SessionStart\",\"cwd\":\"{}\"}}", p.dir.display());
    let output = run_binary(&["hook", "SessionStart"], &input);
    let context = common::stdout(&output);
    assert!(context.contains("familiar-ai: the plan is still being written"), "{context}");
    assert!(!context.contains("not set up"), "{context}");
}

#[test]
fn setup_appends_to_an_existing_gitignore_and_leaves_one_that_already_covers_the_record() {
    let p = Scratch::new("setup-append");
    p.git_init();
    std::fs::write(p.dir.join(".gitignore"), "target/").expect("write"); // no final newline
    p.git(&["add", ".gitignore"]);
    p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    let (code, out, _) = p.run(&["setup"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(p.read(".gitignore"), "target/\n.familiar/record\n.familiar/judge.log\n.familiar/investigator.log\n.familiar/runs\n.familiar/investigator\n.familiar/verify-report.json\n");
    assert_eq!(p.git(&["status", "--porcelain"]), "", "setup's edits are all in its commit");

    let p = Scratch::new("setup-covered");
    p.git_init();
    std::fs::write(p.dir.join(".gitignore"), ".familiar/*\n!.familiar/plan\n!.familiar/adapter\n").expect("write");
    p.git(&["add", ".gitignore"]);
    p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    let (code, out, _) = p.run(&["setup"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(".familiar/record is already ignored by git") && out.contains(".familiar/judge.log is already ignored by git") && out.contains(".familiar/investigator.log is already ignored by git") && out.contains(".familiar/runs is already ignored by git") && out.contains(".familiar/investigator is already ignored by git") && out.contains(".familiar/verify-report.json is already ignored by git"), "{out}");
    assert_eq!(p.read(".gitignore"), ".familiar/*\n!.familiar/plan\n!.familiar/adapter\n", "already covered: not touched");
}

#[test]
fn setup_outside_a_repository_is_refused_on_one_line_and_makes_nothing() {
    let p = Scratch::new("setup-norepo");
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("not inside a git repository"), "{err}");
    assert_eq!(err.trim_end_matches('\n').matches('\n').count(), 0, "one line: {err:?}");
    assert!(!p.dir.join(".familiar").exists());
    assert!(!p.dir.join(".gitignore").exists());
}

#[test]
fn setup_refuses_when_the_plan_would_be_ignored_too() {
    let p = Scratch::new("setup-plan-ignored");
    p.git_init();
    std::fs::write(p.dir.join(".gitignore"), ".familiar/\n").expect("write");
    p.git(&["add", ".gitignore"]);
    p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    let (code, _, err) = p.run(&["setup"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(".familiar/plan is ignored by git"), "{err}");
    assert_eq!(p.git(&["log", "--format=%s"]), "base", "a refusal commits nothing");
}

#[test]
fn status_and_claim_outside_a_project_exit_1_naming_the_folder_on_one_line() {
    // a temp folder, since cargo's cwd is this checkout, which has its own `.familiar`
    let p = Scratch::new("status-norepo");
    for args in [vec!["status"], vec!["claim", "criterion_head", "--criterion_id", "c1"]] {
        let (code, out, err) = p.run(&args);
        assert_eq!(code, 1, "{args:?}");
        assert_eq!(out, "", "{args:?}");
        assert!(err.contains(".familiar"), "{args:?}: {err}");
        assert_eq!(err.trim_end_matches('\n').matches('\n').count(), 0, "{args:?}: one line: {err:?}");
    }
}

#[test]
fn setup_never_rides_with_work_staged_changes_or_a_changed_gitignore_refuse_and_nothing_is_made() {
    let p = Scratch::new("setup-staged");
    p.git_init();
    std::fs::write(p.dir.join("work.txt"), "the person's work\n").expect("write");
    p.git(&["add", "work.txt"]);
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("the index holds staged changes, and setup is its own commit that never rides with work"), "{err}");
    assert!(!p.dir.join(".familiar").exists() && !p.dir.join(".gitignore").exists(), "nothing was made");
    assert_eq!(p.git(&["status", "--porcelain"]), "A  work.txt", "nothing stashed, nothing unstaged");

    let p = Scratch::new("setup-own-gitignore");
    p.git_init();
    std::fs::write(p.dir.join(".gitignore"), "target/\n").expect("write");
    p.git(&["add", ".gitignore"]);
    p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    std::fs::write(p.dir.join(".gitignore"), "target/\nnode_modules/\n").expect("the person's own edit");
    let (code, _, err) = p.run(&["setup"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(".gitignore has uncommitted changes of your own"), "{err}");
    assert_eq!(p.read(".gitignore"), "target/\nnode_modules/\n");
    assert!(!p.dir.join(".familiar").exists());

    // an earlier setup's uncommitted lines are committed now
    std::fs::write(p.dir.join(".gitignore"), "target/\n.familiar/record\n").expect("an older setup's line");
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.git(&["log", "--format=%s"]), "familiar-ai setup\nbase");
    assert_eq!(p.git(&["status", "--porcelain"]), "");
}

#[test]
fn setup_in_a_subfolder_project_commits_the_subfolders_files_and_leaves_other_work_alone() {
    let p = Scratch::new("setup-sub");
    p.git_init();
    std::fs::create_dir_all(p.dir.join("pkg")).expect("pkg");
    std::fs::write(p.dir.join("pkg/a.py"), "x = 1\n").expect("write");
    std::fs::write(p.dir.join("notes.txt"), "unstaged, untracked work\n").expect("write");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
        .env("FAMILIAR_AI_JUDGE", "off")
        .env("FAMILIAR_AI_INVESTIGATOR", "off")
        .env("FAMILIAR_AI_HOME", common::test_home())
        .env("CLAUDE_PROJECT_DIR", p.dir.join("pkg"))
        .arg("setup")
        .current_dir(p.dir.join("pkg"))
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(p.git(&["show", "--format=%s", "--name-only", "HEAD"]), "familiar-ai setup\n\npkg/.claude/settings.json\npkg/.gitignore\npkg/CLAUDE.md");
    assert_eq!(p.git(&["status", "--porcelain"]), "?? notes.txt\n?? pkg/a.py");
}
