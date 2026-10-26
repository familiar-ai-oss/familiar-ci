//! `.familiar/adapter` (plan A, lane a-py): the record's choices of how each runner is read,
//! mirrored as JSON Lines sorted by (root, runner), the latest choice each, reader ids only,
//! byte-stable; written by the binary beside the plan, never by hand. `status` shows the reading
//! in one line and `status --json` as `reading`, beside `ci`.

mod common;

use common::Project;
use familiar_ai::door::{adapter_lines, project_adapter, FAMILIAR_FOLDER};
use familiar_ai_facts::{append_raw, Log, Record};
use serde_json::{json, Value};

fn chosen(log: &Log, root: &str, runner: &str, reader: &str, gaps: &[&str]) -> Log {
    let f = json!({
        "run_id": format!("d-{}", log.len()), "root": root, "runner": runner, "reader": reader, "command": "python -m pytest",
        "commit": "abc", "familiar": "0.1.0", "watched": ["pytest.ini"], "gaps": gaps, "why": "the dry run"
    });
    append_raw(log, "reader_chosen", f.as_object().cloned().unwrap(), log.len() as f64 + 1.0, "t").expect("reader_chosen")
}

#[test]
fn the_adapter_names_the_latest_reader_per_root_and_runner_sorted_and_byte_stable() {
    let log = chosen(&Vec::new(), "sub", "pytest", "pytest:tables", &["P5"]);
    let log = chosen(&log, "", "pytest", "pytest:tables", &[]);
    let log = chosen(&log, "", "pytest", "pytest:reporter", &[]);
    assert_eq!(
        adapter_lines(&log),
        "{\"root\":\"\",\"runner\":\"pytest\",\"reader\":\"pytest:reporter\",\"familiar\":\"0.1.0\",\"gaps\":[]}\n\
         {\"root\":\"sub\",\"runner\":\"pytest\",\"reader\":\"pytest:tables\",\"familiar\":\"0.1.0\",\"gaps\":[\"P5\"]}\n"
    );
    assert_eq!(adapter_lines(&Vec::new()), "", "no choice, no line");
}

#[test]
fn the_adapter_file_is_written_beside_the_plan_only_once_there_is_a_choice() {
    let dir = std::env::temp_dir().join(format!("familiar-ai-adapter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(FAMILIAR_FOLDER)).unwrap();
    project_adapter(&dir, &Vec::new()).expect("nothing to write");
    assert!(!dir.join(FAMILIAR_FOLDER).join("adapter").exists(), "no file before a choice");
    let log = chosen(&Vec::new(), "", "pytest", "pytest:reporter", &[]);
    project_adapter(&dir, &log).expect("written");
    let text = std::fs::read_to_string(dir.join(FAMILIAR_FOLDER).join("adapter")).expect("the adapter");
    assert_eq!(text, adapter_lines(&log));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_reading_line_says_how_each_runner_is_read() {
    use familiar_ai::teach::reading_line;
    assert_eq!(reading_line(&Vec::new(), 1.0), None, "nothing chosen, nothing running");
    let log = chosen(&Vec::new(), "", "pytest", "pytest:reporter", &[]);
    assert_eq!(reading_line(&log, 9.0).as_deref(), Some("reading: pytest by Familiar's own reporter"));
    // A-7 (lane a-surf): each gap is the outcome that will ask, in words, never a probe's id
    let log = chosen(&Vec::new(), "", "pytest", "pytest:tables", &["P5"]);
    assert_eq!(reading_line(&log, 9.0).as_deref(), Some("reading: pytest by its output tables; asks you on: a pass beside a skipped test"));
    let log = chosen(&Vec::new(), "sub", "pytest", "pytest:tables", &["P5", "P7"]);
    assert_eq!(
        reading_line(&log, 9.0).as_deref(),
        Some("reading: pytest in sub by its output tables; asks you on: a pass beside a skipped test, an error raised in the code")
    );
    let log = chosen(&Vec::new(), "", "phpunit", "phpunit:report", &[]);
    assert_eq!(reading_line(&log, 9.0).as_deref(), Some("reading: phpunit by its report files"));
    let log = chosen(&Vec::new(), "", "pytest", "none", &[]);
    assert_eq!(reading_line(&log, 9.0).as_deref(), Some("reading: pytest by no reader: a plugin or setting changes what its runs print, so every run asks you"));
    // CI's own choice (`ran_in: pipeline`) is CI's; the laptop's reading stays its own
    let laptop = chosen(&Vec::new(), "", "pytest", "pytest:reporter", &[]);
    let mut f = json!({
        "run_id": "d-ci", "root": "", "runner": "pytest", "reader": "pytest:tables", "command": "python -m pytest",
        "commit": "abc", "familiar": "0.1.0", "watched": [], "gaps": ["P5"], "why": "the dry run", "ran_in": "pipeline"
    });
    let ci = append_raw(&laptop, "reader_chosen", f.as_object_mut().cloned().unwrap(), 50.0, "t").expect("CI's choice");
    assert_eq!(reading_line(&ci, 60.0).as_deref(), Some("reading: pytest by Familiar's own reporter"));
    let dry = append_raw(
        &Vec::new(),
        "run_reserved",
        json!({"run_id": "d-9", "criterion_id": "", "criterion_rev": "", "commit": "K", "command": "pytest", "resources": ["tests"], "wait": "100", "by": "dry_run"}).as_object().cloned().unwrap(),
        1.0,
        "t",
    )
    .unwrap();
    let line = reading_line(&dry, 2.0).expect("a line while the dry run runs");
    assert!(line.contains("dry run") && line.contains("d-9"), "{line}");
}

#[test]
fn status_shows_the_reading_beside_ci() {
    let p = Project::builder("adapter-status").session("5c67ae1d-1111-4222-8333-944455566600").create();
    let record = Record::open(&p.path(".familiar/record")).expect("the record");
    let f = json!({
        "run_id": "d-1", "root": "", "runner": "pytest", "reader": "pytest:reporter", "command": "python -m pytest",
        "commit": "abc", "familiar": "0.1.0", "watched": [], "gaps": [], "why": "the dry run"
    });
    record.append_raw("reader_chosen", f.as_object().cloned().unwrap(), 1.0, "familiar-ai").expect("append");
    drop(record);
    let (code, out, err) = p.run(&["status", "--json"], "");
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("one JSON object");
    assert!(v["reading"].as_str().is_some_and(|r| r.contains("Familiar's own reporter")), "{}", v["reading"]);
    let (code, out, err) = p.run(&["status"], "");
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().filter(|l| l.contains("Familiar's own reporter")).count(), 1, "one line: {out}");
}
