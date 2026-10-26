//! The line diff, the range mapping and the put-back agree with each other on
//! generated files whose edits are known to fall inside or outside the range.

use familiar_ai_git::{line_hunks, lines_put_back, map_range, overlap, MappedRange};

/// A small fixed-seed generator: the same cases on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// One old line's fate, and what is inserted after it.
#[derive(Clone)]
struct Edit {
    keep: Option<String>, // None = deleted; Some = the line as it stands in the new file
    insert_after: Vec<String>,
}

fn build(edits_before_first: &[String], edits: &[Edit]) -> Vec<u8> {
    let mut out = String::new();
    for l in edits_before_first {
        out.push_str(l);
    }
    for e in edits {
        if let Some(l) = &e.keep {
            out.push_str(l);
        }
        for l in &e.insert_after {
            out.push_str(l);
        }
    }
    out.into_bytes()
}

#[test]
fn putting_the_range_back_undoes_the_inside_edits_and_keeps_the_outside_ones() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let (mut ran, mut all_deleted) = (0, 0);
    for case in 0..4000u64 {
        let n = 5 + rng.below(36); // lines in the old file
        let s = 1 + rng.below(n);
        let e = s + rng.below(n - s + 1);
        let old_lines: Vec<String> = (1..=n).map(|i| format!("L{i}\n")).collect();
        let old: Vec<u8> = old_lines.concat().into_bytes();

        // `tip` gets every edit; `want` gets only the edits outside the range.
        let untouched = |i: u64| Edit {
            keep: Some(format!("L{i}\n")),
            insert_after: vec![],
        };
        let mut tip: Vec<Edit> = (1..=n).map(untouched).collect();
        let mut want: Vec<Edit> = tip.clone();
        let mut inside_edit = false;
        let mut first_tip: Vec<String> = vec![];
        let mut first_want: Vec<String> = vec![];
        if s > 2 && rng.below(4) == 0 {
            first_tip.push(format!("new-top-{case}\n")); // before line 1: outside
            first_want = first_tip.clone();
        }
        for i in 1..=n {
            let idx = (i - 1) as usize;
            let inside = s <= i && i <= e;
            // keep one untouched line each side of the range between inside and outside edits
            let outside = i + 1 < s || i > e + 1;
            if !inside && !outside {
                continue;
            }
            let roll = rng.below(10);
            let mut edit = untouched(i);
            if roll == 0 {
                edit.keep = None;
            } else if roll == 1 {
                edit.keep = Some(format!("changed-{case}-{i}\n"));
            }
            if rng.below(8) == 0 {
                let count = 1 + rng.below(3);
                edit.insert_after = (0..count)
                    .map(|j| format!("new-{case}-{i}-{j}\n"))
                    .collect();
            }
            let touched = roll <= 1 || !edit.insert_after.is_empty();
            tip[idx] = edit.clone();
            if inside {
                inside_edit |= touched;
            } else {
                want[idx] = edit;
            }
        }
        let tip_bytes = build(&first_tip, &tip);
        let want_bytes = build(&first_want, &want);
        let hunks = line_hunks(&old, &tip_bytes);

        assert_eq!(
            overlap(&hunks, s, e),
            inside_edit,
            "case {case}: overlap, range {s}..={e}, hunks {hunks:?}"
        );

        let got = lines_put_back(&old, &tip_bytes, &hunks, "f", s, e)
            .unwrap_or_else(|err| panic!("case {case}: {err}"));
        assert_eq!(
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&want_bytes),
            "case {case}: range {s}..={e} of {n} lines, hunks {hunks:?}\nTIP:\n{}",
            String::from_utf8_lossy(&tip_bytes)
        );
        // the range, wholly deleted and nothing put in its place, maps to nothing
        let nothing_left = (s..=e).all(|i| {
            tip[(i - 1) as usize].keep.is_none() && tip[(i - 1) as usize].insert_after.is_empty()
        });
        assert_eq!(
            map_range(&hunks, s, e) == MappedRange::Deleted,
            nothing_left,
            "case {case}: hunks {hunks:?}"
        );
        ran += 1;
        if nothing_left {
            all_deleted += 1;
        }
    }
    assert_eq!(ran, 4000);
    assert!(
        all_deleted > 0,
        "the generator never produced a wholly deleted range"
    );
}

#[test]
fn a_range_nobody_touched_maps_to_the_same_lines_moved() {
    let mut rng = Rng(0xD1B54A32D192ED03);
    for case in 0..2000u64 {
        let n = 5 + rng.below(30);
        let s = 2 + rng.below(n - 2);
        let e = s + rng.below(n - s);
        let old_lines: Vec<String> = (1..=n).map(|i| format!("L{i}\n")).collect();
        // edits strictly above the range only: delete some lines, add some
        let mut new_lines: Vec<String> = vec![];
        for i in 1..=n {
            if i + 1 < s && rng.below(4) == 0 {
                continue; // deleted
            }
            new_lines.push(format!("L{i}\n"));
            if i + 1 < s && rng.below(4) == 0 {
                new_lines.push(format!("new-{case}-{i}\n"));
            }
        }
        let old = old_lines.concat().into_bytes();
        let new = new_lines.concat().into_bytes();
        let hunks = line_hunks(&old, &new);
        let MappedRange::Range {
            line_start,
            line_end,
        } = map_range(&hunks, s, e)
        else {
            panic!("case {case}: deleted");
        };
        let got: Vec<&String> = new_lines[(line_start - 1) as usize..line_end as usize]
            .iter()
            .collect();
        let want: Vec<&String> = old_lines[(s - 1) as usize..e as usize].iter().collect();
        assert_eq!(got, want, "case {case}");
        assert!(!overlap(&hunks, s, e), "case {case}");
    }
}

#[test]
fn a_statement_added_at_the_end_of_the_anchor_is_inside_it_and_is_removed_on_put_back() {
    let old = b"def f():\n    a = 1\n    return a\nprint(f())\n";
    let tip = b"def f():\n    a = 1\n    return a\n    unreachable()\nprint(f())\n";
    let hunks = line_hunks(old, tip);
    assert!(overlap(&hunks, 1, 3));
    assert_eq!(
        map_range(&hunks, 1, 3),
        MappedRange::Range {
            line_start: 1,
            line_end: 4
        }
    );
    assert_eq!(
        lines_put_back(old, tip, &hunks, "f.py", 1, 3).unwrap(),
        old.to_vec()
    );
    // a line added just above the anchor is outside it and stays
    let tip2 = b"import os\ndef f():\n    a = 1\n    return a\nprint(f())\n";
    let hunks2 = line_hunks(old, tip2);
    assert!(!overlap(&hunks2, 1, 3));
    assert_eq!(
        map_range(&hunks2, 1, 3),
        MappedRange::Range {
            line_start: 2,
            line_end: 4
        }
    );
    assert_eq!(
        lines_put_back(old, tip2, &hunks2, "f.py", 1, 3).unwrap(),
        tip2.to_vec()
    );
}

#[test]
fn a_change_that_runs_past_the_edge_of_the_range_is_undone_whole() {
    // Lines 2 and 3 both changed, side by side: one hunk. Putting line 2
    // back must not leave half of that hunk behind (a file that is neither
    // version): the whole hunk is undone.
    let old = b"a\nb\nc\nd\n";
    let tip = b"a\nB\nC\nd\n";
    let hunks = line_hunks(old, tip);
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        lines_put_back(old, tip, &hunks, "f", 2, 2).unwrap(),
        old.to_vec()
    );
}

#[test]
fn a_file_without_a_final_newline_is_handled() {
    let old = b"a\nb\nc";
    let tip = b"a\nB\nc\nd";
    let hunks = line_hunks(old, tip);
    assert!(overlap(&hunks, 2, 2));
    // line 3 changed too (it gained a newline), in the same hunk as line 2
    assert_eq!(
        lines_put_back(old, tip, &hunks, "f", 2, 2).unwrap(),
        old.to_vec()
    );
    // a range past the end of the earlier file is an error, not a guess
    assert!(lines_put_back(old, tip, &hunks, "f", 3, 9).is_err());
    assert!(lines_put_back(old, tip, &hunks, "f", 0, 1).is_err());
}

#[test]
fn a_range_deleted_outright_comes_back() {
    let old = b"a\nb\nc\nd\n";
    let tip = b"a\nd\n";
    let hunks = line_hunks(old, tip);
    assert_eq!(map_range(&hunks, 2, 3), MappedRange::Deleted);
    assert_eq!(
        lines_put_back(old, tip, &hunks, "f", 2, 3).unwrap(),
        old.to_vec()
    );
}
