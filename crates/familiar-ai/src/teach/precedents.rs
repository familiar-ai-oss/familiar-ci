//! The precedents the seal's judge reads (ports 2026-10-24, piece 4) [draft names]: lines outside
//! the tests, at the seal commit, that write a compound name the sealed test passes, or writes in
//! its own f-string, inside the `{...}` of a format string: how the code already writes that value
//! out. This module only formats; `door::precedents` finds the lines.

/// The cap on the whole section, its head included [draft name]. The judge reads the reading and
/// the anchor under `MOST_INPUTS`; this section has a cap of its own.
pub const MOST_PRECEDENTS: usize = 1_000;
/// The most entries printed [draft name].
pub const MOST_ENTRIES: usize = 3;
/// Each code line is cut to this many characters [draft name].
pub const MOST_PER_LINE: usize = 160;

/// The head of the section [draft words].
pub const PRECEDENTS_HEAD: &str = "PRECEDENTS (lines outside the tests, at the seal commit, that format a name the sealed test passes or formats itself: how the code already writes that value out)";

/// The check asked of each line, numbered after the question's other checks [draft words].
pub const PRECEDENTS_CHECK: &str = "For each line under PRECEDENTS: does the sealed test expect that name's value written the way that line writes it? \
If the test writes it differently and the person's words do not ask for the difference, answers_intent is false and gap names the line as file:line.";

/// A line the lister found, as the judge is shown it [draft name].
#[derive(Debug, Clone, PartialEq)]
pub struct Precedent {
    pub file: String,
    /// 1-based.
    pub line: u64,
    /// The `def`/`class` path the line is in, dotted; empty outside any.
    pub def: String,
    /// The test's names the line writes, sorted.
    pub names: Vec<String>,
    /// The previous non-blank line, with its number, when there is one.
    pub before: Option<(u64, String)>,
    pub code: String,
}

/// One code line as printed: its number and its text, trimmed and cut to `MOST_PER_LINE`.
fn code_line(n: u64, text: &str) -> String {
    format!("  {n}: {}", text.trim().chars().take(MOST_PER_LINE).collect::<String>())
}

/// One entry: where the line is and the names it writes, the line before it, the line.
fn entry(p: &Precedent) -> String {
    let def = if p.def.is_empty() { String::new() } else { format!(", in {}", p.def) };
    let mut out = format!("{}:{}{def} (names {})\n", p.file, p.line, p.names.join(", "));
    if let Some((n, text)) = &p.before {
        out.push_str(&code_line(*n, text));
        out.push('\n');
    }
    out.push_str(&code_line(p.line, &p.code));
    out.push('\n');
    out
}

/// The section, best first: at most `MOST_ENTRIES` whole entries within `MOST_PRECEDENTS`, the
/// first always; empty when nothing was found.
pub fn section(found: &[Precedent]) -> String {
    let Some((first, rest)) = found.split_first() else {
        return String::new();
    };
    let mut out = format!("{PRECEDENTS_HEAD}\n{}", entry(first));
    for p in rest.iter().take(MOST_ENTRIES - 1) {
        let e = entry(p);
        if out.chars().count() + e.chars().count() > MOST_PRECEDENTS {
            break;
        }
        out.push_str(&e);
    }
    out
}

/// The check as the question's check number `n`.
pub fn check(n: usize) -> String {
    format!("{n}. {PRECEDENTS_CHECK}\n")
}
