//! Pure reads over an in-memory log: the latest fact of a kind, and a kind's facts for one criterion.

use crate::append::{Fact, Log};

/// The latest fact of a kind.
pub fn last_of_kind<'a>(log: &'a Log, kind: &str) -> Option<&'a Fact> {
    log.iter().rev().find(|f| f.kind == kind)
}

/// Every fact of a kind for one criterion, in log order.
pub fn of_criterion<'a>(log: &'a Log, kind: &str, criterion_id: &str) -> Vec<&'a Fact> {
    log.iter()
        .filter(|f| f.kind == kind && f.criterion_id() == Some(criterion_id))
        .collect()
}
