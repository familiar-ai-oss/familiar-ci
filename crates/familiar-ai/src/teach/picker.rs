//! Words and question JSON for the person's card and the push question, asked through
//! Claude Code's `AskUserQuestion` card. The agent writes the question's prose; the
//! PreToolUse hook refuses it until it quotes every piece, then sets the choices, and
//! PostToolUse reads the answer. `decide/picker.rs` decides and `door/picker.rs` writes.

use familiar_ai_rules::terminal::Card;
use familiar_ai_rules::View;
use serde_json::{json, Value};

use crate::teach::person_words::{self as pw, Asked, AGENTS_WORDS, SOMETHING_ELSE_WORDS, TYPE_THE_COMMAND_WORDS, WHAT_NOW};
use crate::teach::words::{quoted, LEAD};

pub use crate::teach::person_words::{SOMETHING_ELSE, TYPE_THE_COMMAND};

/// The question's `header`: how the hook knows its own card again.
pub const HEADER: &str = "familiar-ai";
/// The tool the question card is.
pub const TOOL: &str = "AskUserQuestion";
/// `hold_answered.choice` for an answer the person typed; the model only names the offered choices.
pub const TYPED_CHOICE: &str = "typed";

/// The push question.
pub const PUSH_QUESTION: &str = "Every criterion is locally green. Push and open the pull request so CI proves it?";
pub const PUSH_YES: &str = "yes, push and open it";
pub const PUSH_NOT_NOW: &str = "not now";
const PUSH_YES_WORDS: &str = "the agent pushes the branch and opens the pull request";
const PUSH_NOT_NOW_WORDS: &str = "nothing is pushed now";

/// The card's question as the record words it, naming the criterion by its own text.
pub fn question(v: &View, card: &Card) -> String {
    let q = crate::teach::words::named(v, &card.question);
    // A fix_elsewhere card also carries where the fix lives and what it needs.
    let more = crate::teach::words::elsewhere(v, card);
    if more.is_empty() {
        q
    } else {
        format!("{q} ({})", more.join("; "))
    }
}

fn one(question: &str, options: Vec<Value>) -> Value {
    json!({ "questions": [{ "question": question, "header": HEADER, "options": options, "multiSelect": false }] })
}

fn option(label: &str, description: &str) -> Value {
    json!({ "label": label, "description": description })
}

/// The text of the first question the agent asked, when it asked one.
pub fn question_of(asked: &Value) -> Option<&str> {
    asked.get("questions")?.as_array()?.first()?.get("question")?.as_str()
}

/// The whole `updatedInput` for a card: the agent's `question` as written, with the table's choices.
/// It carries no `answers`, and the hook must return `"ask"`, since `"allow"` with `updatedInput` runs the tool with no card and empty answers (`tests/fixtures/*ask_user_question*`).
pub fn card_input(v: &View, card: &Card, question: &str) -> Value {
    one(question, card_options(v, card).as_array().cloned().unwrap_or_default())
}

/// The most options Claude Code's picker shows; it adds its own "Other" beside them.
pub const MAX_OPTIONS: usize = 4;

/// The card's options: each choice's line as label and description, then `SOMETHING_ELSE` while
/// there is room; `no_test_stack` has its one option.
pub fn card_options(v: &View, card: &Card) -> Value {
    if card.hold == "no_test_stack" {
        return json!([option(TYPE_THE_COMMAND, TYPE_THE_COMMAND_WORDS)]);
    }
    // the one question [draft]: its two choices, the words typed under Claude Code's own "Other"
    if card.hold == familiar_ai_rules::trace::REPRO {
        return json!([option(familiar_ai_rules::trace::TYPE_IT_WORDS, pw::TYPE_IT_LINE), option(familiar_ai_rules::trace::GO_AHEAD_WORDS, pw::GO_AHEAD_LINE)]);
    }
    let a = pw::asked(v, card);
    // the cases card [draft]: its options by the finding; Claude Code's own "Other" takes words typed
    if let Some(s) = &a.cases {
        return Value::Array(s.options.iter().map(|(_, label, line)| option(label, line)).collect());
    }
    // the ask's card [draft]: each reading is its own label, verbatim
    let reading = card.hold == familiar_ai_rules::ask::HOLD;
    let choices = a.choices.iter().map(|(ch, line)| if reading { option(line, pw::READING_LINE) } else { option(pw::label_of(&card.hold, ch).unwrap_or(line), line) });
    // the picker takes at most four options; with four choices Claude Code's own "Other" is the typed answer
    let typed = (a.choices.len() < MAX_OPTIONS).then(|| option(SOMETHING_ELSE, SOMETHING_ELSE_WORDS));
    Value::Array(choices.chain(typed).collect())
}

/// The `hold_answered.choice` of the line the person picked.
pub fn choice_of(v: &View, card: &Card, line: &str) -> Option<String> {
    pw::asked(v, card).choices.into_iter().find(|(ch, l)| l == line || pw::label_of(&card.hold, ch) == Some(line)).map(|(ch, _)| ch)
}

/// The whole `updatedInput` for the push question.
pub fn push_input() -> Value {
    one(PUSH_QUESTION, vec![option(PUSH_YES, PUSH_YES_WORDS), option(PUSH_NOT_NOW, PUSH_NOT_NOW_WORDS)])
}

/// The shape the agent writes its question in, each line indented two spaces.
fn shape(a: &Asked) -> String {
    if let Some(s) = &a.cases {
        return crate::teach::cases_card::shape(s);
    }
    let mut lines: Vec<String> = pw::lines(a).into_iter().map(|l| format!("  {l}")).collect();
    lines.push(format!("  {AGENTS_WORDS} \"<one sentence of yours, in plain words: {}>\"", a.sentence));
    lines.push(format!("  {WHAT_NOW}"));
    lines.join("\n")
}

/// What the agent is told while the card is open, including that it must ask through the question card.
pub fn facts(v: &View, card: &Card) -> String {
    let a = pw::asked(v, card);
    if let Some(s) = &a.cases {
        return crate::teach::cases_card::facts(s, a.part, &question(v, card));
    }
    let (choices, answer) = if card.hold == "no_test_stack" {
        ("the person types the command in the question card", "answer")
    } else {
        ("leave the choices to the hook", "choice")
    };
    let open_for = if card.criterion_id == familiar_ai_rules::reading::SETUP_PART {
        pw::THE_TEST_SETUP.to_string()
    } else {
        format!("part {} of what they asked", a.part)
    };
    format!(
        "{LEAD}a question for the person is open for {open_for}. Write your question to them as prose, in this shape and order:\n{}\nQuote the person's words and each part's text exactly as above, write the one sentence yourself, and {choices}. The record's card: {} Ask the person with {TOOL}: the hook turns that call into the question card, and their {answer} is recorded; do not claim hold_answered yourself.",
        shape(&a),
        question(v, card)
    )
}

/// What the person's session is told of another checkout's open card: [`facts`], naming
/// the checkout, and the header that makes the hook ask that card.
pub fn facts_in(v: &View, card: &Card, folder: &str) -> String {
    let f = facts(v, card);
    let rest = f.strip_prefix(LEAD).unwrap_or(&f);
    format!("{LEAD}in the checkout {folder}, {rest} Give the question the header {HEADER}; the answer is recorded in the checkout {folder}.")
}

/// The person's line once their answer is written in another checkout's record.
pub fn recorded_in(line: &str, folder: &str) -> String {
    format!("{line} · in the checkout {folder}")
}

/// The agent's line once the person's answer is written in another checkout's record.
pub fn told_in(line: &str, folder: &str) -> String {
    if line.ends_with('.') {
        format!("{line} It went to the checkout {folder}.")
    } else {
        format!("{line} · in the checkout {folder}")
    }
}

/// The refusal of a question that does not carry every piece.
pub fn ask_again(v: &View, card: &Card, missing: &[String]) -> String {
    let a = pw::asked(v, card);
    format!("{LEAD}ask again; your question must quote: {}. The shape, in this order:\n{}", missing.join("; "), shape(&a))
}

/// The card as the person reads it in a terminal, below its question, with the choices numbered.
pub fn terminal(v: &View, card: &Card) -> Vec<String> {
    let a = pw::asked(v, card);
    if let Some(s) = &a.cases {
        return crate::teach::cases_card::terminal(s);
    }
    let mut lines: Vec<String> = pw::lines(&a).into_iter().map(|l| format!("  {l}")).collect();
    lines.push(format!("  {WHAT_NOW}"));
    lines.extend(a.choices.iter().enumerate().map(|(i, (_, line))| format!("    {}) {line}", i + 1)));
    let typed = if card.hold == "no_test_stack" { pw::TYPED_NO_TEST_STACK } else { pw::TYPED };
    lines.push(format!("  > {typed}"));
    lines
}

/// The person's line once their answer is written.
pub fn recorded(v: &View, c: &str, what: &str) -> String {
    format!("{LEAD}recorded your answer for {}: {what}", quoted(v, c))
}

/// The agent's line once the person's answer is written.
/// It says nothing is left to claim, since an agent told only the answer went on to claim it itself.
pub fn told(v: &View, c: &str, what: &str) -> String {
    format!("{LEAD}the person answered the question for {}: {what}. The answer is recorded; nothing is left to claim for it.", quoted(v, c))
}

/// The agent's line once the person's pick is written (plan §6.7): its `AFTER_PICK` row read on
/// the log after the pick (`after`), or today's line when the table has none.
pub fn told_after(before: &View, after: &View, c: &str, hold: &str, choice: &str, what: &str) -> String {
    match crate::teach::words::after_pick(before, after, c, hold, choice) {
        Some(cta) => format!("{LEAD}the person answered the question for {}: {what}. The answer is recorded. {cta}", quoted(after, c)),
        None => told(before, c, what),
    }
}

/// The orchestrator's line once the person's answer is written for a part someone else holds:
/// the holder is told what to do next (plan §6.7).
pub fn told_holder(v: &View, c: &str, what: &str) -> String {
    format!("{LEAD}the person answered the question for {}: {what}. The answer is recorded; the holder will be told what to do next.", quoted(v, c))
}

/// The line for `SOMETHING_ELSE` or `TYPE_THE_COMMAND`, or the cases card's "Add my own" [draft],
/// picked with nothing typed.
pub fn nothing_typed(v: &View, c: &str, label: &str) -> String {
    format!("{LEAD}the person picked \"{label}\" for {} and typed nothing; nothing is recorded and the question stays open", quoted(v, c))
}

/// What the agent is told when the push question is answered; no fact is written.
pub fn push_chosen(answer: &str) -> String {
    match answer {
        PUSH_YES => format!("{LEAD}the person chose: push and open the pull request"),
        PUSH_NOT_NOW => format!("{LEAD}the person chose: not now"),
        typed => format!("{LEAD}the person answered the push question: \"{typed}\""),
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
