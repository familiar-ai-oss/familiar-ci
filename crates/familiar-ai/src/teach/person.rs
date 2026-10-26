//! The person's words, the agent's reading of them, and the run they open.
//! Every prompt the person types is a `person_said`; an `intent_submitted` that
//! differs from them is the agent's reading. Mode and done are derived from each
//! criterion's state now, never replayed at a past point of the log.

use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::criterion::confirmed;
use familiar_ai_rules::{lifecycle, View};
use serde_json::{Map, Value};

use crate::door::claim::command_form;
use crate::teach::example::Project;

/// The person's words are cut to this many characters as an intent or a judge's input.
pub const MOST_WORDS: usize = 2_000;
/// How much of a person's prompt a refusal or a status line shows.
const MOST_SHOWN: usize = 120;

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

/// `s` cut to its first `n` characters (no mark: the intent's text).
pub fn first_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn said(log: &Log) -> Vec<&Fact> {
    log.iter().filter(|f| f.kind == "person_said").collect()
}

/// Every `person_said` text in order; an older record with none gives the first `intent_submitted`.
pub fn words(log: &Log) -> Vec<String> {
    // When the run's intent names a prompt with `for`, keep only it and the prompts its criteria name.
    let named = run_intent(log).and_then(answers).map(|first| {
        let since = run_intent(log).map_or(0, |i| i.t);
        let mut ts = vec![first];
        ts.extend(log.iter().filter(|f| f.kind == "criterion" && f.t > since).filter_map(answers));
        ts
    });
    let said: Vec<String> = said(log).iter().filter(|f| named.as_ref().is_none_or(|ts| ts.contains(&f.t))).filter_map(|f| text(f, "text")).map(str::to_string).collect();
    if !said.is_empty() {
        return said;
    }
    log.iter().find(|f| f.kind == "intent_submitted").and_then(|f| text(f, "text")).map(|t| vec![t.to_string()]).unwrap_or_default()
}

/// The person's answer on the card of an ask read more than one way, as their words [draft words].
pub fn picked_words(answer: &str) -> String {
    format!("asked which way they meant it, the person answered: {answer}")
}

/// The person's answer to the one question, how they make it happen, as their words [draft words].
pub fn how_words(answer: &str) -> String {
    format!("asked how they make it happen, the person answered: {answer}")
}

/// [`words`], then each answer the person gave on the card of an ask read more than one way
/// (the owner's plan of 2026-10-21, step 2) [draft]: the reading picked or the words typed are
/// the person's words too, and reach the judge as theirs.
pub fn words_and_picks(log: &Log) -> Vec<String> {
    let mut out = words(log);
    out.extend(familiar_ai_rules::ask::answers(&View::new(log)).iter().map(|a| picked_words(a)));
    // the one question (the owner's plan of 2026-10-23, change D) [draft]: words typed are theirs
    out.extend(familiar_ai_rules::trace::answers(&View::new(log)).iter().map(|a| how_words(a)));
    out
}

/// The latest `intent_submitted` when it differs from the person's own words.
/// The person's words here are the first prompt cut to `MOST_WORDS`, or the first intent in a record with no `person_said`.
pub fn agents_reading(log: &Log) -> Option<&str> {
    let first = log.iter().find(|f| f.kind == "intent_submitted")?;
    let last = log.iter().rev().find(|f| f.kind == "intent_submitted")?;
    let reading = text(last, "text")?;
    let persons = match said(log).first().and_then(|f| text(f, "text")) {
        Some(p) => first_chars(p, MOST_WORDS),
        None if last.t == first.t => return None,
        None => text(first, "text")?.to_string(),
    };
    (reading != persons).then_some(reading)
}

/// No run is open, or every criterion claimed since the run's intent is done.
pub const IDLE: &str = "idle";
/// A run is open: its intent is claimed and a criterion of it is not done.
pub const PROVING: &str = "proving";

/// Done, as the stop reads it: green, proven or released, but not a green
/// criterion an announced edit reopened, until that edit is run and stamped, nor
/// one a reviewer's verified "no" stands against, so its fix can be changed.
fn done(v: &View, c: &str) -> bool {
    let green = matches!(lifecycle::state(v, c), "green" | "proven") && !lifecycle::runnable(v, c) && familiar_ai_rules::review::standing_no(v, c).is_none();
    green || lifecycle::holds(v, c).iter().any(|h| h.why == "released")
}

/// Each criterion once, with the `t` of its first `criterion` fact.
fn claimed(log: &Log) -> Vec<(u64, &str)> {
    let mut out: Vec<(u64, &str)> = Vec::new();
    for f in log.iter().filter(|f| f.kind == "criterion") {
        if let Some(c) = f.criterion_id() {
            if !out.iter().any(|(_, x)| *x == c) {
                out.push((f.t, c));
            }
        }
    }
    out
}

/// Whether `i` is an old door's intent: the prompt's cut words at the next `t`, with no `for`.
fn old_doors(log: &Log, i: &Fact) -> bool {
    i.fields.get("for").is_none()
        && i.t > 0
        && log.iter().any(|p| p.kind == "person_said" && p.t + 1 == i.t && text(p, "text").map(|w| first_chars(w, MOST_WORDS)).as_deref() == text(i, "text"))
}

/// The intent that opened the current or last run; `None` when no run was ever opened.
/// A later intent opens a new run only when every criterion claimed before it is done
/// (at least one); an old door's intent that no criterion followed opens nothing.
pub fn run_intent(log: &Log) -> Option<&Fact> {
    let v = View::new(log);
    let crits = claimed(log);
    let mut run: Option<&Fact> = None;
    for i in log.iter().filter(|f| f.kind == "intent_submitted") {
        if old_doors(log, i) && !crits.iter().any(|(t, _)| *t > i.t) {
            continue;
        }
        let before: Vec<&str> = crits.iter().filter(|(t, _)| *t < i.t).map(|(_, c)| *c).collect();
        if run.is_none() || (!before.is_empty() && before.iter().all(|c| done(&v, c))) {
            run = Some(i);
        }
    }
    run
}

/// Whether the open run has a criterion claimed after its intent.
/// The plan's line 2 cannot answer this after a first run, since it reads every run's criteria.
pub fn run_has_criterion(log: &Log) -> bool {
    let since = run_intent(log).map_or(0, |i| i.t);
    claimed(log).iter().any(|(t, _)| *t > since)
}

/// The refusal of code in a run with no criterion of its own, with the criterion's claim.
pub fn criterion_first(log: &Log, project: &Project) -> String {
    let mut known = Map::new();
    if let Some(t) = run_intent(log).and_then(|i| i.fields.get("for")).and_then(Value::as_u64) {
        known.insert("for".to_string(), Value::from(t));
    }
    let mut lines = vec![format!("{}the plan comes first · next: write the criterion", crate::teach::words::LEAD)];
    lines.extend(command_form("criterion", &known, Some(project)).map(|c| format!("  claim it: {c}")));
    lines.join("\n")
}

/// `IDLE` or `PROVING`.
pub fn mode(log: &Log) -> &'static str {
    let Some(i) = run_intent(log) else {
        return IDLE;
    };
    let v = View::new(log);
    let since: Vec<&str> = claimed(log).into_iter().filter(|(t, _)| *t > i.t).map(|(_, c)| c).collect();
    if !since.is_empty() && since.iter().all(|c| done(&v, c)) {
        IDLE
    } else {
        PROVING
    }
}

/// How many of the person's prompts the first edit's refusal lists.
const MOST_LISTED: usize = 5;

/// The last `MOST_LISTED` prompts since the latest criterion fact, from which the agent picks the request.
pub fn recent(log: &Log) -> Vec<&Fact> {
    let after = log.iter().rev().find(|f| f.criterion_id().is_some()).map_or(0, |f| f.t + 1);
    let since: Vec<&Fact> = said(log).into_iter().filter(|p| p.t >= after).collect();
    since[since.len().saturating_sub(MOST_LISTED)..].to_vec()
}

/// The refusal of code while no run is open, listing recent prompts and the claim that names one.
pub fn demanded(log: &Log, code: &str, line: Option<&(u64, String)>, project: &Project) -> String {
    let mut lines = vec![format!("{}the plan comes first · line 1: the intent · {code} is code", crate::teach::words::LEAD)];
    if let Some((n, l)) = line {
        lines.push(format!("  line {n} is code: {}", one_line(l)));
    }
    let listed = recent(log);
    for (i, p) in listed.iter().enumerate() {
        let lead = if i == 0 { "the person said" } else { "               " };
        lines.push(format!("  {lead}  t{}  \"{}\"", p.t, one_line(text(p, "text").unwrap_or(""))));
    }
    if !listed.is_empty() {
        lines.push("  name the prompt that asked for this work; --text is your reading of it".to_string());
    }
    lines.extend(intent_claim(log, project).map(|c| format!("  claim it: {c}")));
    lines.extend(crate::teach::words::plan_checklist(project));
    lines.push(EDIT_WAITS.to_string());
    lines.join("\n")
}

/// The last line of the first edit's refusal while no run is open.
pub const EDIT_WAITS: &str = "  nothing is edited until the plan is confirmed; this edit waits";

/// The intent's claim: naming the latest of the recent prompts, else the form's.
pub fn intent_claim(log: &Log, project: &Project) -> Option<String> {
    match recent(log).last() {
        Some(p) => {
            let words = Value::from(one_line(text(p, "text").unwrap_or("")));
            Some(format!("familiar-ai claim intent_submitted --for {} --text {}", p.t, crate::door::claim::shell_word(&words)))
        }
        None => command_form("intent_submitted", &Map::new(), Some(project)),
    }
}

/// The `t` of the `person_said` a fact answers.
fn answers(f: &Fact) -> Option<u64> {
    f.fields.get("for").and_then(Value::as_u64)
}

/// The open run's prompts that no criterion or `person_said_answered` names with `for`.
/// Prompts typed while planning are answered by the plan, and with no run open nothing must answer them.
pub fn unabsorbed(log: &Log) -> Vec<&Fact> {
    if mode(log) == IDLE {
        return Vec::new();
    }
    let since = run_intent(log).map_or(0, |i| i.t);
    let any_criterion = log.iter().any(|f| f.kind == "criterion");
    said(log)
        .into_iter()
        .filter(|p| p.t > since)
        .filter(|p| {
            let named = log.iter().any(|f| matches!(f.kind, "criterion" | "person_said_answered") && answers(f) == Some(p.t));
            let prefix = log.get(..usize::try_from(p.t).unwrap_or(usize::MAX)).unwrap_or(log);
            let while_planning = any_criterion && !confirmed(&View::new(prefix));
            !named && !while_planning
        })
        .collect()
}

/// A prompt on one line, cut at `MOST_SHOWN` characters.
pub fn one_line(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MOST_SHOWN {
        return flat;
    }
    format!("{}…", first_chars(&flat, MOST_SHOWN))
}

/// The refusal of a stop while `p` is unabsorbed, with a criterion that names it
/// or the mark that it needs no work.
pub fn refusal(p: &Fact, project: &Project) -> String {
    let mut lines = vec![format!(
        "{}not done yet · the person said \"{}\"; no criterion names it yet, and it is not marked as needing no work",
        crate::teach::words::LEAD,
        one_line(text(p, "text").unwrap_or(""))
    )];
    lines.extend(answering_claims(p, project));
    lines.join("\n")
}

/// The claims that answer `p`: a criterion that names it, or the mark that it needs no work.
fn answering_claims(p: &Fact, project: &Project) -> Vec<String> {
    let mut lines = Vec::new();
    let mut known = Map::new();
    known.insert("for".to_string(), Value::from(p.t));
    if let Some(c) = command_form("criterion", &known, Some(project)) {
        lines.push(format!("  claim it: {c}"));
    }
    known.insert("how".to_string(), Value::from("no_work"));
    if let Some(a) = command_form("person_said_answered", &known, Some(project)) {
        lines.push(format!("  or:       {a}"));
    }
    lines
}

/// What the agent is told when the person types `p` during an open run: that nothing answers it,
/// that it holds the stop, and the claims that answer it.
pub fn heard_during_run(p: &Fact, project: &Project) -> String {
    let t = p.t;
    let mut lines = vec![format!(
        "{}the person said \"{}\" (t {t}); nothing answers it yet; it holds the stop until a criterion names it with --for {t}, one criterion per part of it, or person_said_answered marks it as needing no work",
        crate::teach::words::LEAD,
        one_line(text(p, "text").unwrap_or(""))
    )];
    lines.extend(answering_claims(p, project));
    lines.join("\n")
}

/// One line per unabsorbed prompt, for `familiar-ai status` and the session's start.
pub fn status_lines(log: &Log) -> Vec<String> {
    unabsorbed(log)
        .into_iter()
        .map(|p| format!("{}the person said \"{}\"; nothing answers it yet", crate::teach::words::LEAD, one_line(text(p, "text").unwrap_or(""))))
        .collect()
}

/// The line the person is shown when their first words become the intent.
pub fn intent_line(said: &str) -> String {
    format!("{}your request is the intent: \"{}\"", crate::teach::words::LEAD, one_line(said))
}

#[cfg(test)]
#[path = "person_tests.rs"]
mod tests;
