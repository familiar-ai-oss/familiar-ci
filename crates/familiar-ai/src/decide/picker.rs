//! The hook's decisions around Claude Code's picker. On PreToolUse the open card is asked in place of
//! the agent's question, or the agent is told to ask again; on PostToolUse the person's pick is read back
//! for the door to write.

use familiar_ai_facts::Log;
use familiar_ai_rules::terminal::{self, Card};
use familiar_ai_rules::{ladder, plan, View};
use serde_json::Value;

use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::person_words;
use crate::teach::picker::{self as words, HEADER, PUSH_QUESTION, SOMETHING_ELSE, TOOL, TYPE_THE_COMMAND};

/// The first open card for this conversation: it holds the criterion, or it is the main conversation and the
/// holder is one of its investigators or there is none. A hold that sends for an investigator is the person's only once no rung is left.
pub fn open_card(v: &View, input: &HookInput) -> Option<Card> {
    let me = input.agent_conversation_id()?;
    terminal::cards(v).into_iter().find(|card| {
        let c = card.criterion_id.as_str();
        let main = !input.in_subagent() && input.conversation.is_none();
        let mine = match plan::assigned_to(v, c) {
            Some(h) => h == me || (main && ladder::session_of(Some(h)) == ladder::session_of(Some(me.as_str()))),
            None => main,
        };
        let persons = !ladder::SENT_FOR.contains(&card.hold.as_str()) || crate::door::investigator::rung(v, c).is_none();
        mine && persons
    })
}

/// The first of another checkout's relayed cards that is the person's to
/// answer, whoever holds it: a hold that sends for an investigator is the
/// person's only once no rung is left.
pub fn persons_card(other: &Elsewhere) -> Option<Card> {
    let v = View::new(&other.log);
    other
        .cards
        .iter()
        .find(|card| !ladder::SENT_FOR.contains(&card.hold.as_str()) || crate::door::investigator::rung(&v, &card.criterion_id).is_none())
        .cloned()
}

/// Another checkout's record as read, for the person's picker there: its
/// folder's name, its facts and the cards it relays (`relayed_cards`).
#[derive(Debug, Clone)]
pub struct Elsewhere {
    pub folder: String,
    pub log: Log,
    pub cards: Vec<Card>,
}

/// True for the person's own session: the main conversation, not a subagent,
/// not a session the binary launched.
pub fn persons_session(input: &HookInput) -> bool {
    !input.in_subagent() && input.conversation.is_none()
}

/// The `header` of the one question asked, when it is the hook's own.
fn headed(input: &HookInput) -> bool {
    input.tool_input.as_ref().and_then(asked).is_some_and(|(_, header, _)| header == HEADER)
}

/// True for a call of the picker tool.
pub fn is_picker(input: &HookInput) -> bool {
    input.tool_name.as_deref() == Some(TOOL)
}

/// What the hook does with the picker's PreToolUse.
#[derive(Debug, Clone, PartialEq)]
pub enum Rewrite {
    /// Ask, with this as the `updatedInput`.
    Ask(Value),
    /// Refused, with the words that tell the agent to ask again.
    Refuse(String),
}

/// Rewrite the picker's PreToolUse to ask the open card, or refuse the agent's question; `None` leaves it untouched.
/// With no card of this checkout open, the person's own question with the hook's header asks the first
/// card of another checkout (`elsewhere`, read only then) in the same way.
/// `push` says whether the push question is due, and is asked only when no card is open.
pub fn rewrite(input: &HookInput, log: &Log, push: impl FnOnce() -> bool, elsewhere: impl FnOnce() -> Vec<Elsewhere>) -> Option<Rewrite> {
    if input.event != HookEvent::PreToolUse || !is_picker(input) {
        return None;
    }
    let question = input.tool_input.as_ref().and_then(words::question_of).unwrap_or("");
    let ask = |v: &View, card: &Card| {
        let missing = person_words::missing(&person_words::asked(v, card), question);
        if missing.is_empty() {
            Rewrite::Ask(words::card_input(v, card, question))
        } else {
            Rewrite::Refuse(words::ask_again(v, card, &missing))
        }
    };
    let v = View::new(log);
    if let Some(card) = open_card(&v, input) {
        return Some(ask(&v, &card));
    }
    if persons_session(input) && headed(input) {
        for other in elsewhere() {
            let v = View::new(&other.log);
            if let Some(card) = persons_card(&other) {
                return Some(ask(&v, &card));
            }
        }
    }
    (persons_session(input) && push()).then(|| Rewrite::Ask(words::push_input()))
}

/// What the person picked, read from the picker's PostToolUse.
#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    /// One of the card's choices (`choice`) or typed words (`typed`).
    Answer { criterion_id: String, hold: String, choice: String, typed: String },
    /// no_test_stack: the command typed.
    Command { criterion_id: String, command: String },
    /// "something else" / "type the command" picked with nothing typed.
    NothingTyped { criterion_id: String, label: String },
    /// The push question's answer, which writes no fact.
    Push(String),
}

/// The question's text, header and options, when the input is one question.
fn asked(input: &Value) -> Option<(&str, &str, &Value)> {
    let qs = input.get("questions")?.as_array()?;
    let [q] = qs.as_slice() else {
        return None;
    };
    Some((q.get("question")?.as_str()?, q.get("header")?.as_str()?, q.get("options")?))
}

/// The answer the picker returned for `question`: the label picked or the text typed, both as the value.
fn answer_of<'a>(input: &'a HookInput, question: &str) -> Option<&'a str> {
    let a = input.tool_response.as_ref()?.get("answers")?.get(question)?.as_str()?.trim();
    (!a.is_empty()).then_some(a)
}

/// The person's pick from the picker's PostToolUse, when the question was the hook's own card or the push question.
/// The PostToolUse input carries `answers` whoever gave them, so a card counts only when it was open before this call
/// and now, with its header, every piece in the question and exactly its options.
pub fn picked(input: &HookInput, before: &Log, log: &Log) -> Option<Picked> {
    if input.event != HookEvent::PostToolUse || !is_picker(input) {
        return None;
    }
    let (question, header, options) = asked(input.tool_input.as_ref()?)?;
    if header != HEADER {
        return None;
    }
    let answer = answer_of(input, question)?;
    let (b, v) = (View::new(before), View::new(log));
    let placed = |v: &View, card: &Card| *options == words::card_options(v, card) && person_words::missing(&person_words::asked(v, card), question).is_empty();
    let was_open = open_card(&b, input).is_some_and(|card| placed(&b, &card));
    let Some(card) = open_card(&v, input).filter(|card| was_open && placed(&v, card)) else {
        let persons = !input.in_subagent() && input.conversation.is_none();
        return (persons && question == PUSH_QUESTION).then(|| Picked::Push(answer.to_string()));
    };
    Some(pick_of(&v, &card, answer))
}

/// The person's pick on `card`, from the picker's `answer`.
fn pick_of(v: &View, card: &Card, answer: &str) -> Picked {
    let c = card.criterion_id.clone();
    if card.hold == "no_test_stack" {
        return match answer {
            TYPE_THE_COMMAND => Picked::NothingTyped { criterion_id: c, label: TYPE_THE_COMMAND.into() },
            typed => Picked::Command { criterion_id: c, command: typed.to_string() },
        };
    }
    if answer == SOMETHING_ELSE {
        return Picked::NothingTyped { criterion_id: c, label: SOMETHING_ELSE.into() };
    }
    let (choice, typed) = match words::choice_of(v, card, answer) {
        Some(ch) => (ch, String::new()),
        None => (words::TYPED_CHOICE.to_string(), answer.to_string()),
    };
    Picked::Answer { criterion_id: c, hold: card.hold.clone(), choice, typed }
}

/// The person's pick from the picker's PostToolUse on another checkout's card: with no card of this
/// checkout open (`own`), in the person's own session, the question with the hook's header, every piece
/// and exactly the options of the first person's card `other` relays.
pub fn picked_elsewhere(input: &HookInput, own: &Log, other: &Elsewhere) -> Option<Picked> {
    if input.event != HookEvent::PostToolUse || !is_picker(input) || !persons_session(input) {
        return None;
    }
    let (question, header, options) = asked(input.tool_input.as_ref()?)?;
    if header != HEADER || open_card(&View::new(own), input).is_some() {
        return None;
    }
    let answer = answer_of(input, question)?;
    let card = persons_card(other)?;
    let v = View::new(&other.log);
    let placed = *options == words::card_options(&v, &card) && person_words::missing(&person_words::asked(&v, &card), question).is_empty();
    placed.then(|| pick_of(&v, &card, answer))
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
