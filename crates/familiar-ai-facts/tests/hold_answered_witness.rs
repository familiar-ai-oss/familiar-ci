//! `hold_answered` may name the card's run, the commit it was answered at and
//! the witness's id (`approval`, the `t` of the signed approval), as fields of
//! the fact read from its field map (plan §6.9, S9d). The typed `HoldAnswered`
//! keeps the four fields the sealed tests build; an answer without them still
//! appends, and a value that is not text is refused.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "facts_tests";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

#[test]
fn hold_answered_may_name_its_run_commit_and_approval() {
    let ok = json!({"criterion_id": "c1", "hold": "anchor_moved", "choice": "release", "typed": "", "run_id": "", "commit": "K1", "approval": "1760000000123", "from": "clear"});
    let log = append_raw(&Vec::new(), "hold_answered", fields(ok.clone()), AT, TENANT).expect("appends");
    for (k, v) in [("run_id", ""), ("commit", "K1"), ("approval", "1760000000123"), ("from", "clear")] {
        assert_eq!(log[0].fields.get(k).and_then(Value::as_str), Some(v), "{k}");
    }
    let sealed = HoldAnswered { criterion_id: "c1".into(), hold: "anchor_moved".into(), choice: "release".into(), typed: String::new() };
    assert_eq!(log[0].fields_as::<HoldAnswered>().unwrap(), sealed, "the typed form reads the rest");

    let mut without = fields(ok.clone());
    for k in ["run_id", "commit", "approval", "from"] {
        without.remove(k);
    }
    let bare = append_raw(&Vec::new(), "hold_answered", without, AT, TENANT).expect("appends");
    assert_eq!(append(&Vec::new(), &sealed, AT, TENANT).unwrap(), bare, "the typed form writes none of them");

    for k in ["run_id", "commit", "approval", "from"] {
        let mut bad = fields(ok.clone());
        bad.insert(k.to_string(), json!(7));
        assert!(matches!(append_raw(&Vec::new(), "hold_answered", bad, AT, TENANT), Err(AppendError::BadValue(_))), "{k}");
    }
}

#[test]
fn the_form_registers_them_optional() {
    let row = FORM.iter().find(|r| r.kind == "hold_answered").expect("row");
    // `from` (the owner, 2026-10-28): "clear" on a release familiar-ai clear wrote, the person's button
    assert_eq!(row.fields, &["criterion_id", "hold", "choice", "typed", "run_id", "commit", "approval", "from"]);
    assert_eq!(row.optional, &["run_id", "commit", "approval", "from"]);
}
