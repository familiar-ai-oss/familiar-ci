//! A run at a commit builds in its copy's own target folder, never the
//! project's: test binaries there would bake in the copy's path and fail to
//! spawn once the copy is gone.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, TENANT};
use familiar_ai_facts::{
    AnchorShaMatched, CheckedAt, Criterion, CriterionAssigned, CriterionHead, FixLocation, FixLocationCategory, IntentSubmitted,
    Path as CriterionPath, Place, PlaceChosen, Record, TestPlanned, TestStackDetected, Will,
};
use serde_json::{json, Value};

const SESSION: &str = "5c67ae1d-1111-4222-8333-944455566677";
const MAIN: &str = "fn main() { println!(\"hi\"); }\n";
const FIXED: &str = "fn main() { println!(\"hello\"); }\n";
const CARGO: &str = "[package]\nname = \"fx\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const TEST: &str = "#[test]\nfn says_hello() {\n    let out = std::process::Command::new(env!(\"CARGO_BIN_EXE_fx\")).output().expect(\"spawn\");\n    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), \"hello\");\n}\n";
const COMMAND: &str = "cargo test -q --test spawn";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().no_setup().file(".gitignore", ".familiar/record\ntarget\n").file("Cargo.toml", CARGO).file("src/main.rs", MAIN).env_remove("CARGO_TARGET_DIR").create()
}

impl Project {
    fn record(&self) -> Record {
        Record::open_waiting(&self.record_path(), std::time::Duration::from_secs(5)).expect("record")
    }
}

/// The plan as facts in the record, with a test command that asks git.
fn write_plan(r: &Project) {
    std::fs::create_dir_all(r.path(FAMILIAR_FOLDER)).expect("folder");
    let base = r.head();
    let record = r.record();
    let lines_hash = familiar_ai_git::lines_hash(&r.repo(), &base, "src/main.rs", 1, 1).expect("lines hash");
    let at = 1.0;
    record.append(&IntentSubmitted { text: "greet with hello".into(), sha: base.clone(), for_t: None }, at, TENANT).expect("intent");
    record
        .append(
            &Criterion { criterion_id: "c1".into(), text: "greet says hello".into(), path: CriterionPath::Light, behavior_id: None, promise_id: None, for_t: None },
            at,
            TENANT,
        )
        .expect("criterion");
    record
        .append(
            &PlaceChosen {
                criterion_id: "c1".into(),
                places: vec![Place { file_path: "src/main.rs".into(), symbol: "main".into(), line_start: 1, line_end: 1 }],
                hold: None,
                choice: None,
            },
            at,
            TENANT,
        )
        .expect("place");
    record
        .append(
            &AnchorShaMatched {
                criterion_id: "c1".into(),
                file_path: "src/main.rs".into(),
                symbol: "main".into(),
                line_start: 1,
                line_end: 1,
                sha: base.clone(),
                lines_hash,
                matched: true,
                checked_at: CheckedAt::Planning,
                tip_matched: None,
            },
            at,
            TENANT,
        )
        .expect("check");
    record
        .append(&TestPlanned { criterion_id: "c1".into(), file_path: "tests/spawn.rs".into(), will: Will::Create, test_command: None }, at, TENANT)
        .expect("test planned");
    record
        .append(
            &TestStackDetected { codebase_id: "cb1".into(), project_root: "".into(), test_stack: "cargo".into(), test_command: COMMAND.into() },
            at,
            TENANT,
        )
        .expect("stack");
    record.append(&CriterionHead { criterion_id: "c1".into() }, at, TENANT).expect("head");
    record.append(&CriterionAssigned { criterion_id: "c1".into(), agent_conversation_id: SESSION.into() }, at, TENANT).expect("assigned");
    record
        .append(&FixLocationCategory { criterion_id: "c1".into(), category: FixLocation::ThisRepository, where_: String::new(), needs: None }, at, TENANT)
        .expect("claim");
}

fn carries(dir: &std::path::Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            found.extend(carries(&p, needle));
        } else if std::fs::read(&p).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle)) {
            found.push(p);
        }
    }
    found
}

#[test]
fn a_run_at_a_commit_never_builds_into_the_projects_target_so_no_test_binary_carries_the_copys_path() {
    let r = project("run");
    write_plan(&r);
    r.write("tests/spawn.rs", TEST);
    r.land("c1: tests first");
    // the project already has a target folder
    let built = Command::new("cargo").current_dir(r.repo()).env_remove("CARGO_TARGET_DIR").args(["test", "-q", "--offline", "--no-run"]).output().expect("cargo build");
    assert!(built.status.success(), "the project's own build: {}", String::from_utf8_lossy(&built.stderr));
    let mut post = r.fixture("post_tool_use_bash", json!({}));
    post["tool_input"]["command"] = Value::from("git add -A && git commit -m 'c1: tests first'");
    r.hook_with("PostToolUse", &post);
    r.write("src/main.rs", FIXED);
    r.land("c1: fix");

    let cargo = Command::new("cargo")
        .current_dir(r.repo())
        .env_remove("CARGO_TARGET_DIR")
        .args(["test", "-q", "--offline"])
        .output()
        .expect("cargo test");
    let text = format!("{}{}", String::from_utf8_lossy(&cargo.stdout), String::from_utf8_lossy(&cargo.stderr));
    assert!(cargo.status.success(), "cargo test in the project: {text}");
    let dirty = carries(&r.repo().join("target/debug/deps"), b"familiar-ai-run-");
    assert!(dirty.is_empty(), "the copy's path is baked into: {dirty:?}");
}
