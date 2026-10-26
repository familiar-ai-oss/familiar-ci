//! `anchor_is_documentation` (the owner, 2026-10-11): the binary's check that
//! a light criterion's anchor is documentation. It mirrors
//! `anchor_sha_matched` without `lines_hash` and `checked_at`; a new-work
//! anchor names its file alone, so `symbol` and the lines may be absent.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "lane_a_tests";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

#[test]
fn the_form_registers_the_place_and_the_verdict() {
    let row = form_row("anchor_is_documentation").expect("registered");
    assert_eq!(row.fields, &["criterion_id", "file_path", "symbol", "line_start", "line_end", "sha", "matched"]);
    assert_eq!(row.optional, &["symbol", "line_start", "line_end"]);
}

#[test]
fn a_place_and_a_new_work_file_both_append() {
    let place = json!({"criterion_id": "c2", "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 10, "line_end": 30, "sha": "base", "matched": true});
    let log = append_raw(&Vec::new(), "anchor_is_documentation", fields(place.clone()), AT, TENANT).expect("a place");
    let d: AnchorIsDocumentation = log[0].fields_as().expect("typed");
    assert_eq!((d.symbol.as_deref(), d.line_start, d.line_end, d.matched), (Some("reset_link"), Some(10), Some(30), true));
    let file = json!({"criterion_id": "c2", "file_path": "docs/guide.md", "sha": "base", "matched": false});
    let log = append_raw(&log, "anchor_is_documentation", fields(file), AT, TENANT).expect("a file alone");
    let j = log[1].to_json();
    assert!(j.get("symbol").is_none() && j.get("line_start").is_none() && j.get("line_end").is_none(), "{j}");
}

#[test]
fn a_hash_a_moment_or_a_missing_verdict_is_refused() {
    let base = json!({"criterion_id": "c2", "file_path": "README.md", "sha": "base", "matched": true});
    for (k, v) in [("lines_hash", json!("L0")), ("checked_at", json!("planning"))] {
        let mut f = fields(base.clone());
        f.insert(k.into(), v);
        assert!(append_raw(&Vec::new(), "anchor_is_documentation", f, AT, TENANT).is_err(), "{k} is not registered");
    }
    let mut f = fields(base);
    f.remove("matched");
    assert!(append_raw(&Vec::new(), "anchor_is_documentation", f, AT, TENANT).is_err(), "matched is required");
}
