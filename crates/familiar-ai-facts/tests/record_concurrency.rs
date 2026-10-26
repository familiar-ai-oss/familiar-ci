//! Many processes appending to one record at once each wait their turn, and every fact lands once, in order.
//! Each child reruns this test binary with `FAMILIAR_AI_TEST_RECORD` set and appends one fact.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use familiar_ai_facts::{Record, RecordError};
use serde_json::json;

const CHILD_ENV: &str = "FAMILIAR_AI_TEST_RECORD";

fn fields(id: &str) -> serde_json::Map<String, serde_json::Value> {
    json!({ "criterion_id": id }).as_object().cloned().unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("familiar-ai-turns-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// The child's work. A no-op when run as an ordinary test.
#[test]
fn child_appends_one_fact() {
    let Ok(path) = std::env::var(CHILD_ENV) else {
        return;
    };
    let who = std::env::var("FAMILIAR_AI_TEST_WHO").unwrap();
    let rec =
        Record::open_waiting(&PathBuf::from(path), Duration::from_secs(10)).expect("open in turn");
    rec.append_raw("criterion_head", fields(&who), 1.0, "_default_")
        .expect("append");
}

#[test]
fn twenty_at_once_all_land_in_order_with_no_gap_or_repeat() {
    let path = scratch("twenty");
    let exe = std::env::current_exe().unwrap();
    let children: Vec<_> = (0..20)
        .map(|i| {
            Command::new(&exe)
                .args(["--exact", "child_appends_one_fact", "--test-threads=1"])
                .env(CHILD_ENV, &path)
                .env("FAMILIAR_AI_TEST_WHO", format!("c{i}"))
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for (i, child) in children.into_iter().enumerate() {
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "child {i} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let rec = Record::open(&path).unwrap();
    let all = rec.read_all().unwrap();
    let positions: Vec<u64> = all.iter().map(|f| f.t).collect();
    assert_eq!(positions, (0..20).collect::<Vec<u64>>());
    let mut who: Vec<String> = all
        .iter()
        .map(|f| f.criterion_id().unwrap().to_string())
        .collect();
    who.sort();
    who.dedup();
    assert_eq!(who.len(), 20, "every child's fact is there exactly once");
    drop(rec);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_held_record_is_busy_not_broken() {
    let path = scratch("busy");
    let holder = Record::open(&path).unwrap();
    holder
        .append_raw("criterion_head", fields("c1"), 1.0, "_default_")
        .unwrap();
    // no wait: busy at once
    assert!(matches!(Record::open(&path), Err(RecordError::Busy)));
    // a short wait runs out: still busy, and it did wait
    let started = std::time::Instant::now();
    assert!(matches!(
        Record::open_waiting(&path, Duration::from_millis(150)),
        Err(RecordError::Busy)
    ));
    assert!(started.elapsed() >= Duration::from_millis(150));
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(holder);
    // released: opens, and the fact written before is intact
    let rec = Record::open_waiting(&path, Duration::from_secs(2)).unwrap();
    assert_eq!(rec.len().unwrap(), 1);
    drop(rec);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_waiter_gets_in_when_the_holder_lets_go() {
    let path = scratch("handover");
    let holder = Record::open(&path).unwrap();
    let p2 = path.clone();
    let waiter = std::thread::spawn(move || {
        Record::open_waiting(&p2, Duration::from_secs(5)).map(|r| r.len())
    });
    std::thread::sleep(Duration::from_millis(200));
    drop(holder);
    assert!(waiter.join().unwrap().is_ok());
    let _ = std::fs::remove_file(&path);
}
