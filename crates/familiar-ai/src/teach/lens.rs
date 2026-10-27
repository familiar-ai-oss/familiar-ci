//! The lens panel's words (the owner's plan of 2026-10-26, lens-panel.md, approved with its names)
//! [draft words]: the five lenses' briefs and the synthesis's, which the binary hands each
//! subagent in place of the main agent's prompt, and what the agent is told once an answer is
//! recorded. The words are the offline trial's (scratchpad/lens/run_lens.py: its PANEL, the five
//! lenses' bodies, RULES and FINDINGS_OUT; the synthesis is its SYNTH, its output the cases the
//! person's card offers and the questions for the person in place of plan lines). Every variable
//! is read from a recorded fact: the person's words, their typed answers, the part's plan lines
//! (its intent, behavior, promise with its cases, criterion, planned tests, its fix location and a
//! standing place) and, for the synthesis, the five lenses' answers.
//! The lens panel by layer: decide/ the spawn and its reads · door/ the answers and their facts · teach/ the words.

use familiar_ai_facts::Fact;
use familiar_ai_rules::{cases, View};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The most cases the synthesis's answer keeps (the owner's ruling, 2026-10-25, as the finder's).
pub const MAX_CASES: usize = 8;
/// The most questions the synthesis asks the person (the owner's answer, 2026-10-26).
pub const MAX_QUESTIONS: usize = 3;

/// The note a found finding's why carries when the synthesis listed `listed` cases, more than
/// `MAX_CASES`, and the first `MAX_CASES` are kept (the owner's ruling, 2026-10-25, "Keep what
/// fits") [draft words]; the person's card shows it after the found checks.
pub fn left_out(listed: usize) -> String {
    format!("the lens panel listed {listed}, the first {MAX_CASES} are kept: {} left out", listed.saturating_sub(MAX_CASES))
}

/// The trial's opening of every lens's brief.
pub const PANEL: &str = "You are one advisory lens on a panel reviewing a DRAFT plan a coding agent made from a person's change request:
its parts, the promise each part makes with the test cases it will check, and where it plans to fix it. ";

/// The trial's rules for every lens.
pub const RULES: &str = "RULES:
- You NEVER answer a question about what the person wants — you surface what should be ASKED. Humans answer.
- Only surface findings whose answer would plausibly CHANGE the implementation or the tests. Quality over
  quantity; an empty findings list is a valid, good answer.
- You only read: Read, Grep and Glob on the repository in the current directory, at the commit the person is on.
  Gather your own context from the code; cite file:line for what you rely on.";

/// The trial's answer for every lens.
pub const FINDINGS_OUT: &str = r#"Output ONLY the JSON object, no prose, no markdown:
{"findings": [{"kind": "<your lens>", "note": "<what the draft guessed or missed and why it matters>",
  "target_symbol": "<qualified name or null>", "file_path": "<path or null>",
  "candidate_question": "<the question a human should be asked, or null>"}]}"#;

/// Each lens's body after `PANEL`, by its id (the trial's, its ids the approved names).
pub const BODIES: [(&str, &str); 5] = [
    (
        "ambiguity",
        "Your lens: AMBIGUITY — places where the request is underspecified and the draft silently guessed.

Hunt for:
- vague qualifiers in the request (\"gracefully\", \"should handle\", \"correctly\") that the draft resolved to a specific
  reading without evidence;
- unstated scope boundaries (which components/paths are in or out) the draft assumed;
- missing constraints (compatibility, ordering, error behavior) where two reasonable readings of the request diverge
  and the draft picked one.",
    ),
    (
        "completeness",
        "Your lens: COMPLETENESS — what the request implies that no draft case covers, and what the draft
added that the request never asked for.

Hunt for:
- behaviors or edge cases the request implies (error paths, empty inputs, concurrent use, every clause of the
  request) that no draft case makes checkable;
- affected surfaces visible in the codebase (callers, siblings, tests) the draft ignored;
- draft cases that OVERREACH the stated request (scope invented by the agent, not the person).",
    ),
    (
        "happy_path",
        "Your lens: the HAPPY PATH SEAM (founder-center's planning stage S2, which reads the code as
ENTRY -> CALLS / FLOWS -> functions: where the person's action enters the code and the path it takes).

Gather that context yourself: find where the person's action enters the code (the command, route, hook or public call
they use, as their words describe it), then follow the calls and the data from that entry to the code the draft plans
to change, and on to where the person's reported failure would actually surface.

Hunt for:
- the draft's planned place not on that path, or the failure surfacing at a step past it;
- a step on the path (a caller, a handler, an except clause, a wrapper) where the person's outcome is decided that no
  draft case reaches.",
    ),
    (
        "edge_cases",
        "Your lens: EDGE CASES (founder-center's planning stage S2-L, which reads the code as
function <- PROVES <- test, plus the function's inputs: the expected result and the bad input).

Gather that context yourself: read the existing tests that exercise the draft's planned place and nearby behavior, and
the place's inputs and parameters.

Hunt for:
- the expected result the draft's cases assert versus what the existing tests, the docs and the code's own contract
  say the result must be (the exact correct answer, not just \"no error\");
- inputs the person's situation includes that the draft's cases leave out (the exact shape of their input, other
  types, other call paths into the same place).",
    ),
    (
        "order_and_coupling",
        "Your lens: GAPS IN ORDER AND COUPLING (founder-center's planning stage S3, which reads the code as
function -SEQUENCES-> function, file -COCHANGES-> file, function -TOUCHES-> shared state).

Gather that context yourself: what runs before and after the draft's planned place (hooks, callbacks, registration
and plugin order, setup and teardown), what else reads or writes the same state, and which files change together
with it (git history is not available; use the code).

Hunt for:
- an order of execution under which the draft's planned fix would still fail (something that runs earlier, later, or
  in between);
- another reader or writer of the same state that would undo or bypass the fix;
- coupled code that must change with it.",
    ),
];

/// The question the person's typed answers answered, as the trial named it.
const TYPED_HEAD: &str = "THEIR ANSWER TO \"How do you make it happen? What do you run, and what do you see?\":";

/// The synthesis's opening (the trial's SYNTH, its output the cases and the questions) [draft words].
const SYNTHESIS_HEAD: &str = "You merge the findings of an advisory lens panel into the test cases a coding agent's change must also satisfy,
before any test or fix is written, plus at most 3 questions for the person.";

/// The synthesis's ask after the findings (the trial's SYNTH, adapted) [draft words].
pub const SYNTHESIS_ASK: &str = r#"Produce the cases a complete change must also satisfy, beyond the draft's own:
- Keep the draft where no finding changes it. Where a finding shows the draft guessed, narrowed or missed something
  the person's words imply, write a case: the situation, concretely, and what a correct change gives (normal: the
  behavior asked for; opposite: the same call with the trigger absent; edge: a boundary). These are the cases the
  person's card offers to add to the plan; list up to 8, the ones that matter most first, and leave out details the
  person's words do not imply.
- DEDUPLICATE: findings that describe the same unknown become one case or one question.
- Never answer a question about what the person wants: when an unknown would change the plan and the person's words
  do not settle it, ask it (text: one direct question a non-engineer can parse; options: 2 to 4 answers, not your
  recommendation; why: a short grounding note), and keep the draft's reading for that point. At most 3 questions.
- An empty question list is valid. If the draft's cases already cover it, list no case and say why.

You have no tools: answer from what is here.

Output ONLY the JSON object, no prose, no markdown:
{"cases": [{"short": "<one line, at most 70 characters, for the person>",
            "given": "<the situation, concretely>",
            "expect": "<what a correct change gives>",
            "why": "<the finding or the person's words that make it necessary>"}],
 "questions": [{"text": "...", "options": ["...", "..."], "why": "<short grounding note>"}],
 "none_because": "<when the case list is empty: why the draft's cases cover it>"}"#;

/// `value` as the trial printed it (Python's `json.dumps(..., indent=1)`): one space per level.
fn pretty(value: &Value) -> String {
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(b" "));
    match value.serialize(&mut ser) {
        Ok(()) => String::from_utf8(out).unwrap_or_default(),
        Err(_) => value.to_string(),
    }
}

/// A fact as a plan line: its kind first, then its fields in the form's order, as the plan file
/// writes it (a field the form does not name last, as held).
fn line(f: &Fact) -> Value {
    let mut m = Map::new();
    m.insert("kind".into(), json!(f.kind));
    for &k in familiar_ai_facts::form_row(f.kind).map(|row| row.fields).unwrap_or(&[]) {
        if let Some(v) = f.fields.get(k) {
            m.insert(k.to_string(), v.clone());
        }
    }
    for (k, v) in &f.fields {
        m.entry(k.clone()).or_insert_with(|| v.clone());
    }
    Value::Object(m)
}

/// Part `c`'s plan lines as the record holds them before the panel, in record order: its intent,
/// its behavior, its latest promise (with its cases), its criterion, a standing `place_chosen`
/// (plan::anchor_of), its planned tests and its latest fix location (lifecycle::fix_location).
pub fn draft(v: &View, c: &str) -> String {
    use familiar_ai_rules::{ask, lifecycle, plan};
    let crit = v.of("criterion", c).last();
    let bid = crit.and_then(|f| f.fields.get("behavior_id")).and_then(Value::as_str);
    let behavior = v.kind("behavior").filter(|b| b.fields.get("behavior_id").and_then(Value::as_str) == bid).last();
    let place = plan::anchor_of(v, c).filter(|a| a.kind == "place_chosen");
    let mut facts: Vec<&Fact> = [ask::intent_of(v, c), behavior, plan::promise_of(v, c), crit, place, lifecycle::fix_location(v, c)].into_iter().flatten().collect();
    facts.extend(v.of("test_planned", c));
    facts.sort_by_key(|f| f.t);
    facts.dedup_by_key(|f| f.t);
    pretty(&Value::Array(facts.into_iter().map(line).collect()))
}

/// The person's typed answers to how they make it happen (`trace::answers`), one per line, or
/// "(nothing typed)" as the trial wrote it.
fn typed(v: &View) -> String {
    let answers = familiar_ai_rules::trace::answers(v);
    if answers.is_empty() {
        "(nothing typed)".to_string()
    } else {
        answers.join("\n")
    }
}

/// The person's words, their typed answers and the draft, as every brief carries them.
fn read(v: &View, c: &str) -> String {
    format!(
        "THE PERSON ASKED:\n{}\n\n{TYPED_HEAD}\n{}\n\nTHE AGENT'S DRAFT PLAN (plan lines as it submitted them):\n{}",
        familiar_ai_rules::ask::words_of(v, c).unwrap_or(""),
        typed(v),
        draft(v, c)
    )
}

/// The brief of lens `lens` on part `c`: `PANEL` and its body, `RULES`, the person's words and
/// typed answers, the draft, then `FINDINGS_OUT`; `None` for a name that is not a lens.
pub fn lens_brief(v: &View, c: &str, lens: &str) -> Option<String> {
    let body = BODIES.iter().find(|(id, _)| *id == lens)?.1;
    Some(format!("{PANEL}{body}\n\n{RULES}\n\n{}\n\n{FINDINGS_OUT}", read(v, c)))
}

/// A lens's finding as the synthesis reads it, its empty fields null, as the trial's lenses wrote them.
fn finding_line(f: &Value) -> Value {
    let s = |k: &str| f.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map_or(Value::Null, |x| json!(x));
    json!({ "kind": s("kind"), "note": s("note"), "target_symbol": s("target_symbol"), "file_path": s("file_path"), "candidate_question": s("candidate_question") })
}

/// The five lenses' findings of the latest panel on `c`, by lens in release order (`cases::lens_findings`).
fn findings(v: &View, c: &str) -> String {
    let mut m = Map::new();
    for f in cases::lens_findings(v, c) {
        let Some(lens) = f.fields.get("lens").and_then(Value::as_str) else { continue };
        let fs: Vec<Value> = f.fields.get("findings").and_then(Value::as_array).map(|fs| fs.iter().map(finding_line).collect()).unwrap_or_default();
        m.insert(lens.to_string(), Value::Array(fs));
    }
    pretty(&Value::Object(m))
}

/// The synthesis's brief on part `c` (the trial's SYNTH, adapted) [draft words]: the person's
/// words and typed answers, the draft, the five lenses' findings, then the ask.
pub fn synthesis_brief(v: &View, c: &str) -> String {
    format!(
        "{SYNTHESIS_HEAD}\n\n{}\n\nTHE PANEL'S FINDINGS (five lenses: ambiguity, completeness, the happy path from the person's entry, edge cases, order and coupling):\n{}\n\n{SYNTHESIS_ASK}",
        read(v, c),
        findings(v, c)
    )
}

/// The brief of `lens` on `c`: a lens's, or the synthesis's.
pub fn brief(v: &View, c: &str, lens: &str) -> String {
    if lens == cases::SYNTHESIS {
        return synthesis_brief(v, c);
    }
    lens_brief(v, c, lens).unwrap_or_default()
}

/// A lens's id in words: "the happy path lens", "the synthesis".
pub fn named(lens: &str) -> String {
    if lens == cases::SYNTHESIS {
        return "the synthesis".to_string();
    }
    format!("the {} lens", lens.replace('_', " "))
}

/// Several lenses in words: "the ambiguity and completeness lenses".
fn listed(lenses: &[&str]) -> String {
    let words: Vec<String> = lenses.iter().map(|l| l.replace('_', " ")).collect();
    match words.split_last() {
        Some((last, [])) => format!("the {last} lens"),
        Some((last, rest)) => format!("the {} and {last} lenses", rest.join(", ")),
        None => String::new(),
    }
}

/// A count in words, to eight.
fn count(n: usize) -> String {
    const WORDS: [&str; 9] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight"];
    WORDS.get(n).map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// What comes next on `c`'s panel, as the agent is told it [draft words]: the lenses still to
/// start (one Agent call each, in parallel), those running, the synthesis once the five answered;
/// `None` once nothing is owed.
pub fn next(v: &View, c: &str) -> Option<String> {
    use crate::door::lens::{running, LENS};
    cases::owed_next(v, c)?;
    let owed = cases::lenses_owed(v, c);
    let (out, to): (Vec<&str>, Vec<&str>) = owed.into_iter().partition(|l| running(v, c, l));
    let mut said: Vec<String> = Vec::new();
    if !to.is_empty() {
        said.push(format!("{} still to start, one Agent call each, in parallel; each runs as {LENS} on familiar-ai's brief", listed(&to)));
    }
    if !out.is_empty() {
        said.push(format!("{} running; each answer is recorded when its call returns", listed(&out)));
    }
    if to.is_empty() && out.is_empty() {
        said.push(if cases::to_release(v, c) == Some(cases::SYNTHESIS) {
            format!("the five lenses answered; the synthesis is next, one Agent call; it runs as {LENS} on familiar-ai's brief, with the five answers and no tools")
        } else {
            "the synthesis is running; its answer is recorded when its call returns".to_string()
        });
    }
    Some(said.join("; "))
}

/// What the agent is told once an answer on `c` is recorded [draft words]: the lens's, and what
/// comes next on the panel; or the synthesis's, what it found and asks, then the open cases card
/// as the agent is told it (`picker::facts`).
pub fn after_answer(v: &View, c: &str) -> String {
    use crate::teach::words::{quoted, LEAD};
    use familiar_ai_rules::terminal;
    let latest = v.facts.iter().rev().find(|f| (f.kind == cases::LENS_FOUND || f.kind == cases::FOUND) && f.criterion_id() == Some(c));
    let outcome = latest.and_then(|f| f.fields.get("outcome")).and_then(Value::as_str).unwrap_or("");
    let q = quoted(v, c);
    if let Some(f) = latest.filter(|f| f.kind == cases::LENS_FOUND) {
        let lens = f.fields.get("lens").and_then(Value::as_str).unwrap_or("");
        let n = f.fields.get("findings").and_then(Value::as_array).map_or(0, Vec::len);
        let what = match outcome {
            "found" if n == 1 => "it found one thing worth asking about".to_string(),
            "found" => format!("it found {} things worth asking about", count(n)),
            "none" => "it found nothing".to_string(),
            _ => "it could not finish looking".to_string(),
        };
        let head = format!("{LEAD}{} answered for {q}: {what}.", named(lens));
        return match next(v, c) {
            Some(n) => format!("{head} The lens panel: {n}."),
            None => head,
        };
    }
    let n = latest.map(cases::found_ids).unwrap_or_default().len();
    let what = match outcome {
        "found" => format!("it found {} more checks worth adding", count(n)),
        "none" => "it found nothing more worth checking".to_string(),
        _ => "it could not finish".to_string(),
    };
    let asks = match cases::questions(v, c).len() {
        0 => String::new(),
        1 => " and asks the person one question".to_string(),
        k => format!(" and asks the person {} questions", count(k)),
    };
    let head = format!("{LEAD}the lens panel answered for {q}: {what}{asks}.");
    match terminal::cards(v).into_iter().find(|k| k.criterion_id == c && k.hold == cases::CARD) {
        Some(card) => format!("{head}\n{}", crate::teach::picker::facts(v, &card)),
        None => head,
    }
}
