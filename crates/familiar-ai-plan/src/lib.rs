//! Reads and writes the committed plan file: the log's claim facts without `t`, `at` and `tenant`.
//! See docs/decisions.md#the-plan-is-committed.

mod format;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use familiar_ai_facts::{AppendError, Log};

pub use format::{Claim, LineError, PLAN_KINDS};

/// An error reading or writing a plan.
#[derive(Debug)]
pub enum PlanError {
    /// A kind the plan does not carry, refused on write.
    RefusedKind(String),
    /// The facts crate refused the claim's fields.
    Append(AppendError),
    Line(LineError),
    BadEncoding(String),
    /// The plan file does not exist; an empty file is an empty plan instead.
    NotFound(PathBuf),
    Io(io::Error),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::RefusedKind(k) => {
                write!(f, "plan: kind '{k}' is refused in a plan (claims only)")
            }
            PlanError::Append(e) => write!(f, "{e}"),
            PlanError::Line(e) => write!(f, "{e}"),
            PlanError::BadEncoding(e) => write!(f, "plan: the file is not UTF-8: {e}"),
            PlanError::NotFound(p) => write!(f, "plan: no plan file at {}", p.display()),
            PlanError::Io(e) => write!(f, "plan: I/O: {e}"),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<AppendError> for PlanError {
    fn from(e: AppendError) -> Self {
        PlanError::Append(e)
    }
}

impl From<io::Error> for PlanError {
    fn from(e: io::Error) -> Self {
        PlanError::Io(e)
    }
}

/// Serializes the claims; the same claims always give the same bytes.
pub fn to_bytes(claims: &[Claim]) -> Vec<u8> {
    format::write_bytes(claims)
}

/// Parses the claims in file order; empty bytes are an empty plan.
pub fn from_bytes(bytes: &[u8]) -> Result<Vec<Claim>, PlanError> {
    format::read_bytes(bytes)
}

/// Writes the claims to `path`, creating parent folders.
/// It writes a temporary file beside `path` and renames it, so a crash never leaves half a file.
pub fn write(path: &Path, claims: &[Claim]) -> Result<(), PlanError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let written = std::fs::write(&tmp, to_bytes(claims));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Reads the claims from `path` in file order; a missing file is `PlanError::NotFound`.
pub fn read(path: &Path) -> Result<Vec<Claim>, PlanError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(PlanError::NotFound(path.to_path_buf()));
        }
        Err(e) => return Err(e.into()),
    };
    from_bytes(&bytes)
}

/// Keeps the log's facts whose kind is in `PLAN_KINDS`, in log order, as claims: of the person's
/// answers only those on the cases card (the owner, 2026-10-25: "Mirror them"). In a record
/// setup seeded (the owner, 2026-10-15), the seed's plan, then the claims made here: the first
/// `plan_lines` facts before the latest `record_seeded` (the committed plan the seed copied
/// first), then those after it; the seed's walk, verify's facts and its laptop twins between them
/// never become lines, so the plan file stays the committed one.
pub fn claims_from_log(log: &Log) -> Vec<Claim> {
    // the pipeline's stamps are CI's own, never mirrored (plan §6.9, S9d)
    let plan = |fact: &&familiar_ai_facts::Fact| {
        format::is_plan_line(fact.kind, &fact.fields) && !(fact.kind == "fingerprint_stamped" && fact.fields.get("ran_in").and_then(serde_json::Value::as_str) == Some("pipeline"))
    };
    let claim = |fact: &familiar_ai_facts::Fact| Claim {
        kind: fact.kind,
        fields: fact.fields.clone(),
    };
    match log.iter().rposition(|f| f.kind == "record_seeded") {
        Some(m) => {
            let lines = log[m].fields.get("plan_lines").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let lines = usize::try_from(lines).unwrap_or(usize::MAX).min(m);
            log[..lines].iter().chain(&log[m + 1..]).filter(plan).map(claim).collect()
        }
        None => log.iter().filter(plan).map(claim).collect(),
    }
}
