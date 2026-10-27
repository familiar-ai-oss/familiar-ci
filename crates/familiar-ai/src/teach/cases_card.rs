//! The cases card as the person sees it (the owner's plan of 2026-10-25, §3, wording approved by
//! the owner) [draft names]: its lines, its options by the finding, the hint under Claude Code's own
//! "Other", the pieces a question must quote, and what the agent reads around it (the plan's line 3
//! and the line after the person's pick). The card's question, lines and options are the rules'
//! (`terminal::cases_lines`, `terminal::cases_options`); this file lays them out.

use familiar_ai_rules::terminal::{self, Card, CASES_ALSO, CASES_FAILED, CASES_NONE};
use familiar_ai_rules::{cases, View};
use serde_json::Value;

use crate::teach::words::quoted;

/// The hint under Claude Code's own "Other": on found cases, and on none or a failed panel.
pub const OTHER_FOUND: &str = "(Other: say which to add, like \"1 and 3\", or describe one)";
pub const OTHER_NONE: &str = "(Other: describe a case to add)";
/// The column the options' descriptions start at in the terminal, past the label.
const OPTIONS_AT: usize = 30;

/// The card as the person reads it: its lines (the question first), the hint under "Other", and
/// its options `(choice, label, description)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub lines: Vec<String>,
    pub other: &'static str,
    pub options: Vec<(String, String, String)>,
}

/// Reads the card's pieces: the question naming the part by its text, a blank line, the planned
/// checks, then the found ones under "Also worth checking:" and the finding's note of what was
/// left out (the owner's ruling, 2026-10-25, "Keep what fits"), or what the panel said.
pub fn shown(v: &View, card: &Card) -> Shown {
    let mut lines = vec![crate::teach::words::named(v, &card.question), String::new()];
    let mut found = false;
    let note = terminal::cases_note(v, &card.criterion_id);
    for l in &card.summary {
        if l == CASES_ALSO || l == CASES_NONE || l == CASES_FAILED || note.as_ref() == Some(l) {
            found |= l == CASES_ALSO;
            lines.push(String::new());
            lines.push(l.clone());
        } else {
            lines.push(format!("  {l}"));
        }
    }
    let options = terminal::cases_options(v, &card.criterion_id).into_iter().map(|(ch, label, line)| (ch.to_string(), label, line)).collect();
    Shown { lines, other: if found { OTHER_FOUND } else { OTHER_NONE }, options }
}

/// The question the agent writes, each line indented two spaces: the card's lines and the hint.
pub fn shape(s: &Shown) -> String {
    s.lines.iter().map(|l| if l.is_empty() { String::new() } else { format!("  {l}") }).chain(std::iter::once(format!("  {}", s.other))).collect::<Vec<_>>().join("\n")
}

/// The lines `question` leaves out, each named; whitespace runs compare as one space. Nothing else
/// is asked of it: no "you asked", no "what happened", no sentence of the agent's.
pub fn missing(s: &Shown, question: &str) -> Vec<String> {
    let q = crate::teach::person_words::one_line(question);
    s.lines
        .iter()
        .map(|l| crate::teach::person_words::one_line(l))
        .filter(|l| !l.is_empty())
        .chain(std::iter::once(s.other.to_string()))
        .filter(|l| !q.contains(l.as_str()))
        .map(|l| format!("the line \"{l}\""))
        .collect()
}

/// The card in a terminal, below its question: its lines, then the options with their
/// descriptions lined up, then the hint.
pub fn terminal(s: &Shown) -> Vec<String> {
    let mut out: Vec<String> = s.lines.iter().skip(1).map(|l| if l.is_empty() { String::new() } else { format!("  {l}") }).collect();
    out.push(String::new());
    let width = s.options.iter().map(|(_, label, _)| label.chars().count() + 3).max().unwrap_or(0).max(OPTIONS_AT);
    out.extend(s.options.iter().map(|(_, label, line)| format!("    • {label:<width$}{line}")));
    out.push(format!("    {}", s.other));
    out
}

/// The lens panel's questions still open on `c`, in the synthesis's order.
pub fn open_questions(v: &View, c: &str) -> Vec<familiar_ai_facts::FoundQuestion> {
    let open = cases::unanswered_questions(v, c);
    cases::questions(v, c).into_iter().filter(|q| open.contains(&q.question_id)).collect()
}

/// A question of the lens panel's as the person reads it in the picker [draft words]: its text,
/// then why it matters when the synthesis said.
pub fn question_text(q: &familiar_ai_facts::FoundQuestion) -> String {
    let why = q.why.split_whitespace().collect::<Vec<_>>().join(" ");
    if why.is_empty() {
        q.text.clone()
    } else {
        format!("{}\n\nWhy it matters: {why}", q.text)
    }
}

/// What the agent is told while the card is open: the card's lines to write while its choice is
/// owed, and the lens panel's questions the hook adds to the same call [draft words]; once the
/// choice is answered, only that the panel's questions are open.
pub fn facts(v: &View, c: &str, s: &Shown, part: usize, record: &str) -> String {
    use crate::teach::picker::TOOL;
    let lead = crate::teach::words::LEAD;
    let open = open_questions(v, c);
    let n = match open.len() {
        1 => "its question".to_string(),
        k => format!("its {k} questions"),
    };
    if cases::choice_answered(v, c) {
        return format!(
            "{lead}the lens panel's questions are open for part {part} of what they asked. Ask the person with {TOOL}: the hook sets {n} and their options from the record, and each pick is recorded; do not claim question_answered yourself."
        );
    }
    let questions = if open.is_empty() { String::new() } else { format!(" The hook adds the lens panel's {} to the same call, one question each, with their options.", n.trim_start_matches("its ")) };
    format!(
        "{lead}the cases card is open for part {part} of what they asked. Write your question to them as these lines, word for word:\n{}\nLeave the choices to the hook: it sets them, and Claude Code adds its own Other for words typed.{questions} The record's card: {record} Ask the person with {TOOL}: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself.",
        shape(s),
    )
}

/// The cases the person asked for on `c` that its promise does not hold yet, each with what it
/// says: a found case's given and expect, the person's own case in their words.
pub fn wanted(v: &View, c: &str) -> String {
    let found = |id: &str| {
        cases::findings(v, c).iter().rev().find_map(|f| {
            f.fields.get("cases").and_then(Value::as_array)?.iter().find(|k| k.get("case_id").and_then(Value::as_str) == Some(id)).map(|k| {
                let s = |key: &str| k.get(key).and_then(Value::as_str).unwrap_or("").to_string();
                format!("{id} (given {} → expect {})", s("given"), s("expect"))
            })
        })
    };
    cases::missing(v, c)
        .iter()
        .filter_map(|id| found(id).or_else(|| cases::persons_case(v, c, id).map(|w| format!("{id} (the person's words: \"{w}\")"))))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The plan's line 3 while `c` waits on the cases card: the lens panel owed, the card open, or the
/// cases the person asked for not in its promise yet; `None` otherwise.
pub fn line_3(v: &View, c: &str) -> Option<String> {
    let q = quoted(v, c);
    if cases::owed(v, c) {
        // the lens panel (the owner's plan of 2026-10-26, approved) [draft words]
        let next = crate::teach::lens::next(v, c).unwrap_or_default();
        return Some(format!("{q}: the lens panel reads the plan for the checks the planned cases miss: {next}"));
    }
    if cases::card_open(v, c) {
        return Some(format!("{q}: the person checks the cases it will cover: ask them with AskUserQuestion"));
    }
    if cases::missing(v, c).is_empty() {
        return None;
    }
    let pid = v.of("criterion", c).last().and_then(|f| f.fields.get("promise_id")).and_then(Value::as_str).unwrap_or("");
    Some(format!("{q}: the person asked for {}: claim promise {pid} again with each added, its kind and a short line", wanted(v, c)))
}

/// The line after the person's pick on `c`'s cases card, read on the log after it.
pub fn after_pick(after: &View, c: &str, choice: &str) -> Option<String> {
    let q = quoted(after, c);
    let keep = format!("The planned cases stand for {q}: write its test.");
    Some(match choice {
        cases::AGAIN => format!("One more lens panel for {q}: its five lenses, one Agent call each, in parallel, then the synthesis; each runs as {} on familiar-ai's brief.", crate::door::lens::LENS),
        cases::ADD_ALL | cases::TYPED => match wanted(after, c) {
            w if w.is_empty() => keep,
            w => format!("Add the cases the person asked for to the promise of {q} and claim it again: {w}. Then write its test."),
        },
        _ => keep,
    })
}

