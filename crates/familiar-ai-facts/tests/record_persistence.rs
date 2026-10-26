//! The record keeps facts in order across a reopen and across two handles, reads by kind and criterion,
//! and writes nothing for an append it refuses.

mod common;

use common::{obj, sample_for, samples, AT, TENANT};
use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

#[test]
fn record_survives_reopen_and_keeps_order() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-reopen-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let written: Vec<Value>;
    {
        let rec = Record::open(&path).unwrap();
        for (kind, fields) in samples() {
            rec.append_raw(kind, fields, AT, TENANT).unwrap();
        }
        written = rec.read_all().unwrap().iter().map(Fact::to_json).collect();
    } // dropped: the file is closed
    {
        let rec = Record::open(&path).unwrap();
        let reread = rec.read_all().unwrap();
        let reread_json: Vec<Value> = reread.iter().map(Fact::to_json).collect();
        assert_eq!(written, reread_json);
        for (i, fact) in reread.iter().enumerate() {
            assert_eq!(fact.t, i as u64);
        }
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn positions_stay_unique_when_two_handles_append_in_turn() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-handles-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let rec = Record::open(&path).unwrap();
        for _ in 0..3 {
            rec.append_raw(
                "criterion_head",
                obj(&[("criterion_id", json!("c1"))]),
                1.0,
                "_default_",
            )
            .unwrap();
        }
    }
    {
        let rec = Record::open(&path).unwrap();
        let f = rec
            .append_raw(
                "criterion_head",
                obj(&[("criterion_id", json!("c2"))]),
                2.0,
                "_default_",
            )
            .unwrap();
        assert_eq!(f.t, 3);
        let all = rec.read_all().unwrap();
        assert_eq!(
            all.iter().map(|f| f.t).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        // a refused append writes nothing
        assert!(rec
            .append_raw("no_such_kind", Map::new(), 3.0, "_default_")
            .is_err());
        assert_eq!(rec.len().unwrap(), 4);
    }
    let _ = std::fs::remove_file(&path);
}

// The record refuses what the in-memory append refuses.
#[test]
fn record_append_refuses_what_append_refuses() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-refuse-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let rec = Record::open(&path).unwrap();
    let err = rec.append_raw("nope", Map::new(), AT, TENANT).unwrap_err();
    assert!(matches!(
        err,
        RecordError::Append(AppendError::UnknownKind(_))
    ));
    assert_eq!(rec.len().unwrap(), 0);
    let mut bad = sample_for("test_planned");
    bad.insert("will".to_string(), json!("erase"));
    assert!(rec.append_raw("test_planned", bad, AT, TENANT).is_err());
    assert_eq!(rec.len().unwrap(), 0);
    // A good append stamps t as the next position.
    let fact = rec
        .append_raw("test_planned", sample_for("test_planned"), AT, TENANT)
        .unwrap();
    assert_eq!(fact.t, 0);
    let fact = rec
        .append_raw("test_planned", sample_for("test_planned"), AT, TENANT)
        .unwrap();
    assert_eq!(fact.t, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_record_reads_by_kind_and_criterion() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-reads-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let rec = Record::open(&path).unwrap();
    for (kind, fields) in samples() {
        rec.append_raw(kind, fields, AT, TENANT).unwrap();
    }
    assert_eq!(rec.len().unwrap(), samples().len() as u64);
    let heads = rec.read_by_kind("criterion_head").unwrap();
    assert_eq!(heads.len(), 1);
    let written = rec
        .read_by_kind_and_criterion("test_written", "c1")
        .unwrap();
    assert_eq!(written.len(), 1);
    assert!(rec
        .read_by_kind_and_criterion("test_written", "nope")
        .unwrap()
        .is_empty());
    assert_eq!(
        rec.last_of_kind("pipeline_run_seen").unwrap().unwrap().kind,
        "pipeline_run_seen"
    );
    assert!(rec.last_of_kind("intent_submitted").unwrap().is_some());
    let _ = std::fs::remove_file(&path);
}

// A batch is written in one write (Assumption 75, the seed of 2026-10-15): every fact at the next
// positions in order, in the file on `Ok`; a batch holding a fact its form refuses writes none of
// it, so a seed that cannot finish leaves the record as it was.
#[test]
fn append_all_writes_the_whole_batch_in_one_write_or_none_of_it() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-batch-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let rec = Record::open(&path).unwrap();
    rec.append_raw("criterion_head", sample_for("criterion_head"), AT, TENANT).unwrap();
    let batch: Vec<(&str, Map<String, Value>, f64)> = samples().into_iter().map(|(k, f)| (k, f, AT)).collect();
    let written = rec.append_all(&batch, TENANT).unwrap();
    assert_eq!(written.iter().map(|f| f.t).collect::<Vec<_>>(), (1..=batch.len() as u64).collect::<Vec<_>>());
    assert_eq!(written.iter().map(|f| f.kind).collect::<Vec<_>>(), batch.iter().map(|(k, _, _)| *k).collect::<Vec<_>>());
    let all = rec.read_all().unwrap();
    assert_eq!(all[1..], written[..], "read back as written");
    let mut refused = batch.clone();
    refused.insert(2, ("criterion_head", obj(&[("criterion_id", json!(7))]), AT));
    assert!(rec.append_all(&refused, TENANT).is_err());
    assert_eq!(rec.len().unwrap(), 1 + batch.len() as u64, "nothing of a refused batch is written");
    assert!(rec.append_all(&[], TENANT).unwrap().is_empty());
    assert_eq!(rec.len().unwrap(), 1 + batch.len() as u64);
    drop(rec);
    assert_eq!(Record::open(&path).unwrap().read_all().unwrap(), all, "kept across a reopen");
    let _ = std::fs::remove_file(&path);
}

// A timestamp that is not a finite number is refused, and the record writes nothing.
#[test]
fn a_non_finite_at_writes_nothing() {
    let path = std::env::temp_dir().join(format!("familiar-ai-facts-nan-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let rec = Record::open(&path).unwrap();
    assert!(rec.append_raw("criterion_head", sample_for("criterion_head"), f64::NAN, TENANT).is_err());
    assert_eq!(rec.len().unwrap(), 0, "nothing written");
    rec.append_raw("criterion_head", sample_for("criterion_head"), AT, TENANT).unwrap();
    assert_eq!(rec.read_all().unwrap().len(), 1, "the record still reads");
    let _ = std::fs::remove_file(&path);
}
