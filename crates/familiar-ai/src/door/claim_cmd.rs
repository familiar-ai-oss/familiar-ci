//! The `claim` and `plan` commands: `claim` appends one fact to the project
//! record, and `plan` appends a JSON array of them from stdin through the same
//! path, stopping at the first refusal. A refusal shows the kind's form so the
//! agent does not guess twice.

use familiar_ai_facts::{Fact, Log, Record};
use serde_json::{Map, Value};

use crate::Outcome;
use crate::door::project::{find_project, open_project, open_record, plain, start_dir};
use crate::door::{catch_up, claim, project_plan, stack, TENANT};
use crate::input::Surroundings;
use crate::teach::example::{self, Project};
use crate::teach::words::LEAD;
use crate::teach::{form, picture, state_lines};

/// `familiar-ai claim [<kind> [--<field> <value> ...]]`.
pub fn run_claim(
    kind: Option<&str>,
    args: &[String],
    input: &str,
    around: &Surroundings,
) -> Outcome {
    let Some(kind) = kind else {
        return plain(0, form::listing(), String::new());
    };
    if args.is_empty() && input.trim().is_empty() {
        // Show the form, with the project's values when there is a project here.
        let here = project_here(around);
        let project = here.as_ref().map(|(log, root)| Project { log, root });
        return match form::form_text(kind, project.as_ref()) {
            Some(text) => plain(0, text, String::new()),
            None => refused(kind, &not_claimable(kind), None),
        };
    }
    match claim_one(kind, args, input, around) {
        OneClaim::Refused(out) => out,
        OneClaim::Appended { fact, tail } => {
            let said = format!("{}claimed {}\n", crate::teach::words::LEAD, fact.to_json());
            match tail.exit_code {
                0 => plain(0, format!("{said}{}", tail.stdout_text), String::new()),
                code => plain(code, said, tail.stderr_text),
            }
        }
    }
}

/// `familiar-ai plan`: claim each `{"kind": …}` object of the stdin array in
/// order through `claim_one`, or print the array's form when stdin is empty.
/// Lines before a refusal stay appended and nothing after it is appended.
pub fn run_plan(input: &str, around: &Surroundings) -> Outcome {
    let text = input.trim();
    if text.is_empty() {
        return plain(0, plan_form(), String::new());
    }
    let lines: Vec<Value> = match serde_json::from_str(text) {
        Ok(Value::Array(lines)) => lines,
        _ => return plan_shape_refusal(),
    };
    // Check every line before appending any, so a malformed line appends nothing.
    let kinds: Vec<String> = match lines
        .iter()
        .map(|line| line.get("kind").and_then(Value::as_str).map(str::to_string))
        .collect::<Option<Vec<_>>>()
    {
        Some(kinds) => kinds,
        None => return plan_shape_refusal(),
    };
    // F15 (finding 35): no fact says a checklist was shown, so a planning already begun (the
    // intent claimed, the plan not confirmed) is a repeat, and its reply names only what is missing.
    let repeat = project_here(around).is_some_and(|(log, _)| {
        let v = familiar_ai_rules::View::new(&log);
        familiar_ai_rules::plan::line_1(&v) && familiar_ai_rules::plan::plan_state(&v) == "planning"
    });
    let mut claimed = 0usize;
    let mut last: Option<Outcome> = None;
    for (i, (kind, line)) in kinds.iter().zip(&lines).enumerate() {
        let mut fields = line.as_object().expect("an object, checked above").clone();
        fields.remove("kind");
        match claim_one(kind, &[], &Value::Object(fields).to_string(), around) {
            OneClaim::Refused(out) => {
                return plain(
                    1,
                    out.stdout_text,
                    format!(
                        "{LEAD}line {} of {}, {kind}: {}",
                        i + 1,
                        lines.len(),
                        out.stderr_text
                    ),
                );
            }
            OneClaim::Appended { fact: _, tail } => {
                claimed += 1;
                last = Some(tail);
            }
        }
    }
    let tail = match last {
        Some(tail) => tail,
        // An empty array appends nothing; print the project's status as it is.
        None => {
            let (root, record) = match open_project(around) {
                Ok(x) => x,
                Err(out) => return out,
            };
            let before: Log = match record.read_all() {
                Ok(log) => log,
                Err(e) => {
                    return plain(
                        1,
                        String::new(),
                        format!(
                            "familiar-ai: {}\n",
                            crate::door::cannot("read its record", &e)
                        ),
                    )
                }
            };
            state_and_picture(&record, &root, &before, around)
        }
    };
    let said = format!("{LEAD}{claimed} lines claimed\n");
    match tail.exit_code {
        0 if repeat => plain(0, format!("{said}{}", crate::teach::words::only_missing(&tail.stdout_text)), String::new()),
        0 => plain(0, format!("{said}{}", tail.stdout_text), String::new()),
        code => plain(code, said, tail.stderr_text),
    }
}

/// The refusal of a claim naming the reserved part `setup` as its criterion (plan A-4) [draft words].
pub const RESERVED_SETUP: &str = "setup is the reserved part that carries the project's own cards (the test setup); give the criterion another id, such as c1";

/// The result of one claim: its refusal, or the appended fact with what
/// `claim` prints after it.
pub(crate) enum OneClaim {
    Refused(Outcome),
    Appended { fact: Fact, tail: Outcome },
}

/// Check, fill in and append one fact, then catch up as a hook call. Both
/// `claim` and `plan` use it, so a line of `plan` leaves the record as the
/// same `claim` would.
pub(crate) fn claim_one(
    kind: &str,
    args: &[String],
    fields_stdin: &str,
    around: &Surroundings,
) -> OneClaim {
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return OneClaim::Refused(out),
    };
    let before: Log = match record.read_all() {
        Ok(log) => log,
        Err(e) => {
            return OneClaim::Refused(refused(
                kind,
                &crate::door::cannot("read its record", &e),
                None,
            ))
        }
    };
    let project = Project {
        log: &before,
        root: &root,
    };
    if claim::BINARY_KINDS.contains(&kind) {
        return OneClaim::Refused(refused(
            kind,
            &claim::ClaimError::BinaryKind(kind.to_string()),
            Some(&project),
        ));
    }
    let mut fields = match claim::fields_of(args, fields_stdin) {
        Ok(f) => f,
        Err(e) => return OneClaim::Refused(refused(kind, &e, Some(&project))),
    };
    // plan A-4: `setup` is the reserved part that carries the project's own cards; only an
    // answer to one of its cards names it
    if kind != "hold_answered" && fields.get("criterion_id").and_then(serde_json::Value::as_str) == Some(familiar_ai_rules::reading::SETUP_PART) {
        return OneClaim::Refused(refused(kind, &claim::ClaimError::Door(RESERVED_SETUP.into()), Some(&project)));
    }
    form::coerce(kind, &mut fields);
    if let Err(e) = fill_in(kind, &mut fields, &root, &before, around) {
        return OneClaim::Refused(refused(kind, &e, Some(&project)));
    }
    // F12: a place carrying the person's keep_going is written only by the binary, right after the answer.
    if kind == "place_chosen" && (fields.contains_key("hold") || fields.contains_key("choice")) {
        let e = claim::ClaimError::Door(
            "a place_chosen with hold and choice is the place the binary appends right after the person's keep_going; claim a place without them".into(),
        );
        return OneClaim::Refused(refused(kind, &e, Some(&project)));
    }
    // Only the person answers a card hold, so an agent's shell is refused.
    if let Err(e) =
        crate::teach::card::card_check(kind, &fields, around.session_id.as_deref(), &project)
    {
        return OneClaim::Refused(refused(kind, &e, Some(&project)));
    }
    let fact = match record.append_raw(kind, fields, around.now, TENANT) {
        Ok(fact) => fact,
        Err(e) => {
            return OneClaim::Refused(refused(
                kind,
                &claim::ClaimError::Door(e.to_string()),
                Some(&project),
            ))
        }
    };
    OneClaim::Appended {
        fact,
        tail: state_and_picture(&record, &root, &before, around),
    }
}

/// The warning for a claim that replaced the test command under an already sealed part (the
/// owner, 2026-10-16): stacks are per root, so the latest at a root is every part's command
/// there. Told, never refused.
fn replaced_commands(before: &Log, after: &Log) -> Vec<String> {
    use familiar_ai_rules::{criterion, lifecycle, View};
    // The rules' "no bound": the command in force after every fact.
    const INF: i64 = i64::MAX;
    let (was, now) = (View::new(before), View::new(after));
    familiar_ai_rules::plan::criteria(&now)
        .into_iter()
        .filter(|c| criterion::sealed_at(&now, c, criterion::Place::Laptop).is_some())
        .filter_map(|c| {
            let (old, new) = (lifecycle::command_at(&was, &c, INF)?, lifecycle::command_at(&now, &c, INF)?);
            (old != new).then(|| {
                format!(
                    "{}this stack replaces the test command of sealed {} ('{old}' becomes '{new}'): it reads as changed after its seal until its tests are proven again under the new command",
                    crate::teach::words::LEAD,
                    crate::teach::words::quoted(&now, &c)
                )
            })
        })
        .collect()
}

/// Catch up, project the plan, and print the state lines and the picture.
/// A catch-up that cannot run exits 1 with its error.
fn state_and_picture(
    record: &Record,
    root: &std::path::Path,
    before: &Log,
    around: &Surroundings,
) -> Outcome {
    // places before the fix (the owner's plan of 2026-10-21, step 3) [draft]: after a claim, once
    // the plan is confirmed and a place's lines checked, the sites that name what it names
    let listed = |mut log: Log| -> Result<Log, String> {
        crate::door::places::write_owed(record, root, &mut log, around.now, TENANT).map_err(|e| crate::door::cannot("write its record", &e))?;
        Ok(log)
    };
    let log = match catch_up(record, root, around)
        .and_then(listed)
        .and_then(|log| project_plan(root, &log).map(|()| log))
    {
        Ok(log) => log,
        Err(message) => return plain(1, String::new(), format!("familiar-ai: {message}\n")),
    };
    let mut out = String::new();
    for line in replaced_commands(before, &log) {
        out.push_str(&line);
        out.push('\n');
    }
    for line in state_lines(before, &log, around.judge_dir.is_some(), root) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(&picture(&log, root, around.now, around.judge_dir.is_some(), None));
    out.push('\n');
    plain(0, out, String::new())
}

/// The refusal for stdin that is not a JSON array of objects with a `kind`.
fn plan_shape_refusal() -> Outcome {
    plain(
        1,
        String::new(),
        "familiar-ai: plan takes a JSON array of {\"kind\": …, …} objects on stdin\n".to_string(),
    )
}

/// The JSON array `plan` prints with nothing on stdin: each plan kind in
/// order with its form's example, so the example cannot drift from the forms.
pub fn plan_form() -> String {
    const PLAN_KINDS: [&str; 14] = [
        "intent_submitted",
        "behavior",
        "promise",
        "criterion",
        "place_chosen",
        "new_work_spot_chosen",
        "new_work_no_spot",
        "test_planned",
        "test_stack_detected",
        "touching_test_declared",
        "criterion_head",
        "criterion_strung_on_head",
        "criterion_assigned",
        "fix_location_category",
    ];
    let mut lines: Vec<Value> = Vec::new();
    for kind in PLAN_KINDS {
        let example = form::example_args(kind).expect("the plan's kinds have examples");
        let mut line = Map::new();
        line.insert("kind".to_string(), Value::String(kind.to_string()));
        for (field, value) in example {
            line.insert((*field).to_string(), claim::value_of(value));
        }
        lines.push(Value::Object(line));
    }
    serde_json::to_string_pretty(&Value::Array(lines)).expect("json")
}

/// The error for a kind with no form: the binary's own, or one the door does not know.
fn not_claimable(kind: &str) -> claim::ClaimError {
    if claim::BINARY_KINDS.contains(&kind) {
        claim::ClaimError::BinaryKind(kind.to_string())
    } else {
        claim::ClaimError::Door(format!("append: unknown kind '{kind}'"))
    }
}

/// A refusal line on stderr followed by the kind's form, or the listing for
/// an unknown kind.
fn refused(kind: &str, e: &dyn std::fmt::Display, project: Option<&Project>) -> Outcome {
    plain(
        1,
        String::new(),
        format!(
            "familiar-ai: not claimed yet · {e}\n{}",
            form::after_refusal(kind, project)
        ),
    )
}

/// The log and root of the project here, or `None` when there is none or it
/// cannot be read.
fn project_here(around: &Surroundings) -> Option<(Log, std::path::PathBuf)> {
    let root = find_project(&start_dir(around))?;
    let record = open_record(&root, around.record_wait).ok().flatten()?;
    let log = record.read_all().ok()?;
    Some((log, root))
}

/// Fill in the fields the binary supplies when left out, normalise
/// `project_root`, and check `test_command` against the project.
pub(crate) fn fill_in(
    kind: &str,
    fields: &mut Map<String, Value>,
    root: &std::path::Path,
    log: &Log,
    around: &Surroundings,
) -> Result<(), claim::ClaimError> {
    // The intent answers the person's latest prompt; with none yet it is refused.
    if kind == "intent_submitted" && !fields.contains_key("for") {
        match log.iter().rev().find(|f| f.kind == "person_said") {
            Some(p) => {
                fields.insert("for".to_string(), Value::from(p.t));
            }
            None => return Err(claim::ClaimError::NoPrompt),
        }
    }
    // places before the fix [draft]: a listed site only, and not_affected with its reason
    if kind == familiar_ai_rules::places::CLASSIFIED {
        let s = |k: &str| fields.get(k).and_then(Value::as_str).map(str::to_string);
        if let (Some(c), Some(file), Some(line)) = (s("criterion_id"), s("file"), fields.get("line").and_then(Value::as_u64)) {
            let v = familiar_ai_rules::View::new(log);
            let (verdict, reason) = (s("verdict").unwrap_or_default(), s("reason").unwrap_or_default());
            if let Some(why) = familiar_ai_rules::places::refusal(&v, &c, &file, line, &verdict, &reason) {
                return Err(claim::ClaimError::Door(why));
            }
            if reason.trim() == crate::teach::form::PLACE_REASON_EXAMPLE {
                return Err(claim::ClaimError::Door(format!("the reason is the example; say in one line why the behavior asked for does not reach {file}:{line}, or what it must give back there")));
            }
        }
    }
    // a place off the try's list [draft]: its reason says something, and is not the example's
    if kind == familiar_ai_rules::trace::OFF_LIST {
        let reason = fields.get("reason").and_then(Value::as_str).unwrap_or("");
        if let Some(refusal) = familiar_ai_rules::trace::refused_reason(reason) {
            return Err(claim::ClaimError::Door(refusal));
        }
        if reason.trim() == crate::teach::form::OFF_LIST_EXAMPLE {
            return Err(claim::ClaimError::Door("the reason is the example's; say in one line why the fix is not in a function the try listed".into()));
        }
    }
    // a clarifying question on a signal [draft]: two or three distinct readings, a why that says something
    if kind == familiar_ai_rules::ask::ALTERNATIVES {
        let readings: Vec<&str> = fields.get("readings").and_then(Value::as_array).map(|rs| rs.iter().map(|r| r.as_str().unwrap_or("")).collect()).unwrap_or_default();
        if let Some(why) = familiar_ai_rules::ask::refused_readings(&readings) {
            return Err(claim::ClaimError::Door(why));
        }
        let example: Vec<String> = serde_json::from_str(crate::teach::form::READINGS_EXAMPLE).unwrap_or_default();
        if readings.iter().any(|r| example.iter().any(|e| e == r.trim())) {
            return Err(claim::ClaimError::Door("a reading is the example's; write the readings you see in the person's words".into()));
        }
    }
    if kind == familiar_ai_rules::ask::ONE {
        let why = fields.get("why").and_then(Value::as_str).unwrap_or("");
        if let Some(refusal) = familiar_ai_rules::ask::refused_why(why) {
            return Err(claim::ClaimError::Door(refusal));
        }
        if why.trim() == crate::teach::form::ONE_READING_EXAMPLE {
            return Err(claim::ClaimError::Door("the why is the example's; say in one line what settles how the person's words read".into()));
        }
    }
    match kind {
        "intent_submitted" if !fields.contains_key("sha") => {
            match familiar_ai_git::head_commit(root) {
                Ok(Some(head)) => {
                    fields.insert("sha".to_string(), Value::from(head));
                }
                Ok(None) => {
                    return Err(claim::ClaimError::NoHead(
                        "the repository has no commit".into(),
                    ))
                }
                Err(e) => return Err(claim::ClaimError::NoHead(e.to_string())),
            }
        }
        // the contract [draft names]: every promise the door takes is made under the contract, so
        // line 3 holds it to its cases; a promise written before carries no version
        "promise" => {
            fields.insert("contract_version".to_string(), Value::from(familiar_ai_rules::plan::CONTRACT_VERSION));
        }
        "criterion_assigned" if !fields.contains_key("agent_conversation_id") => {
            match &around.session_id {
                Some(id) => {
                    fields.insert(
                        "agent_conversation_id".to_string(),
                        Value::from(id.as_str()),
                    );
                }
                None => return Err(claim::ClaimError::NoConversation),
            }
        }
        "test_stack_detected" => {
            if !fields.contains_key("codebase_id") {
                fields.insert(
                    "codebase_id".to_string(),
                    Value::from(stack::codebase_id_of(root)),
                );
            }
            if let Some(Value::String(given)) = fields.get("project_root") {
                let stored = stack::normalise_project_root(root, given)
                    .map_err(claim::ClaimError::RootOutside)?;
                fields.insert("project_root".to_string(), Value::from(stored));
            }
            // the command gate (plan §6.3): while the person's fix_what_broke_it stands, no new
            // command for the stack root covering that part
            let v = familiar_ai_rules::View::new(log);
            if let Some(Value::String(r)) = fields.get("project_root") {
                if let Some(c) = familiar_ai_rules::lifecycle::command_locked_at(&v, r) {
                    return Err(claim::ClaimError::Command(crate::teach::words::command_locked(&v, &c)));
                }
            }
            if let Some(Value::String(command)) = fields.get("test_command") {
                stack::check_command(root, command, &example::planned_files(log))
                    .map_err(claim::ClaimError::Command)?;
            }
        }
        // A test file is named after what it tests, never after a plan id (2 of 19 shipped tests/test_c1.py).
        "test_planned" => {
            if let (Some(Value::String(c)), Some(Value::String(file))) = (fields.get("criterion_id"), fields.get("file_path")) {
                if example::named_for_a_plan_id(file, c) {
                    return Err(claim::ClaimError::Door(format!("{file} is the example; name the test file after what it tests")));
                }
            }
        }
        // Every link names its string's head, and the links' order is the order claimed (the owner, 2026-10-15).
        "criterion_strung_on_head" => {
            if let (Some(Value::String(c)), Some(Value::String(head))) = (fields.get("criterion_id"), fields.get("head")) {
                let v = familiar_ai_rules::View::new(log);
                const RULE: &str = "(the links of a string all name its head; their order is the order they were claimed in)";
                if c == head {
                    return Err(claim::ClaimError::Door(format!("{c} is not strung on itself; name the head of the string it joins")));
                }
                if !example::is_head(&v, head) {
                    let said = match example::head_of(&v, head) {
                        Some(h) => format!("{head} is not a head; the head of its string is {h}: --head {h} {RULE}"),
                        None => match example::last_strings_head(&v) {
                            Some(h) => format!("{head} is not a head; it is on no string: claim criterion_head --criterion_id {head} first, or name a head: --head {h} {RULE}"),
                            None => format!("{head} is not a head; it is on no string: claim criterion_head --criterion_id {head} first {RULE}"),
                        },
                    };
                    return Err(claim::ClaimError::Door(said));
                }
            }
        }
        // The integration point is kept as typed, only trimmed.
        "new_work_no_spot" => {
            if let Some(Value::String(ip)) = fields.get("integration_point") {
                let trimmed = ip.trim().to_string();
                fields.insert("integration_point".to_string(), Value::from(trimmed));
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn around() -> Surroundings {
        Surroundings {
            now: 0.0,
            project_dir: None,
            cwd: Some(std::env::temp_dir()),
            session_id: None,
            data_dir: None,
            record_wait: Duration::ZERO,
            test_run_wait: Duration::from_secs(1),
            run_inline: Duration::ZERO,
            runner: None,
            judge_dir: None,
            judge_wait: Duration::ZERO,
            investigator: None,
            conversation: None,
            bench: false,
        }
    }

    fn fields(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn the_stacks_root_is_straightened_and_its_id_filled_in() {
        let root = PathBuf::from("/w/pydantic");
        let log: Log = Vec::new();
        let mut f = fields(&[
            ("project_root", Value::from(".")),
            ("test_stack", Value::from("pytest")),
            ("test_command", Value::from("pytest -q")),
        ]);
        fill_in("test_stack_detected", &mut f, &root, &log, &around()).expect("filled");
        assert_eq!(f["project_root"], Value::from(""));
        assert_eq!(f["codebase_id"], Value::from("pydantic"));

        let mut f = fields(&[
            ("codebase_id", Value::from("mine")),
            ("project_root", Value::from("/w/pydantic/pydantic")),
        ]);
        fill_in("test_stack_detected", &mut f, &root, &log, &around()).expect("filled");
        assert_eq!(f["project_root"], Value::from("pydantic/"));
        assert_eq!(f["codebase_id"], Value::from("mine"), "a given id stays");

        let mut f = fields(&[("project_root", Value::from("/work/repo"))]);
        let err =
            fill_in("test_stack_detected", &mut f, &root, &log, &around()).expect_err("outside");
        assert!(matches!(err, claim::ClaimError::RootOutside(_)), "{err}");
        assert!(
            err.to_string()
                .contains("/work/repo is outside the project at /w/pydantic"),
            "{err}"
        );

        // a root that is not text is left to the facts door to refuse
        let mut f = fields(&[("project_root", Value::from(["a"]))]);
        fill_in("test_stack_detected", &mut f, &root, &log, &around()).expect("left alone");
        assert_eq!(f["project_root"], Value::from(["a"]));

        // the example command claimed verbatim, nothing planned and no such file: refused as the example
        let mut f = fields(&[
            ("project_root", Value::from(".")),
            ("test_stack", Value::from("pytest")),
            ("test_command", Value::from(stack::example_command())),
        ]);
        let err = fill_in("test_stack_detected", &mut f, &root, &log, &around())
            .expect_err("the example");
        assert!(matches!(err, claim::ClaimError::Command(_)), "{err}");
        assert!(err.to_string().contains("is the example"), "{err}");
    }

    #[test]
    fn the_listing_and_the_form_need_no_project() {
        let out = run_claim(None, &[], "", &around());
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.stdout_text, form::listing());
        let out = run_claim(Some("test_stack_detected"), &[], "  \n", &around());
        assert_eq!(out.exit_code, 0);
        assert_eq!(
            out.stdout_text,
            form::form_text("test_stack_detected", None).expect("form")
        );
        assert!(out.stderr_text.is_empty());
    }

    #[test]
    fn plan_with_nothing_on_stdin_prints_the_array_form_of_the_plan_kinds() {
        let out = run_plan("  \n", &around());
        assert_eq!(out.exit_code, 0);
        assert!(out.stderr_text.is_empty());
        let arr: serde_json::Value =
            serde_json::from_str(&out.stdout_text).expect("the form is a JSON array");
        let arr = arr.as_array().expect("an array");
        assert_eq!(arr.len(), 14, "the plan's kinds, in the plan's order");
        assert_eq!(arr[0]["kind"], "intent_submitted");
        assert_eq!(arr[13]["kind"], "fix_location_category");
        // the form's examples, as claim reads them: a value that reads as JSON is JSON
        // definition change (the owner, 2026-10-13): the example is a code anchor's: the full path, its behavior and promise before it
        assert_eq!(arr[3]["criterion_id"], "c1");
        assert_eq!(arr[3]["path"], "full");
        assert_eq!(arr[3]["behavior_id"], "b1");
        assert_eq!(arr[3]["promise_id"], "p1");
        assert!(arr[4]["places"].is_array(), "{arr:?}");
    }

    #[test]
    fn plan_with_stdin_that_is_not_an_array_of_objects_with_kinds_refuses_with_one_line() {
        for stdin in [
            r#"{"kind":"criterion"}"#,
            "[1,2]",
            "not json at all",
            r#"{"kind": 5}"#,
        ] {
            let out = run_plan(stdin, &around());
            assert_eq!(out.exit_code, 1, "{stdin}");
            assert_eq!(out.stdout_text, "", "{stdin}");
            assert_eq!(
                out.stderr_text,
                "familiar-ai: plan takes a JSON array of {\"kind\": …, …} objects on stdin\n",
                "{stdin}"
            );
        }
    }

    #[test]
    fn a_kind_with_no_form_is_refused_with_the_listing_or_its_reason() {
        let out = run_claim(Some("criterion_behavior_chosen"), &[], "", &around());
        assert_eq!(out.exit_code, 1);
        assert_eq!(out.stdout_text, "");
        assert!(
            out.stderr_text.starts_with(
                "familiar-ai: not claimed yet · append: unknown kind 'criterion_behavior_chosen'\n"
            ),
            "{}",
            out.stderr_text
        );
        assert!(
            out.stderr_text.ends_with(&form::listing()),
            "{}",
            out.stderr_text
        );
        let out = run_claim(Some("commit_seen"), &[], "", &around());
        assert_eq!(out.exit_code, 1);
        assert!(
            out.stderr_text.contains("written by familiar-ai itself"),
            "{}",
            out.stderr_text
        );
    }
}
