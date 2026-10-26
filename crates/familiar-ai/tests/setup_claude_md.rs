//! `familiar-ai setup` adds the familiar-ai lines to the end of the project's
//! CLAUDE.md: a plugin cannot ship a CLAUDE.md, and it is what an agent re-reads
//! after compaction. What is above them is left byte for byte; a file that has
//! them already is not touched; the lines ride in setup's own commit.

mod common;

use common::Scratch;

/// The lines, as the owner wrote them.
const LINES: &str = "## familiar-ai

This project is governed by the familiar-ai plugin: proof that the work
is done, each requirement by a test that failed first, was not changed,
and passes. The rulebook arrives in your context at the session's start
and again after compaction and resume, as lines beginning
`familiar-ai:`; they are the binary's facts. Follow them. When a hook
refuses, the `familiar-ai:plan` skill says what to do.
`familiar-ai status` is the only picture of where the work stands; the
record is a database file, never opened by hand. The switch
(`familiar-ai on` / `off`) and the question cards are the person's.
";

fn repo(name: &str, claude_md: Option<&str>) -> Scratch {
    let p = Scratch::new(name);
    p.git_init();
    if let Some(text) = claude_md {
        std::fs::write(p.dir.join("CLAUDE.md"), text).expect("write");
        p.git(&["add", "CLAUDE.md"]);
        p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    }
    p
}

fn setup(p: &Scratch) -> String {
    let (code, out, err) = p.run(&["setup"]);
    assert_eq!(code, 0, "{out}{err}");
    out
}

#[test]
fn a_project_with_no_claude_md_gets_one_with_the_lines_alone_in_setups_commit() {
    let p = repo("claude-md-none", None);
    let out = setup(&p);
    assert!(out.contains(&format!("familiar-ai: added the familiar-ai lines to {}\n", p.dir.join("CLAUDE.md").display())), "{out}");
    assert_eq!(p.read("CLAUDE.md"), LINES);
    assert_eq!(p.git(&["show", "--format=", "--name-only", "HEAD"]), ".claude/settings.json\n.gitignore\nCLAUDE.md");
    assert_eq!(p.git(&["status", "--porcelain"]), "");
    // again: nothing is written
    let out = setup(&p);
    assert!(out.contains("familiar-ai: CLAUDE.md already carries the familiar-ai lines\n"), "{out}");
    assert_eq!(p.read("CLAUDE.md"), LINES);
    assert_eq!(p.git(&["log", "--format=%s"]), "familiar-ai setup");
}

#[test]
fn the_lines_go_at_the_end_after_one_blank_line_and_nothing_above_changes() {
    // a file that ends in a newline
    let mine = "# my project\n\nBuild with make.\n";
    let p = repo("claude-md-newline", Some(mine));
    setup(&p);
    assert_eq!(p.read("CLAUDE.md"), format!("{mine}\n{LINES}"));
    assert_eq!(p.git(&["show", "--format=%s", "--name-only", "HEAD"]), "familiar-ai setup\n\n.claude/settings.json\n.gitignore\nCLAUDE.md");
    assert_eq!(p.git(&["status", "--porcelain"]), "");
    // a file that does not end in a newline gets one first
    let mine = "# my project\n\n  keep   these spaces\t";
    let p = repo("claude-md-no-newline", Some(mine));
    setup(&p);
    assert_eq!(p.read("CLAUDE.md"), format!("{mine}\n\n{LINES}"));
}

#[test]
fn a_claude_md_that_has_the_heading_is_left_as_it_is() {
    let mine = "# my project\n\n## familiar-ai\n\nmy own words about it\n";
    let p = repo("claude-md-has", Some(mine));
    let out = setup(&p);
    assert!(out.contains("familiar-ai: CLAUDE.md already carries the familiar-ai lines\n"), "{out}");
    assert_eq!(p.read("CLAUDE.md"), mine);
    assert_eq!(p.git(&["show", "--format=", "--name-only", "HEAD"]), ".claude/settings.json\n.gitignore", "CLAUDE.md is not in the commit");
}

#[test]
fn an_uncommitted_change_of_the_persons_own_refuses_and_an_earlier_setups_lines_are_committed() {
    let p = repo("claude-md-own", Some("# my project\n"));
    std::fs::write(p.dir.join("CLAUDE.md"), "# my project\n\nwork in progress\n").expect("the person's own edit");
    let (code, _, err) = p.run(&["setup"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("CLAUDE.md has uncommitted changes of your own"), "{err}");
    assert_eq!(p.read("CLAUDE.md"), "# my project\n\nwork in progress\n");
    assert!(!p.dir.join(".familiar").exists(), "nothing was made");

    // an earlier setup's lines, not yet committed, are setup's own
    std::fs::write(p.dir.join("CLAUDE.md"), format!("# my project\n\n{LINES}")).expect("an older setup's lines");
    let out = setup(&p);
    assert!(out.contains("familiar-ai: CLAUDE.md already carries the familiar-ai lines\n"), "{out}");
    assert_eq!(p.git(&["show", "--format=%s", "--name-only", "HEAD"]), "familiar-ai setup\n\n.claude/settings.json\n.gitignore\nCLAUDE.md");
    assert_eq!(p.git(&["status", "--porcelain"]), "");
}

#[test]
fn a_claude_md_git_ignores_stays_out_of_the_commit() {
    let p = Scratch::new("claude-md-ignored");
    p.git_init();
    std::fs::write(p.dir.join(".gitignore"), "CLAUDE.md\n").expect("write");
    p.git(&["add", ".gitignore"]);
    p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"]);
    let out = setup(&p);
    assert!(out.contains("familiar-ai: CLAUDE.md is ignored by git, so it stays out of the setup commit\n"), "{out}");
    assert_eq!(p.read("CLAUDE.md"), LINES);
    assert_eq!(p.git(&["show", "--format=", "--name-only", "HEAD"]), ".claude/settings.json\n.gitignore");
}
