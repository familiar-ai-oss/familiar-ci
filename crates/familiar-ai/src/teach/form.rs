//! The claim forms the door prints: the list of claimable kinds, and each
//! kind's fields, shapes and one complete example, also shown after every
//! refusal. The fields are the facts crate's `FORM`; nothing here adds or
//! drops one.

use familiar_ai_facts::{form_row, FormRow};
use serde_json::{Map, Value};

use crate::teach::example::{self, Project};
use crate::door::claim::{claimable, command_form, shell_word};

/// The claimable kinds in planning order, each with one line of what it says.
pub const IN_ORDER: [(&str, &str); 23] = [
    ("intent_submitted", "line 1: what the person asked for (the sha is the head, read by the binary)"),
    ("one_reading", "after line 1, when the binary asks how the person's words read: they read one way, and why [draft]"),
    ("reading_alternatives", "after line 1, when the person's words read more than one way: the two or three readings, for the person to pick from [draft]"),
    ("criterion", "line 2: one thing that must hold, on the light or the full path"),
    ("behavior", "line 3, full path only: the behavior a criterion belongs to"),
    ("promise", "line 3, full path only: what that behavior accepts, gives back and refuses, and its cases"),
    ("place_chosen", "line 4: the anchor: the file, symbol and lines the criterion's code sits on"),
    ("new_work_spot_chosen", "line 4: new work beside an existing symbol"),
    ("new_work_no_spot", "line 4: new work with no spot in the code yet"),
    ("off_list_reason", "line 4, a pytest part whose place is not on its try's list: one line on why the fix is not in a function the failing test ran [draft]"),
    ("place_classified", "after line 4: each site the binary listed that names what the place names: applies (a test of its own shows the behavior there) or not_affected, with its reason [draft]"),
    ("test_planned", "line 6: a test file the criterion will create, or modify when it names an existing test file the tests-first commit edits (one per file)"),
    ("test_stack_detected", "line 7: the command that runs the tests, and the folder it covers"),
    ("touching_test_declared", "line 8: an existing test the change may break, run at the head and never edited by this criterion (none is allowed)"),
    ("criterion_head", "line 9: the first criterion on the string"),
    ("criterion_strung_on_head", "line 9: a further criterion, strung on its string's head; the links take their turns in the order claimed"),
    ("criterion_assigned", "line 10: the agent conversation that holds the criterion"),
    ("fix_location_category", "line 11: where the criterion's fix lives, claimed by the conversation that holds it"),
    ("test_change_announced", "while testing: announce a change to a sealed test before editing it"),
    ("existing_test_announced", "after a commit changed a test that stood before the plan: why the person's words call for its old expectation to change [draft]"),
    ("hold_answered", "the person's answer to a card (anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, no_test_stack)"),
    ("criterion_transfer", "hand a criterion to another agent conversation"),
    ("person_said_answered", "a prompt of the person's that needs no criterion: no work, or a question answered"),
];

/// What `familiar-ai claim` prints with no kind.
pub fn listing() -> String {
    let width = IN_ORDER.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let mut out = format!("familiar-ai claim: the {} claimable kinds, in the plan's order:\n", IN_ORDER.len());
    for (kind, line) in IN_ORDER {
        out.push_str(&format!("  {kind:<width$}  {line}\n"));
    }
    out.push_str("A kind alone (familiar-ai claim criterion, no fields) prints that kind's form and an example.\n");
    out
}

/// The promise example's cases (the owner's plan of 2026-10-25, §4) [draft words]: two normal
/// cases, an edge case (a boundary the person's words name) and the opposite, the same call with
/// the trigger absent; each with the short line the person reads on the cases card. Cases the
/// case-finder found (a1, …) or the person typed (p1, …) are added to it later.
pub const CASES: &str = r#"[{"case_id":"n1","kind":"normal","short":"a plain name is greeted","given":"the name Ada","expect":"hello Ada"},{"case_id":"n2","kind":"normal","short":"a name with a space is greeted in full","given":"a name with a space, Ada Lovelace","expect":"hello Ada Lovelace"},{"case_id":"e1","kind":"edge","short":"a very long name is greeted in full","given":"a name of 200 characters","expect":"greeted in full"},{"case_id":"o1","kind":"opposite","short":"an empty name is refused","given":"an empty name","expect":"refused, no greeting"}]"#;

/// The one_reading example's why, refused when claimed as it is [draft].
pub const ONE_READING_EXAMPLE: &str = "the person names the function and the value it must return";
/// The reading_alternatives example's readings, refused when claimed as they are [draft].
pub const READINGS_EXAMPLE: &str = r#"["greet says hello in place of hi","greet says hello before hi"]"#;

/// The off_list_reason example's reason, refused when claimed as it is [draft].
pub const OFF_LIST_EXAMPLE: &str = "the greeting's words come from a template the failing test never renders";

/// The place_classified example's reason, refused when claimed as it is [draft].
pub const PLACE_REASON_EXAMPLE: &str = "it only passes the name on to greet and returns its answer";

/// One complete example per kind as `(field, value)` pairs. The fields the
/// binary fills are left out on purpose.
pub fn example_args(kind: &str) -> Option<&'static [(&'static str, &'static str)]> {
    Some(match kind {
        "intent_submitted" => &[("text", "greet with hello")],
        "behavior" => &[("behavior_id", "b1"), ("text", "greeting by name")],
        "one_reading" => &[("why", ONE_READING_EXAMPLE)],
        "reading_alternatives" => &[("readings", READINGS_EXAMPLE)],
        "promise" => &[
            ("promise_id", "p1"),
            ("behavior_id", "b1"),
            ("accepts", "a name"),
            ("gives_back", "a greeting"),
            ("refuses", "an empty name"),
            ("cases", CASES),
        ],
        "criterion" => &[("criterion_id", "c1"), ("text", "greet says hello"), ("path", "full"), ("behavior_id", "b1"), ("promise_id", "p1")],
        "place_chosen" => &[
            ("criterion_id", "c1"),
            ("places", r#"[{"file_path":"src/app.py","symbol":"greet","line_start":10,"line_end":20}]"#),
        ],
        "new_work_spot_chosen" => &[
            ("criterion_id", "c1"),
            ("file_path", "src/app.py"),
            ("symbol", "greet_loudly"),
            ("beside", "greet"),
        ],
        "new_work_no_spot" => &[("criterion_id", "c1"), ("integration_point", "src/app.py:main")],
        "off_list_reason" => &[("criterion_id", "c1"), ("reason", OFF_LIST_EXAMPLE)],
        "place_classified" => &[
            ("criterion_id", "c1"),
            ("file", "src/app.py"),
            ("line", "42"),
            ("verdict", "not_affected"),
            ("reason", PLACE_REASON_EXAMPLE),
        ],
        "test_planned" => &[("criterion_id", "c1"), ("file_path", "tests/test_greet.py"), ("will", "create")],
        "test_stack_detected" => {
            static STACK: std::sync::OnceLock<[(&str, &str); 3]> = std::sync::OnceLock::new();
            STACK.get_or_init(|| [("project_root", "."), ("test_stack", "pytest"), ("test_command", crate::door::stack::example_command())])
        }
        "touching_test_declared" => &[
            ("criterion_id", "c1"),
            ("file_path", "tests/test_old.py"),
            ("test_name", "test_greet"),
        ],
        "criterion_head" => &[("criterion_id", "c1")],
        "criterion_strung_on_head" => &[("criterion_id", "c2"), ("head", "c1")],
        "criterion_assigned" => &[("criterion_id", "c1")],
        "fix_location_category" => &[("criterion_id", "c1"), ("category", ""), ("where", "")],
        "test_change_announced" => &[("criterion_id", "c1"), ("reason", "the assertion named the wrong file")],
        "existing_test_announced" => &[("file_path", "tests/test_old.py"), ("reason", "the person asked that leading slashes be kept, which this test said were trimmed")],
        "hold_answered" => &[
            ("criterion_id", "c1"),
            ("hold", "anchor_moved"),
            ("choice", "keep_going"),
            ("typed", ""),
        ],
        "criterion_transfer" => &[
            ("criterion_id", "c1"),
            ("from_conversation", "84be6339-0cc7-40ec-91b9-cae44365931b"),
            ("to_conversation", "84be6339-0cc7-40ec-91b9-cae44365931b/agent-7"),
        ],
        "person_said_answered" => &[("for", "12"), ("how", "no_work")],
        _ => return None,
    })
}

pub fn example_command(kind: &str) -> Option<String> {
    let args = example_args(kind)?;
    let mut parts = vec![format!("familiar-ai claim {kind}")];
    for (field, value) in args {
        parts.push(format!("--{field} {}", shell_word(&Value::String((*value).to_string()))));
    }
    Some(parts.join(" "))
}

pub fn shape(field: &str) -> &'static str {
    match field {
        "places" | "cases" | "readings" => "list",
        "path" => "one of full|light",
        "will" => "one of create|modify",
        "how" => "one of no_work|question",
        "category" => "one of this_repository|dependency|environment|request",
        "for" | "contract_version" | "line" => "whole number",
        "verdict" => "one of applies|not_affected",
        "project_root" => "folder",
        _ => "text",
    }
}

fn is_text(field: &str) -> bool {
    matches!(shape(field), "text" | "folder")
}

fn note(kind: &str, field: &str) -> &'static str {
    match (kind, field) {
        ("intent_submitted", "text") => "what the person asked for, in their words",
        ("intent_submitted", "sha") => "left out: the repository's head, read by the binary",
        ("intent_submitted", "for") => "the t of the person's prompt that asked for the work (the refusal of the first edit lists them: t12); left out: the person's latest prompt",
        ("behavior", "behavior_id") => "the behavior's id, e.g. b1",
        ("behavior", "text") => "the behavior in one sentence",
        ("promise", "promise_id") => "the promise's id, e.g. p1",
        ("promise", "behavior_id") => "the behavior it is the promise of",
        ("promise", "accepts") => "what it accepts, in words",
        ("promise", "gives_back") => "what it gives back, in words",
        ("promise", "refuses") => "what it refuses, in words",
        ("promise", "cases") => {
            "a JSON list of {case_id, kind, short, given, expect}: the calls the tests make and what each must give back; kind normal: the behavior asked for; opposite: the same call with the trigger absent; edge: a boundary the person's words name; at least one normal and one opposite (or --opposite_none); short: one line the person reads on the cases card, at most 70 characters; the cases the person adds there (a1, p1, …) join the list later"
        }
        ("promise", "contract_version") => "left out: written by the binary; a promise made under the contract needs its cases",
        ("promise", "opposite_none") => "only when no opposite case exists: why the trigger cannot be absent",
        ("criterion", "text") => "what must hold, in one sentence",
        // the owner (2026-10-11): the rulebook's approved line 2 note
        ("criterion", "path") => "--path light only when the anchor is documentation: a markdown file or a docstring; the binary checks it; everything else is --path full and also needs line 3",
        ("criterion", "behavior_id") => "full path only: the behavior it belongs to",
        ("criterion", "promise_id") => "full path only: the promise it belongs to",
        ("criterion", "for") => "the t of the person's prompt it answers (familiar-ai status names it: the person said (t12))",
        ("person_said_answered", "for") => "the t of the person's prompt it answers (familiar-ai status names it: the person said (t12))",
        ("person_said_answered", "how") => "no_work: it asked for no change; question: it was a question, answered",
        ("place_chosen", "places") => {
            "a JSON list of {file_path, symbol, line_start, line_end}: the file relative to the project, the symbol's name, the lines as whole numbers"
        }
        ("place_chosen", "hold") | ("place_chosen", "choice") => {
            "never claimed: the binary writes it on the place where the change landed, right after the person's keep_going"
        }
        ("new_work_spot_chosen", "file_path") => "the file the new work goes in, relative to the project",
        ("new_work_spot_chosen", "symbol") => "the new symbol's name",
        ("new_work_spot_chosen", "beside") => "the existing symbol the new work goes beside",
        ("new_work_no_spot", "integration_point") => "file:symbol, where the new work is reached from (a file and symbol that will call it), or, when nothing calls it yet, the file it is stored in; the teeth run puts it back",
        ("criterion_strung_on_head", "criterion_id") => "the criterion being strung on",
        ("criterion_strung_on_head", "head") => "the string's head, the criterion claimed with criterion_head; every link names it, a third part too",
        ("test_planned", "file_path") => "the test file, relative to the project (one claim per file)",
        ("test_planned", "will") => "create a new test file, or modify an existing one",
        ("test_planned", "test_command") => "the command that runs this criterion's tests, when it has its own; left out: the stack's",
        ("test_stack_detected", "codebase_id") => {
            "left out: the repository folder's name (a label only; this variant keeps no index)"
        }
        ("test_stack_detected", "project_root") => {
            ". is the whole project; src/ covers only the anchors under src/; an absolute path inside the project is stored relative; outside the project is refused"
        }
        ("test_stack_detected", "test_stack") => "the stack's name: pytest, cargo, sh, ...",
        ("test_stack_detected", "test_command") => {
            "what the binary runs from the project's root after each commit; not passed = failed"
        }
        ("touching_test_declared", "file_path") => "the existing test file, relative to the project",
        ("touching_test_declared", "test_name") => "the test's name in that file",
        ("criterion_assigned", "agent_conversation_id") => {
            "left out: this conversation (CLAUDE_CODE_SESSION_ID); a subagent passes session_id/agent_id"
        }
        ("criterion_transfer", "from_conversation") => "the conversation that holds it now",
        ("criterion_transfer", "to_conversation") => "the conversation it goes to (session_id, or session_id/agent_id)",
        ("fix_location_category", "category") => "choose one, the example picks none: this_repository: the fix is made in this checkout's own files; dependency: the code to change is in an installed package or library; environment: a tool or setting this machine lacks (no cargo, no network); request: someone else must do it; the last three cannot be made in this repository, and the person is asked",
        ("fix_location_category", "where") => "empty for this_repository; else where the fix lives (the library, the setting, whom to ask)",
        ("fix_location_category", "needs") => "optional: what the fix elsewhere needs",
        ("hold_answered", "hold") => "the card's hold: anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched or no_test_stack",
        ("hold_answered", "choice") => "one of the card's choices, as offered (re_plan, keep_going, release, try_again, take_over)",
        ("hold_answered", "typed") => "the person's own words, or empty",
        ("hold_answered", "run_id") => "the run behind the card, empty for a card with none; written by familiar-ai approve",
        ("hold_answered", "commit") => "the commit the card was answered at; written by familiar-ai approve",
        ("hold_answered", "approval") => "the t of the signed approval in .familiar/approvals/ that witnesses the answer; written by familiar-ai approve",
        ("test_change_announced", "reason") => "why the sealed test must change",
        ("existing_test_announced", "file_path") => "the existing test file the commit changed, relative to the project",
        ("existing_test_announced", "reason") => "what in the person's words calls for this test's old expectation to change",
        ("one_reading", "why") => "one line: what in the person's words, or in the code, settles how they read",
        ("reading_alternatives", "readings") => {
            "a JSON list of two or three readings, each one plain sentence the person can pick: the ways their words can be read that would change the work"
        }
        ("off_list_reason", "reason") => "one line: why the fix is not in a function the try listed (familiar-ai status shows the list); the seal's judge reads it",
        ("place_classified", "file") => "the listed site's file, as familiar-ai status lists it",
        ("place_classified", "line") => "the listed site's line, as listed",
        ("place_classified", "verdict") => "applies: the behavior asked for reaches this site too, and a test of its own shows it there; not_affected: it does not",
        ("place_classified", "reason") => "one line: for not_affected, why the behavior asked for does not reach this site (required); for applies, what it must give back there",
        (_, "criterion_id") => "the criterion's id, e.g. c1",
        _ => "",
    }
}

fn binary_fills(kind: &str, field: &str) -> bool {
    matches!(
        (kind, field),
        ("intent_submitted", "sha")
            | ("promise", "contract_version")
            | ("criterion_assigned", "agent_conversation_id")
            | ("test_stack_detected", "codebase_id")
    )
}

fn needed(row: &FormRow, field: &str) -> &'static str {
    if row.optional.contains(&field) {
        "optional"
    } else if binary_fills(row.kind, field) {
        "may be left out"
    } else {
        "required"
    }
}

/// The kind's form, with the project's own example when there is a project.
/// `None` for a kind the door does not claim.
pub fn form_text(kind: &str, project: Option<&Project>) -> Option<String> {
    if !claimable(kind) {
        return None;
    }
    let row = form_row(kind)?;
    let what = IN_ORDER.iter().find(|(k, _)| *k == kind).map(|(_, l)| *l).unwrap_or("");
    let mut example = command_form(kind, &Map::new(), project)?;
    if let Some(note) = project.and_then(|p| example::note(kind, p)) {
        example.push_str("\n  ");
        example.push_str(&note);
    }
    let field_width = row.fields.iter().map(|f| f.len() + 2).max().unwrap_or(0);
    let shape_width = row.fields.iter().map(|f| shape(f).len()).max().unwrap_or(0);
    let mut out = format!("familiar-ai claim {kind}: {what}\n");
    for &field in row.fields {
        let flag = format!("--{field}");
        out.push_str(&format!(
            "  {flag:<field_width$}  {:<shape_width$}  {:<15}  {}\n",
            shape(field),
            needed(row, field),
            note(kind, field)
        ));
    }
    out.push_str(&format!("example:\n  {example}\n"));
    out.push_str("The fields may instead come as one JSON object on stdin. A value that reads as JSON is JSON; anything else is text.\n");
    Some(out)
}

/// The kind's form, or the listing for a kind the door does not know.
pub fn after_refusal(kind: &str, project: Option<&Project>) -> String {
    form_text(kind, project).unwrap_or_else(listing)
}

/// Coerces each value toward the form's shape where nothing is lost; an
/// unregistered key is left for the door to refuse.
pub fn coerce(kind: &str, fields: &mut Map<String, Value>) {
    let Some(row) = form_row(kind) else {
        return;
    };
    for &field in row.fields {
        let Some(v) = fields.get_mut(field) else {
            continue;
        };
        match v {
            Value::Number(_) | Value::Bool(_) if is_text(field) || shape(field).starts_with("one of") => {
                *v = Value::String(v.to_string());
            }
            Value::String(s) if field == "line" => {
                if let Ok(n) = s.trim().parse::<u64>() {
                    *v = Value::from(n);
                }
            }
            Value::String(s) if field == "for" => {
                if let Ok(n) = s.trim().trim_start_matches('t').parse::<u64>() {
                    *v = Value::from(n);
                }
            }
            Value::String(s) if field == "places" || field == "cases" || field == "readings" => {
                if let Ok(parsed @ Value::Array(_)) = serde_json::from_str::<Value>(s) {
                    *v = parsed;
                }
            }
            _ => {}
        }
        if field == "places" {
            if let Value::Array(items) = v {
                for item in items.iter_mut() {
                    whole_numbers(item, &["line_start", "line_end"]);
                }
            }
        }
    }
}

/// Turns text that is a whole number into the number, for the keys named.
fn whole_numbers(item: &mut Value, keys: &[&str]) {
    let Value::Object(m) = item else {
        return;
    };
    for &k in keys {
        if let Some(Value::String(s)) = m.get(k) {
            if let Ok(n) = s.trim().parse::<u64>() {
                m.insert(k.to_string(), Value::from(n));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::door::claim::{fields_from_args, BINARY_KINDS};
    use familiar_ai_facts::{append_raw, KINDS};
    use serde_json::json;

    /// The promise example shows several cases (the owner's plan of 2026-10-25, §4) [draft words]:
    /// two normal cases, an edge case and the opposite, each with the short line the person reads.
    #[test]
    fn the_promise_example_shows_two_normal_cases_an_edge_and_the_opposite_each_with_a_short_line() {
        let cases: Vec<Value> = serde_json::from_str(CASES).expect("CASES is a JSON list");
        let at = |k: &str| cases.iter().map(|c| c[k].as_str().unwrap_or("").to_string()).collect::<Vec<_>>();
        assert_eq!(at("case_id"), ["n1", "n2", "e1", "o1"]);
        assert_eq!(at("kind"), ["normal", "normal", "edge", "opposite"]);
        assert_eq!(at("given"), ["the name Ada", "a name with a space, Ada Lovelace", "a name of 200 characters", "an empty name"]);
        assert_eq!(at("expect"), ["hello Ada", "hello Ada Lovelace", "greeted in full", "refused, no greeting"]);
        assert!(at("short").iter().all(|s| !s.is_empty() && s.chars().count() <= 70), "{:?}", at("short"));
        assert!(note("promise", "cases").contains("short"), "the note names the short line");
    }

    #[test]
    fn the_listing_names_the_twenty_two_claimable_kinds_once_each_in_the_plans_order() {
        let listed: Vec<&str> = IN_ORDER.iter().map(|(k, _)| *k).collect();
        for k in KINDS {
            assert_eq!(listed.contains(&k), claimable(k), "{k}");
        }
        for k in BINARY_KINDS {
            assert!(!listed.contains(&k), "{k}");
        }
        let text = listing();
        // definition change (the owner, 2026-10-20): existing_test_announced [draft] is claimable (20 became 21)
        // definition change (the owner's plan of 2026-10-21, step 3): place_classified [draft] is claimable (21 became 22)
        // definition change (the owner's plan of 2026-10-21, step 2): one_reading and reading_alternatives [draft] are claimable (22 became 24)
        // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23): off_list_reason [draft] is claimable (24 became 25)
        assert_eq!(text.lines().count(), 25, "{text}");
        assert!(text.find("  intent_submitted ") < text.find("  one_reading ") && text.find("  reading_alternatives ") < text.find("  criterion "), "{text}");
        assert!(text.find("  new_work_no_spot ") < text.find("  place_classified ") && text.find("  place_classified ") < text.find("  test_planned "), "{text}");
        let pos = |k: &str| text.find(&format!("  {k} ")).unwrap_or_else(|| panic!("{k} missing: {text}"));
        assert!(pos("intent_submitted") < pos("criterion"));
        assert!(pos("criterion") < pos("place_chosen"));
        assert!(pos("test_planned") < pos("test_stack_detected"));
        assert!(pos("criterion_head") < pos("criterion_assigned"));
        assert!(pos("criterion_assigned") < pos("hold_answered"));
    }

    #[test]
    fn every_example_claims_through_the_facts_door_with_the_binarys_fields_filled() {
        for (kind, _) in IN_ORDER {
            let args = example_args(kind).unwrap_or_else(|| panic!("{kind}: no example"));
            let argv: Vec<String> = args.iter().flat_map(|(f, v)| [format!("--{f}"), v.to_string()]).collect();
            let mut fields = fields_from_args(&argv).unwrap_or_else(|e| panic!("{kind}: {e}"));
            coerce(kind, &mut fields);
            let row = form_row(kind).expect("row");
            for &field in row.fields {
                if binary_fills(kind, field) {
                    assert!(!fields.contains_key(field), "{kind}: the example gives --{field}, which the binary fills");
                    fields.insert(field.to_string(), if field == "contract_version" { json!(1) } else { json!("filled") });
                }
            }
            // definition change (the owner, 2026-10-13): the example leaves the category for the agent to choose; empty is refused naming the four values
            if kind == "fix_location_category" {
                let e = append_raw(&Vec::new(), kind, fields.clone(), 0.0, "t").expect_err("an empty category is refused");
                assert!(["this_repository", "dependency", "environment", "request"].iter().all(|v| e.to_string().contains(v)), "{e}");
                fields.insert("category".to_string(), json!("this_repository"));
            }
            append_raw(&Vec::new(), kind, fields, 0.0, "t").unwrap_or_else(|e| panic!("{kind}: the example is refused: {e}"));
        }
    }

    #[test]
    fn the_example_command_is_shell_quoted_and_carries_no_placeholder() {
        for (kind, _) in IN_ORDER {
            let cmd = example_command(kind).expect(kind);
            assert!(cmd.starts_with(&format!("familiar-ai claim {kind} --")), "{cmd}");
            assert!(!cmd.contains('<') && !cmd.contains('>'), "{cmd}");
        }
        // definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
        assert_eq!(
            example_command("test_stack_detected").expect("form"),
            // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
            format!("familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{} -m pytest -q tests/test_greet.py'", crate::teach::example::python())
        );
        assert_eq!(
            example_command("hold_answered").expect("form"),
            "familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''"
        );
        assert!(example_command("commit_seen").is_none());
    }

    #[test]
    fn the_form_names_every_field_its_shape_whether_it_is_needed_and_the_example() {
        for (kind, _) in IN_ORDER {
            let text = form_text(kind, None).expect(kind);
            assert!(text.starts_with(&format!("familiar-ai claim {kind}: ")), "{text}");
            let row = form_row(kind).expect("row");
            for &field in row.fields {
                let line = text.lines().find(|l| l.trim_start().starts_with(&format!("--{field} "))).unwrap_or_else(|| panic!("{kind}: --{field} missing\n{text}"));
                assert!(line.contains(shape(field)), "{line}");
                assert!(line.contains(needed(row, field)), "{line}");
                assert!(!note(kind, field).is_empty(), "{kind}.{field} has no note");
            }
            assert!(text.contains(&format!("example:\n  {}\n", example_command(kind).expect("example"))), "{text}");
        }
        let t = form_text("test_stack_detected", None).expect("form");
        assert!(t.contains("--project_root  folder  required         . is the whole project"), "{t}");
        assert!(t.contains("--codebase_id   text    may be left out  left out: the repository folder's name"), "{t}");
        let t = form_text("criterion", None).expect("form");
        assert!(t.contains("--path          one of full|light  required"), "{t}");
        assert!(t.contains("--behavior_id   text               optional"), "{t}");
        // definition change (the owner, 2026-10-15): every link names the string's head; the order is the order claimed
        let t = form_text("criterion_strung_on_head", None).expect("form");
        assert!(t.starts_with("familiar-ai claim criterion_strung_on_head: line 9: a further criterion, strung on its string's head; the links take their turns in the order claimed\n"), "{t}");
        assert!(t.contains("--head          text  required         the string's head, the criterion claimed with criterion_head; every link names it, a third part too\n"), "{t}");
        for k in BINARY_KINDS {
            assert!(form_text(k, None).is_none(), "{k}");
        }
        assert!(form_text("criterion_behavior_chosen", None).is_none());
        assert_eq!(after_refusal("criterion_behavior_chosen", None), listing());
    }

    #[test]
    fn inside_a_project_the_forms_example_is_the_projects_with_the_note_when_the_command_is_not_known() {
        let root = std::env::temp_dir();
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], false, false, 0.0, "t").expect("fixture");
        let project = Project { log: &log, root: &root };
        let t = form_text("test_stack_detected", Some(&project)).expect("form");
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert!(t.contains(&format!("example:\n  familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{} -m pytest -q tests/test_c1.py'\nThe fields", crate::teach::example::python())), "the planned .py file: {t}");
        // Tests that are not pytest's get the form's example and a note naming them.
        let mut shell = Map::new();
        shell.insert("criterion_id".into(), Value::from("c1"));
        shell.insert("file_path".into(), Value::from("tests/test_c1.sh"));
        shell.insert("will".into(), Value::from("create"));
        let log2 = append_raw(&log, "test_planned", shell, 1.0, "t").expect("planned");
        let project2 = Project { log: &log2, root: &root };
        let t = form_text("test_stack_detected", Some(&project2)).expect("form");
        // definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert!(t.contains(&format!("example:\n  familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{} -m pytest -q tests/test_greet.py'\n  (the example's command is pytest's; this plan's tests are tests/test_c1.py tests/test_c1.sh: --test_command is the command that runs these files, from the project's root, and --test_stack its stack)\nThe fields", crate::teach::example::python())), "{t}");
        let t = form_text("criterion", Some(&project)).expect("form");
        assert!(t.contains("example:\n  familiar-ai claim criterion --criterion_id c2 "), "{t}");
        assert!(!t.contains('<') && !t.contains('>'), "{t}");
    }

    #[test]
    fn a_bare_number_where_text_is_wanted_becomes_the_text_and_a_list_as_text_becomes_the_list() {
        let mut f = Map::new();
        f.insert("criterion_id".into(), json!(1));
        f.insert("head".into(), json!(true));
        coerce("criterion_strung_on_head", &mut f);
        assert_eq!(f["criterion_id"], json!("1"));
        assert_eq!(f["head"], json!("true"));

        let mut f = Map::new();
        f.insert("criterion_id".into(), json!("c1"));
        f.insert("places".into(), json!(r#"[{"file_path":"src/a.py","symbol":"f","line_start":"375","line_end":"380"}]"#));
        coerce("place_chosen", &mut f);
        assert_eq!(f["places"], json!([{"file_path":"src/a.py","symbol":"f","line_start":375,"line_end":380}]));

        // A closed-set field given as a number becomes text, which the door then refuses.
        let mut f = Map::new();
        f.insert("path".into(), json!(1));
        coerce("criterion", &mut f);
        assert_eq!(f["path"], json!("1"));

        // Lossy coercions are not made.
        let mut f = Map::new();
        f.insert("text".into(), json!(["a"]));
        f.insert("places".into(), json!("not a list"));
        f.insert("line_start".into(), json!(375));
        coerce("place_chosen", &mut f);
        assert_eq!(f["text"], json!(["a"]));
        assert_eq!(f["places"], json!("not a list"));
        assert_eq!(f["line_start"], json!(375));
    }
}
