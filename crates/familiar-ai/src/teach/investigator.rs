//! What an investigator is told at its start, and the lines shown when it
//! returns. A spawned subagent gets `context` with the discipline; one the
//! binary launches gets `launch_brief` on stdin, and the diagnosis of a test that never
//! went red `diagnosis_brief`. Every line is
//! a fact; the only instruction is what finishes the work.
//! The investigator by layer: decide/ the spawn and the wait · door/ the facts, the run, the brief · teach/ the words.

use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::{criterion, ladder, lifecycle, plan, View};
use serde_json::Value;

use crate::teach::discipline::discipline;
use crate::teach::words::{anchor_words, model, quoted, standing, LEAD};

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

/// The SubagentStart context for `conv` when it is a criterion's open
/// investigator.
pub fn context(log: &Log, conv: &str) -> Option<String> {
    let v = View::new(log);
    let (c, inv) = plan::criteria(&v).into_iter().find_map(|c| {
        let inv = ladder::investigator_of(&v, &c)?;
        (text(inv, "to_conversation") == Some(conv)).then_some((c, inv))
    })?;
    let from = text(inv, "from_conversation").unwrap_or("?");
    let back = ladder::chain_of(&v, &c).first().and_then(|f| text(f, "from_conversation")).unwrap_or(from).to_string();
    let rung = ladder::rung_of(&v, Some(conv)).unwrap_or("unknown");
    let tests = criterion::test_files(&v, &c).join(", ");
    let command = lifecycle::command_at(&v, &c, i64::MAX).unwrap_or("none recorded");
    let q = quoted(&v, &c);
    let lines = [
        format!("{LEAD}this subagent ({conv}, {rung}) is the stronger model taking {q} over from {from}, where two tries failed."),
        format!("  its anchor: {}", anchor_words(&v, &c)),
        format!("  its tests: {tests}; they are sealed: familiar-ai runs {command} after each commit; not passed = failed"),
        format!("  where it stands: {}; this subagent's own two tries start now", standing(&v, &c)),
        format!(
            "  where its fix lives, as claimed before: {}; this subagent's first act, before any commit, is its own claim: familiar-ai claim fix_location_category --criterion_id {c} --category this_repository --where '' when the fix is made here; else --category dependency, environment or request, with --where naming where it lives, and the person is asked",
            lifecycle::fix_location(&v, &c).and_then(|f| text(f, "category")).unwrap_or("none claimed")
        ),
        format!("  when its tests pass (the code on its anchor, the test unchanged) {q} goes back to {back}, which continues; this subagent's stop waits until then"),
        "  two more failed tries here hold it again: a stronger model still, or the person's card".to_string(),
    ];
    Some(format!("{}\n\n{}", lines.join("\n"), discipline()))
}

/// What a binary-launched investigator is shown, as `door::investigator_brief` reads it
/// from the record, git and the kept runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Brief {
    pub criterion_id: String,
    /// Each of the person's prompts, in order.
    pub said: Vec<String>,
    pub criterion: String,
    /// The judge's verdict on the seal in force: covers, or the gap it found.
    pub judge: Option<(bool, String)>,
    /// The anchor's places, each followed by its lines as they are now.
    pub place: String,
    /// Each planned test file with its test functions at the seal.
    pub tests: Vec<(String, String)>,
    pub command: String,
    pub planned: Vec<String>,
    /// The category of the latest `fix_location_category`.
    pub category: String,
    /// The failed tries since the seal shown, oldest first.
    pub tries: Vec<Try>,
}

/// One failed try since the seal, with each changed file's lines added and
/// removed and its kept run's output.
#[derive(Debug, Clone, PartialEq)]
pub struct Try {
    pub n: usize,
    pub commit: String,
    pub files: Vec<(String, u64, u64)>,
    pub output: Option<String>,
}

/// How many of a try's last output lines are shown.
pub const OUTPUT_LINES: usize = 40;

/// The brief a binary-launched investigator gets on stdin after two failed
/// tries to go green, with no discipline.
pub fn launch_brief(b: &Brief) -> String {
    use crate::teach::judge::cut_test;
    let judge = match &b.judge {
        Some((true, _)) => "covers it".to_string(),
        Some((false, gap)) => format!("does not cover: {}", gap.trim().trim_end_matches('.')),
        None => "not judged".to_string(),
    };
    let mut out = vec![
        format!("{LEAD}you are taking over one piece of work that a cheaper model could not finish."),
        person_asked(&b.said),
        format!("THE CRITERION (the cheaper model's reading): {}", b.criterion.trim()),
        format!("THE JUDGE said of its sealed test: {judge}"),
        format!("THE PLACE IN THE CODE: {}", b.place),
        first_act(b),
    ];
    out.extend(b.tests.iter().map(|(path, shown)| format!("THE SEALED TEST: {path} {}", cut_test(shown.trim_end()))));
    out.push(format!("THE TEST COMMAND: {}; familiar-ai runs it after each commit; not passed = failed", b.command));
    out.push(tried(&b.tries));
    out.push(format!(
        "WHAT FINISHES IT: the sealed test passes with the code changed at that place; the code is committed; \
         familiar-ai runs the test and records it. If the sealed test does not answer what the person asked, it may be \
         changed: familiar-ai claim test_change_announced --criterion_id {} --reason '<why>', then the new test is \
         committed alone and must fail first, then the code.",
        b.criterion_id
    ));
    out.join("\n\n")
}

/// What the binary-run diagnosis of a test that never went red is shown [draft words] (S7b), as
/// `door::diagnosis` reads it from the record, git and the kept runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosing {
    pub criterion_id: String,
    /// Each of the person's prompts with its turn, in order.
    pub said: Vec<(u64, String)>,
    pub criterion: String,
    /// The promise's accepts, gives back and refuses, on the full path.
    pub promise: Option<[String; 3]>,
    /// The planning commit and the place's lines there, numbered.
    pub planning: (String, String),
    /// HEAD and the place's lines there, numbered.
    pub head: (String, String),
    /// The tests-first commit.
    pub tests_commit: String,
    /// Each planned test file at the tests-first commit, numbered.
    pub tests: Vec<(String, String)>,
    pub command: String,
    /// The two runs, oldest first: a line each and the end of its kept output.
    pub runs: Vec<(String, Option<String>)>,
}

/// The decision tree the diagnosis follows [draft words].
pub const DECIDE: &str = "DECIDE exactly one category, in this order:\n\
  1. wrong_test: the test does not check what the person asked, so it passes on code that does not do it (a test that passes for the wrong reason is wrong_test too). Give guidance: what the test should check instead.\n\
  2. behavior_exists: the code as planned already does what the person asked, so no honest test of it can fail there.\n\
  3. requirement_missed: the part, as the agent wrote it, leaves out something the person asked for, so its test checks only what already works. Give missed: what was left out, and scope: what the part should cover.";

/// How the proof is quoted.
pub const PROOF: &str = "PROOF: quote what shows it. A quote of the test or the code gives role test or code, file_path, line_start, line_end, the commit (one named above) and the exact text of those lines; a quote of the person gives role person, said (the turn shown before their words) and their exact words. familiar-ai reads every quote again where you say it is; a quote not found there voids the whole finding.";

/// The brief of the read-only diagnosis after two tries to go red [draft words] (S7b). It
/// reads and answers; it changes nothing.
pub fn diagnosis_brief(b: &Diagnosing) -> String {
    let said: Vec<String> = b.said.iter().map(|(t, w)| format!("[turn {t}] {}", crate::teach::judge::cut(w, crate::teach::person::MOST_WORDS))).collect();
    let mut out = vec![
        format!("{LEAD}you are reading one piece of work to say why its test never went red: twice its tests passed before any fix. You read only; you change nothing."),
        format!("THE PERSON ASKED (their words, each after its turn):\n{}", if said.is_empty() { "(nothing recorded)".to_string() } else { said.join("\n") }),
        format!("THE PART (the agent's reading): {}", b.criterion.trim()),
    ];
    if let Some([accepts, gives_back, refuses]) = &b.promise {
        out.push(format!("ITS PROMISE: accepts {accepts}; gives back {gives_back}; refuses {refuses}"));
    }
    out.push(format!("THE PLACE IN THE CODE at planning (commit {}): {}", b.planning.0, b.planning.1));
    out.push(format!("THE PLACE NOW (commit {}): {}", b.head.0, b.head.1));
    for (path, shown) in &b.tests {
        out.push(format!("THE TEST at its tests-first commit ({}): {path}\n{shown}", b.tests_commit));
    }
    out.push(format!("THE TEST COMMAND: {}", b.command));
    let mut runs = vec!["THE TWO RUNS (oldest first):".to_string()];
    for (line, output) in &b.runs {
        runs.push(format!("  {line}"));
        if let Some(o) = output.as_deref().filter(|o| !o.trim().is_empty()) {
            let lines: Vec<&str> = o.trim_end().lines().collect();
            let from = lines.len().saturating_sub(OUTPUT_LINES);
            runs.extend(lines.iter().skip(from).map(|l| format!("      {l}")));
        }
    }
    out.push(runs.join("\n"));
    out.push(DECIDE.to_string());
    out.push(PROOF.to_string());
    out.push("ANSWER with the one JSON object the schema asks: category, reason (one sentence), quotes, and the category's own field.".to_string());
    out.join("\n\n")
}

/// The line the person and the agent read after a diagnosis [draft words].
pub fn diagnosed(log: &Log, c: &str, alias: &str, came: &crate::door::diagnosis::Came) -> String {
    use crate::door::diagnosis::Came;
    let q = quoted(&View::new(log), c);
    let head = format!("{LEAD}a stronger model ({alias}) read why the tests of {q} never went red");
    match came {
        Came::Found { category } => format!("{head}: {}; the question for the person quotes its proof", crate::teach::words::found_words(category)),
        Came::Missed { why, how } => format!("{head} and gave no finding ({why}: {how}); the question for the person goes without one"),
        Came::Nothing => head,
    }
}

/// The person's prompts as every brief shows them, each cut to `MOST_WORDS`.
fn person_asked(said: &[String]) -> String {
    let said: Vec<String> = said.iter().map(|w| crate::teach::judge::cut(w, crate::teach::person::MOST_WORDS)).collect();
    format!("THE PERSON ASKED (their words):\n{}", said.join("\n\nLater, the person also said:\n"))
}

/// The SubagentStart context for any other subagent while a run is open: the
/// person's words for the run, and the part `conv` holds when it holds one.
pub fn subagent_context(log: &Log, conv: Option<&str>) -> Option<String> {
    use crate::teach::person;
    if person::mode(log) != person::PROVING {
        return None;
    }
    let said = person::words_and_picks(log);
    if said.is_empty() {
        return None;
    }
    let v = View::new(log);
    let mut out = vec![format!("{LEAD}this subagent works for the person, in a run that is open."), person_asked(&said)];
    if let Some(c) = conv.and_then(|conv| plan::criteria(&v).into_iter().find(|c| plan::assigned_to(&v, c) == Some(conv))) {
        out.push(format!("THE PART THIS SUBAGENT HOLDS: {}", quoted(&v, &c)));
    }
    Some(out.join("\n\n"))
}

/// The line saying the investigator's first act is its own claim of where the
/// fix lives, as `lifecycle::first_act` requires.
fn first_act(b: &Brief) -> String {
    format!(
        "WHERE THE FIX LIVES (the cheaper model's claim): {}. YOUR FIRST ACT, before any commit, is your own claim: \
         familiar-ai claim fix_location_category --criterion_id {} --category this_repository --where '' when the fix is made \
         in this repository; when it cannot be: --category dependency, environment or request, --where names where it lives, \
         --needs what it needs; then the person is asked, and no test and no code are written.",
        b.category, b.criterion_id
    )
}

/// Each try's commit, its changes as `git diff --stat` shows them, and the
/// end of its kept run.
fn tried(tries: &[Try]) -> String {
    let mut out = vec!["WHAT WAS TRIED (the failed tries since the seal, oldest first, at most the last two):".to_string()];
    if tries.is_empty() {
        out.push("  (no failed try is recorded since the seal)".to_string());
    }
    for t in tries {
        let short = t.commit.get(..7).unwrap_or(&t.commit);
        out.push(format!("  try {} ({short}): {}", t.n, summary(&t.files)));
        let wide = t.files.iter().map(|(p, _, _)| p.chars().count()).max().unwrap_or(0);
        out.extend(t.files.iter().map(|(p, a, r)| format!("     {p:<wide$} | {} {}", a + r, bars(*a, *r))));
        match t.output.as_deref() {
            None => out.push("    the test's output: (output not kept)".to_string()),
            Some(o) if o.trim().is_empty() => out.push("    the test's output: (it printed nothing)".to_string()),
            Some(o) => {
                out.push("    the test's output:".to_string());
                let lines: Vec<&str> = o.trim_end().lines().collect();
                let from = lines.len().saturating_sub(OUTPUT_LINES);
                out.extend(lines.iter().skip(from).map(|l| format!("      {l}")));
            }
        }
    }
    out.join("\n")
}

/// The summary line of `git diff --stat`.
fn summary(files: &[(String, u64, u64)]) -> String {
    if files.is_empty() {
        return "(its changes could not be read)".to_string();
    }
    let (a, r) = files.iter().fold((0, 0), |(a, r), (_, x, y)| (a + x, r + y));
    let n = |k: u64, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    let mut parts = vec![n(files.len() as u64, "file changed", "files changed")];
    if a > 0 || r == 0 {
        parts.push(n(a, "insertion(+)", "insertions(+)"));
    }
    if r > 0 || a == 0 {
        parts.push(n(r, "deletion(-)", "deletions(-)"));
    }
    parts.join(", ")
}

/// One file's `+`/`-` bar, scaled to at most 40 marks as git does.
fn bars(a: u64, r: u64) -> String {
    const MOST: u64 = 40;
    let (a, r) = match a + r {
        0 => (0, 0),
        all if all <= MOST => (a, r),
        all => {
            let scaled = |k: u64| if k == 0 { 0 } else { (k * MOST / all).max(1) };
            (scaled(a), scaled(r))
        }
    };
    format!("{}{}", "+".repeat(a as usize), "-".repeat(r as usize))
}

pub fn taking_over(log: &Log, c: &str, rung: &str) -> String {
    format!("{LEAD}a stronger model ({}) is taking {} over", model(rung), quoted(&View::new(log), c))
}

/// The line the holder's stop carries after a binary-launched investigator
/// returns, for the person and the agent alike.
pub fn returned(log: &Log, r: &crate::door::investigator_run::Returned) -> String {
    let v = View::new(log);
    let c = r.criterion_id.as_str();
    let q = quoted(&v, c);
    if sent_red(&v, c, &r.conversation) {
        return format!("{LEAD}two tries to go red failed; a stronger model ({}) {}", model(&r.model), red_end(&v, c, r));
    }
    if matches!(lifecycle::state(&v, c), "green" | "proven") {
        return format!("{LEAD}a stronger model ({}) proved {q}", model(&r.model));
    }
    let cost = r.cost_usd.map(|x| format!(", ${x:.2}")).unwrap_or_default();
    let stderr = if r.stderr.is_empty() { String::new() } else { format!("; its stderr began: {}", r.stderr) };
    format!(
        "{LEAD}a stronger model ({}, {}) did not prove {q} ({}, {} s{cost}): {}; {}{stderr}",
        model(&r.model),
        r.conversation,
        r.how,
        r.wall_secs,
        standing(&v, c),
        what_next(&v, c)
    )
}

/// True when `conv` was sent for `c`'s `two_tries_not_red` hold.
pub(crate) fn sent_red(v: &View, c: &str, conv: &str) -> bool {
    v.of("criterion_transfer", c).any(|f| {
        f.fields.get("to_conversation").and_then(serde_json::Value::as_str) == Some(conv) && ladder::investigated_for(v, f) == Some("two_tries_not_red")
    })
}

/// How a red-phase investigator ended: sealed and handed back, or not.
fn red_end(v: &View, c: &str, r: &crate::door::investigator_run::Returned) -> String {
    let q = quoted(v, c);
    let back = plan::assigned_to(v, c).filter(|h| *h != r.conversation);
    match (criterion::sealed_at(v, c, criterion::Place::Laptop), back) {
        (Some(_), Some(h)) => format!("sealed a test of {q} that fails on the code as it is; {q} is back with {h}, which makes the fix"),
        _ => format!("did not seal a test of {q} that fails on the code as it is: {}", standing(v, c)),
    }
}

/// The holder's PostToolUse line after a red-phase investigator returns, for
/// the agent and the person; the holder's turn goes on.
pub fn red_returned(log: &Log, r: &crate::door::investigator_run::Returned) -> String {
    use crate::teach::judge::{found, said, Said, CHANGE_THE_TEST};
    let v = View::new(log);
    let c = r.criterion_id.as_str();
    let q = quoted(&v, c);
    let head = format!("{LEAD}two tries to go red failed; a stronger model ({})", model(&r.model));
    let holder = plan::assigned_to(&v, c);
    let back = holder.filter(|h| *h != r.conversation);
    let (Some(_), Some(h)) = (criterion::sealed_at(&v, c, criterion::Place::Laptop), back) else {
        let with = holder.map(|h| format!("; {q} is with {h}")).unwrap_or_default();
        return format!("{head} did not seal a test of {q} that fails on the code as it is: {}{with}", standing(&v, c));
    };
    let (judge, next) = match said(&v, c) {
        Some(Said::Covers) => ("the judge covers it".to_string(), "which writes the code now".to_string()),
        Some(Said::DoesNotCover { model, gap }) => (found(&model, &gap), format!("next: {CHANGE_THE_TEST}")),
        Some(Said::NotJudged { why }) => (format!("not judged ({why}, twice)"), "which writes the code now".to_string()),
        _ => ("the judge is still to read it".to_string(), "which writes the code now".to_string()),
    };
    format!("{head} sealed a test of {q} that fails on the code as it is; {judge}; {q} is back with {h}, {next}")
}

/// What comes after an investigator that did not prove `c`.
fn what_next(v: &View, c: &str) -> String {
    let hs = lifecycle::holds(v, c);
    let held = ladder::SENT_FOR.iter().copied().find(|w| hs.iter().any(|h| h.why == *w));
    match (crate::door::investigator::rung(v, c), held) {
        (Some(rung), Some(_)) => format!("the next stop hands it to a stronger model still ({rung}), or the person answers its card"),
        // With no rung left, the hold goes to the person's card.
        (None, Some(why)) => crate::teach::words::hold(v, Some(c), why),
        _ => "it did not end on two failed tries, so it stays with that model".to_string(),
    }
}

/// The stop reason of the holder's PostToolUse that launched the
/// investigator, after it returns.
pub fn escalated(log: &Log, r: &crate::door::investigator_run::Returned) -> String {
    let v = View::new(log);
    let c = r.criterion_id.as_str();
    let head = format!("{LEAD}escalated to the investigator — a stronger model ({})", model(&r.model));
    if sent_red(&v, c, &r.conversation) {
        return format!("{LEAD}two tries to go red failed; escalated to the investigator — a stronger model ({}) {}", model(&r.model), red_end(&v, c, r));
    }
    if matches!(lifecycle::state(&v, c), "green" | "proven") {
        return format!("{head} proved {}", quoted(&v, c));
    }
    format!("{head} did not prove {}: {}; {}", quoted(&v, c), standing(&v, c), what_next(&v, c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief() -> Brief {
        Brief {
            criterion_id: "c1".into(),
            said: vec!["greet the person with hello".into(), "and nothing else".into()],
            criterion: "greet says hello".into(),
            judge: Some((false, "an empty name.".into())),
            place: "src/app.sh · greet · lines 1–3, and those lines as they are now:\n#!/bin/sh\ngreet() { echo howdy; }\ngreet".into(),
            tests: vec![("tests/test_c1.sh".into(), "(new in the seal commit)\nsh src/app.sh | grep -qx hello\n".into())],
            command: "sh tests/test_c1.sh".into(),
            planned: vec!["tests/test_c1.sh".into()],
            category: "this_repository".into(),
            tries: vec![
                Try { n: 1, commit: "0123456789abcdef".into(), files: vec![("src/app.sh".into(), 1, 1)], output: None },
                Try {
                    n: 2,
                    commit: "fedcba9876543210".into(),
                    files: vec![("src/app.sh".into(), 1, 1), ("src/lib/more.sh".into(), 3, 0)],
                    output: Some((1..=45).map(|n| format!("line {n}\n")).collect()),
                },
            ],
        }
    }

    #[test]
    fn the_brief_is_the_facts_of_the_work_and_what_finishes_it() {
        let text = launch_brief(&brief());
        let output: String = (6..=45).map(|n| format!("\n      line {n}")).collect();
        assert_eq!(
            text,
            format!(
                "familiar-ai: you are taking over one piece of work that a cheaper model could not finish.\n\n\
                 THE PERSON ASKED (their words):\ngreet the person with hello\n\nLater, the person also said:\nand nothing else\n\n\
                 THE CRITERION (the cheaper model's reading): greet says hello\n\n\
                 THE JUDGE said of its sealed test: does not cover: an empty name\n\n\
                 THE PLACE IN THE CODE: src/app.sh · greet · lines 1–3, and those lines as they are now:\n#!/bin/sh\ngreet() {{ echo howdy; }}\ngreet\n\n\
                 WHERE THE FIX LIVES (the cheaper model's claim): this_repository. YOUR FIRST ACT, before any commit, is your own \
                 claim: familiar-ai claim fix_location_category --criterion_id c1 --category this_repository --where '' when the fix \
                 is made in this repository; when it cannot be: --category dependency, environment or request, --where names where \
                 it lives, --needs what it needs; then the person is asked, and no test and no code are written.\n\n\
                 THE SEALED TEST: tests/test_c1.sh (new in the seal commit)\nsh src/app.sh | grep -qx hello\n\n\
                 THE TEST COMMAND: sh tests/test_c1.sh; familiar-ai runs it after each commit; not passed = failed\n\n\
                 WHAT WAS TRIED (the failed tries since the seal, oldest first, at most the last two):\n\
                 \x20 try 1 (0123456): 1 file changed, 1 insertion(+), 1 deletion(-)\n\
                 \x20    src/app.sh | 2 +-\n\
                 \x20   the test's output: (output not kept)\n\
                 \x20 try 2 (fedcba9): 2 files changed, 4 insertions(+), 1 deletion(-)\n\
                 \x20    src/app.sh      | 2 +-\n\
                 \x20    src/lib/more.sh | 3 +++\n\
                 \x20   the test's output:{output}\n\n\
                 WHAT FINISHES IT: the sealed test passes with the code changed at that place; the code is committed; \
                 familiar-ai runs the test and records it. If the sealed test does not answer what the person asked, it may be \
                 changed: familiar-ai claim test_change_announced --criterion_id c1 --reason '<why>', then the new test is \
                 committed alone and must fail first, then the code."
            )
        );
        assert!(!text.contains("subagent") && !text.contains(crate::teach::discipline::discipline()), "no discipline, no subagent");
    }

    #[test]
    fn the_words_are_cut_the_judge_and_the_tries_have_their_plain_absences() {
        let mut b = brief();
        b.said = vec!["x".repeat(2_500)];
        b.tests = vec![("tests/a.sh".into(), "y".repeat(9_000))];
        b.judge = None;
        b.tries = vec![Try { n: 3, commit: "abc".into(), files: vec![], output: Some(" \n".into()) }];
        let text = launch_brief(&b);
        assert!(text.contains("(cut at 2000 characters)") && text.contains("(cut at 8000 characters)") && !text.contains(&"y".repeat(8_001)), "{text}");
        assert!(text.contains("THE JUDGE said of its sealed test: not judged\n"), "{text}");
        assert!(text.contains("  try 3 (abc): (its changes could not be read)\n    the test's output: (it printed nothing)\n"), "{text}");
        b.judge = Some((true, String::new()));
        b.tries = vec![];
        let text = launch_brief(&b);
        assert!(text.contains("THE JUDGE said of its sealed test: covers it\n"), "{text}");
        assert!(text.contains("at most the last two):\n  (no failed try is recorded since the seal)\n\nWHAT FINISHES IT"), "{text}");
        assert_eq!((bars(100, 0), bars(30, 90).len(), bars(0, 0)), ("+".repeat(40), 40, String::new()));
        assert_eq!(summary(&[("a".into(), 0, 0)]), "1 file changed, 0 insertions(+), 0 deletions(-)");
        assert_eq!(summary(&[("a".into(), 0, 2)]), "1 file changed, 2 deletions(-)");
    }
}
