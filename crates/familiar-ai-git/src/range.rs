//! Pure line arithmetic on hunks; nothing is read but the arguments.

use crate::{Hunk, MappedRange};

/// Whether any hunk touches the old-side line range `line_start..=line_end`
/// (the rule is `touches`).
///
/// An insertion right after `line_end` counts, because in an indentation
/// language adding a statement at the end of a function is exactly that; an
/// insertion before the first line does not.
pub fn overlap(hunks: &[Hunk], line_start: u64, line_end: u64) -> bool {
    hunks.iter().any(|h| touches(h, line_start, line_end))
}

/// Whether one hunk touches the range: a change or deletion touches it when
/// its old lines intersect the range, an insertion when it follows a line of
/// the range. [`overlap`], [`map_range`] and the put-back share this rule so
/// they cannot disagree.
pub(crate) fn touches(h: &Hunk, line_start: u64, line_end: u64) -> bool {
    if h.old_count == 0 {
        // inserted after old line `old_start - 1`, which is a line of the range
        line_start < h.old_start && h.old_start <= line_end.saturating_add(1)
    } else {
        // changes or deletes a line of the range
        h.old_start <= line_end && h.old_start.saturating_add(h.old_count) > line_start
    }
}

/// Map a 1-based inclusive range numbered in version X to its numbering in
/// version Y, given the line diff from X to Y in file order.
///
/// The result spans the range's untouched lines plus the whole new side of
/// every hunk that touches it, so putting the lines back also removes
/// insertions after the range's last line. It is [`MappedRange::Deleted`] when
/// no line of Y belongs to the range.
pub fn map_range(hunks: &[Hunk], line_start: u64, line_end: u64) -> MappedRange {
    if line_start == 0 || line_end < line_start {
        return MappedRange::Deleted;
    }
    let (s, e) = (i128::from(line_start), i128::from(line_end));
    let mut first: Option<i128> = None;
    let mut last: Option<i128> = None;
    let mut take = |from: i128, to: i128| {
        if from <= to {
            first = Some(first.map_or(from, |f| f.min(from)));
            last = Some(last.map_or(to, |l| l.max(to)));
        }
    };
    // `next_old`: the first line of X not yet passed. `delta`: how far a
    // line of X the diff left alone has moved by this point (Y minus X).
    let mut next_old: i128 = 1;
    let mut delta: i128 = 0;
    for h in hunks {
        let (os, oc) = (i128::from(h.old_start), i128::from(h.old_count));
        let (ns, nc) = (i128::from(h.new_start), i128::from(h.new_count));
        // Lines of X left alone before this hunk: next_old ..= os - 1.
        take(next_old.max(s) + delta, (os - 1).min(e) + delta);
        if touches(h, line_start, line_end) {
            take(ns, ns + nc - 1);
        }
        next_old = os + oc;
        delta += nc - oc;
    }
    // Lines of X left alone after the last hunk.
    take(next_old.max(s) + delta, e + delta);
    match (first, last) {
        (Some(f), Some(l)) if f >= 1 => MappedRange::Range {
            line_start: f as u64,
            line_end: l as u64,
        },
        _ => MappedRange::Deleted,
    }
}
