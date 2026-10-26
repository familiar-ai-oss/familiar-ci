//! The status line shows each ask by its words with a map of its parts, and
//! `setup` adds the setting without disturbing the person's own.

use super::*;
use std::time::Duration;
use familiar_ai_rules::sample::criterion_fixture;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("familiar-ai-statusline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

fn folder(root: &Path) -> String {
    root.file_name().and_then(|n| n.to_str()).expect("a folder name").to_string()
}

#[test]
fn a_two_criteria_record_is_the_ask_by_its_words_and_a_map_of_its_parts() {
    let log = criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture");
    let root = scratch("ask");
    let l = line(&log, &root, false);
    // The ask's words are whole, and each part shows by number and state, never by id.
    let name = folder(&root);
    assert_eq!(l, format!("{name} | Familiar | ON | your ask: let a person log in and reset a forgotten password · 1 testing · 2 waits"));
    assert!(!l.contains('…') && !l.contains('"') && !l.contains("c1"), "{l}");
    // With no run ever opened, the line is idle.
    assert_eq!(line(&Vec::new(), &root, false), format!("{name} | Familiar | ON | idle"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_off_switch_prints_the_codebase_familiar_and_off_with_nothing_after() {
    let root = scratch("off");
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    let log = familiar_ai_facts::append_raw(&log, "familiar_switch", serde_json::Map::from_iter([("to".to_string(), Value::from("off"))]), 1.0, "t").expect("off");
    assert!(crate::door::switch::off_in(&log), "the record says off");
    assert_eq!(line(&log, &root, false), format!("{} | Familiar | OFF", folder(&root)));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_open_card_is_its_part_needing_you() {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    let outside = familiar_ai_rules::moves::criterion_admissible(&View::new(&log))
        .into_iter()
        .find(|m| m.label.as_deref() == Some("outside_edit_seen"))
        .expect("the start check with the lines moved is offered");
    let log = familiar_ai_rules::sample::apply_move(&log, &outside, 1.0, "t").expect("applied");
    let l = line(&log, &scratch("card"), false);
    assert!(l.starts_with(&format!("{} | Familiar | ON | ", folder(&scratch("card")))), "{l}");
    assert!(l.ends_with(" · 1 needs you"), "{l}");
    assert!(!l.contains("waiting on you"), "{l}");
}

#[test]
fn parts_in_the_same_state_fold_into_a_run() {
    let mut log = criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture");
    for i in 3..=9 {
        let fields = serde_json::json!({ "criterion_id": format!("c{i}"), "text": format!("part number {i} has a long text"), "path": "light" });
        let Value::Object(m) = fields else { panic!("object") };
        log = familiar_ai_facts::append_raw(&log, "criterion", m, 0.0, "t").expect("criterion");
    }
    let root = scratch("fold");
    let l = line(&log, &root, false);
    assert!(l.starts_with(&format!("{} | Familiar | ON | ", folder(&root))), "{l}");
    // With c3 to c9 unplanned the plan is incomplete, so every part is planning.
    assert!(l.ends_with(": let a person log in and reset a forgotten password · 1–9 planning"), "{l}");
    assert!(!l.contains('…') && !l.contains("more") && !l.contains("c3"), "{l}");
}

#[test]
fn outside_any_repository_nothing_is_shown_and_the_input_names_the_folder() {
    assert_eq!(shown(&std::env::temp_dir(), false, Duration::ZERO), None);
    assert_eq!(from_input(r#"{"cwd":"/a","workspace":{"current_dir":"/b","project_dir":"/c"}}"#), Some(PathBuf::from("/c")));
    assert_eq!(from_input(r#"{"cwd":"/a","workspace":{"current_dir":"/b"}}"#), Some(PathBuf::from("/b")));
    assert_eq!(from_input(r#"{"cwd":"/a"}"#), Some(PathBuf::from("/a")));
    assert_eq!(from_input("not json"), None);
}

#[test]
fn setup_adds_the_setting_only_when_none_is_set_and_keeps_the_persons_own() {
    let root = scratch("offer");
    let path = root.join(".claude").join("settings.json");
    let added = offer(&root).expect("added");
    assert_eq!(added, format!("added the familiar-ai status line to {}", path.display()));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(v, serde_json::json!({ "statusLine": { "type": "command", "command": "familiar-ai statusline" } }));
    assert_eq!(offer(&root).expect("again"), format!("the familiar-ai status line is already set in {}", path.display()));

    std::fs::write(&path, "{\n  \"zeta\": 1,\n  \"alpha\": {\"b\": 2}\n}\n").expect("own");
    offer(&root).expect("added beside");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.find("zeta").expect("z") < text.find("alpha").expect("a"), "the person's order is kept: {text}");
    let v: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(v["statusLine"]["command"], "familiar-ai statusline");
    assert_eq!(v["alpha"]["b"], 2);

    std::fs::write(&path, r#"{"statusLine":{"type":"command","command":"my-line"}}"#).expect("theirs");
    assert!(offer(&root).expect("left").starts_with("a status line of your own is already set"));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), r#"{"statusLine":{"type":"command","command":"my-line"}}"#);

    std::fs::write(&path, "[1, 2]").expect("not an object");
    assert!(offer(&root).expect("left").ends_with("is not a JSON object, so no status line was added to it"));
    std::fs::write(&path, "{}").expect("empty object");
    offer(&root).expect("added");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(v["statusLine"]["type"], "command");
    let _ = std::fs::remove_dir_all(&root);
}

/// A project folder whose record another thread holds open for `hold`.
fn held_project(name: &str, hold: Duration) -> (PathBuf, std::thread::JoinHandle<()>) {
    let root = scratch(name);
    std::fs::create_dir_all(root.join(FAMILIAR_FOLDER)).expect("folder");
    let held = Record::open(&root.join(FAMILIAR_FOLDER).join(RECORD_FILE)).expect("open");
    let t = std::thread::spawn(move || {
        std::thread::sleep(hold);
        drop(held);
    });
    (root, t)
}

#[test]
fn a_record_held_a_moment_is_waited_for_and_the_real_line_shows() {
    // The line waits its turn at the record rather than reporting it busy.
    let (root, t) = held_project("held-briefly", Duration::from_millis(150));
    let l = shown(&root, false, Duration::from_secs(2));
    t.join().expect("thread");
    assert_eq!(l.as_deref(), Some(format!("{} | Familiar | ON | idle", folder(&root)).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_record_held_past_the_wait_says_it_is_in_use_by_another_step() {
    let (root, t) = held_project("held-long", Duration::from_millis(1500));
    let l = shown(&root, false, Duration::from_millis(100));
    t.join().expect("thread");
    assert_eq!(l.as_deref(), Some(format!("{} | Familiar | ON | its record is in use by another step", folder(&root)).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

/// The record with one choice of how `runner` is read (plan A, lane a-surf).
fn chose(log: &Log, runner: &str, reader: &str, gaps: &[&str]) -> Log {
    let f = serde_json::json!({
        "run_id": format!("d-{}", log.len()), "root": "", "runner": runner, "reader": reader, "command": "python -m pytest",
        "commit": "abc", "familiar": "0.1.0", "watched": [], "gaps": gaps, "why": "the dry run"
    });
    let Value::Object(m) = f else { panic!("object") };
    familiar_ai_facts::append_raw(log, "reader_chosen", m, 1.0, "t").expect("reader_chosen")
}

#[test]
fn the_reading_is_a_short_segment_after_the_map() {
    let log = criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture");
    let root = scratch("reading");
    let map = format!("{} | Familiar | ON | your ask: let a person log in and reset a forgotten password · 1 testing · 2 waits", folder(&root));
    assert_eq!(line(&log, &root, false), map, "no choice, no segment");
    assert_eq!(line(&chose(&log, "pytest", "pytest:reporter", &[]), &root, false), format!("{map} · reading: pytest"));
    assert_eq!(line(&chose(&log, "pytest", "pytest:tables", &["P5", "P7"]), &root, false), format!("{map} · reading: pytest, 2 outcomes ask you"));
    assert_eq!(line(&chose(&log, "pytest", "pytest:tables", &["P5"]), &root, false), format!("{map} · reading: pytest, 1 outcome asks you"));
    assert_eq!(line(&chose(&log, "pytest", "none", &[]), &root, false), format!("{map} · reading: pytest, every run asks you"));
    let two = chose(&chose(&log, "pytest", "pytest:reporter", &[]), "jest", "jest:tables", &["P6"]);
    assert_eq!(line(&two, &root, false), format!("{map} · reading: jest, 1 outcome asks you; pytest"));
    // before any ask, the reading still shows; switched off, nothing does
    let idle = chose(&Vec::new(), "pytest", "pytest:reporter", &[]);
    assert_eq!(line(&idle, &root, false), format!("{} | Familiar | ON | idle · reading: pytest", folder(&root)));
    let off = familiar_ai_facts::append_raw(&idle, "familiar_switch", serde_json::Map::from_iter([("to".to_string(), Value::from("off"))]), 2.0, "t").expect("off");
    assert_eq!(line(&off, &root, false), format!("{} | Familiar | OFF", folder(&root)));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_dry_run_going_is_the_reading_segment() {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_secs_f64();
    let f = serde_json::json!({"run_id": "d-1", "criterion_id": "", "criterion_rev": "", "commit": "K", "command": "pytest", "resources": ["tests"], "wait": "600", "by": "dry_run"});
    let Value::Object(m) = f else { panic!("object") };
    let dry = familiar_ai_facts::append_raw(&log, "run_reserved", m, now, "t").expect("the dry run");
    let root = scratch("reading-dry");
    let l = line(&dry, &root, false);
    assert!(l.ends_with(" · reading: dry run going"), "{l}");
    let _ = std::fs::remove_dir_all(&root);
}
