//! `append_if` checks its predicate inside the inserting transaction, so of two racing writers only one is
//! accepted, and a false predicate writes nothing.

use std::sync::{Arc, Barrier};
use std::thread;

use familiar_ai_facts::{AppendIf, Record};
use serde_json::{json, Map, Value};

fn reserved(run_id: &str) -> Map<String, Value> {
    json!({
        "run_id": run_id, "criterion_id": "c1", "criterion_rev": "r1", "commit": "abc",
        "command": "true", "resources": [], "wait": "1800", "by": "binary"
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}

fn live_for_the_work(log: &[familiar_ai_facts::Fact]) -> bool {
    log.iter().any(|f| f.kind == "run_reserved" && f.fields.get("commit") == Some(&json!("abc")))
}

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("familiar-ai-append-if-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dir");
    d
}

#[test]
fn a_false_predicate_writes_nothing() {
    let dir = temp("false");
    let rec = Record::open(&dir.join("record")).expect("open");
    let got = rec.append_if_raw("run_reserved", reserved("r-1"), 1.0, "t", |_| false).expect("no error");
    assert!(matches!(got, AppendIf::Refused), "{got:?}");
    assert_eq!(rec.len().expect("len"), 0);
    let got = rec.append_if_raw("run_reserved", reserved("r-1"), 1.0, "t", |_| true).expect("no error");
    assert!(matches!(got, AppendIf::Appended(_)), "{got:?}");
    assert_eq!(rec.len().expect("len"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_writers_racing_for_the_same_work_get_exactly_one_reservation() {
    let dir = temp("race");
    let rec = Arc::new(Record::open(&dir.join("record")).expect("open"));
    for round in 0..20 {
        let work = format!("w{round}");
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let (rec, barrier, work) = (Arc::clone(&rec), Arc::clone(&barrier), work.clone());
                thread::spawn(move || {
                    barrier.wait();
                    let mut fields = reserved(&format!("{work}-{i}"));
                    fields.insert("commit".into(), json!(work.clone()));
                    rec.append_if_raw("run_reserved", fields, 1.0, "t", |log| {
                        !log.iter().any(|f| f.kind == "run_reserved" && f.fields.get("commit") == Some(&json!(work.clone())))
                    })
                    .map(|a| matches!(a, AppendIf::Appended(_)))
                    .expect("no error")
                })
            })
            .collect();
        let accepted = handles.into_iter().map(|h| h.join().expect("join")).filter(|a| *a).count();
        assert_eq!(accepted, 1, "round {round}: exactly one reservation for the work");
    }
    assert!(!live_for_the_work(&rec.read_all().expect("read")), "the harness's own commit was never reserved");
    let _ = std::fs::remove_dir_all(&dir);
}
