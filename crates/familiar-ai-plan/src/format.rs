//! The plan file's format, kept in this one module: JSON Lines, one claim per line, `kind`
//! first and then the fields in the form's order. There are no blank lines and a newline ends
//! the last line, so the same claims give the same bytes and a diff shows one line per claim.

use std::fmt;

use familiar_ai_facts::form_row;
use serde_json::{Map, Value};

use crate::PlanError;

/// The kinds a plan file may carry: claims about the plan, never a result, a runtime sighting or
/// the person's answer to a card. Nothing mirrored here may let CI accept a person's approval (the
/// owner, 2026-10-14), so the test outcome translation's kinds stay out. The one answer the plan
/// carries is the person's on the cases card, beside the case-finder's finding (the owner,
/// 2026-10-25: "Mirror them"), so a record rebuilt from the plan never asks again: a
/// `hold_answered` is a plan line only when it answers that card with one of its own choices
/// (`is_plan_line`).
pub const PLAN_KINDS: [&str; 20] = [
    "intent_submitted",
    "behavior",
    "promise",
    "criterion",
    "place_chosen",
    "new_work_spot_chosen",
    "new_work_no_spot",
    "criterion_head",
    "criterion_strung_on_head",
    "test_stack_detected",
    "test_planned",
    "touching_test_declared",
    "test_written",
    "anchor_sha_matched",
    "fingerprint_stamped",
    "criterion_assigned",
    "test_change_announced",
    // The holder's claim of where the fix lives (plan line 11).
    "fix_location_category",
    // The case-finder's finding and the person's answer on the cases card (the owner, 2026-10-25:
    // "Mirror them") [draft names]; no other card's answer.
    "cases_found",
    "hold_answered",
];

/// The cases card's hold, the rules' `cases::CARD` [draft name]: the one card whose answer the
/// plan carries.
const CASES_CARD: &str = "cases_unconfirmed";

/// The cases card's choices and the choice of words typed, the rules' `cases::ADD_ALL`, `KEEP`,
/// `AGAIN` and `TYPED` [draft names]. Any other choice there is refused: the rules read a
/// `release` on any hold as the part released.
const CASES_CHOICES: [&str; 4] = ["add_all", "keep_planned", "look_again", "typed"];

/// Whether a fact of `kind` with `fields` is a plan line: a kind in `PLAN_KINDS`, and a
/// `hold_answered` only when it answers the cases card with one of its choices. Read, write and
/// the mirror all ask this.
pub(crate) fn is_plan_line(kind: &str, fields: &Map<String, Value>) -> bool {
    let said = |k: &str| fields.get(k).and_then(Value::as_str);
    PLAN_KINDS.contains(&kind) && (kind != "hold_answered" || (said("hold") == Some(CASES_CARD) && said("choice").is_some_and(|c| CASES_CHOICES.contains(&c))))
}

/// One line of the plan file: a fact's kind and fields, without `t`, `at` and `tenant`.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    // Private outside this crate so a claim can only come from `Claim::new` or
    // `claims_from_log`, which admit only plan kinds with registered fields.
    pub(crate) kind: &'static str,
    pub(crate) fields: Map<String, Value>,
}

impl Claim {
    /// Builds a claim, refusing a kind not in `PLAN_KINDS` and an answer on any card but the cases
    /// card (`is_plan_line`); the facts crate validates the fields.
    pub fn new(kind: &str, fields: Map<String, Value>) -> Result<Claim, PlanError> {
        if !is_plan_line(kind, &fields) {
            return Err(PlanError::RefusedKind(kind.to_string()));
        }
        // Appending to a scratch log validates the fields and yields the kind's `&'static str`.
        let log = familiar_ai_facts::append_raw(&Vec::new(), kind, fields, 0.0, "")?;
        Ok(Claim {
            kind: log[0].kind,
            fields: log[0].fields.clone(),
        })
    }

    /// The claim's line: `kind` first, then the fields in the form's order.
    pub fn line(&self) -> String {
        let mut m = Map::with_capacity(self.fields.len() + 1);
        m.insert("kind".to_string(), Value::String(self.kind.to_string()));
        let order = form_row(self.kind).map(|row| row.fields).unwrap_or(&[]);
        for &field in order {
            if let Some(v) = self.fields.get(field) {
                m.insert(field.to_string(), v.clone());
            }
        }
        Value::Object(m).to_string()
    }

    pub fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn fields(&self) -> &Map<String, Value> {
        &self.fields
    }
}

/// A refused line of the plan file; `line` is 1-based.
#[derive(Debug, Clone, PartialEq)]
pub enum LineError {
    NotAnObject { line: usize },
    /// The facts crate registers no such kind.
    UnknownKind { line: usize, kind: String },
    /// The kind exists but is not in `PLAN_KINDS`.
    RefusedKind { line: usize, kind: String },
    /// A key the kind's form does not register, including `t`, `at` and `tenant`.
    UnregisteredKey {
        line: usize,
        kind: String,
        key: String,
    },
    MissingField {
        line: usize,
        kind: String,
        field: String,
    },
    /// A value outside a closed set, or of the wrong shape.
    BadValue { line: usize, message: String },
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::NotAnObject { line } => {
                write!(f, "plan: line {line} is not a JSON object")
            }
            LineError::UnknownKind { line, kind } => {
                write!(f, "plan: line {line}: unknown kind '{kind}'")
            }
            LineError::RefusedKind { line, kind } => {
                write!(
                    f,
                    "plan: line {line}: kind '{kind}' is refused in a plan (claims only)"
                )
            }
            LineError::UnregisteredKey { line, kind, key } => {
                write!(
                    f,
                    "plan: line {line}: {kind} does not register the key '{key}'"
                )
            }
            LineError::MissingField { line, kind, field } => {
                write!(
                    f,
                    "plan: line {line}: {kind} is missing the required field '{field}'"
                )
            }
            LineError::BadValue { line, message } => {
                write!(f, "plan: line {line}: bad value: {message}")
            }
        }
    }
}

/// Serializes the claims, one line each; an empty plan is zero bytes.
pub(crate) fn write_bytes(claims: &[Claim]) -> Vec<u8> {
    let mut out = String::new();
    for claim in claims {
        out.push_str(&claim.line());
        out.push('\n');
    }
    out.into_bytes()
}

/// Parses the file's bytes into claims, in file order.
pub(crate) fn read_bytes(bytes: &[u8]) -> Result<Vec<Claim>, PlanError> {
    let text = std::str::from_utf8(bytes).map_err(|e| PlanError::BadEncoding(e.to_string()))?;
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len() - 1; // split always gives at least one piece
    let mut claims = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            // The piece after the final newline is not a line; a blank line elsewhere is refused.
            if i != last {
                return Err(PlanError::Line(LineError::NotAnObject { line: i + 1 }));
            }
            continue;
        }
        claims.push(parse_line(line, i + 1).map_err(PlanError::Line)?);
    }
    Ok(claims)
}

/// Parses one line into a claim; `n` is its 1-based line number.
fn parse_line(line: &str, n: usize) -> Result<Claim, LineError> {
    let obj = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(m)) => m,
        _ => return Err(LineError::NotAnObject { line: n }),
    };
    let kind = obj
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(LineError::NotAnObject { line: n })?;
    let mut fields = Map::with_capacity(obj.len());
    for (k, v) in &obj {
        if k != "kind" {
            fields.insert(k.clone(), v.clone());
        }
    }
    if !is_plan_line(kind, &fields) {
        if form_row(kind).is_some() {
            return Err(LineError::RefusedKind {
                line: n,
                kind: kind.to_string(),
            });
        }
        return Err(LineError::UnknownKind {
            line: n,
            kind: kind.to_string(),
        });
    }
    Claim::new(kind, fields).map_err(|e| match e {
        PlanError::Append(ae) => line_error_from_append(n, ae),
        other => LineError::BadValue {
            line: n,
            message: other.to_string(),
        },
    })
}

/// Maps the facts crate's refusal to a `LineError` at line `n`.
fn line_error_from_append(n: usize, e: familiar_ai_facts::AppendError) -> LineError {
    use familiar_ai_facts::AppendError as A;
    match e {
        A::UnknownKind(kind) => LineError::UnknownKind { line: n, kind },
        A::UnregisteredKey { kind, key } => LineError::UnregisteredKey { line: n, kind, key },
        A::StampedKey(key) => LineError::UnregisteredKey {
            line: n,
            kind: String::new(),
            key,
        },
        A::MissingField { kind, field } => LineError::MissingField {
            line: n,
            kind,
            field,
        },
        A::BadValue(m) => LineError::BadValue {
            line: n,
            message: m,
        },
        A::FieldsNotObject => LineError::NotAnObject { line: n },
    }
}
