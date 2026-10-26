//! The judge's words: its prompt, the lines the person and the agent are told,
//! and how a verdict changes the state shown. This module only formats;
//! `door::judge` asks the judge and writes the facts.

use familiar_ai_facts::{AnswersIntent, CaseTest, Fact, Log};
use familiar_ai_rules::criterion::{sealed_at, test_files, Place};
use familiar_ai_rules::{plan, View};
use serde_json::Value;

use crate::teach::words::LEAD;

/// The line when the judge gave no readable answer.
pub fn unanswered(how: &str) -> String {
    format!("{LEAD}the judge did not answer ({how}); the sealed test stands, not judged; its call is logged in .familiar/judge.log")
}

/// The line when the judge says it cannot see the tests; a cut prompt never
/// counts as a no.
pub fn unseen(model: &str, reason: &str) -> String {
    let model = crate::teach::words::model(model);
    format!("{LEAD}the judge ({model}) could not see the sealed test ({}); it stands, not judged; its call is logged in .familiar/judge.log", reason.trim().trim_end_matches('.'))
}

/// The cap on the agent's reading and the anchor together; the person's words
/// and each test file have their own caps.
pub const MOST_INPUTS: usize = 2_500;
/// The cap per sealed test file. A smaller cap once hid the new test deep in
/// a long file.
pub const MOST_PER_TEST: usize = 8_000;
/// How much of the intent and criterion a status line shows.
const MOST_ON_A_LINE: usize = 120;

/// What the judge is shown, read by `door::judge` from the record and git.
pub struct Inputs {
    pub criterion_id: String,
    /// The person's words, one entry per prompt.
    pub intent: Vec<String>,
    /// The agent's claim of the intent, when it differs from the person's words.
    pub reading: Option<String>,
    pub criterion: String,
    /// One line per anchor place with its lines as checked, or the new work's spot.
    pub anchor: String,
    /// Each planned test file with its test functions at the seal.
    pub tests: Vec<(String, String)>,
    /// What an investigator found before this test was written, when it found (S7b).
    pub finding: Option<String>,
    /// Each gap an earlier "no" for this criterion named, once, oldest first (the owner's plan of
    /// 2026-10-20, step 2): a later seal is read against what was asked for.
    pub asked_before: Vec<String>,
    /// The promise's contract, when it gives cases (the owner's plan of 2026-10-21, step 1) [draft
    /// names]: the judge names, for each case, the test function that exercises it.
    pub contract: Option<Contract>,
    /// The precedents found at the seal, best first (ports 2026-10-24, piece 4) [draft names].
    pub precedents: Vec<crate::teach::precedents::Precedent>,
}

/// A promise's contract as the judge is shown it [draft names].
#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    pub accepts: String,
    pub gives_back: String,
    pub refuses: String,
    pub cases: Vec<familiar_ai_rules::plan::Case>,
    /// Why the promise has no opposite case, when it says so.
    pub opposite_none: Option<String>,
    /// The listed places the agent judged not affected, each with its reason and code [draft names]
    /// (the owner's plan of 2026-10-21, step 3).
    pub places: Vec<NotAffected>,
}

/// A listed place the agent judged not affected, as the judge is shown it [draft names].
#[derive(Debug, Clone, PartialEq)]
pub struct NotAffected {
    pub file: String,
    pub line: u64,
    /// The line defining what it is in; empty when none.
    pub def: String,
    pub reason: String,
    /// Its code at the tests-first commit, numbered; filled by the door.
    pub code: String,
}

/// The contract of the promise `c` names, when it gives cases or a listed place was classified:
/// the promise's cases, a `place` case for each listed place that applies, and the places judged
/// not affected (their code left for the door to fill).
pub fn contract_of(v: &View, c: &str) -> Option<Contract> {
    let p = plan::promise_of(v, c)?;
    let cases = plan::cases_of(v, c);
    let places: Vec<NotAffected> = familiar_ai_rules::places::not_affected(v, c)
        .into_iter()
        .map(|(s, reason)| NotAffected { file: s.file, line: s.line, def: s.def, reason, code: String::new() })
        .collect();
    if cases.is_empty() && places.is_empty() {
        return None;
    }
    let slot = |k: &str| text(p, k).unwrap_or("").to_string();
    Some(Contract { accepts: slot("accepts"), gives_back: slot("gives_back"), refuses: slot("refuses"), cases, opposite_none: text(p, "opposite_none").filter(|r| !r.trim().is_empty()).map(str::to_string), places })
}

/// The head of the section that shows the places judged not affected.
pub const PLACES_HEAD: &str = "PLACES THE AGENT JUDGED NOT AFFECTED (sites the binary listed before the fix that name what the planned place names; for each, the agent's reason the behavior THE CONTRACT promises does not reach it, and its code at the tests-first commit)";

/// The places section: each place as `file:line, def, reason: …`, then its code.
pub fn places_section(places: &[NotAffected]) -> String {
    let mut out = format!("{PLACES_HEAD}\n");
    for p in places {
        let def = if p.def.is_empty() { "outside any definition" } else { p.def.as_str() };
        out.push_str(&format!("{}:{}, {def}, reason: {}\n", p.file, p.line, p.reason));
        if !p.code.is_empty() {
            out.push_str(&p.code);
            if !p.code.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// The fourth check, asked only when a place was judged not affected.
pub const PLACES_CHECK: &str = "4. For each place under PLACES THE AGENT JUDGED NOT AFFECTED, read its code shown: ok is true only when the agent's reason is sound \
given that code, so the behavior THE CONTRACT promises does not reach that place. A place not ok makes answers_intent false and gap names it as file:line.\n";

/// A place in the words a gap names it with.
fn place_words(p: &NotAffected) -> String {
    let def = if p.def.is_empty() { String::new() } else { format!(" ({})", p.def) };
    format!("place {}:{}{def}", p.file, p.line)
}

/// The judge's word on the places weighed: a place it found the reason unsound for, or left out,
/// makes the verdict a "no" whose gap names it, beside any gap already there. No places shown, or
/// a judge that cannot see, keeps the word as it is.
pub fn weigh_places(contract: Option<&Contract>, said: Option<AnswersIntent>, gap: &str, places: &[familiar_ai_facts::PlaceOk]) -> (Option<AnswersIntent>, String) {
    let (Some(k), Some(_)) = (contract, said) else {
        return (said, gap.to_string());
    };
    let rejected: Vec<String> = k
        .places
        .iter()
        .filter_map(|p| match places.iter().find(|x| x.file == p.file && x.line == p.line) {
            Some(x) if x.ok => None,
            Some(_) => Some(format!("{}: the reason it is not affected does not hold for its code", place_words(p))),
            None => Some(format!("{}: its reason was not read", place_words(p))),
        })
        .collect();
    if rejected.is_empty() {
        return (said, gap.to_string());
    }
    let m = rejected.join("; ");
    if gap.trim().is_empty() {
        (Some(AnswersIntent::No), m)
    } else {
        (Some(AnswersIntent::No), format!("{}; {m}", gap.trim().trim_end_matches('.')))
    }
}

/// The prompt's section that shows the contract of criterion `c`.
pub fn contract_section(c: &str, k: &Contract) -> String {
    let mut out = format!(
        "THE CONTRACT (the agent's promise for criterion {c}: the cases its tests are to exercise; the agent's reading, not the person's words)\naccepts: {} · gives back: {} · refuses: {}\n",
        k.accepts, k.gives_back, k.refuses
    );
    for case in &k.cases {
        out.push_str(&format!("case {} ({}): given {} → expect {}\n", case.case_id, case.kind, case.given, case.expect));
    }
    if let Some(r) = &k.opposite_none {
        out.push_str(&format!("no opposite case: {r}\n"));
    }
    if !k.places.is_empty() {
        out.push_str(&places_section(&k.places));
    }
    out
}

/// A contract case in the words a gap names it with.
fn case_words(case: &familiar_ai_rules::plan::Case) -> String {
    format!("case {} (given {} → expect {})", case.case_id, case.given, case.expect)
}

/// Whether the judge's name for a case's test says none.
pub fn names_none(test: &str) -> bool {
    let t = test.trim().trim_end_matches('.');
    t.is_empty() || t.eq_ignore_ascii_case("none")
}

/// The judge's word weighed against the contract: a case the answer names no test for, or whose
/// test does not check it, makes the verdict a "no" whose gap names the case, beside any gap of
/// the judge's own. An answer with no cases, or a criterion with no contract, keeps the judge's
/// word; a judge that cannot see the tests (`None`) stays unjudged.
pub fn weigh(contract: Option<&Contract>, said: Option<AnswersIntent>, gap: &str, cases: &[CaseTest]) -> (Option<AnswersIntent>, String) {
    let (Some(k), Some(_)) = (contract, said) else {
        return (said, gap.to_string());
    };
    if cases.is_empty() {
        return (said, gap.to_string());
    }
    let missing = k.cases.iter().find_map(|case| match cases.iter().find(|t| t.case_id == case.case_id) {
        Some(t) if names_none(&t.test) => Some(format!("{}: no test exercises it", case_words(case))),
        Some(t) if !t.ok => Some(format!("{}: {} does not check it", case_words(case), t.test.trim())),
        Some(_) => None,
        None => Some(format!("{}: no test exercises it", case_words(case))),
    });
    match missing {
        None => (said, gap.to_string()),
        Some(m) if gap.trim().is_empty() => (Some(AnswersIntent::No), m),
        Some(m) => (Some(AnswersIntent::No), format!("{}; {m}", gap.trim().trim_end_matches('.'))),
    }
}

/// The cap per earlier gap.
pub const MOST_PER_GAP: usize = 300;

/// The line that asks again for what an earlier "no" named.
pub fn asked_again(gap: &str) -> String {
    format!("An earlier verdict asked for: {}. Is that still exercised? If not, answers_intent is false and gap names it.", cut(gap.trim().trim_end_matches('.'), MOST_PER_GAP))
}

/// `s` cut to at most `n` characters, saying so when it was cut.
pub fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let kept: String = s.chars().take(n).collect();
    format!("{kept}\n… (cut at {n} characters)")
}

/// A sealed test file as `sealed_test::shown` gives it, cut for the prompt: what is above
/// `THE_REST` (the import header and the test functions the seal changed) is kept whole, and the
/// cap, `MOST_PER_TEST`, applies to the rest; with no such line, to the whole of it.
pub fn cut_test(shown: &str) -> String {
    let Some(at) = shown.find(crate::teach::sealed_test::THE_REST) else {
        return cut(shown, MOST_PER_TEST);
    };
    let (changed, rest) = shown.split_at(at);
    let room = MOST_PER_TEST.saturating_sub(changed.chars().count()).max(crate::teach::sealed_test::THE_REST.chars().count());
    format!("{changed}{}", cut(rest, room))
}

/// The agent's reading and the anchor, before `prompt` cuts them at `MOST_INPUTS`.
pub fn reading(i: &Inputs) -> String {
    let mut reading = String::from("THE AGENT'S READING (what the agent wrote down; not the person's words)\n");
    if let Some(r) = &i.reading {
        reading.push_str(&format!("the intent, as the agent put it: {r}\n"));
    }
    reading.push_str(&format!("criterion {}: {}\n\nTHE ANCHOR (the code the agent chose as where the change belongs)\n{}", i.criterion_id, i.criterion, i.anchor));
    reading
}

/// The judge's prompt, each part cut to its cap.
pub fn prompt(i: &Inputs) -> String {
    let most = crate::teach::person::MOST_WORDS;
    let mut out = String::from(
        "A coding agent was given the intent below. It wrote the criterion below, chose the anchor below, \
         and sealed the test files below: they fail on the code as it is now, and the code is to be changed \
         until they pass, without changing the tests.\n\n",
    );
    out.push_str("THE INTENT (what the person asked for, in their own words)\n");
    let said: Vec<String> = i.intent.iter().map(|w| cut(w, most)).collect();
    out.push_str(&said.join("\n\nLater, the person also said:\n"));
    out.push_str("\n\n");
    out.push_str(&cut(&reading(i), MOST_INPUTS));
    if let Some(f) = &i.finding {
        out.push_str(&format!("\n\nAN INVESTIGATOR FOUND, before this test was written: {f}"));
    }
    for (path, shown) in &i.tests {
        out.push_str(&format!("\n\nTHE SEALED TEST {path} {}", cut_test(shown)));
    }
    if let Some(k) = &i.contract {
        out.push_str("\n\n");
        out.push_str(&contract_section(&i.criterion_id, k));
    }
    if !i.asked_before.is_empty() {
        out.push_str("\n\nWHAT AN EARLIER VERDICT ASKED FOR\n");
        out.push_str(&i.asked_before.iter().map(|g| asked_again(g)).collect::<Vec<_>>().join("\n"));
    }
    let precedents = crate::teach::precedents::section(&i.precedents); // [draft names]: under their own cap; nothing found, the prompt as it was
    if !precedents.is_empty() { out.push_str(&format!("\n\n{precedents}")); }
    let (contract, places) = i.contract.as_ref().map_or((false, false), |k| (true, !k.places.is_empty()));
    out.push_str(&question_text(&i.criterion_id, contract, places, !precedents.is_empty()));
    out
}

/// The question and the answer's form, asked only about the part of the
/// person's words this criterion claims; see docs/decisions.md#the-judge-sees-one-part
pub fn question(criterion_id: &str) -> String {
    question_text(criterion_id, false, false, false)
}

/// The question when places were judged not affected: a fourth check reads each reason, and the
/// answer's form carries the places [draft names].
pub fn question_with_places(criterion_id: &str) -> String {
    question_text(criterion_id, true, true, false)
}

/// The question when the criterion's promise gives cases: a third check names the test of each
/// case, and the answer's form carries them [draft names].
pub fn question_with_contract(criterion_id: &str) -> String {
    question_text(criterion_id, true, false, false)
}

/// The third check, asked only with a contract.
pub const CONTRACT_CHECK: &str = "3. For each case of THE CONTRACT, name the test function that makes the case's call and checks its expected result, or say none; \
ok is true only when that test checks exactly that result. A case with none, or not ok, makes answers_intent false and gap names the case. \
The contract may miss a path of the person's words; the question above still holds.\n";

fn question_text(criterion_id: &str, contract: bool, places: bool, precedents: bool) -> String {
    let (check, checks) = match (contract, places) {
        (true, true) => (format!("{CONTRACT_CHECK}{PLACES_CHECK}"), 4),
        (true, false) => (CONTRACT_CHECK.to_string(), 3),
        _ => (String::new(), 2),
    };
    let (check, checks) = if precedents { (format!("{check}{}", crate::teach::precedents::check(checks + 1)), checks + 1) } else { (check, checks) }; // the precedents' check [draft words], numbered after the others
    let cases = match (contract, places) {
        (true, true) => ", \"cases\": [{\"case_id\": \"<each case's id>\", \"test\": \"<the test function that exercises it, or none>\", \"ok\": true or false}], \"places\": [{\"file\": \"<its path>\", \"line\": <its line>, \"ok\": true or false}]",
        (true, false) => ", \"cases\": [{\"case_id\": \"<each case's id>\", \"test\": \"<the test function that exercises it, or none>\", \"ok\": true or false}]",
        _ => "",
    };
    let unseen_cases = match (contract, places) {
        (true, true) => ", \"cases\": [], \"places\": []",
        (true, false) => ", \"cases\": []",
        _ => "",
    };
    format!(
        "\n\nCriterion {c} is the part of the person's words this test answers; its sibling criteria answer the rest.\n\n\
The question: list every path the person's words cover that criterion {c} claims (nested cases, other entry points, \
each input class) and the observable result they expect; for each, name the test function that exercises it, \
or say none. If any has none, answers_intent is false and gap names it. \
A path of the person's words that belongs to a sibling criterion is not a gap.\n\n\
{more} more:\n\
1. Name each noun and each named form in the person's words that criterion {c} claims (a type, an annotation, a function, a mode, \
an input class); for each, quote the test line that exercises exactly that form, or say none. A narrower or \
different form is none. If any has none, answers_intent is false and gap names it.\n\
2. Does at least one assertion run the changed behaviour and check its observable result, rather than \
inspecting source text, an AST, a type alias or union, or the existence of a helper the agent invented? \
If no assertion does, answers_intent is false and gap says so.\n{check}\n\
The answer is exactly one JSON object and nothing else (no code fence, no other text):\n\
{{\"answers_intent\": true or false, \"gap\": \"<the path no test exercises, or empty>\", \
\"reason\": \"<each path, the result expected, and the test function that exercises it, or none>\"{cases}}}\n\
If you cannot see the tests, the answer is:\n\
{{\"answers_intent\": false, \"gap\": \"\", \"reason\": \"the test is not visible\"{unseen_cases}}}\n",
        c = criterion_id,
        more = ["Two checks", "Three checks", "Four checks", "Five checks"][checks - 2],
    )
}

/// The verdict as the person reads it, with the gap on a "no".
pub fn verdict(model: &str, answer: AnswersIntent, gap: &str) -> String {
    let model = crate::teach::words::model(model);
    match answer {
        AnswersIntent::Yes => format!("{LEAD}the judge ({model}) read your request against the test: it covers it"),
        AnswersIntent::No => format!(
            "{LEAD}the judge ({model}) read your request against the test: it does not cover {}; the test may be changed",
            gap.trim().trim_end_matches('.')
        ),
    }
}

/// What the agent is told after the verdict, with the reason on a no.
pub fn told(model: &str, answer: AnswersIntent, gap: &str, reason: &str) -> String {
    let line = verdict(model, answer, gap);
    match answer {
        AnswersIntent::Yes => line,
        AnswersIntent::No => format!(
            "{line}\n  the judge's reason: {}; the next commit of tests alone is sealed again",
            reason.trim().trim_end_matches('.')
        ),
    }
}

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

fn one_line(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MOST_ON_A_LINE {
        return flat;
    }
    let kept: String = flat.chars().take(MOST_ON_A_LINE).collect();
    format!("{kept}…")
}

/// The latest verdict written after `c`'s seal.
pub fn verdict_after<'a>(v: &View<'a>, c: &str, seal: &Fact) -> Option<&'a Fact> {
    v.of("judge_verdict", c).filter(|f| f.t > seal.t).last()
}

/// What the judge has said of `c`'s latest red seal, from the facts alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// No `judge_verdict` after the seal.
    Nothing,
    /// The seal stands.
    Covers,
    /// The test may be changed.
    DoesNotCover { model: String, gap: String },
    /// No verdict after `TWICE` misses; a verdict written later still counts.
    NotJudged { why: String },
}

/// Misses for one seal that make it "not judged".
pub const TWICE: usize = 2;

/// The seal's commit, which a `judge_unanswered` names.
pub fn seal_of(seal: &Fact) -> Option<&str> {
    text(seal, "commit")
}

/// The `judge_unanswered` facts for `c`'s seal.
pub fn unanswered_for<'a>(v: &View<'a>, c: &str, seal: &Fact) -> Vec<&'a Fact> {
    let commit = seal_of(seal);
    v.of("judge_unanswered", c).filter(|f| commit.is_some() && text(f, "seal") == commit).collect()
}

/// What the judge has said of `c`'s latest seal; `None` when `c` has no seal.
pub fn said(v: &View, c: &str) -> Option<Said> {
    let seal = sealed_at(v, c, Place::Laptop)?;
    Some(match verdict_after(v, c, seal) {
        None => match unanswered_for(v, c, seal).as_slice() {
            asked if asked.len() >= TWICE => Said::NotJudged { why: asked.last().and_then(|f| text(f, "why")).unwrap_or("").to_string() },
            _ => Said::Nothing,
        },
        Some(f) if text(f, "answers_intent") == Some("no") => Said::DoesNotCover {
            model: crate::teach::words::model(text(f, "model").unwrap_or("")).to_string(),
            gap: text(f, "gap").unwrap_or("").trim().trim_end_matches('.').to_string(),
        },
        Some(_) => Said::Covers,
    })
}

/// The state shown after a "yes".
pub const COVERED: &str = "sealed (red, covered)";
pub const WAITING: &str = "waiting on the judge";
/// The way out after a "no".
pub const CHANGE_THE_TEST: &str = "change the test, commit it alone, it must fail first";

/// The state shown after a "no", green or not.
pub fn found(model: &str, gap: &str) -> String {
    format!("the judge ({model}) found the test does not cover {gap}; the test may be changed")
}

/// The words for `c` in the rules' state `st` when the judge changes them;
/// `None` keeps the rules' words. With `judge_on`, an unjudged seal waits.
pub fn shown(v: &View, c: &str, st: &str, judge_on: bool) -> Option<String> {
    if !matches!(st, "sealed" | "green" | "proven") {
        return None;
    }
    // the reviewer's verified no on the fix (the owner's plan of 2026-10-20, step 3), after the judge's own
    if st != "sealed" && !not_covered(v, c) {
        if let Some(no) = familiar_ai_rules::review::standing_no(v, c) {
            return Some(crate::teach::review::found(no));
        }
    }
    // the contract, where the runner reads each test [draft names]: a sealed part whose case's
    // test did not run and pass is told so, unless the judge's no already says what changes
    if st == "sealed" && !not_covered(v, c) {
        if let Some(w) = familiar_ai_rules::criterion::contract_unmet(v, c) {
            return Some(not_met(&w));
        }
    }
    match said(v, c)? {
        Said::DoesNotCover { model, gap } => Some(found(&model, &gap)),
        Said::Covers if st == "sealed" => Some(format!("{COVERED} · the code comes next")),
        // Green with no verdict also waits, since green needs a covered seal.
        Said::Nothing if judge_on => Some(WAITING.to_string()),
        _ => None,
    }
}

/// The state shown while the contract keeps green away [draft words].
pub fn not_met(why: &str) -> String {
    format!("not green yet: {}; each case's test runs and passes before green", why.trim().trim_end_matches('.'))
}

/// True when a no stands against `c`'s latest seal.
pub fn not_covered(v: &View, c: &str) -> bool {
    matches!(said(v, c), Some(Said::DoesNotCover { .. }))
}

/// The words when the gate refuses a code edit or commit.
pub fn gate(v: &View, c: &str, said: &Said) -> String {
    let name = crate::teach::words::quoted(v, c);
    match said {
        Said::DoesNotCover { gap, .. } => format!("{LEAD}{name} · the judge found it does not cover {gap} · next: {CHANGE_THE_TEST}"),
        _ => format!("{LEAD}{name} · the judge reads the sealed test against your request first · next: wait for the judge"),
    }
}

/// The stop refusal when a no stands against a criterion the rules call green.
pub fn not_done(v: &View, c: &str, model: &str, gap: &str) -> String {
    format!("{LEAD}not done yet · {} · {}\n  next: {CHANGE_THE_TEST}", crate::teach::words::quoted(v, c), found(model, gap))
}

/// The judge line when no judge is asked here, so the gate is off.
pub const OFF: &str = "off: the person chose no judge, so the seal stands on its red run alone";

/// What the judge said of `c`'s seal at `seal`: its verdict, or that it could not judge;
/// `None` when it said nothing.
pub fn verdict_line(v: &View, c: &str, seal: &Fact) -> Option<String> {
    match (verdict_after(v, c, seal), said(v, c)) {
        (None, Some(Said::NotJudged { why })) => Some(format!("not judged ({why}, twice)")),
        (Some(f), _) => {
            let said = match text(f, "answers_intent") {
                Some("yes") => "it covers the request",
                Some("no") => "it does not cover the request",
                _ => "not judged yet",
            };
            Some(format!("{said} ({})", crate::teach::words::model(text(f, "model").unwrap_or(""))))
        }
        (None, _) => None,
    }
}

/// The judge's word on `c`'s seal at `seal`; with none yet, that it is owed, or that no judge is asked here.
pub fn word_on(v: &View, c: &str, seal: &Fact, judge_on: bool) -> String {
    verdict_line(v, c, seal).unwrap_or_else(|| if judge_on { "not judged yet".to_string() } else { OFF.to_string() })
}

/// The lines under a criterion that say what the person asked for, on one line, and the
/// agent's reading of it when that differs.
pub fn asked_for(log: &Log) -> Vec<String> {
    let words = crate::teach::person::words(log);
    let intent = words.first().map(String::as_str).unwrap_or("");
    let mut out = vec![format!("  asked for: {}", one_line(intent))];
    out.extend(crate::teach::person::agents_reading(log).map(|r| format!("  the agent's reading: {}", one_line(r))));
    // a clarifying question on a signal [draft words]: the reading the person picked
    out.extend(familiar_ai_rules::ask::answers(&familiar_ai_rules::View::new(log)).iter().map(|a| format!("  the reading the person picked: {}", one_line(a))));
    // the one question [draft words]: how the person makes it happen, in their words
    out.extend(familiar_ai_rules::trace::answers(&familiar_ai_rules::View::new(log)).iter().map(|a| format!("  how the person makes it happen: {}", one_line(a))));
    out
}

/// For every sealed criterion: what was asked for, what tests it, and the verdict.
pub fn intent_next_to_test(log: &Log, judge_on: bool) -> Vec<String> {
    let v = View::new(log);
    let mut out = Vec::new();
    for c in plan::criteria(&v) {
        let Some(seal) = sealed_at(&v, &c, Place::Laptop) else {
            continue;
        };
        let judge = word_on(&v, &c, seal, judge_on);
        out.push(format!("{LEAD}{}", crate::teach::words::quoted(&v, &c)));
        out.extend(asked_for(log));
        out.push(format!("  tested by: {}", tested_by(&v, &c)));
        out.push(format!("  judge: {judge}"));
    }
    out
}

/// A part's planned test files, each named once in the order first planned.
pub fn tested_by(v: &View, c: &str) -> String {
    let mut files: Vec<String> = Vec::new();
    for f in test_files(v, c) {
        if !files.contains(&f) {
            files.push(f);
        }
    }
    files.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inputs_are_cut_and_the_tests_each_on_their_own() {
        let i = Inputs {
            criterion_id: "c1".into(),
            intent: vec!["x".repeat(3_000)],
            reading: None,
            criterion: "z".repeat(3_000),
            anchor: "src/app.sh · greet · lines 1–3".into(),
            tests: vec![("tests/a.sh".into(), "y".repeat(9_000))],
            finding: None,
            asked_before: Vec::new(),
            contract: None, precedents: Vec::new(),
        };
        let p = prompt(&i);
        assert!(p.contains("(cut at 2000 characters)") && p.contains("(cut at 2500 characters)") && p.contains("(cut at 8000 characters)"), "{p}");
        assert!(!p.contains(&"x".repeat(2_001)) && !p.contains(&"z".repeat(2_500)) && !p.contains(&"y".repeat(8_001)));
        assert!(p.contains("THE SEALED TEST tests/a.sh ") && p.ends_with(&question("c1")));
    }

    #[test]
    fn the_cut_leaves_the_changed_test_functions_whole_and_applies_to_the_rest() {
        let big = |end: &str| format!("import os\n\n\ndef test_big():\n{}    assert {end}\n\n\ndef test_other():\n    assert UNCHANGED\n", "    x = 1  # pad\n".repeat(600));
        let (before, after) = (big("os.sep"), big("os.name"));
        let shown = crate::teach::sealed_test::shown("tests/test_a.py", Some(before.as_bytes()), after.as_bytes());
        let i = Inputs { criterion_id: "c1".into(), intent: vec!["x".into()], reading: None, criterion: "z".into(), anchor: "a".into(), finding: None, asked_before: Vec::new(), contract: None, precedents: Vec::new(), tests: vec![("tests/test_a.py".into(), shown)] };
        let p = prompt(&i);
        assert!(p.contains("    assert os.name\n"), "the changed function is whole: {}", &p[p.len().saturating_sub(600)..]);
        assert!(!p.contains("UNCHANGED") && p.contains("… (cut at "), "the rest is cut");
        // a file with nothing changed in its blocks is still cut at the same number
        let i = Inputs { tests: vec![("tests/a.sh".into(), "y".repeat(9_000))], ..i };
        assert!(prompt(&i).contains("(cut at 8000 characters)"));
    }

    #[test]
    fn the_prompt_asks_for_each_named_form_and_a_run_of_the_behaviour() {
        let i = Inputs {
            criterion_id: "c1".into(),
            intent: vec!["Make greet say hello instead of hi.".into()],
            reading: Some("greet with hello".into()),
            criterion: "greet says hello".into(),
            anchor: "src/app.sh · greet · lines 1–3".into(),
            tests: vec![("tests/test_c1.sh".into(), "grep -q hello src/app.sh\n".into())],
            finding: None,
            asked_before: Vec::new(),
            contract: None, precedents: Vec::new(),
        };
        let p = prompt(&i);
        assert!(p.contains("1. Name each noun and each named form in the person's words that criterion c1 claims (a type, an annotation, a function, a mode, an input class); for each, quote the test line that exercises exactly that form, or say none. A narrower or different form is none."), "{p}");
        assert!(p.contains("2. Does at least one assertion run the changed behaviour and check its observable result, rather than inspecting source text, an AST, a type alias or union, or the existence of a helper the agent invented? If no assertion does, answers_intent is false and gap says so."), "{p}");
        assert!(p.contains("{\"answers_intent\": true or false, \"gap\": \"<the path no test exercises, or empty>\", \"reason\": \"<each path, the result expected, and the test function that exercises it, or none>\"}"), "{p}");
        assert!(p.ends_with("If you cannot see the tests, the answer is:\n{\"answers_intent\": false, \"gap\": \"\", \"reason\": \"the test is not visible\"}\n"), "{p}");
    }

    #[test]
    fn the_told_lines_are_the_pinned_words() {
        assert_eq!(told("sonnet", AnswersIntent::Yes, "", "r"), "familiar-ai: the judge (sonnet) read your request against the test: it covers it");
        assert_eq!(
            verdict("sonnet", AnswersIntent::No, "the error raised for an empty name."),
            "familiar-ai: the judge (sonnet) read your request against the test: it does not cover the error raised for an empty name; the test may be changed"
        );
        assert_eq!(
            told("sonnet", AnswersIntent::No, "the error raised for an empty name.", "it checks a different error"),
            "familiar-ai: the judge (sonnet) read your request against the test: it does not cover the error raised for an empty name; the test may be changed\n  the judge's reason: it checks a different error; the next commit of tests alone is sealed again"
        );
        assert_eq!(unanswered("exit 1"), "familiar-ai: the judge did not answer (exit 1); the sealed test stands, not judged; its call is logged in .familiar/judge.log");
    }

    fn contract() -> Contract {
        let case = |id: &str, kind: &str, given: &str, expect: &str| familiar_ai_rules::plan::Case { case_id: id.into(), kind: kind.into(), given: given.into(), expect: expect.into() };
        Contract {
            accepts: "a name".into(),
            gives_back: "a greeting".into(),
            refuses: "an empty name".into(),
            cases: vec![case("n1", "normal", "the name Ada", "hello Ada"), case("o1", "opposite", "an empty name", "refused, no greeting")],
            opposite_none: None,
            places: Vec::new(),
        }
    }

    fn inputs(contract: Option<Contract>) -> Inputs {
        Inputs {
            criterion_id: "c1".into(),
            intent: vec!["Make greet say hello.".into()],
            reading: None,
            criterion: "greet says hello".into(),
            anchor: "src/app.sh · greet · lines 1–3".into(),
            tests: vec![("tests/test_greet.py".into(), "def test_hello():\n    assert greet('Ada') == 'hello Ada'\n".into())],
            finding: None,
            asked_before: Vec::new(),
            contract, precedents: Vec::new(),
        }
    }

    // the contract [draft names]: the judge is shown the promise's cases and names, for each, the test
    // function that exercises it, or none; it still derives its own paths from the person's words
    #[test]
    fn the_contract_is_shown_and_each_case_is_asked_for() {
        let p = prompt(&inputs(Some(contract())));
        assert!(
            p.contains("\n\nTHE CONTRACT (the agent's promise for criterion c1: the cases its tests are to exercise; the agent's reading, not the person's words)\naccepts: a name · gives back: a greeting · refuses: an empty name\ncase n1 (normal): given the name Ada → expect hello Ada\ncase o1 (opposite): given an empty name → expect refused, no greeting\n"),
            "{p}"
        );
        assert!(p.find("THE SEALED TEST").unwrap_or(usize::MAX) < p.find("THE CONTRACT").unwrap_or(0), "the contract after the tests: {p}");
        assert!(p.contains("The question: list every path the person's words cover that criterion c1 claims"), "its own derivation stays: {p}");
        assert!(p.contains("3. For each case of THE CONTRACT, name the test function that makes the case's call and checks its expected result, or say none; ok is true only when that test checks exactly that result. A case with none, or not ok, makes answers_intent false and gap names the case. The contract may miss a path of the person's words; the question above still holds."), "{p}");
        assert!(p.contains("\"cases\": [{\"case_id\": \"<each case's id>\", \"test\": \"<the test function that exercises it, or none>\", \"ok\": true or false}]}"), "{p}");
        let none = Contract { cases: vec![contract().cases[0].clone()], opposite_none: Some("the trigger cannot be absent".into()), ..contract() };
        assert!(prompt(&inputs(Some(none))).contains("case n1 (normal): given the name Ada → expect hello Ada\nno opposite case: the trigger cannot be absent\n"));
        // without a contract the prompt is as it was
        let p = prompt(&inputs(None));
        assert!(!p.contains("THE CONTRACT") && !p.contains("\"cases\"") && p.ends_with(&question("c1")), "{p}");
    }

    #[test]
    fn a_case_with_no_test_or_not_ok_is_a_no_that_names_the_case() {
        let k = contract();
        let t = |id: &str, test: &str, ok: bool| familiar_ai_facts::CaseTest { case_id: id.into(), test: test.into(), ok };
        let yes = Some(AnswersIntent::Yes);
        assert_eq!(weigh(Some(&k), yes, "", &[t("n1", "test_hello", true), t("o1", "test_empty", true)]), (yes, String::new()), "every case has its test");
        assert_eq!(
            weigh(Some(&k), yes, "", &[t("n1", "test_hello", true), t("o1", "none", false)]),
            (Some(AnswersIntent::No), "case o1 (given an empty name → expect refused, no greeting): no test exercises it".to_string())
        );
        assert_eq!(
            weigh(Some(&k), yes, "", &[t("n1", "test_hello", true)]),
            (Some(AnswersIntent::No), "case o1 (given an empty name → expect refused, no greeting): no test exercises it".to_string()),
            "a case the answer left out has none"
        );
        assert_eq!(
            weigh(Some(&k), yes, "", &[t("n1", "test_hello", false), t("o1", "test_empty", true)]),
            (Some(AnswersIntent::No), "case n1 (given the name Ada → expect hello Ada): test_hello does not check it".to_string())
        );
        // the judge's own no keeps its gap, the case's beside it
        assert_eq!(
            weigh(Some(&k), Some(AnswersIntent::No), "the nested form", &[t("n1", "test_hello", true), t("o1", "", true)]),
            (Some(AnswersIntent::No), "the nested form; case o1 (given an empty name → expect refused, no greeting): no test exercises it".to_string())
        );
        // an answer with no cases, or no contract: the judge's word stands alone
        assert_eq!(weigh(None, yes, "", &[]), (yes, String::new()));
        assert_eq!(weigh(Some(&k), yes, "", &[]), (yes, String::new()), "an old answer with no cases stands");
        assert_eq!(weigh(Some(&k), None, "", &[t("o1", "none", false)]), (None, String::new()), "a judge that cannot see stays unjudged");
    }
}
