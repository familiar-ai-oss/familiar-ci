//! The append-only record: one `redb` file at a path the caller passes in.
//! The API has no way to change or remove a fact, and appends refuse what the in-memory append refuses.
//! See docs/decisions.md#append-only-facts

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableTable, TableDefinition};

use crate::append::{append_raw, AppendError, Fact, FactFields, Log};
use crate::readers::{last_of_kind, of_criterion};

/// The one table: position `t` to the fact as flat JSON text.
const FACTS: TableDefinition<u64, &str> = TableDefinition::new("facts");

/// Why a record operation failed.
#[derive(Debug)]
pub enum RecordError {
    Append(AppendError),
    Db(String),
    /// A stored fact did not parse back or sits at the wrong position.
    Corrupt(String),
    /// Another process held the record for the whole wait; nothing was written.
    Busy,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Append(e) => write!(f, "record: {e}"),
            RecordError::Db(m) => write!(f, "record: db: {m}"),
            RecordError::Corrupt(m) => write!(f, "record: corrupt fact: {m}"),
            RecordError::Busy => write!(f, "record: in use by another step"),
        }
    }
}

impl std::error::Error for RecordError {}

impl From<AppendError> for RecordError {
    fn from(e: AppendError) -> Self {
        RecordError::Append(e)
    }
}

impl From<serde_json::Error> for RecordError {
    fn from(e: serde_json::Error) -> Self {
        RecordError::Db(e.to_string())
    }
}

macro_rules! db_err {
    ($e:expr) => {
        $e.map_err(|e| RecordError::Db(e.to_string()))
    };
}

/// How long to sleep before trying the record again, growing with each attempt and jittered so
/// that callers that started together do not retry together.
fn back_off(attempt: u32) -> Duration {
    let base = 2u64.saturating_mul(u64::from(attempt.min(5)) + 1); // 2..12 ms
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(0);
    let spread = (nanos ^ u64::from(std::process::id()).wrapping_mul(2_654_435_761)) % (base + 1);
    Duration::from_millis(base + spread)
}

/// Result of a conditional append.
#[derive(Debug)]
pub enum AppendIf {
    /// The predicate held inside the write transaction and the fact is durably in the file.
    Appended(Fact),
    /// The predicate was false and nothing was written.
    Refused,
}

fn db_of<'a>(g: &'a RwLockReadGuard<'_, Option<Database>>) -> Result<&'a Database, RecordError> {
    g.as_ref().ok_or_else(|| RecordError::Db("the record is released".to_string()))
}

/// An append-only record of facts in one redb file.
pub struct Record {
    path: PathBuf,
    /// `None` only while [`Record::while_released`] runs its closure.
    db: RwLock<Option<Database>>,
}

impl Record {
    /// Open the record at `path`, creating the file if needed, and fail with
    /// [`RecordError::Busy`] at once if another process holds it.
    pub fn open(path: &Path) -> Result<Record, RecordError> {
        Record::open_waiting(path, Duration::ZERO)
    }

    /// Open the record, waiting up to `wait` while another process holds it.
    /// Only one process can hold the file, so hooks fired side by side each open, append and close in turn.
    pub fn open_waiting(path: &Path, wait: Duration) -> Result<Record, RecordError> {
        let started = Instant::now();
        let mut attempt: u32 = 0;
        let db = loop {
            match Database::create(path) {
                Ok(db) => break db,
                Err(redb::DatabaseError::DatabaseAlreadyOpen) => {
                    if started.elapsed() >= wait {
                        return Err(RecordError::Busy);
                    }
                    thread::sleep(back_off(attempt));
                    attempt = attempt.saturating_add(1);
                }
                Err(e) => return Err(RecordError::Db(e.to_string())),
            }
        };
        let rec = Record { path: path.to_path_buf(), db: RwLock::new(Some(db)) };
        let g = rec.guard()?;
        let tx = db_err!(db_of(&g)?.begin_write())?;
        db_err!(tx.open_table(FACTS))?;
        db_err!(tx.commit())?;
        drop(g);
        Ok(rec)
    }

    fn guard(&self) -> Result<RwLockReadGuard<'_, Option<Database>>, RecordError> {
        self.db.read().map_err(|_| RecordError::Db("the record's lock is poisoned".to_string()))
    }

    /// Release the file while `f` runs, then reopen it, waiting up to `wait`.
    /// Other processes, or a fresh [`Record`] in this one, can use the record meanwhile.
    pub fn while_released<T>(&self, wait: Duration, f: impl FnOnce() -> T) -> Result<T, RecordError> {
        let mut g = self.db.write().map_err(|_| RecordError::Db("the record's lock is poisoned".to_string()))?;
        *g = None;
        let out = f();
        let again = Record::open_waiting(&self.path, wait)?;
        let fresh = again.db.write().map_err(|_| RecordError::Db("the record's lock is poisoned".to_string()))?.take();
        *g = fresh;
        Ok(out)
    }

    /// The next position: one past the highest key.
    pub fn len(&self) -> Result<u64, RecordError> {
        let g = self.guard()?;
        let tx = db_err!(db_of(&g)?.begin_read())?;
        let table = db_err!(tx.open_table(FACTS))?;
        let last = db_err!(table.last())?;
        Ok(match last {
            Some((k, _)) => k.value() + 1,
            None => 0,
        })
    }

    pub fn is_empty(&self) -> Result<bool, RecordError> {
        Ok(self.len()? == 0)
    }

    /// Append a fact given as a kind name and a JSON object of fields; it is durably in the file on `Ok`.
    pub fn append_raw(
        &self,
        kind: &str,
        fields: serde_json::Map<String, serde_json::Value>,
        at: f64,
        tenant: &str,
    ) -> Result<Fact, RecordError> {
        // Validate before anything is written.
        let probe = append_raw(&Vec::new(), kind, fields, at, tenant)?;
        let mut fact = probe
            .into_iter()
            .next()
            .ok_or_else(|| RecordError::Corrupt("append produced no fact".to_string()))?;
        // Read the position and write the fact in one write transaction, so two appends never share a position.
        let g = self.guard()?;
        let tx = db_err!(db_of(&g)?.begin_write())?;
        {
            let mut table = db_err!(tx.open_table(FACTS))?;
            fact.t = match db_err!(table.last())? {
                Some((k, _)) => k.value() + 1,
                None => 0,
            };
            let json = fact.to_json().to_string();
            db_err!(table.insert(fact.t, json.as_str()))?;
        }
        db_err!(tx.commit())?;
        Ok(fact)
    }

    /// Append a batch of facts, each a kind name, a JSON object of fields and its time, in one
    /// write transaction: on `Ok` every fact is durably in the file at the next positions, in
    /// order; a batch holding a fact the in-memory append refuses writes none of it.
    pub fn append_all(
        &self,
        facts: &[(&str, serde_json::Map<String, serde_json::Value>, f64)],
        tenant: &str,
    ) -> Result<Vec<Fact>, RecordError> {
        // Validate every fact before anything is written.
        let mut out: Log = Vec::with_capacity(facts.len());
        for (kind, fields, at) in facts {
            let probe = append_raw(&Vec::new(), kind, fields.clone(), *at, tenant)?;
            out.extend(probe);
        }
        if out.is_empty() {
            return Ok(out);
        }
        let g = self.guard()?;
        let tx = db_err!(db_of(&g)?.begin_write())?;
        {
            let mut table = db_err!(tx.open_table(FACTS))?;
            let mut next = match db_err!(table.last())? {
                Some((k, _)) => k.value() + 1,
                None => 0,
            };
            for fact in &mut out {
                fact.t = next;
                let json = fact.to_json().to_string();
                db_err!(table.insert(fact.t, json.as_str()))?;
                next += 1;
            }
        }
        db_err!(tx.commit())?;
        Ok(out)
    }

    /// Append only if `pred` holds on the whole log, read in the same write transaction as the insert.
    /// No other writer can append between the check and the write.
    pub fn append_if_raw(
        &self,
        kind: &str,
        fields: serde_json::Map<String, serde_json::Value>,
        at: f64,
        tenant: &str,
        pred: impl Fn(&Log) -> bool,
    ) -> Result<AppendIf, RecordError> {
        let probe = append_raw(&Vec::new(), kind, fields, at, tenant)?;
        let mut fact = probe
            .into_iter()
            .next()
            .ok_or_else(|| RecordError::Corrupt("append produced no fact".to_string()))?;
        let g = self.guard()?;
        let tx = db_err!(db_of(&g)?.begin_write())?;
        let appended = {
            let mut table = db_err!(tx.open_table(FACTS))?;
            let mut log: Log = Vec::new();
            {
                let iter = db_err!(table.iter())?;
                for row in iter {
                    let (_, json) = db_err!(row)?;
                    let value: serde_json::Value = serde_json::from_str(json.value())?;
                    log.push(Fact::from_json(&value).map_err(|e| RecordError::Corrupt(e.to_string()))?);
                }
            }
            if pred(&log) {
                fact.t = log.last().map_or(0, |f| f.t + 1);
                let json = fact.to_json().to_string();
                db_err!(table.insert(fact.t, json.as_str()))?;
                true
            } else {
                false
            }
        };
        if appended {
            db_err!(tx.commit())?;
            Ok(AppendIf::Appended(fact))
        } else {
            db_err!(tx.abort())?;
            Ok(AppendIf::Refused)
        }
    }

    /// [`Record::append_if_raw`] for a typed fact.
    pub fn append_if<T: FactFields>(
        &self,
        fields: &T,
        at: f64,
        tenant: &str,
        pred: impl Fn(&Log) -> bool,
    ) -> Result<AppendIf, RecordError> {
        self.append_if_raw(T::KIND, fields.to_fields()?, at, tenant, pred)
    }

    /// [`Record::append_raw`] for a typed fact.
    pub fn append<T: FactFields>(
        &self,
        fields: &T,
        at: f64,
        tenant: &str,
    ) -> Result<Fact, RecordError> {
        self.append_raw(T::KIND, fields.to_fields()?, at, tenant)
    }

    /// Every fact, in log order.
    pub fn read_all(&self) -> Result<Log, RecordError> {
        let g = self.guard()?;
        let tx = db_err!(db_of(&g)?.begin_read())?;
        let table = db_err!(tx.open_table(FACTS))?;
        let mut out = Vec::new();
        let iter = db_err!(table.iter())?;
        for row in iter {
            let (t, json) = db_err!(row)?;
            let t = t.value();
            let value: serde_json::Value = serde_json::from_str(json.value())?;
            let fact = Fact::from_json(&value).map_err(|e| RecordError::Corrupt(e.to_string()))?;
            if fact.t != t {
                return Err(RecordError::Corrupt(format!(
                    "stored position {t} but the fact says t={}",
                    fact.t
                )));
            }
            out.push(fact);
        }
        Ok(out)
    }

    /// Every fact of a kind, in log order.
    pub fn read_by_kind(&self, kind: &str) -> Result<Log, RecordError> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|f| f.kind == kind)
            .collect())
    }

    /// Every fact of a kind for one criterion, in log order.
    pub fn read_by_kind_and_criterion(
        &self,
        kind: &str,
        criterion_id: &str,
    ) -> Result<Log, RecordError> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|f| f.kind == kind && f.criterion_id() == Some(criterion_id))
            .collect())
    }

    /// The latest fact of a kind.
    pub fn last_of_kind(&self, kind: &str) -> Result<Option<Fact>, RecordError> {
        Ok(last_of_kind(&self.read_all()?, kind).cloned())
    }

    /// Every fact of a kind for one criterion, in log order, through [`of_criterion`].
    pub fn of_kind_for(&self, kind: &str, criterion_id: &str) -> Result<Log, RecordError> {
        let log = self.read_all()?;
        Ok(of_criterion(&log, kind, criterion_id)
            .into_iter()
            .cloned()
            .collect())
    }
}
