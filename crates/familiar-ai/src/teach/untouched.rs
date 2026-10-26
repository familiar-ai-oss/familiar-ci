//! The teaching line after `anchor_untouched`: what the code commits since the seal
//! did change, set against the place the plan named, so the agent sees why its commits
//! missed the anchor. It reads the `files` of `commit_seen` facts and stores nothing.

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{sealed_at, Place};
use familiar_ai_rules::{plan, View};
use serde_json::Value;

/// The line for criterion `c`, or `None` when it has no anchor to name.
/// The anchor's own file comes first when a commit changed it; test files and Familiar's folder are left out.
pub fn teaching(log: &Log, c: &str) -> Option<String> {
    let v = View::new(log);
    let a = plan::anchor_of(&v, c)?;
    let place = plan::places_of(a).into_iter().next()?;
    let anchor = match (place.line_start, place.line_end) {
        (Some(s), Some(e)) => format!("{} {} {s}–{e}", place.file_path, place.symbol),
        _ => format!("{} {}", place.file_path, place.symbol),
    };
    // The seal is the stamp at the tests-first commit; before any seal, every commit counts.
    let since = sealed_at(&v, c, Place::Laptop).map(|s| s.t);
    let tests = familiar_ai_rules::what_to_read(log).plan_test_files;
    let mut files: Vec<String> = Vec::new();
    for f in v.kind("commit_seen").filter(|f| since.is_none_or(|t| f.t > t)) {
        let names = f.fields.get("files").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
        for file in names {
            if tests.iter().any(|t| t == file) || crate::door::commits::is_familiars_own(file) {
                continue;
            }
            if !files.iter().any(|x| x == file) {
                files.push(file.to_string());
            }
        }
    }
    let changed = match files.iter().position(|f| *f == place.file_path) {
        Some(i) => {
            let first = files.remove(i);
            Some((first, files.len()))
        }
        None => files.first().cloned().map(|f| (f, files.len() - 1)),
    };
    let what = match changed {
        Some((first, 0)) => format!("the commits since the seal changed {first}"),
        Some((first, 1)) => format!("the commits since the seal changed {first} (and 1 more)"),
        Some((first, n)) => format!("the commits since the seal changed {first} (and {n} more)"),
        None => "no commit since the seal changed a code file".to_string(),
    };
    Some(format!("  {what}; the place the plan named is {anchor}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use serde_json::{json, Value};

    fn with(log: &Log, kind: &str, fields: Value, at: f64) -> Log {
        let Value::Object(m) = fields else { panic!("object") };
        append_raw(log, kind, m, at, "t").unwrap_or_else(|e| panic!("{kind}: {e}"))
    }

    #[test]
    fn the_line_names_the_anchors_file_first_the_count_of_others_and_the_anchor() {
        // The fixture anchors c1 on src/login.py check_password 40–62, with tests in tests/test_c1.py.
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        let log = with(&log, "commit_seen", json!({ "commit": "k1", "files": ["tests/test_c1.py", ".familiar/plan"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }), 1.0);
        let log = with(&log, "commit_seen", json!({ "commit": "k2", "files": ["src/other.py", "src/login.py", "src/third.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": [] }), 2.0);
        assert_eq!(
            teaching(&log, "c1").as_deref(),
            Some("  the commits since the seal changed src/login.py (and 2 more); the place the plan named is src/login.py check_password 40–62")
        );
        let log2 = with(&log, "commit_seen", json!({ "commit": "k3", "files": ["src/login.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": [] }), 3.0);
        assert!(teaching(&log2, "c1").expect("line").contains("src/login.py (and 2 more)"), "distinct files");
        // With no code file changed the line says so; the tests and the plan file are not code.
        let only_tests = with(&familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture"), "commit_seen", json!({ "commit": "k1", "files": ["tests/test_c1.py", ".familiar/plan"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }), 1.0);
        assert_eq!(
            teaching(&only_tests, "c1").as_deref(),
            Some("  no commit since the seal changed a code file; the place the plan named is src/login.py check_password 40–62")
        );
        // One other file, not the anchor's.
        let one = with(&only_tests, "commit_seen", json!({ "commit": "k2", "files": ["src/other.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": [] }), 2.0);
        assert_eq!(
            teaching(&one, "c1").as_deref(),
            Some("  the commits since the seal changed src/other.py; the place the plan named is src/login.py check_password 40–62")
        );
        let two = with(&one, "commit_seen", json!({ "commit": "k3", "files": ["src/more.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": [] }), 3.0);
        assert!(teaching(&two, "c1").expect("line").starts_with("  the commits since the seal changed src/other.py (and 1 more);"));
        // No anchor gives no line.
        let bare = with(&Vec::new(), "intent_submitted", json!({ "text": "x", "sha": "s" }), 0.0);
        let bare = with(&bare, "criterion", json!({ "criterion_id": "c1", "text": "t", "path": "light" }), 0.0);
        assert!(teaching(&bare, "c1").is_none());
    }
}
