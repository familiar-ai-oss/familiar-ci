//! `commit_seen.by` is optional and accepts only `agent`.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

fn fields(by: Option<Value>) -> Map<String, Value> {
    let mut v = json!({"commit": "K1", "files": ["src/a.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": []});
    if let Some(by) = by {
        v["by"] = by;
    }
    v.as_object().cloned().unwrap_or_default()
}

#[test]
fn by_is_optional_and_its_one_word_is_agent() {
    let log = append_raw(&Vec::new(), "commit_seen", fields(None), 0.0, "_default_").expect("absent");
    assert_eq!(log[0].fields_as::<CommitSeen>().expect("typed").by, None);
    assert!(!log[0].fields.contains_key("by"), "absent stays absent");
    let log = append_raw(&log, "commit_seen", fields(Some(json!("agent"))), 1.0, "_default_").expect("agent");
    assert_eq!(log[1].fields_as::<CommitSeen>().expect("typed").by.as_deref(), Some("agent"));
    for word in [json!("person"), json!("Agent"), json!(true)] {
        let got = append_raw(&log, "commit_seen", fields(Some(word.clone())), 2.0, "_default_");
        assert!(matches!(got, Err(AppendError::BadValue(_))), "{word} gave {got:?}");
    }
}
