//! The sample world every walk uses, the move type, and the plan's moves,
//! ported from the reference model (not in this repository). The hooks read
//! these moves to name what must come first, so they live in the rules crate.

use familiar_ai_facts::{append_raw, AppendError, Fact, Log};
use serde_json::{json, Map, Value};

use crate::plan;
use crate::reader::{last_of, text, View};

/// A place in the code: the four fields a planning check names.
pub struct SamplePlace {
    pub file_path: &'static str,
    pub symbol: &'static str,
    pub line_start: i64,
    pub line_end: i64,
}

impl SamplePlace {
    pub fn to_json(&self) -> Value {
        json!({ "file_path": self.file_path, "symbol": self.symbol, "line_start": self.line_start, "line_end": self.line_end })
    }
}

/// A test of the sample world: the file it lives in and its name.
pub struct SampleTest {
    pub file_path: &'static str,
    pub test_name: &'static str,
}

/// One criterion of the sample world.
pub struct SampleCriterion {
    pub criterion_id: &'static str,
    pub text: &'static str,
    pub path: &'static str,
    pub behavior_id: &'static str,
    pub promise_id: &'static str,
    pub place: SamplePlace,
    pub tests: &'static [SampleTest],
    pub touching: &'static [SampleTest],
    pub agent_conversation_id: &'static str,
    /// The `tests_hash` as sealed, and after an edit.
    pub hash_sealed: &'static str,
    pub hash_edited: &'static str,
    /// The sample's opaque `criterion_rev` on `tests_observed`; the model
    /// never computes one.
    pub criterion_rev: &'static str,
}

pub const CODEBASE_ID: &str = "cb1";
/// The intent the sample plans against.
pub const INTENT_TEXT: &str = "let a person log in and reset a forgotten password";
/// Main's sha at submit.
pub const INTENT_SHA: &str = "base";
pub const BEHAVIOR_ID: &str = "b1";
pub const BEHAVIOR_TEXT: &str = "a person logs in with a password";
pub const PROMISE_ID: &str = "p1";
/// What the promise accepts.
pub const PROMISE_ACCEPTS: &str = "a user name and a password";
/// What the promise gives back.
pub const PROMISE_GIVES_BACK: &str = "a session";
/// What the promise refuses.
pub const PROMISE_REFUSES: &str = "a wrong password";
/// The two agent conversations (a1 holds both criteria at first).
pub const AGENTS: [&str; 2] = ["a1", "a2"];
/// The lines hash at planning.
pub const LINES_PLANNING: &str = "L0";
/// The lines hash after an outside edit.
pub const LINES_OUTSIDE: &str = "LX";
/// The lines hash after another criterion's change (the owner, 2026-10-11).
pub const LINES_CHANGED: &str = "L1";
/// [reading] The sample's: another criterion's commit moved the anchor's
/// lines down three (the binary maps the range from the diff).
pub const MAPPED_SHIFT: i64 = 3;
/// [reading] The sample's: the lines the change landed on when it landed
/// beside the anchor, which the binary reads from the repository's hunks (F12).
pub const LINES_LANDED: (i64, i64) = (63, 63);
/// The project root the stack is detected at.
pub const PROJECT_ROOT: &str = "src/";
/// The detected stack's name. Free text; it selects nothing.
pub const TEST_STACK: &str = "python_pytest";
/// The command that runs the sample's tests.
pub const TEST_COMMAND: &str = "python -m pytest";

/// The adapter's sample world (plan A step 2, lane a5) [draft]: a log holds it once it holds a
/// `reader_chosen`; before that no move of it is offered, so every walk without one is as before.
/// The Familiar version the sample's binary is.
pub const FAMILIAR_VERSION: &str = "0.1.0";
/// The runner the sample's stack is read as, and the reader the dry run chooses.
pub const RUNNER: &str = "pytest";
pub const READER: &str = "pytest:reporter";
/// The files the choice watches, relative to the stack's root.
pub const WATCHED: [&str; 2] = ["pyproject.toml", "**/conftest.py"];
/// The setup file the agent's setup commit changes.
pub const SETUP_FILE: &str = "src/conftest.py";
/// The probe the dry run could not read, when it has a gap.
pub const GAP: &str = "P5";

/// The sample's two criteria: c1 on the full path, c2 on the light path.
pub const CRITERIA: [SampleCriterion; 2] = [
    SampleCriterion {
        criterion_id: "c1",
        text: "a right password opens a session",
        path: "full",
        behavior_id: "b1",
        promise_id: "p1",
        place: SamplePlace { file_path: "src/login.py", symbol: "check_password", line_start: 40, line_end: 62 },
        tests: &[SampleTest { file_path: "tests/test_c1.py", test_name: "t1" }],
        touching: &[SampleTest { file_path: "tests/test_login_old.py", test_name: "t_old" }],
        agent_conversation_id: "a1",
        hash_sealed: "S1",
        hash_edited: "S1x",
        criterion_rev: "R1",
    },
    SampleCriterion {
        criterion_id: "c2",
        text: "a reset link reaches the person",
        path: "light",
        behavior_id: "",
        promise_id: "",
        place: SamplePlace { file_path: "src/mail.py", symbol: "reset_link", line_start: 10, line_end: 30 },
        tests: &[SampleTest { file_path: "tests/test_c2.py", test_name: "t1" }],
        touching: &[],
        agent_conversation_id: "a1",
        hash_sealed: "S2",
        hash_edited: "S2x",
        criterion_rev: "R2",
    },
];

/// The dependency the binary drafts after the person's `a_dependency` (plan §7.3; lane S9a)
/// [draft]: the next free id; its text, place and test are the one-shot's (S9c), its path and
/// ids copied from the part that needs it. Kept out of `CRITERIA`, which the plan's walks plan
/// from: it enters a log only through the binary's draft.
pub const DEPENDENCY: SampleCriterion = SampleCriterion {
    criterion_id: "c3",
    text: "the hashing helper returns text, not bytes",
    path: "full",
    behavior_id: "b1",
    promise_id: "p1",
    place: SamplePlace { file_path: "src/hashing.py", symbol: "digest", line_start: 3, line_end: 9 },
    tests: &[SampleTest { file_path: "tests/test_c3.py", test_name: "t1" }],
    touching: &[],
    agent_conversation_id: "a1",
    hash_sealed: "S3",
    hash_edited: "S3x",
    criterion_rev: "R3",
};

/// The sample's row for a criterion id, the dependency's among them.
pub fn row_of(c: &str) -> Option<&'static SampleCriterion> {
    CRITERIA.iter().chain(std::iter::once(&DEPENDENCY)).find(|r| r.criterion_id == c)
}

/// The other conversation, for a transfer.
pub fn other_agent(a: Option<&str>) -> &'static str {
    AGENTS.iter().copied().find(|x| Some(*x) != a).unwrap_or(AGENTS[0])
}

/// A fact that lands together with a move's own fact.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveFact {
    pub kind: String,
    pub fields: Map<String, Value>,
}

/// One fact, or a few that land together. Only the criterion's moves carry
/// `more` and a label.
#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    pub kind: String,
    pub fields: Map<String, Value>,
    pub more: Option<Vec<MoveFact>>,
    pub label: Option<String>,
}

impl Move {
    pub fn plain(kind: &str, fields: Value) -> Move {
        Move {
            kind: kind.to_string(),
            fields: fields_of(fields),
            more: None,
            label: None,
        }
    }

    /// The move as the model prints it, with the label under `as`.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("kind".into(), Value::from(self.kind.as_str()));
        m.insert("fields".into(), Value::Object(self.fields.clone()));
        if let Some(more) = &self.more {
            m.insert(
                "more".into(),
                Value::Array(
                    more.iter()
                        .map(|f| json!({ "kind": f.kind, "fields": Value::Object(f.fields.clone()) }))
                        .collect(),
                ),
            );
        }
        if let Some(label) = &self.label {
            m.insert("as".into(), Value::from(label.as_str()));
        }
        Value::Object(m)
    }

    /// The facts the move lands, first the move's own.
    pub fn facts(&self) -> Vec<(&str, &Map<String, Value>)> {
        let mut out = vec![(self.kind.as_str(), &self.fields)];
        if let Some(more) = &self.more {
            out.extend(more.iter().map(|f| (f.kind.as_str(), &f.fields)));
        }
        out
    }
}

pub(crate) fn fields_of(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// Appends every fact of the move through `familiar_ai_facts::append_raw`.
pub fn apply_move(log: &Log, m: &Move, at: f64, tenant: &str) -> Result<Log, AppendError> {
    let mut out = log.clone();
    for (kind, fields) in m.facts() {
        out = append_raw(&out, kind, fields.clone(), at, tenant)?;
    }
    Ok(out)
}

fn promise_filled(p: &Fact) -> bool {
    ["accepts", "gives_back", "refuses"]
        .iter()
        .all(|k| text(p, k).is_some_and(|s| !s.is_empty()))
}

/// Every planning move over the sample world, including the wrong ones the
/// model must hold; fields come only from the sample.
pub fn plan_admissible(v: &View) -> Vec<Move> {
    let mut out: Vec<Move> = Vec::new();
    let mut push = |kind: &str, fields: Value| out.push(Move::plain(kind, fields));
    if v.last("intent_submitted").is_none() {
        push("intent_submitted", json!({ "text": INTENT_TEXT, "sha": INTENT_SHA }));
    }
    if !v.kind("behavior").any(|f| text(f, "behavior_id") == Some(BEHAVIOR_ID)) {
        push("behavior", json!({ "behavior_id": BEHAVIOR_ID, "text": BEHAVIOR_TEXT })); // supplies line_3
    } else {
        let p = last_of(v.kind("promise").filter(|f| text(f, "promise_id") == Some(PROMISE_ID)).collect());
        if !p.is_some_and(promise_filled) {
            let base = |refuses: &str| {
                json!({ "promise_id": PROMISE_ID, "behavior_id": BEHAVIOR_ID, "accepts": PROMISE_ACCEPTS, "gives_back": PROMISE_GIVES_BACK, "refuses": refuses })
            };
            push("promise", base(PROMISE_REFUSES));
            push("promise", base("")); // the wrong one: a slot left empty
        }
    }
    for s in &CRITERIA {
        if v.kind("criterion").any(|f| f.criterion_id() == Some(s.criterion_id)) {
            continue;
        }
        if s.path == "full" {
            push("criterion", json!({ "criterion_id": s.criterion_id, "text": s.text, "path": "full", "behavior_id": s.behavior_id, "promise_id": s.promise_id }));
        } else {
            push("criterion", json!({ "criterion_id": s.criterion_id, "text": s.text, "path": "light" }));
        }
    }
    let confirmed = plan::plan_state(v) == "confirmed";
    for c in plan::criteria(v) {
        let Some(s) = row_of(&c) else {
            continue;
        };
        let a = plan::anchor_of(v, &c);
        let recheck = a.is_some_and(|a| {
            a.kind == "place_chosen"
                && plan::places_of(a).iter().any(|p| {
                    plan::check_of(v, &c, p).is_some_and(|ch| ch.fields.get("matched") == Some(&Value::Bool(false)))
                })
        });
        if a.is_none() || recheck {
            // the three anchor exits, also offered again after a false match
            push("place_chosen", json!({ "criterion_id": c, "places": [s.place.to_json()] }));
            // `beside` is above or below in the answer form; the sample uses below
            push("new_work_spot_chosen", json!({ "criterion_id": c, "file_path": s.place.file_path, "symbol": s.place.symbol, "beside": "below" }));
            // the integration point is where the new work is reached from
            push("new_work_no_spot", json!({ "criterion_id": c, "integration_point": format!("{}:{}", s.place.file_path, s.place.symbol) }));
        }
        if let Some(a) = a.filter(|a| a.kind == "place_chosen") {
            for p in plan::places_of(a) {
                if plan::check_of(v, &c, &p).is_none() {
                    let check = |hash: &str, matched: bool| {
                        json!({ "criterion_id": c, "file_path": p.file_path, "symbol": p.symbol, "line_start": p.line_start, "line_end": p.line_end, "sha": INTENT_SHA, "lines_hash": hash, "matched": matched, "checked_at": "planning" })
                    };
                    push("anchor_sha_matched", check(LINES_PLANNING, true)); // the binary's planning check
                    push("anchor_sha_matched", check(LINES_OUTSIDE, false)); // the wrong one: a false anchor match
                }
            }
        }
        if let Some(a) = a.filter(|_| plan::path_of(v, &c) == Some("light")) {
            // the owner (2026-10-11): the binary's documentation check of a light anchor, and the WRONG one: not documentation
            for x in plan::doc_spots_of(a).unwrap_or_default() {
                if plan::doc_of(v, &c, &x).is_none() {
                    let doc = |matched: bool| {
                        let mut f = fields_of(json!({ "criterion_id": c }));
                        f.extend(x.to_fields());
                        f.extend(fields_of(json!({ "sha": INTENT_SHA, "matched": matched })));
                        Value::Object(f)
                    };
                    push("anchor_is_documentation", doc(true));
                    push("anchor_is_documentation", doc(false));
                }
            }
            if plan::not_documentation(v, &c) {
                // its cure: the agent claims it again --path full (the latest criterion counts) [reading] the sample's one behavior and promise
                push("criterion", json!({ "criterion_id": c, "text": s.text, "path": "full", "behavior_id": BEHAVIOR_ID, "promise_id": PROMISE_ID }));
            }
        }
        if v.of("test_planned", &c).next().is_none() {
            if let Some(t) = s.tests.first() {
                push("test_planned", json!({ "criterion_id": c, "file_path": t.file_path, "will": "create" }));
            }
        }
        for t in s.touching {
            let declared = v.of("touching_test_declared", &c).any(|f| {
                text(f, "file_path") == Some(t.file_path) && text(f, "test_name") == Some(t.test_name)
            });
            if !declared {
                push("touching_test_declared", json!({ "criterion_id": c, "file_path": t.file_path, "test_name": t.test_name }));
            }
        }
        if !plan::on_string(v, &c) {
            push("criterion_head", json!({ "criterion_id": c }));
            for h in plan::criteria(v) {
                if h != c {
                    push("criterion_strung_on_head", json!({ "criterion_id": c, "head": h })); // strung on a non-head is the wrong one
                }
            }
        }
        if v.of("criterion_assigned", &c).next().is_none() {
            push("criterion_assigned", json!({ "criterion_id": c, "agent_conversation_id": s.agent_conversation_id }));
        } else if v.of("fix_location_category", &c).next().is_none() {
            // line 11's supply: the holder's claim at planning, offered once the criterion
            // is held; the plan's walk claims this_repository only
            push("fix_location_category", json!({ "criterion_id": c, "category": "this_repository", "where": "" }));
        } else if confirmed {
            let holder = plan::assigned_to(v, &c);
            push("criterion_transfer", json!({ "criterion_id": c, "from_conversation": holder, "to_conversation": other_agent(holder) }));
        }
    }
    let root_with_command = v
        .kind("test_stack_detected")
        .any(|f| text(f, "project_root") == Some(PROJECT_ROOT) && plan::has_command(f));
    if !root_with_command {
        let st = |cmd: &str| json!({ "codebase_id": CODEBASE_ID, "project_root": PROJECT_ROOT, "test_stack": TEST_STACK, "test_command": cmd });
        push("test_stack_detected", st(TEST_COMMAND));
        if !v.kind("test_stack_detected").any(|f| text(f, "project_root") == Some(PROJECT_ROOT)) {
            push("test_stack_detected", st("")); // the wrong one: a stack with no command, offered once
        }
    }
    out
}

/// Appends a planning fact only if it matches an admissible move exactly; an
/// extra key is refused, not stored.
pub fn plan_append(log: &Log, kind: &str, fields: &Map<String, Value>, at: f64, tenant: &str) -> Result<Log, String> {
    let v = View::new(log);
    let ok = plan_admissible(&v).iter().any(|e| {
        e.kind == kind
            && e.fields.iter().all(|(k, val)| fields.get(k) == Some(val))
            && fields.keys().all(|k| e.fields.contains_key(k))
    });
    if !ok {
        return Err(format!("{kind} is not admissible (plan {})", plan::plan_state(&v)));
    }
    append_raw(log, kind, fields.clone(), at, tenant).map_err(|e| e.to_string())
}

/// The sample world with planning confirmed, built through `append_raw`;
/// `greenfield` makes every anchor a `new_work_no_spot`.
pub fn criterion_fixture(criteria: &[&str], stack: bool, greenfield: bool, at: f64, tenant: &str) -> Result<Log, AppendError> {
    criterion_fixture_claimed(criteria, stack, greenfield, false, at, tenant)
}

/// `criterion_fixture` whose line-11 claims are `this_repository`, or with
/// `elsewhere` the sample's claim that the fix lives elsewhere.
pub fn criterion_fixture_claimed(criteria: &[&str], stack: bool, greenfield: bool, elsewhere: bool, at: f64, tenant: &str) -> Result<Log, AppendError> {
    let rows: Vec<&SampleCriterion> = CRITERIA.iter().filter(|r| criteria.contains(&r.criterion_id)).collect();
    let mut log: Log = Vec::new();
    let ap = |log: &Log, kind: &str, fields: Value| append_raw(log, kind, fields_of(fields), at, tenant);
    log = ap(&log, "intent_submitted", json!({ "text": INTENT_TEXT, "sha": INTENT_SHA }))?;
    if rows.iter().any(|r| r.path == "full") {
        log = ap(&log, "behavior", json!({ "behavior_id": BEHAVIOR_ID, "text": BEHAVIOR_TEXT }))?;
        log = ap(&log, "promise", json!({ "promise_id": PROMISE_ID, "behavior_id": BEHAVIOR_ID, "accepts": PROMISE_ACCEPTS, "gives_back": PROMISE_GIVES_BACK, "refuses": PROMISE_REFUSES }))?;
    }
    for r in &rows {
        let fields = if r.path == "full" {
            json!({ "criterion_id": r.criterion_id, "text": r.text, "path": r.path, "behavior_id": r.behavior_id, "promise_id": r.promise_id })
        } else {
            json!({ "criterion_id": r.criterion_id, "text": r.text, "path": r.path })
        };
        log = ap(&log, "criterion", fields)?;
    }
    for r in &rows {
        if greenfield {
            // the integration point is where the new work is reached from
            log = ap(&log, "new_work_no_spot", json!({ "criterion_id": r.criterion_id, "integration_point": format!("{}:{}", r.place.file_path, r.place.symbol) }))?;
        } else {
            log = ap(&log, "place_chosen", json!({ "criterion_id": r.criterion_id, "places": [r.place.to_json()] }))?;
            let mut check = fields_of(json!({ "criterion_id": r.criterion_id }));
            check.extend(fields_of(r.place.to_json()));
            check.extend(fields_of(json!({ "sha": INTENT_SHA, "lines_hash": LINES_PLANNING, "matched": true, "checked_at": "planning" })));
            log = ap(&log, "anchor_sha_matched", Value::Object(check))?;
        }
    }
    for r in &rows {
        if r.path != "light" {
            continue;
        }
        // the owner (2026-10-11): the binary's check that a light anchor is documentation
        let spots = crate::plan::anchor_of(&View::new(&log), r.criterion_id).and_then(crate::plan::doc_spots_of).unwrap_or_default();
        for x in spots {
            let mut f = fields_of(json!({ "criterion_id": r.criterion_id }));
            f.extend(x.to_fields());
            f.extend(fields_of(json!({ "sha": INTENT_SHA, "matched": true })));
            log = ap(&log, "anchor_is_documentation", Value::Object(f))?;
        }
    }
    for r in &rows {
        for t in r.tests {
            log = ap(&log, "test_planned", json!({ "criterion_id": r.criterion_id, "file_path": t.file_path, "will": "create" }))?;
        }
    }
    if stack {
        log = ap(&log, "test_stack_detected", json!({ "codebase_id": CODEBASE_ID, "project_root": PROJECT_ROOT, "test_stack": TEST_STACK, "test_command": TEST_COMMAND }))?;
    }
    for r in &rows {
        for t in r.touching {
            log = ap(&log, "touching_test_declared", json!({ "criterion_id": r.criterion_id, "file_path": t.file_path, "test_name": t.test_name }))?;
        }
    }
    for (i, r) in rows.iter().enumerate() {
        log = if i == 0 {
            ap(&log, "criterion_head", json!({ "criterion_id": r.criterion_id }))?
        } else {
            ap(&log, "criterion_strung_on_head", json!({ "criterion_id": r.criterion_id, "head": rows[0].criterion_id }))?
        };
    }
    for r in &rows {
        log = ap(&log, "criterion_assigned", json!({ "criterion_id": r.criterion_id, "agent_conversation_id": r.agent_conversation_id }))?;
    }
    for r in &rows {
        // line 11: the agent's claim at planning
        let (category, place) = if elsewhere { crate::moves::FIX_ELSEWHERE } else { ("this_repository", "") };
        log = ap(&log, "fix_location_category", json!({ "criterion_id": r.criterion_id, "category": category, "where": place }))?;
    }
    Ok(log)
}
