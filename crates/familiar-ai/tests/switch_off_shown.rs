//! While familiar-ai is off for a project, `familiar-ai statusline` prints
//! `OFF` and `familiar-ai status` opens with a line saying so. The band's
//! button is the mod's, tested in plugin/hooks.

mod common;

use common::Project;

/// The binary runs from the person's own terminal: no session id.
fn project(name: &str) -> Project {
    Project::builder(name).no_session().no_app().file("a.py", "x = 1\n").create()
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

#[test]
fn the_statusline_reads_familiar_off_while_off() {
    let p = project("bar");
    let bar = |p: &Project| p.run(&["statusline"], "").1;
    let name = p.repo().file_name().and_then(|n| n.to_str()).expect("a folder name").to_string();
    assert_eq!(bar(&p), format!("{name} | Familiar | ON | idle\n"));
    assert_eq!(p.run(&["off"], "").0, 0);
    assert_eq!(bar(&p), format!("{name} | Familiar | OFF\n"));
    assert_eq!(p.run(&["on"], "").0, 0);
    assert_eq!(bar(&p), format!("{name} | Familiar | ON | idle\n"));
}

#[test]
fn the_status_text_starts_with_off_then_the_picture() {
    let p = project("text");
    let (code, on_text, _) = p.run(&["status"], "");
    assert_eq!(code, 0);
    assert!(!on_text.contains("turned off"), "{on_text}");
    assert_eq!(p.run(&["off"], "").0, 0);
    let (code, off_text, _) = p.run(&["status"], "");
    assert_eq!(code, 0);
    assert_eq!(first_line(&off_text), "Familiar is turned off for this project.", "{off_text}");
    assert_eq!(off_text.lines().skip(1).collect::<Vec<_>>().join("\n"), on_text.trim_end_matches('\n'), "the picture as today");
    assert_eq!(p.run(&["on"], "").0, 0);
    assert_eq!(p.run(&["status"], "").1, on_text);
}
