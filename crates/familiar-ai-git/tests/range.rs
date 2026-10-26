//! `overlap` and `map_range` are right at every edge of the range, here lines
//! 40..=62.

mod common;

use familiar_ai_git::{map_range, overlap, Hunk, MappedRange};

const S: u64 = 40;
const E: u64 = 62;

/// A change hunk over old lines old_start..old_start+old_count replaced by
/// new_count lines, with the diff's net delta so far (0 for a lone hunk):
/// new_start = old_start + shift.
fn change(old_start: u64, old_count: u64, new_count: u64) -> Hunk {
    Hunk {
        old_start,
        old_count,
        new_start: old_start,
        new_count,
    }
}

/// New lines inserted after old line k: a hunk between old lines k and k+1,
/// so old_start = k + 1, new_start = k + 1 (no shift so far).
fn insert(after_old_line: u64, n: u64) -> Hunk {
    Hunk {
        old_start: after_old_line + 1,
        old_count: 0,
        new_start: after_old_line + 1,
        new_count: n,
    }
}

#[test]
fn overlap_changes_at_every_edge() {
    // A change just before the range: no overlap.
    assert!(!overlap(&[change(38, 2, 2)], S, E));
    // At the first line.
    assert!(overlap(&[change(40, 1, 1)], S, E));
    // Inside.
    assert!(overlap(&[change(50, 3, 3)], S, E));
    // At the last line.
    assert!(overlap(&[change(62, 1, 1)], S, E));
    // Just after: no overlap.
    assert!(!overlap(&[change(63, 1, 1)], S, E));
    // Spanning the whole range.
    assert!(overlap(&[change(1, 100, 100)], S, E));
    // Deleting the first line.
    assert!(overlap(&[change(40, 1, 0)], S, E));
    // Deleting just before.
    assert!(!overlap(&[change(39, 1, 0)], S, E));
}

#[test]
fn overlap_insertions_at_every_edge() {
    // Inserting before the first line (after old line 39): no overlap.
    assert!(!overlap(&[insert(39, 5)], S, E));
    // Inserting after the first line: overlap.
    assert!(overlap(&[insert(40, 5)], S, E));
    // Inside.
    assert!(overlap(&[insert(50, 5)], S, E));
    // After the last line: overlap (adding a statement at a function's end).
    assert!(overlap(&[insert(62, 5)], S, E));
    // After the line past the range: no overlap.
    assert!(!overlap(&[insert(63, 5)], S, E));
}

#[test]
fn map_range_untouched() {
    assert_eq!(
        map_range(&[], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: E
        }
    );
}

#[test]
fn map_range_shifted_by_changes_above() {
    // Lines added above the range: it shifts, same size.
    assert_eq!(
        map_range(&[change(10, 2, 5)], S, E),
        MappedRange::Range {
            line_start: 43,
            line_end: 65
        }
    );
    // Lines removed above the range.
    assert_eq!(
        map_range(&[change(10, 4, 1)], S, E),
        MappedRange::Range {
            line_start: 37,
            line_end: 59
        }
    );
    // Pure insertion before the first line shifts the whole range.
    assert_eq!(
        map_range(&[insert(39, 3)], S, E),
        MappedRange::Range {
            line_start: 43,
            line_end: 65
        }
    );
}

#[test]
fn map_range_changes_inside_grow_and_shrink() {
    // A 2-line change inside becomes 5 lines: the range grows by 3.
    assert_eq!(
        map_range(&[change(50, 2, 5)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: 65
        }
    );
    // A 5-line change inside becomes 1: the range shrinks by 4.
    assert_eq!(
        map_range(&[change(50, 5, 1)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: 58
        }
    );
}

#[test]
fn map_range_insertion_at_last_line_grows() {
    // Insertion after the last line becomes part of the range (it grows), so
    // that putting the lines back removes them too.
    assert_eq!(
        map_range(&[insert(62, 2)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: 64
        }
    );
    // Insertion after a line just past the range does not.
    assert_eq!(
        map_range(&[insert(63, 2)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: E
        }
    );
}

#[test]
fn map_range_change_straddling_the_first_line() {
    // A hunk from line 38 over 4 lines (38..=41) becomes 2 lines: the two
    // cut lines before the range shift the start; the range keeps its own
    // surviving lines.
    assert_eq!(
        map_range(&[change(38, 4, 2)], S, E),
        MappedRange::Range {
            line_start: 38,
            line_end: 60
        }
    );
}

#[test]
fn map_range_change_straddling_the_last_line() {
    // Hunk over 61..=63 replaced by 5 lines: it eats range lines 61..=62 and
    // one line past the range; the replacement (5 lines) joins the range, so
    // the range grows by 5 - 2 = 3.
    assert_eq!(
        map_range(&[change(61, 3, 5)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: 65
        }
    );
}

#[test]
fn map_range_every_line_deleted() {
    assert_eq!(map_range(&[change(40, 23, 0)], S, E), MappedRange::Deleted);
    // A hunk wider than the range that replaces all of it with 3 lines: the
    // range maps onto the replacement, since only a range with nothing in its
    // place is deleted.
    assert_eq!(
        map_range(&[change(30, 40, 3)], S, E),
        MappedRange::Range {
            line_start: 30,
            line_end: 32
        }
    );
    // The same hunk replacing it with nothing: deleted.
    assert_eq!(map_range(&[change(30, 40, 0)], S, E), MappedRange::Deleted);
    // Deleting all but one line: still a range (of the survivor).
    match map_range(&[change(41, 22, 0)], S, E) {
        MappedRange::Range {
            line_start,
            line_end,
        } => {
            assert_eq!((line_start, line_end), (40, 40));
        }
        other => panic!("expected Range, got {other:?}"),
    }
}

#[test]
fn map_range_changes_after_do_not_move_it() {
    assert_eq!(
        map_range(&[change(70, 2, 9), insert(80, 4)], S, E),
        MappedRange::Range {
            line_start: S,
            line_end: E
        }
    );
}
