//! The fact envelope and the in-memory append that validates each fact against its form.
//! A fact is flat JSON, `{t, at, kind, ...fields, tenant}`; append alone stamps those four keys,
//! and a caller that hands one in is refused.

use std::fmt;

use serde_json::{json, Map, Value};

use crate::form_table::{form_row, FORM};

/// Facts in append order.
pub type Log = Vec<Fact>;

/// A fact in the log: the stamped envelope plus the kind's fields, flat.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    /// The fact's position in the log.
    pub t: u64,
    /// The caller's time.
    pub at: f64,
    pub kind: &'static str,
    /// The kind's own fields, without the four stamped keys.
    pub fields: Map<String, Value>,
    /// The caller's scope.
    pub tenant: String,
}

const STAMPED: [&str; 4] = ["t", "at", "kind", "tenant"];

impl Fact {
    /// The `criterion_id` field, if this fact's kind carries one.
    pub fn criterion_id(&self) -> Option<&str> {
        self.fields.get("criterion_id")?.as_str()
    }

    /// The fields as the kind's own type; an error when `T` is another kind.
    pub fn fields_as<T: FactFields + serde::de::DeserializeOwned>(&self) -> Result<T, AppendError> {
        if T::KIND != self.kind {
            return Err(AppendError::BadValue(format!(
                "this fact is a {}, not a {}",
                self.kind,
                T::KIND
            )));
        }
        serde_json::from_value(Value::Object(self.fields.clone()))
            .map_err(|e| AppendError::BadValue(format!("{}: {e}", self.kind)))
    }

    /// The flat JSON form of the fact.
    pub fn to_json(&self) -> Value {
        let mut m = Map::with_capacity(self.fields.len() + 4);
        m.insert("t".to_string(), json!(self.t));
        m.insert("at".to_string(), json!(self.at));
        m.insert("kind".to_string(), json!(self.kind));
        for (k, v) in &self.fields {
            m.insert(k.clone(), v.clone());
        }
        m.insert("tenant".to_string(), json!(self.tenant));
        Value::Object(m)
    }

    /// Parse a flat-JSON fact, validating it against its form.
    pub fn from_json(v: &Value) -> Result<Fact, AppendError> {
        let obj = v
            .as_object()
            .ok_or_else(|| AppendError::BadValue("a fact is a JSON object".to_string()))?;
        let t = obj
            .get("t")
            .and_then(Value::as_u64)
            .ok_or_else(|| AppendError::BadValue("fact.t must be a non-negative integer".into()))?;
        let at = obj
            .get("at")
            .and_then(Value::as_f64)
            .ok_or_else(|| AppendError::BadValue("fact.at must be a number".into()))?;
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| AppendError::BadValue("fact.kind must be text".into()))?;
        let tenant = obj
            .get("tenant")
            .and_then(Value::as_str)
            .ok_or_else(|| AppendError::BadValue("fact.tenant must be text".into()))?
            .to_string();
        let mut fields = Map::new();
        for (k, val) in obj {
            if !STAMPED.contains(&k.as_str()) {
                fields.insert(k.clone(), val.clone());
            }
        }
        // A stored fact must still match its form.
        let kind = validate(kind, &fields)?;
        Ok(Fact {
            t,
            at,
            kind,
            fields,
            tenant,
        })
    }
}

/// The whole log as an array of flat-JSON facts.
pub fn log_to_json(log: &Log) -> Value {
    Value::Array(log.iter().map(Fact::to_json).collect())
}

/// Why an append was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum AppendError {
    UnknownKind(String),
    UnregisteredKey { kind: String, key: String },
    /// The caller handed in one of `t`, `at`, `kind`, `tenant`.
    StampedKey(String),
    MissingField { kind: String, field: String },
    /// A value outside a closed set, or of the wrong shape.
    BadValue(String),
    FieldsNotObject,
}

impl fmt::Display for AppendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppendError::UnknownKind(k) => write!(f, "append: unknown kind '{k}'"),
            AppendError::UnregisteredKey { kind, key } => {
                write!(f, "append: {kind} does not register the key '{key}'")
            }
            AppendError::StampedKey(k) => {
                write!(
                    f,
                    "append: '{k}' is stamped by append alone, never handed in"
                )
            }
            AppendError::MissingField { kind, field } => {
                write!(f, "append: {kind} is missing the required field '{field}'")
            }
            AppendError::BadValue(m) => write!(f, "append: bad value: {m}"),
            AppendError::FieldsNotObject => write!(f, "append: fields must be a JSON object"),
        }
    }
}

impl std::error::Error for AppendError {}

/// The words `ci_report_seen.outcome` allows: what a hosted CI run said at a commit.
pub const CI_OUTCOMES: &[&str] = &["proven", "not_proven", "could_not", "cancelled", "timed_out"];

/// Closed sets that belong to one kind's field, as (kind, field, allowed words); each replaces
/// the shared [`CLOSED`] set for that field.
const CLOSED_FOR_KIND: &[(&str, &str, &[&str])] = &[
    ("ci_report_seen", "outcome", CI_OUTCOMES),
    // the cases card (the owner's plan of 2026-10-25) [draft]: what the case-finder's answer was
    ("cases_found", "outcome", &["found", "none", "failed"]),
    // the lens panel (the owner's plan of 2026-10-26, approved): a lens's answer is one of the
    // five; an Agent call is released to one of them or to the synthesis
    ("lens_found", "outcome", &["found", "none", "failed"]),
    ("lens_found", "lens", LENSES),
    ("lens_released", "lens", &["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling", "synthesis"]),
    ("question_answered", "question_id", QUESTION_IDS),
    ("familiar_switch", "to", SWITCH_TO),
    // the place where the change landed carries only the answer that appends it (F12)
    ("place_chosen", "hold", &["anchor_untouched"]),
    ("place_chosen", "choice", &["keep_going"]),
    // the test outcome translation (the owner's plan of 2026-10-14): no model labels a run
    ("test_log_translation", "by", &["regex", "binary", "reporter"]),
    (
        "test_log_translation",
        "label",
        &[
            "passed",
            "tests_failed",
            "collection_error",
            "missing_target_symbol",
            "no_tests_collected",
            "setup_error",
            "usage_error",
            "environment_error",
            "runner_crash",
            "killed",
            "crashed",
            "timed_out",
            "unclassified",
        ],
    ),
    ("test_log_translation", "classified_on", &["full", "cut"]),
];

/// The lens panel's five lenses, by id (the owner's plan of 2026-10-26, approved).
pub const LENSES: &[&str] = &["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];

/// The ids of the synthesis's questions, at most three.
pub const QUESTION_IDS: &[&str] = &["q1", "q2", "q3"];

/// The words `familiar_switch.to` allows.
pub const SWITCH_TO: &[&str] = &["off", "on"];

/// Closed sets shared by every kind with the field, as (field, allowed words).
const CLOSED: &[(&str, &[&str])] = &[
    ("path", &["full", "light"]),
    ("outcome", &["passed", "failed", "did_not_run"]),
    ("ran_in", &["laptop", "pipeline"]),
    ("checked_at", &["planning", "start"]),
    ("will", &["create", "modify"]),
    ("how", &["no_work", "question"]),
    ("by", &["agent"]),
];

/// Check fields against the kind's form, closed sets and value types; returns the kind's static name.
fn validate(kind: &str, fields: &Map<String, Value>) -> Result<&'static str, AppendError> {
    let row = form_row(kind).ok_or_else(|| AppendError::UnknownKind(kind.to_string()))?;
    for key in fields.keys() {
        if STAMPED.contains(&key.as_str()) {
            return Err(AppendError::StampedKey(key.clone()));
        }
        if !row.fields.contains(&key.as_str()) {
            return Err(AppendError::UnregisteredKey {
                kind: kind.to_string(),
                key: key.clone(),
            });
        }
    }
    for &field in row.fields {
        if row.optional.contains(&field) {
            continue;
        }
        if !fields.contains_key(field) {
            return Err(AppendError::MissingField {
                kind: kind.to_string(),
                field: field.to_string(),
            });
        }
    }
    let own = CLOSED_FOR_KIND.iter().filter(|(k, _, _)| *k == kind).map(|(_, f, a)| (*f, *a));
    let shared = CLOSED
        .iter()
        // `run_reserved.by` names who reserved the run: free text
        .filter(|(field, _)| !(kind == "run_reserved" && *field == "by"))
        .filter(|(field, _)| !CLOSED_FOR_KIND.iter().any(|(k, f, _)| *k == kind && f == field))
        .map(|(f, a)| (*f, *a));
    for (field, allowed) in own.chain(shared) {
        if let Some(v) = fields.get(field) {
            let word = v.as_str().ok_or_else(|| {
                AppendError::BadValue(format!("{kind}.{field} must be one of {allowed:?}"))
            })?;
            if !allowed.contains(&word) {
                return Err(AppendError::BadValue(format!(
                    "{kind}.{field} must be one of {allowed:?}, got '{word}'"
                )));
            }
        }
    }
    check_types(row.kind, fields)?;
    Ok(row.kind)
}

/// Append a fact given as a kind name and a JSON object of fields.
/// The given log is left untouched; the result is a new log.
pub fn append_raw(
    log: &Log,
    kind: &str,
    fields: Map<String, Value>,
    at: f64,
    tenant: &str,
) -> Result<Log, AppendError> {
    if !at.is_finite() {
        return Err(AppendError::BadValue(format!("at must be a finite number, got {at}")));
    }
    let kind = validate(kind, &fields)?;
    let fact = Fact {
        t: log.len() as u64,
        at,
        kind,
        fields,
        tenant: tenant.to_string(),
    };
    let mut out = log.clone();
    out.push(fact);
    Ok(out)
}

/// Append a typed fact; it goes through [`append_raw`] so the stored JSON is identical.
pub fn append<T: FactFields>(
    log: &Log,
    fields: &T,
    at: f64,
    tenant: &str,
) -> Result<Log, AppendError> {
    append_raw(log, T::KIND, fields.to_fields()?, at, tenant)
}

/// A typed fact kind, rendered to the flat JSON object that [`append_raw`] validates.
pub trait FactFields {
    const KIND: &'static str;
    fn to_fields(&self) -> Result<Map<String, Value>, AppendError>;
}

macro_rules! impl_fact_fields {
    ($ty:ty, $kind:literal) => {
        impl FactFields for $ty {
            const KIND: &'static str = $kind;
            fn to_fields(&self) -> Result<Map<String, Value>, AppendError> {
                match serde_json::to_value(self) {
                    Ok(Value::Object(m)) => Ok(m),
                    Ok(_) => Err(AppendError::FieldsNotObject),
                    Err(e) => Err(AppendError::BadValue(e.to_string())),
                }
            }
        }
    };
}

use crate::forms::*;

impl_fact_fields!(IntentSubmitted, "intent_submitted");
impl_fact_fields!(Behavior, "behavior");
impl_fact_fields!(Promise, "promise");
impl_fact_fields!(Criterion, "criterion");
impl_fact_fields!(PlaceChosen, "place_chosen");
impl_fact_fields!(NewWorkSpotChosen, "new_work_spot_chosen");
impl_fact_fields!(NewWorkNoSpot, "new_work_no_spot");
impl_fact_fields!(CriterionHead, "criterion_head");
impl_fact_fields!(CriterionStrungOnHead, "criterion_strung_on_head");
impl_fact_fields!(TestStackDetected, "test_stack_detected");
impl_fact_fields!(TestWritten, "test_written");
impl_fact_fields!(PassPleasePass, "pass_please_pass");
impl_fact_fields!(FingerprintStamped, "fingerprint_stamped");
impl_fact_fields!(AnchorShaMatched, "anchor_sha_matched");
impl_fact_fields!(TestPlanned, "test_planned");
impl_fact_fields!(TouchingTestDeclared, "touching_test_declared");
impl_fact_fields!(CriterionAssigned, "criterion_assigned");
impl_fact_fields!(CriterionTransfer, "criterion_transfer");
impl_fact_fields!(CommitSeen, "commit_seen");
impl_fact_fields!(HoldAnswered, "hold_answered");
impl_fact_fields!(PipelineRunSeen, "pipeline_run_seen");
impl_fact_fields!(TestChangeAnnounced, "test_change_announced");
impl_fact_fields!(ModelSeen, "model_seen");
impl_fact_fields!(JudgeVerdict, "judge_verdict");
impl_fact_fields!(PersonSaid, "person_said");
impl_fact_fields!(PersonSaidAnswered, "person_said_answered");
impl_fact_fields!(JudgeUnanswered, "judge_unanswered");
impl_fact_fields!(InvestigatorUnfinished, "investigator_unfinished");
impl_fact_fields!(FixLocationCategory, "fix_location_category");
impl_fact_fields!(RunReserved, "run_reserved");
impl_fact_fields!(RunStarted, "run_started");
impl_fact_fields!(RunEnded, "run_ended");
impl_fact_fields!(RunCancelRequested, "run_cancel_requested");
impl_fact_fields!(TestsObserved, "tests_observed");
impl_fact_fields!(CiReportSeen, "ci_report_seen");
impl_fact_fields!(AnchorIsDocumentation, "anchor_is_documentation");
impl_fact_fields!(FamiliarSwitch, "familiar_switch");
impl_fact_fields!(TestLogTranslation, "test_log_translation");
impl_fact_fields!(DependencyDrafted, "dependency_drafted");
impl_fact_fields!(DependencyUnanswered, "dependency_unanswered");
impl_fact_fields!(Told, "told");
impl_fact_fields!(CriterionCovered, "criterion_covered");
impl_fact_fields!(SubagentEnded, "subagent_ended");
impl_fact_fields!(RecordSeeded, "record_seeded");
impl_fact_fields!(HolderResumed, "holder_resumed");
impl_fact_fields!(InvestigatorFinding, "investigator_finding");
impl_fact_fields!(ReaderChosen, "reader_chosen");
impl_fact_fields!(FixVerdict, "fix_verdict");
impl_fact_fields!(ExistingTestAnnounced, "existing_test_announced");
impl_fact_fields!(ExistingTestVerdict, "existing_test_verdict");
impl_fact_fields!(TouchingTestDerived, "touching_test_derived");
impl_fact_fields!(PlacesListed, "places_listed");
impl_fact_fields!(PlaceClassified, "place_classified");
impl_fact_fields!(ReadingAlternatives, "reading_alternatives");
impl_fact_fields!(OneReading, "one_reading");
impl_fact_fields!(TraceListed, "trace_listed");
impl_fact_fields!(OffListReason, "off_list_reason");
impl_fact_fields!(CasesFound, "cases_found");
impl_fact_fields!(LensReleased, "lens_released");
impl_fact_fields!(LensFound, "lens_found");
impl_fact_fields!(QuestionAnswered, "question_answered");

/// Refuse a value whose shape does not deserialize into the kind's type.
fn check_types(kind: &str, fields: &Map<String, Value>) -> Result<(), AppendError> {
    fn typed<T: serde::de::DeserializeOwned>(
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<(), AppendError> {
        serde_json::from_value::<T>(Value::Object(fields.clone()))
            .map(|_| ())
            .map_err(|e| AppendError::BadValue(format!("{kind}: {e}")))
    }
    match kind {
        "intent_submitted" => typed::<IntentSubmitted>(kind, fields),
        "behavior" => typed::<Behavior>(kind, fields),
        // cases and opposite_none, the contract [draft names], are fields of the fact, not of the
        // typed form the sealed tests build: a list of cases and text when present
        "promise" => typed::<Promise>(kind, fields).and_then(|()| {
            if let Some(v) = fields.get("cases") {
                serde_json::from_value::<Vec<PromiseCase>>(v.clone()).map_err(|e| AppendError::BadValue(format!("{kind}.cases: {e}")))?;
            }
            match (fields.get("opposite_none"), fields.get("contract_version")) {
                (Some(v), _) if !v.is_string() => Err(AppendError::BadValue(format!("{kind}.opposite_none must be text"))),
                (_, Some(v)) if !v.is_u64() => Err(AppendError::BadValue(format!("{kind}.contract_version must be a whole number"))),
                _ => Ok(()),
            }
        }),
        "criterion" => typed::<Criterion>(kind, fields),
        "place_chosen" => typed::<PlaceChosen>(kind, fields),
        "new_work_spot_chosen" => typed::<NewWorkSpotChosen>(kind, fields),
        "new_work_no_spot" => typed::<NewWorkNoSpot>(kind, fields),
        "criterion_head" => typed::<CriterionHead>(kind, fields),
        "criterion_strung_on_head" => typed::<CriterionStrungOnHead>(kind, fields),
        "test_stack_detected" => typed::<TestStackDetected>(kind, fields),
        "test_written" => typed::<TestWritten>(kind, fields),
        // run_id is a field of the fact, not of the typed form (the sealed tests build that
        // form, the owner 2026-10-14); text when present
        "pass_please_pass" => typed::<PassPleasePass>(kind, fields).and_then(|()| match fields.get("run_id") {
            Some(v) if !v.is_string() => Err(AppendError::BadValue(format!("{kind}.run_id must be text"))),
            _ => Ok(()),
        }),
        "fingerprint_stamped" => typed::<FingerprintStamped>(kind, fields),
        "anchor_sha_matched" => typed::<AnchorShaMatched>(kind, fields),
        "test_planned" => typed::<TestPlanned>(kind, fields),
        "touching_test_declared" => typed::<TouchingTestDeclared>(kind, fields),
        "criterion_assigned" => typed::<CriterionAssigned>(kind, fields),
        "criterion_transfer" => typed::<CriterionTransfer>(kind, fields),
        // existing_tests_changed is a field of the fact, not of the typed form the sealed tests
        // build [draft]; a list of changes when present
        "commit_seen" => typed::<CommitSeen>(kind, fields).and_then(|()| match fields.get("existing_tests_changed") {
            Some(v) => serde_json::from_value::<Vec<ExistingTestChange>>(v.clone()).map(|_| ()).map_err(|e| AppendError::BadValue(format!("{kind}.existing_tests_changed: {e}"))),
            None => Ok(()),
        }),
        // run_id, commit, approval (the witness's id, plan §6.9) and from ("clear" on familiar-ai clear's release) are fields of the fact, not of
        // the typed form the sealed tests build; text when present
        "hold_answered" => typed::<HoldAnswered>(kind, fields).and_then(|()| {
            match ["run_id", "commit", "approval", "from"].into_iter().find(|k| fields.get(*k).is_some_and(|v| !v.is_string())) {
                Some(k) => Err(AppendError::BadValue(format!("{kind}.{k} must be text"))),
                None => Ok(()),
            }
        }),
        "pipeline_run_seen" => typed::<PipelineRunSeen>(kind, fields),
        "test_change_announced" => typed::<TestChangeAnnounced>(kind, fields),
        "model_seen" => typed::<ModelSeen>(kind, fields),
        "judge_verdict" => typed::<JudgeVerdict>(kind, fields),
        "person_said" => typed::<PersonSaid>(kind, fields),
        "person_said_answered" => typed::<PersonSaidAnswered>(kind, fields),
        "judge_unanswered" => typed::<JudgeUnanswered>(kind, fields),
        "investigator_unfinished" => typed::<InvestigatorUnfinished>(kind, fields),
        "fix_location_category" => typed::<FixLocationCategory>(kind, fields),
        "run_reserved" => typed::<RunReserved>(kind, fields),
        "run_started" => typed::<RunStarted>(kind, fields),
        "run_ended" => typed::<RunEnded>(kind, fields),
        "run_cancel_requested" => typed::<RunCancelRequested>(kind, fields),
        "tests_observed" => typed::<TestsObserved>(kind, fields),
        "ci_report_seen" => typed::<CiReportSeen>(kind, fields),
        "anchor_is_documentation" => typed::<AnchorIsDocumentation>(kind, fields),
        "familiar_switch" => typed::<FamiliarSwitch>(kind, fields),
        "test_log_translation" => typed::<TestLogTranslation>(kind, fields),
        "dependency_drafted" => typed::<DependencyDrafted>(kind, fields),
        "dependency_unanswered" => typed::<DependencyUnanswered>(kind, fields),
        "told" => typed::<Told>(kind, fields),
        "criterion_covered" => typed::<CriterionCovered>(kind, fields),
        "subagent_ended" => typed::<SubagentEnded>(kind, fields),
        "record_seeded" => typed::<RecordSeeded>(kind, fields),
        "holder_resumed" => typed::<HolderResumed>(kind, fields),
        // a category carries its own fields and no other's
        "investigator_finding" => typed::<InvestigatorFinding>(kind, fields).and_then(|()| {
            let word = fields.get("category").and_then(Value::as_str).unwrap_or("");
            let own = FindingCategory::own_fields(word);
            match ["guidance", "missed", "scope"].into_iter().find(|k| fields.contains_key(*k) && !own.contains(k)) {
                Some(k) => Err(AppendError::BadValue(format!("{kind}: {word} carries no {k}"))),
                None => Ok(()),
            }
        }),
        "reader_chosen" => typed::<ReaderChosen>(kind, fields),
        "fix_verdict" => typed::<FixVerdict>(kind, fields),
        "existing_test_announced" => typed::<ExistingTestAnnounced>(kind, fields),
        "existing_test_verdict" => typed::<ExistingTestVerdict>(kind, fields),
        "touching_test_derived" => typed::<TouchingTestDerived>(kind, fields),
        "places_listed" => typed::<PlacesListed>(kind, fields),
        "place_classified" => typed::<PlaceClassified>(kind, fields),
        "reading_alternatives" => typed::<ReadingAlternatives>(kind, fields),
        "one_reading" => typed::<OneReading>(kind, fields),
        "trace_listed" => typed::<TraceListed>(kind, fields),
        "off_list_reason" => typed::<OffListReason>(kind, fields),
        // the synthesis's questions: at most three, each id q1..q3 once, two to four options
        "cases_found" => typed::<CasesFound>(kind, fields).and_then(|()| {
            let qs = fields.get("questions").map(|v| serde_json::from_value::<Vec<FoundQuestion>>(v.clone())).transpose().map_err(|e| AppendError::BadValue(format!("{kind}.questions: {e}")))?.unwrap_or_default();
            if qs.len() > QUESTION_IDS.len() {
                return Err(AppendError::BadValue(format!("{kind}.questions: at most {}", QUESTION_IDS.len())));
            }
            for (i, q) in qs.iter().enumerate() {
                if !QUESTION_IDS.contains(&q.question_id.as_str()) || qs[..i].iter().any(|p| p.question_id == q.question_id) {
                    return Err(AppendError::BadValue(format!("{kind}.questions: each id is one of {QUESTION_IDS:?}, once, got '{}'", q.question_id)));
                }
                if !(2..=4).contains(&q.options.len()) {
                    return Err(AppendError::BadValue(format!("{kind}.questions: {} has {} options, two to four", q.question_id, q.options.len())));
                }
            }
            Ok(())
        }),
        "lens_released" => typed::<LensReleased>(kind, fields),
        "lens_found" => typed::<LensFound>(kind, fields),
        "question_answered" => typed::<QuestionAnswered>(kind, fields),
        other => Err(AppendError::UnknownKind(other.to_string())),
    }
}

/// The number of kinds `FORM` registers.
pub fn kind_count() -> usize {
    FORM.len()
}
