//! Test first, then pick the spot from what the test ran, and the one question (the owner's plan
//! of 2026-10-23, changes A and D) [draft names].
//!
//! A part is *traced* when the stack its tests run under is `pytest` (`TRACED_STACK`), the one
//! runner with a call recorder. For a traced part the plan's order is: lines 1–3 and 6–10
//! (`plan::test_may_be_written`); the person's card `REPRO` when their words show no way to make
//! the problem happen; the planned test files written; a try (`familiar-ai try`, the binary's
//! `LISTED` fact) that must be red; then line 4, the place, on the try's list or with the agent's
//! `OFF_LIST` reason; then lines 5 and 11 and the rest as before. Any other part keeps today's order.
//!
//! - `TRY_OWED`, the agent's: the test may be written and no try of the tests planned now ran.
//! - `TRY_NOT_RED`, the agent's: the latest try of the tests planned now passed.
//! - `REPRO`, the person's card, before the part's first try: "How do you make it happen? What do
//!   you run, and what do you see?". Any answer closes it; words typed are the person's.
//!
//! A part whose tests-first commit was taken (a `test_written` of it) needs no try: the committed
//! red run is the stronger look, and a record from before the try reads as it did. A light part
//! (documentation only) needs none either: no function a test runs is its spot.

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::plan::{self, PlaceRef};
use crate::reader::{t_of, text, View};

/// The binary's listing of what a try's failing tests ran [draft name].
pub const LISTED: &str = "trace_listed";
/// The agent's one-line reason its place is not on the list [draft name].
pub const OFF_LIST: &str = "off_list_reason";
/// The agent's hold while a try is owed [draft name].
pub const TRY_OWED: &str = "try_owed";
/// The agent's hold while the latest try passed [draft name].
pub const TRY_NOT_RED: &str = "try_not_red";
/// The person's card: how to make it happen [draft name].
pub const REPRO: &str = "repro_unknown";
/// The card's question, the owner's words (2026-10-23).
pub const REPRO_QUESTION: &str = "How do you make it happen? What do you run, and what do you see?";
/// The card's choices, by `hold_answered.choice`, and their words [draft names].
pub const TYPE_IT: &str = "type_it";
pub const GO_AHEAD: &str = "go_ahead";
pub const TYPE_IT_WORDS: &str = "I'll type it";
pub const GO_AHEAD_WORDS: &str = "I don't know — go ahead";
/// The `test_stack` whose parts are traced: the runner with a call recorder.
pub const TRACED_STACK: &str = "pytest";
/// The most functions a listing keeps.
pub const MOST_LISTED: usize = 25;

/// The stack a part's try runs under: the stack covering its planned test files (the longest
/// root), else, when none covers them, any stack with a command.
pub fn stack_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let files: Vec<String> = v.of("test_planned", c).filter_map(|f| text(f, "file_path")).map(str::to_string).collect();
    plan::stack_covering(v, (!files.is_empty()).then_some(files.as_slice()), i64::MAX).or_else(|| plan::stack_covering(v, None, i64::MAX))
}

/// The command a try of `c` runs [draft]: the part's own command, else the stack its try runs
/// under ([`stack_of`]) applied to its planned test files by the runner table, else that stack's
/// command as claimed. Before a place `criterion::command` has no anchor to find its stack by, so
/// the try's stack is read from the planned test files.
pub fn command_of(v: &View, c: &str) -> Option<String> {
    if let Some(own) = crate::criterion::own_command(v, c) {
        return Some(own.to_string());
    }
    let st = stack_of(v, c)?;
    crate::criterion::derive_from(st, &crate::criterion::test_files(v, c)).or_else(|| text(st, "test_command").filter(|t| !t.trim().is_empty()).map(str::to_string))
}

/// Whether `c` is traced: the stack its try runs under is `TRACED_STACK`.
pub fn traced(v: &View, c: &str) -> bool {
    stack_of(v, c).and_then(|s| text(s, "test_stack")) == Some(TRACED_STACK)
}

/// Whether `c` needs a red try before its place: traced, not on the light path (documentation
/// only, where no function the test runs is the spot), its tests not yet committed, and not released.
pub fn needs_try(v: &View, c: &str) -> bool {
    // a released part owes no try (the owner, 2026-10-28)
    traced(v, c) && plan::path_of(v, c) != Some("light") && v.of("test_written", c).next().is_none() && !crate::lifecycle::released(v, c)
}

/// The latest try of `c`, any.
pub fn latest<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.of(LISTED, c).last()
}

/// The latest try of the tests planned now: the latest `LISTED` of `c`, when it is newer than the
/// part's latest `test_planned`.
pub fn current<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let planned = v.of("test_planned", c).last().map_or(-1, t_of);
    latest(v, c).filter(|f| t_of(f) > planned)
}

fn red(f: &Fact) -> bool {
    f.fields.get("red") == Some(&Value::Bool(true))
}

/// The red try line 4 is read against: the current try, when it was red.
pub fn red_try<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    current(v, c).filter(|f| red(f))
}

/// One function of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub rank: u64,
    pub file_path: String,
    pub symbol: String,
    pub line_start: u64,
    pub line_end: u64,
}

/// The functions a listing names, best first.
pub fn top_of(f: &Fact) -> Vec<Listed> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let n = |x: &Value, k: &str| x.get(k).and_then(Value::as_u64).unwrap_or(0);
    f.fields
        .get("top")
        .and_then(Value::as_array)
        .map(|xs| xs.iter().map(|x| Listed { rank: n(x, "rank"), file_path: s(x, "file_path"), symbol: s(x, "symbol"), line_start: n(x, "line_start"), line_end: n(x, "line_end") }).collect())
        .unwrap_or_default()
}

/// A name's segments: split on `.` and `::`, a trailing `()` left out, blanks dropped.
fn segments(name: &str) -> Vec<&str> {
    name.trim().trim_end_matches("()").split(['.', ':']).map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// Whether two names name one function: the shorter's segments end the longer's, so a bare name
/// (`write_rows`) matches a qualified one (`Writer.write_rows`, `export.Writer.write_rows`,
/// `Writer::write_rows`), and two qualified names must agree on every segment they both give
/// (`Reader.write_rows` is not `Writer.write_rows`). A blank name matches nothing.
pub fn names_match(a: &str, b: &str) -> bool {
    let (a, b) = (segments(a), segments(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    long.ends_with(&short)
}

fn same_file(a: &str, b: &str) -> bool {
    a.trim_start_matches("./") == b.trim_start_matches("./")
}

/// Whether a place names a listed function: the same file, and its symbol names the function
/// (`names_match`) or its lines lie inside the function's lines.
pub fn on_list(top: &[Listed], p: &PlaceRef) -> bool {
    top.iter().any(|f| {
        if !same_file(&f.file_path, p.file_path) {
            return false;
        }
        let inside = match (p.line_start, p.line_end) {
            (Some(a), Some(b)) => f.line_end > 0 && a >= f.line_start as i64 && b <= f.line_end as i64 && a <= b,
            _ => false,
        };
        names_match(&f.symbol, p.symbol) || inside
    })
}

/// The agent's latest reason that counts, made after `c`'s place was chosen: not blank, and after
/// the anchor or after the same place claimed before it, identical, with no other anchor exit and
/// no re_plan between (ports 2026-10-24, piece 2; the re_plan, the owner, 2026-10-25) [draft]:
/// claiming the same place again keeps its reason, and a re_plan ends it.
pub fn off_list_reason<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = plan::anchor_of(v, c)?;
    let chosen = v.facts.iter().rev().filter(|f| (plan::EXIT_KINDS.contains(&f.kind) || plan::is_re_plan(f)) && f.criterion_id() == Some(c)).take_while(|f| f.kind == a.kind && f.fields == a.fields).last().unwrap_or(a);
    v.of(OFF_LIST, c).filter(|f| t_of(f) > t_of(chosen) && text(f, "reason").is_some_and(|r| !r.trim().is_empty())).last()
}

/// The places of `c`'s anchor that are not on its red try's list; empty when the anchor is not a
/// chosen place, when no red try stands, or when the try listed nothing.
pub fn off_list<'a>(v: &View<'a>, c: &str) -> Vec<PlaceRef<'a>> {
    let (Some(a), Some(t)) = (plan::anchor_of(v, c), red_try(v, c)) else {
        return Vec::new();
    };
    if a.kind != "place_chosen" || plan::kept(a) {
        return Vec::new();
    }
    let top = top_of(t);
    if top.is_empty() {
        return Vec::new();
    }
    plan::places_of(a).into_iter().filter(|p| !on_list(&top, p)).collect()
}

/// What line 4 asks of `c`'s anchor `a` beyond being there: for a part that needs a try, a red try
/// of the tests planned now, and each place on its list or a reason after the anchor. New work is
/// exempt from the list (its code is not there yet), not from the try.
pub fn placed_after_the_try(v: &View, c: &str, a: &Fact) -> bool {
    if !needs_try(v, c) {
        return true;
    }
    if red_try(v, c).is_none() {
        return false;
    }
    a.kind != "place_chosen" || off_list(v, c).is_empty() || off_list_reason(v, c).is_some()
}

/// Whether the place waits on its reason: a red try stands and a place is off its list with none.
pub fn reason_owed(v: &View, c: &str) -> bool {
    needs_try(v, c) && !off_list(v, c).is_empty() && off_list_reason(v, c).is_none()
}

/// The try's hold on `c`, the agent's: `TRY_NOT_RED` when the current try passed, `TRY_OWED` when
/// none ran; `None` once a red try stands, or while no try is asked of it (the test may not be
/// written yet, or the part needs none).
pub fn try_hold(v: &View, c: &str) -> Option<&'static str> {
    if !needs_try(v, c) || !plan::test_may_be_written(v, c) {
        return None;
    }
    match current(v, c) {
        Some(f) if red(f) => None,
        Some(_) => Some(TRY_NOT_RED),
        None => Some(TRY_OWED),
    }
}

/// Whether `s` reads like a command: a shell prompt starting a line (`$ `, `% `, `> `, `>>> `), a
/// flag (`-k`, `--all`), or a known program followed by what it runs.
fn command_like(s: &str) -> bool {
    const PROGRAMS: &[&str] = &[
        "python", "python3", "pytest", "py.test", "pip", "uv", "poetry", "tox", "npm", "npx", "node", "yarn", "pnpm", "cargo", "bash", "dotnet", "mvn", "gradle",
    ];
    if s.lines().map(str::trim_start).any(|l| ["$ ", "% ", "> ", ">>> "].iter().any(|p| l.starts_with(p))) {
        return true;
    }
    let ws: Vec<&str> = s.split_whitespace().collect();
    let flag = |w: &str| {
        let w = w.trim_start_matches('-');
        w.len() < 40 && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    };
    ws.iter().any(|w| w.starts_with('-') && flag(w))
        || ws.windows(2).any(|p| PROGRAMS.contains(&p[0].trim_matches(|c: char| !c.is_alphanumeric() && c != '.')) && !p[1].is_empty())
}

/// Whether `s` quotes output or an error: a `Traceback`, or a name such as `KeyError`,
/// `ValueError`, `RuntimeWarning`, `NullPointerException`.
fn output_like(s: &str) -> bool {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| {
        w == "Traceback"
            || (w.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && ["Error", "Exception", "Warning"].iter().any(|e| w.len() > e.len() && w.ends_with(e)))
    })
}

/// Whether the person's words show a way to make it happen: an example as the ask's card reads one
/// (`ask::has_example`: a code fence, inline code, a quoted value, a call), a command, or quoted
/// output or an error.
pub fn shows_a_way(s: &str) -> bool {
    crate::ask::has_example(s) || command_like(s) || output_like(s)
}

/// The person's answers to `REPRO` on `c` after its intent.
fn answers_of<'a>(v: &View<'a>, c: &str) -> impl Iterator<Item = &'a Fact> {
    let since = crate::ask::intent_of(v, c).map_or(-1, t_of);
    v.of("hold_answered", c).filter(move |a| t_of(a) > since && text(a, "hold") == Some(REPRO))
}

/// Whether `c` holds `REPRO`, the person's card: it needs a try and none ran yet, its test may be
/// written, the person's words for it show no way to make it happen, and they have not answered.
pub fn repro_unknown(v: &View, c: &str) -> bool {
    needs_try(v, c)
        && latest(v, c).is_none()
        && plan::test_may_be_written(v, c)
        && crate::ask::words_of(v, c).is_some_and(|w| !shows_a_way(w))
        && answers_of(v, c).next().is_none()
}

fn typed(a: &Fact) -> Option<String> {
    text(a, "typed").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// The words the person typed on `c`'s card, the latest; `None` when they typed none.
pub fn answer_words(v: &View, c: &str) -> Option<String> {
    answers_of(v, c).filter_map(typed).last()
}

/// The words the person typed on every `REPRO` card, oldest first: theirs, for the judge and the
/// try's query. A choice picked with no words says nothing.
pub fn answers(v: &View) -> Vec<String> {
    v.kind("hold_answered").filter(|a| text(a, "hold") == Some(REPRO)).filter_map(typed).collect()
}

/// Why the door refuses an `OFF_LIST` reason; `None` when it may be claimed.
pub fn refused_reason(reason: &str) -> Option<String> {
    reason.trim().is_empty().then(|| "off_list_reason needs its reason: one line on why the fix is not in a function the failing test ran".to_string())
}

/// The listed function a place names, by rank, for the words that show a place [draft].
pub fn rank_of(v: &View, c: &str, p: &PlaceRef) -> Option<u64> {
    let t = red_try(v, c)?;
    top_of(t).into_iter().find(|f| on_list(std::slice::from_ref(f), p)).map(|f| f.rank)
}
