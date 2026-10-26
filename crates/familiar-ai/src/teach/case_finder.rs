//! The case-finder's words (the owner's plan of 2026-10-25, the cases card) [draft names, draft words]:
//! the brief the binary hands the subagent in place of the main agent's prompt, and what the agent is
//! told once its answer is recorded. Lane B writes the words; lane A calls these two functions only.

use familiar_ai_rules::View;

/// The most cases the finder lists (the owner's ruling, 2026-10-25).
pub const MAX_CASES: usize = 8;

/// The note a found finding's why carries when the finder listed `listed` cases, more than
/// `MAX_CASES`, and the first `MAX_CASES` are kept (the owner's ruling, 2026-10-25, "Keep what
/// fits") [draft words]; the person's card shows it after the found checks.
pub fn left_out(listed: usize) -> String {
    format!("the finder listed {listed}, the first {MAX_CASES} are kept: {} left out", listed.saturating_sub(MAX_CASES))
}

/// The case-finder's brief for part `c`, every variable from a recorded fact (plan §2) [draft]:
/// the person's words (`ask::words_of`), their typed answers on cards (`trace::answers`, the line
/// left out when there are none), the part's text, the promise's accepts / gives back / refuses and
/// its cases one line each, the plan's place when the record has one (`place`), the intent's sha
/// and `MAX_CASES`. Nothing else goes in. The ask is the push back's steps in order (the owner's
/// plan of 2026-10-25, the case-finder pushes back) [draft words]: the person's situation traced,
/// every clause covered, the case that defeats a fix at the plan's place (only when one is
/// recorded), then the variants.
pub fn brief(v: &View, c: &str) -> String {
    use familiar_ai_rules::{ask, plan, trace};
    let field = |f: Option<&familiar_ai_facts::Fact>, k: &str| f.and_then(|f| f.fields.get(k)).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
    let promise = plan::promise_of(v, c);
    let typed: String = trace::answers(v).iter().map(|a| format!("{a}\n")).collect();
    let cases: Vec<String> = promise.map(plan::cases_in).unwrap_or_default().iter().map(|k| format!("{} ({}): given {} → expect {}", k.case_id, k.kind, k.given, k.expect)).collect();
    let place = place(v, c);
    let at = place.as_ref().map(|p| format!("Where the agent plans to fix it:\n{p}\n\n")).unwrap_or_default();
    let (or_there, defeat, last) = match place {
        Some(_) => (", or not where the agent plans to fix it", "3. Name the case that would defeat a fix where the agent plans it: a situation that fix would still get wrong.\n", 4),
        None => ("", "", 3),
    };
    format!(
        "You find the test cases a change must satisfy, before any test or fix is written. You only read: you may not edit,
write or run anything.

The person asked:
{person}
{typed}
The agent split that into this part:
{part}

What it promises: given {accepts}, it gives back {gives_back}; it refuses {refuses}.

The cases the agent already plans to test:
{cases}

{at}The code as it stands is in the current directory (the commit {sha}). Work in this order:
1. Trace the person's own situation, as they describe it above, through the code to where it really fails. If that
   is not what the part promises{or_there}, the first case is the person's situation
   where it really fails, and its why says what the plan narrowed.
2. Check that each clause of the person's words is covered by the part or its cases. A clause left out is a case,
   and its why names the clause.
{defeat}{last}. Then the variants of what the person described that the code makes reachable (other options, styles, types,
   call paths), and behaviour that must stay as it is.

List up to {MAX_CASES} more cases a complete change must also satisfy, in the order of the steps, only ones the planned
cases do not already cover. Leave out details the person's words do not imply (exact wording of a message, an exact
value nobody asked for). If the planned cases already cover it, say so and list none.

Answer with JSON only:
{{\"cases\": [{{\"short\": \"<one line, at most 70 characters, for the person>\",
            \"given\": \"<the situation, concretely>\",
            \"expect\": \"<what a correct change gives>\",
            \"why\": \"<what in the person's words or the code makes it necessary>\"}}],
 \"none_because\": \"<when the list is empty: why the planned cases cover it>\"}}",
        person = ask::words_of(v, c).unwrap_or(""),
        part = crate::teach::words::text_of(v, c).unwrap_or(""),
        accepts = field(promise, "accepts"),
        gives_back = field(promise, "gives_back"),
        refuses = field(promise, "refuses"),
        cases = cases.join("\n"),
        sha = field(ask::intent_of(v, c), "sha"),
    )
}

/// The plan's place for part `c` as the record has it [draft]: the latest `fix_location_category`'s
/// where (`lifecycle::fix_location`), then a standing `place_chosen`'s places (`plan::anchor_of`,
/// as `words::anchor_words` writes them), one line each; None when neither is recorded.
fn place(v: &View, c: &str) -> Option<String> {
    use familiar_ai_rules::{lifecycle, plan};
    let at = lifecycle::fix_location(v, c).and_then(|f| f.fields.get("where")).and_then(serde_json::Value::as_str).map(str::trim).map(String::from);
    let chosen = plan::anchor_of(v, c).filter(|a| a.kind == "place_chosen").map(|_| crate::teach::words::anchor_words(v, c));
    let lines: Vec<String> = at.into_iter().chain(chosen).filter(|l| !l.is_empty()).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// What the agent is told once the finder's answer on `c` is recorded [draft words]: what it
/// found, then the open cases card as the agent is told it (`picker::facts`).
pub fn after_finding(v: &View, c: &str) -> String {
    use familiar_ai_rules::{cases, terminal};
    let last = cases::findings(v, c).last().copied();
    let outcome = last.and_then(|f| f.fields.get("outcome")).and_then(serde_json::Value::as_str).unwrap_or("");
    const WORDS: [&str; 9] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight"];
    let n = last.map(cases::found_ids).unwrap_or_default().len();
    let what = match outcome {
        "found" => format!("it found {} more checks worth adding", WORDS.get(n).map_or_else(|| n.to_string(), |w| (*w).to_string())),
        "none" => "it found nothing more worth checking".to_string(),
        _ => "it could not finish looking".to_string(),
    };
    let head = format!("{}the case-finder answered for {}: {what}.", crate::teach::words::LEAD, crate::teach::words::quoted(v, c));
    match terminal::cards(v).into_iter().find(|k| k.criterion_id == c && k.hold == cases::CARD) {
        Some(card) => format!("{head}\n{}", crate::teach::picker::facts(v, &card)),
        None => head,
    }
}

