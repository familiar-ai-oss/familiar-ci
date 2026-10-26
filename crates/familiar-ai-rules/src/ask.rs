//! A clarifying question at plan time, only on a signal (the owner's plan of 2026-10-21, step 2)
//! [draft names]. The rules read the person's words for a part: the prompt its intent names (the
//! intent claimed last before the part was first claimed). Fixed, generic triggers fire on them:
//! - `FewWords`: few content words (at most `MOST_CONTENT_WORDS` once stop words are left out)
//!   and no example: no code fence, no inline code, no quoted value, no call such as `f(x)`;
//! - `Dangling`: a word that points at things the text does not name ("other", "remaining", "the
//!   rest", "etc", "as well") with no list in the text it could point at;
//! - `OpenQuestions`: two or more sentences that ask or doubt (a sentence ending in "?", or one
//!   that says "not sure", "why", "what's going on", "I don't think").
//!
//! The agent answers with its word on the ask, made after the intent: `ONE` (it reads one way, and
//! why) or `ALTERNATIVES` (the two or three readings it sees). While a part is in its turn with no
//! tests-first commit:
//! - a trigger fired and the agent has said neither: the part holds `HOLD`, the agent's hold;
//! - the agent's latest word lists readings and the person has not answered since: the part holds
//!   `HOLD` as the person's card, one card per ask, its choices the readings verbatim (at most
//!   `MOST_READINGS`, so the picker's own "something else" is the fourth); any answer, a reading
//!   picked or words typed, closes it, and the answer is the person's.
//!
//! Either way the tests-first commit waits. A record whose intent names no prompt of the person's
//! (written before prompts were recorded) has no words to read: nothing fires.

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::reader::{t_of, text, View};

/// The agent's readings of an ask that reads more than one way [draft name].
pub const ALTERNATIVES: &str = "reading_alternatives";
/// The agent's word that an ask reads one way, and why [draft name].
pub const ONE: &str = "one_reading";
/// The hold, the agent's while its word is owed, the person's card while readings stand [draft name].
pub const HOLD: &str = "reading_unclear";
/// The most readings a card offers: the picker shows four options, its own "something else" one.
pub const MOST_READINGS: usize = 3;
/// At most this many content words, with no example, fire `FewWords`.
pub const MOST_CONTENT_WORDS: usize = 12;
/// The sample's why for `ONE` and readings for `ALTERNATIVES`, offered as moves [draft].
pub const SAMPLE_WHY: &str = "the ask names the one thing it changes";
pub const SAMPLE_READINGS: [&str; 2] = ["the first way the ask reads", "the second way it reads"];

/// A trigger on the person's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    FewWords,
    Dangling,
    OpenQuestions,
}

impl Trigger {
    /// The trigger in words, for the agent's refusal [draft words].
    pub fn words(self) -> &'static str {
        match self {
            Trigger::FewWords => "few words and no example (no code, quoted value or call)",
            Trigger::Dangling => "a word that points at things the ask does not list (other, remaining, the rest, etc, as well)",
            Trigger::OpenQuestions => "two or more open questions or doubts",
        }
    }
}

/// Common English words that carry no content of their own.
const STOP: &[&str] = &[
    "a", "about", "above", "after", "again", "all", "also", "am", "an", "and", "any", "are", "as", "at", "be", "because", "been", "before", "being",
    "below", "between", "both", "but", "by", "can", "could", "did", "do", "does", "doing", "don't", "down", "during", "each", "either", "else",
    "even", "every", "few", "for", "from", "further", "get", "got", "had", "has", "have", "having", "he", "her", "here", "hers", "him", "his", "how",
    "i", "i'm", "i've", "if", "in", "into", "is", "isn't", "it", "it's", "its", "itself", "just", "let", "let's", "me", "more", "most", "much",
    "must", "my", "no", "nor", "not", "now", "of", "off", "on", "once", "one", "only", "or", "our", "ours", "out", "over", "own", "please", "same",
    "she", "should", "so", "some", "such", "than", "that", "that's", "the", "their", "theirs", "them", "then", "there", "these", "they", "this",
    "those", "through", "to", "too", "under", "until", "up", "us", "very", "was", "we", "were", "what", "what's", "when", "where", "which",
    "while", "who", "whom", "why", "will", "with", "would", "you", "your", "yours",
];

/// The words of `s`, lower-cased: runs of letters, digits, `_` and inner apostrophes.
fn words(s: &str) -> Vec<String> {
    s.replace('\u{2019}', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\''))
        .map(|w| w.trim_matches('\'').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// The content words of `s`, in order: its words less the stop words.
pub fn content_words(s: &str) -> Vec<String> {
    words(s).into_iter().filter(|w| !STOP.contains(&w.as_str())).collect()
}

/// `s` with its code left out: fenced blocks and inline code.
fn prose(s: &str) -> String {
    let mut out = String::new();
    for (i, block) in s.split("```").enumerate() {
        if i % 2 == 0 {
            for (j, part) in block.split('`').enumerate() {
                if j % 2 == 0 {
                    out.push_str(part);
                }
                out.push(' ');
            }
        }
        out.push('\n');
    }
    out
}

/// Whether `s` quotes a value: text between double quotes (straight or curly), or between single
/// quotes standing at word edges (an apostrophe inside a word is no quote).
fn quotes(s: &str) -> bool {
    let cs: Vec<char> = s.chars().collect();
    let pair = |open: char, close: char| {
        cs.iter().position(|c| *c == open).is_some_and(|i| cs[i + 1..].iter().position(|c| *c == close).is_some_and(|j| j > 0))
    };
    if pair('"', '"') || pair('\u{201c}', '\u{201d}') {
        return true;
    }
    let edge = |c: Option<&char>| c.is_none_or(|c| !c.is_alphanumeric());
    for (i, c) in cs.iter().enumerate() {
        if !(*c == '\'' || *c == '\u{2018}') || !edge(i.checked_sub(1).and_then(|k| cs.get(k))) || cs.get(i + 1).is_none_or(|n| n.is_whitespace()) {
            continue;
        }
        let closed = (i + 2..cs.len()).any(|j| (cs[j] == '\'' || cs[j] == '\u{2019}') && !cs[j - 1].is_whitespace() && edge(cs.get(j + 1)));
        if closed {
            return true;
        }
    }
    false
}

/// Whether `s` has a call: a name followed at once by `(` and later `)`, such as `f(x)`.
fn calls(s: &str) -> bool {
    let cs: Vec<char> = s.chars().collect();
    cs.iter().enumerate().any(|(i, c)| {
        *c == '(' && i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '_') && cs[i + 1..].contains(&')')
    })
}

/// Whether `s` gives an example: a code fence, inline code, a quoted value or a call.
pub fn has_example(s: &str) -> bool {
    s.contains("```") || s.matches('`').count() >= 2 || quotes(s) || calls(s)
}

/// The sentences of `s`, each with whether it ends in a question mark.
fn sentences(s: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if matches!(c, '.' | '!' | '?' | '\n') {
            if !cur.trim().is_empty() {
                out.push((cur.trim().to_string(), c == '?'));
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push((cur.trim().to_string(), false));
    }
    out
}

/// Whether `s` lists things: two or more lines that start as list items, or a sentence of three
/// or more items (two commas, or a comma and an "and" or "or" after it).
fn lists(s: &str) -> bool {
    let items = s
        .lines()
        .map(str::trim_start)
        .filter(|l| {
            l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") || {
                let n = l.chars().take_while(char::is_ascii_digit).count();
                n > 0 && matches!(l[n..].chars().next(), Some('.' | ')'))
            }
        })
        .count();
    items >= 2
        || s.split(['.', ';', '!', '?', '\n']).any(|sentence| {
            let commas = sentence.matches(',').count();
            let after = sentence.rfind(',').map(|i| words(&sentence[i..])).unwrap_or_default();
            commas >= 2 || (commas == 1 && after.iter().any(|w| w == "and" || w == "or"))
        })
}

/// Whether `s` has a word pointing at things it does not name; idioms that point at nothing
/// ("other than", "each other", "on the other hand", "as well as") are left out.
fn points_away(s: &str) -> bool {
    let ws = words(s);
    let at = |i: usize| ws.get(i).map(String::as_str);
    (0..ws.len()).any(|i| match ws[i].as_str() {
        "other" | "others" => {
            !(at(i + 1) == Some("than") || (i > 0 && at(i - 1) == Some("each")) || (at(i + 1) == Some("hand") && i > 0 && at(i - 1) == Some("the")))
        }
        "remaining" | "etc" => true,
        "rest" => i > 0 && at(i - 1) == Some("the"),
        "well" => i > 0 && at(i - 1) == Some("as") && at(i + 1) != Some("as"),
        _ => false,
    })
}

/// Whether a sentence asks or doubts.
fn open(sentence: &str, asks: bool) -> bool {
    let ws = words(sentence);
    let has = |phrase: &[&str]| ws.windows(phrase.len()).any(|w| w.iter().zip(phrase).all(|(a, b)| a == b));
    asks
        || has(&["not", "sure"])
        || has(&["why"])
        || has(&["what's", "going", "on"])
        || has(&["what", "is", "going", "on"])
        || has(&["i", "don't", "think"])
        || has(&["i", "dont", "think"])
        || has(&["i", "do", "not", "think"])
}

/// The triggers that fire on the person's words `s`, in a fixed order.
pub fn triggers(s: &str) -> Vec<Trigger> {
    let p = prose(s);
    let mut out = Vec::new();
    if content_words(s).len() <= MOST_CONTENT_WORDS && !has_example(s) {
        out.push(Trigger::FewWords);
    }
    if points_away(&p) && !lists(&p) {
        out.push(Trigger::Dangling);
    }
    if sentences(&p).iter().filter(|(x, q)| open(x, *q)).count() >= 2 {
        out.push(Trigger::OpenQuestions);
    }
    out
}

/// The intent `c` was first claimed under: the latest claimed before its first `criterion`.
pub fn intent_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let first = v.of("criterion", c).next()?;
    v.kind("intent_submitted").filter(|i| t_of(i) < t_of(first)).last()
}

/// The person's words for `c`: the prompt its intent names; `None` when it names none.
pub fn words_of<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let t = intent_of(v, c)?.fields.get("for").and_then(Value::as_u64)?;
    v.kind("person_said").find(|p| p.t == t).and_then(|p| text(p, "text"))
}

/// The triggers that fire on `c`'s person's words; none without them.
pub fn fired(v: &View, c: &str) -> Vec<Trigger> {
    words_of(v, c).map(triggers).unwrap_or_default()
}

/// The readings of an `ALTERNATIVES` fact, each trimmed, blanks and repeats left out, in order.
pub fn readings_in(f: &Fact) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in f.fields.get("readings").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        let r = r.trim().to_string();
        if !r.is_empty() && !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// Whether the agent's word says something: two or more readings, or one reading with its why.
fn counts(f: &Fact) -> bool {
    match f.kind {
        ALTERNATIVES => readings_in(f).len() >= 2,
        ONE => text(f, "why").is_some_and(|w| !w.trim().is_empty()),
        _ => false,
    }
}

/// The agent's latest word on `c`'s ask that counts, made after its intent.
pub fn word_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let i = intent_of(v, c)?;
    v.facts.iter().rfind(|f| matches!(f.kind, ALTERNATIVES | ONE) && t_of(f) > t_of(i) && counts(f))
}

/// The readings the person is asked to pick from for `c`, at most `MOST_READINGS`, while the
/// agent's latest word lists them; empty otherwise.
pub fn readings(v: &View, c: &str) -> Vec<String> {
    word_of(v, c).filter(|f| f.kind == ALTERNATIVES).map(|f| readings_in(f).into_iter().take(MOST_READINGS).collect()).unwrap_or_default()
}

/// The person's answer to the readings of `c`'s ask: a `hold_answered` on `HOLD` after them.
pub fn answer<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let w = word_of(v, c).filter(|f| f.kind == ALTERNATIVES)?;
    v.kind("hold_answered").filter(|a| t_of(a) > t_of(w) && text(a, "hold") == Some(HOLD)).last()
}

/// Whether `c` is where the hold is read: its turn, before its tests-first commit.
fn gated(v: &View, c: &str) -> bool {
    crate::lifecycle::turn(v, c) && crate::criterion::tests_first_commit(v, c).is_none()
}

/// Whether the agent's word on `c`'s ask is owed: a trigger fired and it has said nothing since
/// the intent.
pub fn owed(v: &View, c: &str) -> bool {
    gated(v, c) && word_of(v, c).is_none() && !fired(v, c).is_empty()
}

/// Whether `c`'s ask has readings the person has not picked from.
pub fn card_open(v: &View, c: &str) -> bool {
    gated(v, c) && !readings(v, c).is_empty() && answer(v, c).is_none()
}

/// Whether `c` holds `HOLD`, the agent's or the person's.
pub fn holds(v: &View, c: &str) -> bool {
    owed(v, c) || card_open(v, c)
}

/// Whether `c` carries its ask's card: the first part of that ask whose card is open.
pub fn carries(v: &View, c: &str) -> bool {
    let i = intent_of(v, c).map(t_of);
    card_open(v, c) && crate::plan::criteria(v).iter().find(|x| intent_of(v, x).map(t_of) == i && card_open(v, x)).is_some_and(|x| x == c)
}

/// The person's answers to readings, oldest first: the reading picked, or the words typed.
pub fn answers(v: &View) -> Vec<String> {
    v.kind("hold_answered")
        .filter(|a| text(a, "hold") == Some(HOLD))
        .filter_map(|a| {
            let typed = text(a, "typed").map(str::trim).filter(|t| !t.is_empty());
            typed.or_else(|| text(a, "choice").map(str::trim)).filter(|t| !t.is_empty()).map(str::to_string)
        })
        .collect()
}

/// Why the door refuses readings, in the agent's words; `None` when they may be claimed.
pub fn refused_readings(readings: &[&str]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for r in readings.iter().map(|r| r.trim()) {
        if r.is_empty() {
            return Some("a reading is blank: write each reading as one plain sentence".into());
        }
        if seen.contains(&r) {
            return Some(format!("\"{r}\" is listed twice: each reading differs from the others"));
        }
        seen.push(r);
    }
    if !(2..=MOST_READINGS).contains(&seen.len()) {
        return Some(format!(
            "list two or three readings, the ways the person's words can be read that would change the work; {} given (one reading is one_reading, with why)",
            seen.len()
        ));
    }
    None
}

/// Why the door refuses a `ONE` why; `None` when it may be claimed.
pub fn refused_why(why: &str) -> Option<String> {
    why.trim().is_empty().then(|| "one_reading needs its why: one line on what in the person's words, or in the code, settles how the ask reads".to_string())
}
