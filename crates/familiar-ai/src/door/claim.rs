//! Parse and check the fields of `familiar-ai claim <kind>`, and write the
//! exact claim commands a refusal teaches. Kinds only the binary writes are
//! refused here; the facts crate refuses unknown kinds and keys.

use std::fmt;
use std::path::Path;

use familiar_ai_facts::{form_row, Log, KINDS};
use familiar_ai_rules::hooks::{Clears, Refusal};
use familiar_ai_rules::{lifecycle, plan, Event, View};
use serde_json::{Map, Value};

use crate::teach::example::{self, Project};

/// The kinds only the binary writes, refused at the door. `familiar_switch`
/// is written only by `familiar-ai off` and `on`, which the person runs;
/// `anchor_is_documentation` is the anchor check's (the owner, 2026-10-11);
/// the next six are the test outcome translation's (the owner's plan of
/// 2026-10-14, step S2); `record_seeded` is the mark setup's seed writes
/// last (the owner, 2026-10-15); `holder_resumed` a resumed lane's start
/// (the owner, 2026-10-16); `investigator_finding` the diagnosis of a test that never went
/// red [draft] (the owner, 2026-10-16, S7b); `reader_chosen` the adapter's choice of how a runner
/// is read [draft] (the owner, 2026-10-19, lane a1); `fix_verdict` the reviewer's word on a green
/// fix [draft] (the owner's plan of 2026-10-20, step 3); `existing_test_verdict` the judge's word on
/// an announced change to a pre-existing test and `touching_test_derived` an existing test found
/// by the fix's imports [draft] (the owner, 2026-10-20); `places_listed` the place lister's sites
/// [draft] (the owner's plan of 2026-10-21, step 3); `trace_listed` the try's listing of what its
/// failing tests ran [draft] (the owner's plan of 2026-10-23); `cases_found` the case-finder's
/// answer [draft] (the owner's plan of 2026-10-25, the cases card), since the lens panel its synthesis's; `lens_released` and
/// `lens_found` which Agent call got which lens and each lens's answer (the owner's plan of
/// 2026-10-26, the lens panel, approved).
pub const BINARY_KINDS: [&str; 37] = [
    "commit_seen",
    "pass_please_pass",
    "fingerprint_stamped",
    "anchor_sha_matched",
    "test_written",
    "pipeline_run_seen",
    "model_seen",
    "judge_verdict",
    "person_said",
    "judge_unanswered",
    "investigator_unfinished",
    "run_reserved",
    "run_started",
    "run_ended",
    "run_cancel_requested",
    "tests_observed",
    "ci_report_seen",
    "anchor_is_documentation",
    "familiar_switch",
    "test_log_translation",
    "dependency_drafted",
    "dependency_unanswered",
    "told",
    "criterion_covered",
    "subagent_ended",
    "record_seeded",
    "holder_resumed",
    "investigator_finding",
    "reader_chosen",
    "fix_verdict",
    "existing_test_verdict",
    "touching_test_derived",
    "places_listed",
    "trace_listed",
    "cases_found",
    "lens_released",
    "lens_found",
];

/// Whether `kind` is registered and not one of the binary's.
pub fn claimable(kind: &str) -> bool {
    KINDS.contains(&kind) && !BINARY_KINDS.contains(&kind)
}

/// Whether the binary fills this field of this kind when it is left out.
fn filled_by_the_binary(kind: &str, field: &str) -> bool {
    matches!(
        (kind, field),
        ("intent_submitted", "sha")
            | ("promise", "contract_version")
            | ("criterion_assigned", "agent_conversation_id")
            | ("test_stack_detected", "codebase_id")
    )
}

/// A value written as it is typed on a shell command line.
pub(crate) fn shell_word(v: &Value) -> String {
    match v {
        Value::String(s) => {
            let plain = !s.is_empty()
                && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@+=,".contains(c))
                && serde_json::from_str::<Value>(s).is_err();
            if plain {
                s.clone()
            } else {
                format!("'{}'", s.replace('\'', "'\\''"))
            }
        }
        // A number is unquoted so it reads back as a number, as in `--for 12`.
        Value::Number(n) => n.to_string(),
        other => format!("'{}'", other.to_string().replace('\'', "'\\''")),
    }
}

/// The full `familiar-ai claim` command for a kind, from known values, then
/// the project's, then the form's examples, or `None` for a refused kind. It
/// holds no placeholder, because agents paste it verbatim.
pub fn command_form(kind: &str, known: &Map<String, Value>, project: Option<&Project>) -> Option<String> {
    if !claimable(kind) {
        return None;
    }
    let row = form_row(kind)?;
    let example = crate::teach::form::example_args(kind)?;
    let aware = project
        .map(|p| example::known(kind, p, known.get("criterion_id").and_then(Value::as_str)))
        .unwrap_or_default();
    let mut parts = vec![format!("familiar-ai claim {kind}")];
    for &field in row.fields {
        let value = match known.get(field).or_else(|| aware.get(field)) {
            Some(v) => v.clone(),
            None => match example.iter().find(|(f, _)| *f == field) {
                Some((_, v)) => Value::String((*v).to_string()),
                None if row.optional.contains(&field) || filled_by_the_binary(kind, field) => continue,
                None => Value::String(String::new()),
            },
        };
        parts.push(format!("--{field} {}", shell_word(&value)));
    }
    Some(parts.join(" "))
}

/// Why a claim was refused before the door, or by it.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaimError {
    BinaryKind(String),
    /// The command line's `--field value` pairs did not pair up.
    Pairs(String),
    /// stdin was not one JSON object.
    NotAnObject,
    /// `criterion_assigned` with no conversation id anywhere.
    NoConversation,
    /// `intent_submitted` with no sha and no head to read.
    NoHead(String),
    /// `intent_submitted` with no `for` and no prompt of the person's.
    NoPrompt,
    /// `test_stack_detected` with a `project_root` outside the project.
    RootOutside(String),
    /// `test_stack_detected` with a `test_command` naming a test file that
    /// is neither in the tree nor planned, or the form's example verbatim.
    Command(String),
    /// `hold_answered` whose hold is not a card hold.
    NotACard(String),
    /// `hold_answered` from inside an agent's shell, with the person's command
    /// and the open card's text (empty when none is open).
    PersonsCard { command: String, card: String },
    /// The facts door, or the record, refused.
    Door(String),
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaimError::BinaryKind(k) => write!(
                f,
                "{k} is written by familiar-ai itself (from git, the test run, the anchor check, the model Claude Code reports, the judge, the person's own prompt or the person's off switch); it cannot be claimed"
            ),
            ClaimError::Pairs(m) => write!(f, "{m}"),
            ClaimError::NotAnObject => write!(f, "the fields on stdin must be one JSON object"),
            ClaimError::NoConversation => write!(
                f,
                "criterion_assigned needs the agent conversation that holds it: pass --agent_conversation_id <session_id or session_id/agent_id>, or run inside Claude Code, where CLAUDE_CODE_SESSION_ID names this conversation"
            ),
            ClaimError::NoHead(m) => write!(
                f,
                "intent_submitted needs the sha the intent is submitted at and the repository's head could not be read ({m}): pass --sha <sha>"
            ),
            ClaimError::NoPrompt => write!(f, "no prompt to tie the intent to; the person has said nothing yet"),
            ClaimError::NotACard(h) => write!(
                f,
                "hold_answered answers a card, and {h} is not a card hold; the card holds are {}",
                crate::teach::card::card_holds_words()
            ),
            ClaimError::PersonsCard { command, card } => {
                write!(f, "this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: {command})")?;
                if !card.is_empty() {
                    write!(f, "\n{card}")?;
                }
                Ok(())
            }
            ClaimError::RootOutside(m) | ClaimError::Command(m) | ClaimError::Door(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ClaimError {}

/// A command-line value: JSON when it reads as JSON, else text.
pub fn value_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// Parse `--field value` or `--field=value` pairs, each field named once.
pub fn fields_from_args(args: &[String]) -> Result<Map<String, Value>, ClaimError> {
    let mut out = Map::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let Some(name) = arg.strip_prefix("--") else {
            return Err(ClaimError::Pairs(format!(
                "expected --<field> before '{arg}'; the fields are --<field> <value> pairs"
            )));
        };
        let (name, value) = match name.split_once('=') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None => {
                let Some(v) = args.get(i + 1) else {
                    return Err(ClaimError::Pairs(format!("--{name} has no value")));
                };
                i += 1;
                (name.to_string(), v.clone())
            }
        };
        if name.is_empty() {
            return Err(ClaimError::Pairs("a field needs a name after --".into()));
        }
        if out.contains_key(&name) {
            return Err(ClaimError::Pairs(format!("--{name} is given twice")));
        }
        out.insert(name, value_of(&value));
        i += 1;
    }
    Ok(out)
}

/// The fields of a claim: the command line's pairs, else stdin's one JSON
/// object, else none.
pub fn fields_of(args: &[String], stdin_text: &str) -> Result<Map<String, Value>, ClaimError> {
    if !args.is_empty() {
        return fields_from_args(args);
    }
    if stdin_text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(stdin_text) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err(ClaimError::NotAnObject),
    }
}

/// The claimable kinds a refusal's clears name, each with the values the
/// clear fixes.
fn claimable_clears(clears: &Clears) -> Vec<(String, Map<String, Value>)> {
    let mut out: Vec<(String, Map<String, Value>)> = Vec::new();
    match clears {
        Clears::Nothing => {}
        Clears::Name(name) => {
            if name == "announce_test_change" {
                out.push(("test_change_announced".to_string(), Map::new()));
            }
        }
        Clears::Moves(ms) => {
            for m in ms {
                let Some(kind) = m.get("kind").and_then(Value::as_str) else {
                    continue;
                };
                if !claimable(kind) {
                    continue;
                }
                let mut known = Map::new();
                for (k, v) in m {
                    if k != "kind" {
                        known.insert(k.clone(), v.clone());
                    }
                }
                // One line per card choice, else one per kind.
                if !out.iter().any(|(k, f)| k == kind && *f == known) {
                    out.push((kind.to_string(), known));
                }
            }
        }
    }
    out
}

/// The criterion a refusal is about: the refusal's or the event's, else the
/// first one the hold is on. `planning_incomplete` is skipped, since the
/// project picks the criterion its missing line is for.
pub(crate) fn criterion_of(log: &Log, r: &Refusal, event: &Event) -> Option<String> {
    if let Some(c) = r.criterion_id.as_deref().or_else(|| event.criterion_id()) {
        return Some(c.to_string());
    }
    let hold = r.hold.as_deref().filter(|h| *h != "planning_incomplete")?;
    let v = View::new(log);
    plan::criteria(&v)
        .into_iter()
        .find(|c| lifecycle::whys(&v, c).iter().any(|w| w == hold))
}

/// The lines added after a refusal: the claim command for each clearing fact
/// this door can write, plus a note when a value is unknown.
pub fn clearing_commands(log: &Log, root: &Path, r: &Refusal, event: &Event) -> Vec<String> {
    let c = criterion_of(log, r, event);
    let project = Project { log, root };
    let mut out = Vec::new();
    let spent = c.as_deref().is_some_and(|c| crate::door::investigator::launches_spent(&View::new(log), c));
    for (kind, mut known) in claimable_clears(&r.clears) {
        // With the investigator's launches spent, a transfer to it is no way out.
        if spent && kind == "criterion_transfer" && known.contains_key("rung") {
            continue;
        }
        // A card's answer is the person's: the agent is told to ask, never handed their claim.
        if kind == "hold_answered" || kind == familiar_ai_rules::cases::QUESTION_ANSWERED {
            continue;
        }
        if let Some(c) = &c {
            if form_row(&kind).is_some_and(|row| row.fields.contains(&"criterion_id")) {
                known.entry("criterion_id".to_string()).or_insert_with(|| Value::from(c.as_str()));
            }
        }
        if let Some(form) = command_form(&kind, &known, Some(&project)) {
            let lead = if out.is_empty() { "  claim it: " } else { "  or:       " };
            out.push(format!("{lead}{form}"));
            if let Some(note) = example::note(&kind, &project) {
                out.push(format!("            {note}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_binarys_kinds_are_refused_and_the_rest_claimable() {
        // definition change (the owner, 2026-10-11): anchor_is_documentation is the binary's (18 became 19)
        // definition change (the owner's plan of 2026-10-14, test outcome translation step S2):
        // the six kinds the translation needs are the binary's (19 became 25), none claimable
        // definition change (the owner, 2026-10-15, the seed): record_seeded, the mark setup's
        // seed writes last, is the binary's (25 became 26)
        // definition change (the owner, 2026-10-16, S8g): holder_resumed is the binary's (26 became 27)
        // definition change (the owner, 2026-10-16, S7b): investigator_finding is the binary's (27 became 28)
        // definition change (the owner, 2026-10-19, lane a1): reader_chosen [draft] is the binary's (28 became 29)
        // definition change (the owner's plan of 2026-10-20, step 3): fix_verdict [draft] is the binary's (29 became 30)
        // definition change (the owner, 2026-10-20): existing_test_verdict and touching_test_derived
        // [draft] are the binary's (30 became 32)
        // definition change (the owner's plan of 2026-10-21, step 3): places_listed [draft], the
        // place lister's, is the binary's (32 became 33)
        // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23): trace_listed
        // [draft], the try's listing, is the binary's (33 became 34)
        // definition change (the owner's plan of 2026-10-25, the cases card, step 2; the owner's word
        // given): cases_found [draft], the binary's record of the case-finder's answer, is the
        // binary's (34 became 35)
        // definition change (the owner's plan of 2026-10-26, the lens panel, approved): lens_released
        // and lens_found are the binary's (35 became 37); question_answered is the person's
        assert_eq!(BINARY_KINDS.len(), 37);
        assert!(BINARY_KINDS.contains(&"cases_found"));
        assert!(!claimable("cases_found"));
        for k in ["lens_released", "lens_found"] {
            assert!(BINARY_KINDS.contains(&k) && !claimable(k), "{k}");
        }
        assert!(claimable("question_answered"), "the person's, from their own terminal");
        assert!(BINARY_KINDS.contains(&"places_listed"));
        for k in ["test_log_translation", "dependency_drafted", "dependency_unanswered", "told", "criterion_covered", "subagent_ended", "record_seeded", "holder_resumed", "investigator_finding", "reader_chosen", "fix_verdict"] {
            assert!(BINARY_KINDS.contains(&k), "{k}");
        }
        assert!(BINARY_KINDS.contains(&"anchor_is_documentation"));
        assert!(BINARY_KINDS.contains(&"tests_observed"));
        assert!(BINARY_KINDS.contains(&"ci_report_seen"));
        // the person's switch is `familiar-ai off` / `on`, never a claim
        assert!(BINARY_KINDS.contains(&"familiar_switch"));
        for k in BINARY_KINDS {
            assert!(!claimable(k), "{k}");
            assert!(command_form(k, &Map::new(), None).is_none(), "{k}");
        }
        let n = KINDS.iter().filter(|k| claimable(k)).count();
        // existing_test_announced [draft] is the agent's (18 became 19)
        // place_classified [draft] is the agent's (the owner's plan of 2026-10-21, step 3; 19 became 20)
        // one_reading and reading_alternatives [draft] are the agent's (the owner's plan of 2026-10-21, step 2; 20 became 22)
        // off_list_reason [draft] is the agent's (the owner's plan of 2026-10-23; owner approved 2026-10-23; 22 became 23)
        // question_answered is the person's, as hold_answered is (the owner's plan of 2026-10-26; 23 became 24)
        assert_eq!(n, 24);
        assert!(!claimable("not_a_kind"));
    }

    #[test]
    fn pairs_parse_json_values_and_text() {
        let args: Vec<String> = ["--criterion_id", "c1", "--line_start", "40", "--matched", "true", "--text", "greet says hello", "--places=[]"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let m = fields_from_args(&args).expect("pairs");
        assert_eq!(m["criterion_id"], json!("c1"));
        assert_eq!(m["line_start"], json!(40));
        assert_eq!(m["matched"], json!(true));
        assert_eq!(m["text"], json!("greet says hello"));
        assert_eq!(m["places"], json!([]));
    }

    #[test]
    fn pairs_refuse_a_bare_word_a_missing_value_and_a_repeat() {
        let bad = |args: &[&str]| fields_from_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(matches!(bad(&["criterion_id", "c1"]), Err(ClaimError::Pairs(_))));
        assert!(matches!(bad(&["--criterion_id"]), Err(ClaimError::Pairs(_))));
        assert!(matches!(bad(&["--a", "1", "--a", "2"]), Err(ClaimError::Pairs(_))));
        assert!(matches!(bad(&["--", "1"]), Err(ClaimError::Pairs(_))));
    }

    #[test]
    fn fields_come_from_args_else_stdin() {
        let m = fields_of(&[], r#"{"criterion_id": "c1"}"#).expect("stdin");
        assert_eq!(m["criterion_id"], json!("c1"));
        assert!(fields_of(&[], "   ").expect("empty").is_empty());
        assert_eq!(fields_of(&[], "[1]"), Err(ClaimError::NotAnObject));
        let args = vec!["--criterion_id".to_string(), "c2".to_string()];
        let m = fields_of(&args, r#"{"criterion_id": "c1"}"#).expect("args win");
        assert_eq!(m["criterion_id"], json!("c2"));
    }

    #[test]
    fn the_command_form_with_nothing_known_is_the_forms_example_and_names_every_required_field() {
        for k in KINDS.iter().filter(|k| claimable(k)) {
            let form = command_form(k, &Map::new(), None).expect(k);
            assert_eq!(form, crate::teach::form::example_command(k).expect(k), "{k}");
            assert!(!form.contains('<') && !form.contains('>'), "{k}: {form}");
            let row = form_row(k).expect("row");
            for &f in row.fields {
                if !row.optional.contains(&f) && !filled_by_the_binary(k, f) {
                    assert!(form.contains(&format!("--{f} ")), "{k}: {form}");
                }
            }
        }
        let f = command_form("intent_submitted", &Map::new(), None).expect("form");
        assert_eq!(f, "familiar-ai claim intent_submitted --text 'greet with hello'");
        let f = command_form("criterion", &Map::new(), None).expect("form");
        assert_eq!(f, "familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1");
        let f = command_form("criterion_assigned", &Map::new(), None).expect("form");
        assert_eq!(f, "familiar-ai claim criterion_assigned --criterion_id c1");
    }

    #[test]
    fn known_values_are_filled_in_shell_quoted() {
        let mut known = Map::new();
        known.insert("criterion_id".into(), json!("c1"));
        known.insert("hold".into(), json!("anchor_moved"));
        known.insert("choice".into(), json!("re_plan"));
        let f = command_form("hold_answered", &known, None).expect("form");
        assert_eq!(f, "familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice re_plan --typed ''");
        let mut known = Map::new();
        known.insert("criterion_id".into(), json!("c7"));
        known.insert("agent_conversation_id".into(), json!("s/a"));
        let f = command_form("criterion_assigned", &known, None).expect("form");
        assert_eq!(f, "familiar-ai claim criterion_assigned --criterion_id c7 --agent_conversation_id s/a", "a known value the binary would fill is shown");
        let f = command_form("test_change_announced", &known, None).expect("form");
        assert_eq!(f, "familiar-ai claim test_change_announced --criterion_id c7 --reason 'the assertion named the wrong file'");
        let mut known = Map::new();
        known.insert("text".into(), json!("it's done"));
        known.insert("sha".into(), json!("42"));
        let f = command_form("intent_submitted", &known, None).expect("form");
        assert_eq!(f, "familiar-ai claim intent_submitted --text 'it'\\''s done' --sha '42'");
    }

    #[test]
    fn the_project_fills_what_the_record_knows_and_a_known_value_still_wins() {
        let root = std::env::temp_dir();
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], false, false, 0.0, "t").expect("fixture");
        let project = Project { log: &log, root: &root };
        assert_eq!(example::planned_files(&log), ["tests/test_c1.py"], "the fixture plans one .py file");
        let f = command_form("test_stack_detected", &Map::new(), Some(&project)).expect("form");
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert_eq!(f, format!("familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{} -m pytest -q tests/test_c1.py'", crate::teach::example::python()));
        assert!(example::note("test_stack_detected", &project).is_none());
        let f = command_form("criterion", &Map::new(), Some(&project)).expect("form");
        assert_eq!(f, "familiar-ai claim criterion --criterion_id c2 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1", "the next free id");
        let mut known = Map::new();
        known.insert("criterion_id".into(), json!("c9"));
        let f = command_form("test_planned", &known, Some(&project)).expect("form");
        // definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
        assert_eq!(f, "familiar-ai claim test_planned --criterion_id c9 --file_path tests/test_greet.py --will create");
    }

    #[test]
    fn an_empty_log_is_taught_the_intent_first() {
        let log: Log = Vec::new();
        let r = match familiar_ai_rules::decide(&log, &Event::Stops { main: true }, 0.0) {
            familiar_ai_rules::Decision::Refuse(r) => r,
            other => panic!("{other:?}"),
        };
        let lines = clearing_commands(&log, &std::env::temp_dir(), &r, &Event::Stops { main: true });
        assert_eq!(lines, ["  claim it: familiar-ai claim intent_submitted --text 'greet with hello'"]);
    }

    #[test]
    fn a_card_hands_the_agent_no_claim_of_the_persons() {
        let r = Refusal {
            hold: Some("anchor_moved".into()),
            state: None,
            criterion_id: None,
            clears: Clears::Moves(
                ["re_plan", "keep_going", "release"]
                    .iter()
                    .map(|ch| {
                        let mut m = Map::new();
                        m.insert("kind".into(), json!("hold_answered"));
                        m.insert("hold".into(), json!("anchor_moved"));
                        m.insert("choice".into(), json!(ch));
                        m
                    })
                    .collect(),
            ),
        };
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        // the card's answer is the person's: the agent is told to ask, never handed their claim
        let lines = clearing_commands(&log, &std::env::temp_dir(), &r, &Event::AboutToStart { criterion_id: "c1".into() });
        assert_eq!(lines, Vec::<String>::new());
    }

    #[test]
    fn a_commit_as_the_clear_teaches_no_command() {
        let r = Refusal {
            hold: None,
            state: Some("testing"),
            criterion_id: Some("c1".into()),
            clears: Clears::Moves(vec![familiar_ai_rules::holds::tests_only_clear()]),
        };
        assert!(clearing_commands(&Vec::new(), &std::env::temp_dir(), &r, &Event::Stops { main: true }).is_empty());
    }
}
