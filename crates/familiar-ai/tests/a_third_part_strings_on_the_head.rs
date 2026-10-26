//! The string's definition is the rule as coded (the owner, 2026-10-15):
//! every link names the string's head, and the links' order is the order they
//! were claimed in. The checklist's line 9 offers the head, and a claim naming
//! a criterion that is not a head is refused at the claim with the head to name.

mod common;

use common::Project;

const SESSION: &str = "3f1c2a4b-5d6e-4f70-8a9b-0c1d2e3f4a5b";

/// A project with an intent and the criteria named, each on the full path.
fn planned(name: &str, criteria: &[&str]) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    for c in criteria {
        p.claim(&["criterion", "--criterion_id", c, "--text", &format!("greet holds {c}"), "--path", "full"]);
    }
    p
}

fn line_9(p: &Project) -> String {
    let status = p.status();
    status.lines().find(|l| l.starts_with("    9 the string ")).unwrap_or_else(|| panic!("line 9 is missing: {status}")).to_string()
}

#[test]
fn line_9_for_a_third_part_names_the_head_not_the_last_link() {
    let p = planned("third", &["c1", "c2", "c3"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    assert_eq!(line_9(&p), "    9 the string      claim it: familiar-ai claim criterion_strung_on_head --criterion_id c3 --head c1");
}

#[test]
fn with_two_strings_the_example_names_the_head_of_the_string_the_last_link_is_on() {
    let p = planned("two-strings", &["c1", "c2", "c3", "c4", "c5"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    p.claim(&["criterion_head", "--criterion_id", "c3"]);
    // the last criterion on a string is itself a head: it is named
    assert_eq!(line_9(&p), "    9 the string      claim it: familiar-ai claim criterion_strung_on_head --criterion_id c4 --head c3");
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c4", "--head", "c3"]);
    // the last is a link on c3's string: its head is named
    assert_eq!(line_9(&p), "    9 the string      claim it: familiar-ai claim criterion_strung_on_head --criterion_id c5 --head c3");
}

#[test]
fn a_link_named_as_the_head_is_refused_at_the_claim_with_the_head_to_name() {
    let p = planned("refused", &["c1", "c2", "c3"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    let (code, out, err) = p.try_claim(&["criterion_strung_on_head", "--criterion_id", "c3", "--head", "c2"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        err.lines().next(),
        Some("familiar-ai: not claimed yet · c2 is not a head; the head of its string is c1: --head c1 (the links of a string all name its head; their order is the order they were claimed in)"),
        "{err}"
    );
    assert!(err.contains("example:\n  familiar-ai claim criterion_strung_on_head --criterion_id c3 --head c1\n"), "the form's example is the claim to make: {err}");
    assert!(p.criterion_json("c3")["after"].is_null(), "nothing was appended");
    // the head is accepted, and c3's turn comes after c2's: the order claimed
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c3", "--head", "c1"]);
    assert_eq!(p.criterion_json("c2")["after"], "c1");
    assert_eq!(p.criterion_json("c3")["after"], "c2");
    assert!(!p.status().contains("    9 the string "), "line 9 holds: {}", p.status());
}

#[test]
fn a_criterion_on_no_string_or_the_criterion_itself_is_refused_as_the_head() {
    let p = planned("no-string", &["c1", "c2", "c3"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    let (code, _, err) = p.try_claim(&["criterion_strung_on_head", "--criterion_id", "c3", "--head", "c2"]);
    assert_eq!(code, 1, "{err}");
    assert_eq!(
        err.lines().next(),
        Some("familiar-ai: not claimed yet · c2 is not a head; it is on no string: claim criterion_head --criterion_id c2 first, or name a head: --head c1 (the links of a string all name its head; their order is the order they were claimed in)"),
        "{err}"
    );
    let (code, _, err) = p.try_claim(&["criterion_strung_on_head", "--criterion_id", "c1", "--head", "c1"]);
    assert_eq!(code, 1, "{err}");
    assert_eq!(err.lines().next(), Some("familiar-ai: not claimed yet · c1 is not strung on itself; name the head of the string it joins"), "{err}");
}

/// Assumption 1, pinned and not changed: a head that is later strung itself
/// drops its links, since a link is on the string only while its head's latest
/// stringing fact is `criterion_head`.
#[test]
fn a_head_later_strung_itself_drops_its_links() {
    let p = planned("restrung", &["c1", "c2", "c3"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    p.claim(&["criterion_head", "--criterion_id", "c3"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c1", "--head", "c3"]);
    assert_eq!(p.criterion_json("c1")["after"], "c3");
    assert!(p.criterion_json("c2")["after"].is_null(), "c2 fell off with its head");
    assert_eq!(line_9(&p), "    9 the string      claim it: familiar-ai claim criterion_strung_on_head --criterion_id c2 --head c3");
}
