//! The refusal's words when plan line 3 fails because a full-path criterion
//! does not link to a claimed behavior and promise. It names the missing link
//! and gives the claims that make it, then the criterion re-claimed with the ids.

use familiar_ai_facts::Log;
use familiar_ai_rules::{plan, View};
use serde_json::{Map, Value};

use crate::teach::example::Project;
use crate::door::claim::command_form;

/// A full-path criterion whose line 3 fails because nothing links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub criterion_id: String,
    pub text: String,
    /// The id the criterion names, and whether a fact carries it.
    pub names_behavior: Option<(String, bool)>,
    pub names_promise: Option<(String, bool)>,
    /// The latest behavior or promise fact's id.
    pub behavior: Option<String>,
    pub promise: Option<String>,
}

fn text_of(f: &familiar_ai_facts::Fact, k: &str) -> Option<String> {
    f.fields.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The first full-path criterion whose line 3 fails by a missing link, when
/// line 3 is the first missing line. A promise with an empty slot is left to
/// the rules' own words.
pub fn gap(log: &Log) -> Option<Gap> {
    let v = View::new(log);
    if !plan::line_1(&v) || !plan::line_2(&v) || plan::line_3(&v) {
        return None;
    }
    let behavior = v.kind("behavior").last().and_then(|f| text_of(f, "behavior_id"));
    let promise = v.kind("promise").last().and_then(|f| text_of(f, "promise_id"));
    for c in plan::criteria(&v) {
        if plan::path_of(&v, &c) != Some("full") {
            continue;
        }
        let crit = v.of("criterion", &c).last()?;
        let names_behavior = text_of(crit, "behavior_id").map(|id| {
            let carried = v.kind("behavior").any(|f| text_of(f, "behavior_id").as_deref() == Some(id.as_str()));
            (id, carried)
        });
        let names_promise = text_of(crit, "promise_id").map(|id| {
            let carried = v.kind("promise").any(|f| text_of(f, "promise_id").as_deref() == Some(id.as_str()));
            (id, carried)
        });
        let linked = |n: &Option<(String, bool)>| n.as_ref().is_some_and(|(_, carried)| *carried);
        if linked(&names_behavior) && linked(&names_promise) {
            continue;
        }
        return Some(Gap {
            criterion_id: c,
            text: text_of(crit, "text").unwrap_or_default(),
            names_behavior,
            names_promise,
            behavior,
            promise,
        });
    }
    None
}

/// When plan line 2 is the first line missing because a light criterion's
/// anchor was checked not documentation (the owner, 2026-10-11): the command
/// that claims that criterion again on the full path, its text kept.
pub fn light_not_documentation(log: &Log, project: &Project) -> Option<String> {
    let v = View::new(log);
    if !plan::line_1(&v) || plan::line_2(&v) {
        return None;
    }
    let c = plan::criteria(&v).into_iter().find(|c| plan::not_documentation(&v, c))?;
    let text = v.of("criterion", &c).last().and_then(|f| text_of(f, "text")).unwrap_or_default();
    let known: Map<String, Value> = [("criterion_id", c.as_str()), ("text", text.as_str()), ("path", "full")]
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect();
    command_form("criterion", &known, Some(project))
}

/// The ids the criterion names, else the claimed ones, else `b1` and `p1`.
fn ids(g: &Gap) -> (String, String) {
    let b = g.names_behavior.as_ref().map(|(id, _)| id.clone()).or_else(|| g.behavior.clone()).unwrap_or_else(|| "b1".into());
    let p = g.names_promise.as_ref().map(|(id, _)| id.clone()).or_else(|| g.promise.clone()).unwrap_or_else(|| "p1".into());
    (b, p)
}

/// Whether the behavior and the promise claims are still missing.
fn missing(g: &Gap) -> (bool, bool) {
    let (b, p) = ids(g);
    let behavior_missing = match &g.names_behavior {
        Some((_, carried)) => !carried,
        None => g.behavior.as_deref() != Some(b.as_str()),
    };
    let promise_missing = match &g.names_promise {
        Some((_, carried)) => !carried,
        None => g.promise.as_deref() != Some(p.as_str()),
    };
    (behavior_missing, promise_missing)
}

/// The `to clear it:` words for the gap.
pub fn to_clear(g: &Gap) -> String {
    let c = &g.criterion_id;
    let (b, p) = ids(g);
    let names = match (&g.names_behavior, &g.names_promise) {
        (None, None) => "names no behavior and promise".to_string(),
        (Some((bi, false)), Some((pi, false))) => format!("names behavior {bi} and promise {pi}, which no behavior or promise fact carries"),
        (Some((bi, false)), None) => format!("names behavior {bi}, which no behavior fact carries, and no promise"),
        (None, Some((pi, false))) => format!("names no behavior, and promise {pi}, which no promise fact carries"),
        (Some((bi, true)), None) => format!("names behavior {bi} but no promise"),
        (None, Some((pi, true))) => format!("names promise {pi} but no behavior"),
        (Some((bi, true)), Some((pi, false))) => format!("names promise {pi}, which no promise fact carries (behavior {bi} is claimed)"),
        (Some((bi, false)), Some((pi, true))) => format!("names behavior {bi}, which no behavior fact carries (promise {pi} is claimed)"),
        (Some((_, true)), Some((_, true))) => "is linked".to_string(),
    };
    let (bm, pm) = missing(g);
    let claims = match (bm, pm) {
        (true, true) => format!("claim behavior {b} and promise {p}"),
        (true, false) => format!("claim behavior {b} (promise {p} is claimed)"),
        (false, true) => format!("claim promise {p} (behavior {b} is claimed)"),
        (false, false) => format!("behavior {b} and promise {p} are claimed"),
    };
    let unnamed = g.names_behavior.is_none() || g.names_promise.is_none();
    let reclaim = format!("re-claim the criterion with --behavior_id {b} --promise_id {p}");
    let what = match (bm || pm, unnamed) {
        (true, true) => format!("{claims}, then {reclaim}"),
        (true, false) => claims,
        (false, _) => format!("{claims}; {reclaim}"),
    };
    format!("criterion {c} is on the full path but {names}: {what}")
}

/// The missing claims with the ids filled, then the criterion's re-claim when
/// it names fewer than both ids.
pub fn claim_lines(g: &Gap, project: &Project) -> Vec<String> {
    let (b, p) = ids(g);
    let (bm, pm) = missing(g);
    let mut forms: Vec<String> = Vec::new();
    let known = |pairs: &[(&str, &str)]| -> Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), Value::from(*v))).collect()
    };
    if bm {
        forms.extend(command_form("behavior", &known(&[("behavior_id", &b)]), Some(project)));
    }
    if pm {
        forms.extend(command_form("promise", &known(&[("promise_id", &p), ("behavior_id", &b)]), Some(project)));
    }
    if g.names_behavior.is_none() || g.names_promise.is_none() {
        forms.extend(command_form(
            "criterion",
            &known(&[("criterion_id", &g.criterion_id), ("text", &g.text), ("path", "full"), ("behavior_id", &b), ("promise_id", &p)]),
            Some(project),
        ));
    }
    forms
        .into_iter()
        .enumerate()
        .map(|(i, f)| format!("{}{f}", if i == 0 { "  claim it: " } else { "  then:     " }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use serde_json::json;

    fn log_with(claims: &[(&str, Value)]) -> Log {
        let mut log: Log = Vec::new();
        for (i, (kind, fields)) in claims.iter().enumerate() {
            let Value::Object(m) = fields else { panic!("object") };
            log = append_raw(&log, kind, m.clone(), i as f64, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
        }
        log
    }

    fn intent() -> (&'static str, Value) {
        ("intent_submitted", json!({"text": "fix the schema", "sha": "abc"}))
    }
    fn full(c: &str, b: Option<&str>, p: Option<&str>) -> (&'static str, Value) {
        let mut m = json!({"criterion_id": c, "text": "a MutableSequence field gets an array schema", "path": "full"});
        if let Some(b) = b {
            m["behavior_id"] = json!(b);
        }
        if let Some(p) = p {
            m["promise_id"] = json!(p);
        }
        ("criterion", m)
    }
    fn behavior(id: &str) -> (&'static str, Value) {
        ("behavior", json!({"behavior_id": id, "text": "schema generation"}))
    }
    fn promise(id: &str, b: &str, filled: bool) -> (&'static str, Value) {
        let slot = |s: &str| if filled { s.to_string() } else { String::new() };
        ("promise", json!({"promise_id": id, "behavior_id": b, "accepts": slot("a model"), "gives_back": slot("a schema"), "refuses": slot("nothing")}))
    }

    #[test]
    fn the_session_13563_shape_names_the_missing_link_and_the_reclaim() {
        let root = std::env::temp_dir();
        // The criterion names nothing and nothing is claimed.
        let log = log_with(&[intent(), full("c1", None, None)]);
        let g = gap(&log).expect("a gap");
        assert_eq!(
            to_clear(&g),
            "criterion c1 is on the full path but names no behavior and promise: claim behavior b1 and promise p1, then re-claim the criterion with --behavior_id b1 --promise_id p1"
        );
        let lines = claim_lines(&g, &Project { log: &log, root: &root });
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "  claim it: familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'");
        // definition change (the owner's plan of 2026-10-21, step 1) [draft names]: the promise's example carries its cases
        assert_eq!(lines[1], format!("  then:     familiar-ai claim promise --promise_id p1 --behavior_id b1 --accepts 'a name' --gives_back 'a greeting' --refuses 'an empty name' --cases '{}'", crate::teach::form::CASES));
        assert_eq!(lines[2], "  then:     familiar-ai claim criterion --criterion_id c1 --text 'a MutableSequence field gets an array schema' --path full --behavior_id b1 --promise_id p1");
        // Behavior and promise claimed twice, the criterion still naming nothing.
        let log = log_with(&[intent(), full("c1", None, None), behavior("b1"), promise("p1", "b1", true), behavior("b1"), promise("p1", "b1", true)]);
        let g = gap(&log).expect("a gap");
        assert_eq!(
            to_clear(&g),
            "criterion c1 is on the full path but names no behavior and promise: behavior b1 and promise p1 are claimed; re-claim the criterion with --behavior_id b1 --promise_id p1"
        );
        let lines = claim_lines(&g, &Project { log: &log, root: &root });
        assert_eq!(lines, ["  claim it: familiar-ai claim criterion --criterion_id c1 --text 'a MutableSequence field gets an array schema' --path full --behavior_id b1 --promise_id p1"]);
        // One claimed, one not.
        let log = log_with(&[intent(), full("c1", None, None), behavior("b2")]);
        let g = gap(&log).expect("a gap");
        assert_eq!(
            to_clear(&g),
            "criterion c1 is on the full path but names no behavior and promise: claim promise p1 (behavior b2 is claimed), then re-claim the criterion with --behavior_id b2 --promise_id p1"
        );
        let lines = claim_lines(&g, &Project { log: &log, root: &root });
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("  claim it: familiar-ai claim promise --promise_id p1 --behavior_id b2 "), "{}", lines[0]);
        assert!(lines[1].ends_with("--path full --behavior_id b2 --promise_id p1"), "{}", lines[1]);
    }

    #[test]
    fn ids_no_fact_carries_are_named_and_a_linked_criterion_with_an_empty_slot_is_not_a_gap() {
        let root = std::env::temp_dir();
        let log = log_with(&[intent(), full("c1", Some("b9"), Some("p9"))]);
        let g = gap(&log).expect("a gap");
        assert_eq!(
            to_clear(&g),
            "criterion c1 is on the full path but names behavior b9 and promise p9, which no behavior or promise fact carries: claim behavior b9 and promise p9"
        );
        let lines = claim_lines(&g, &Project { log: &log, root: &root });
        assert_eq!(lines.len(), 2, "no re-claim: the ids are named: {lines:?}");
        assert!(lines[0].contains("claim behavior --behavior_id b9 ") && lines[1].contains("claim promise --promise_id p9 --behavior_id b9 "), "{lines:?}");
        let log = log_with(&[intent(), full("c1", Some("b1"), None), behavior("b1")]);
        let g = gap(&log).expect("a gap");
        assert_eq!(
            to_clear(&g),
            "criterion c1 is on the full path but names behavior b1 but no promise: claim promise p1 (behavior b1 is claimed), then re-claim the criterion with --behavior_id b1 --promise_id p1"
        );
        // Linked with the promise's slots empty is not a gap.
        let log = log_with(&[intent(), full("c1", Some("b1"), Some("p1")), behavior("b1"), promise("p1", "b1", false)]);
        assert_eq!(gap(&log), None);
        // Linked and filled, line 3 holds.
        let log = log_with(&[intent(), full("c1", Some("b1"), Some("p1")), behavior("b1"), promise("p1", "b1", true)]);
        assert_eq!(gap(&log), None);
        // A light criterion never has a gap.
        let log = log_with(&[intent(), ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"}))]);
        assert_eq!(gap(&log), None);
        // With no intent the refusal is about line 1.
        let log = log_with(&[full("c1", None, None)]);
        assert_eq!(gap(&log), None);
        // The second criterion is the one with the gap.
        let log = log_with(&[intent(), ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"})), full("c2", None, None)]);
        assert_eq!(gap(&log).map(|g| g.criterion_id).as_deref(), Some("c2"));
    }
}
