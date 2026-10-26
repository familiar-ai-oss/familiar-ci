//! H-1, lane H2 (plan of record 2026-10-19): exit 0 with output a table does not know is not a
//! pass. No row whose exit condition holds at 0 and whose pattern matches the empty text may
//! read `passed` or `no_tests_collected` — such a row speaks for any output at all, including
//! none, so a run that printed nothing would be a green (or an honest "no tests") at a fix
//! commit. The one exception is the shell's: H-2 makes a sealed script's own exit 0 its green.
//! And the exit default for 0 is `unclassified`.

use familiar_ai::door::kept::{classify_with, Streams};
use familiar_ai::door::runner_tables::TABLES;
use familiar_ai_run::End;

/// Whether a row's exit condition holds at exit 0 (`any`, `0`, or a comma list holding 0).
fn holds_at_zero(cond: &str) -> bool {
    match cond.trim() {
        "any" | "0" => true,
        "nonzero" | "sig" => false,
        list => list.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).any(|n| n == 0),
    }
}

#[test]
fn no_row_that_holds_at_exit_zero_and_matches_nothing_reads_passed_or_no_tests() {
    let mut bad = Vec::new();
    for t in TABLES {
        for (i, row) in t.rows.iter().enumerate() {
            if !holds_at_zero(row.exit) || !matches!(row.outcome, "passed" | "no_tests_collected") {
                continue;
            }
            let Ok(rx) = regex::RegexBuilder::new(row.regex).size_limit(1 << 28).dfa_size_limit(1 << 26).build() else { continue };
            if rx.is_match("") && t.runner != "sh" {
                bad.push(format!("{} row {i} reads {} for any output: {}", t.runner, row.outcome, row.regex.chars().take(60).collect::<String>()));
            }
        }
    }
    assert!(bad.is_empty(), "rows that read passed or no_tests_collected at exit 0 for empty output:\n{}", bad.join("\n"));
}

#[test]
fn exit_zero_with_nothing_any_table_knows_is_unclassified_not_passed() {
    let empty = Streams { stdout: "", stderr: "", report: "" };
    for t in TABLES.iter().filter(|t| t.runner != "sh") {
        let got = classify_with(&[t], false, End::Exited(0), &empty).map(|x| x.label);
        assert_eq!(got.as_deref(), Some("unclassified"), "{}: empty output at exit 0", t.runner);
    }
    let junk = Streams { stdout: "something nobody wrote a row for\n", stderr: "", report: "" };
    for t in TABLES.iter().filter(|t| t.runner != "sh") {
        let got = classify_with(&[t], false, End::Exited(0), &junk).map(|x| x.label);
        assert_eq!(got.as_deref(), Some("unclassified"), "{}: junk at exit 0", t.runner);
    }
}
