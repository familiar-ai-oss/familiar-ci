//! The terminal's text, ported from the reference model (not in this
//! repository): every line is a read of the log, and the terminal writes
//! nothing itself. The one fact a person may write through it is
//! `hold_answered`, and a card names exactly what that fact would carry.

use serde_json::{json, Value};

use crate::criterion::{answers_of, Clear, ANSWERS, CRIT_ORDER};
use crate::hooks::{decide, Clears, Decision, Event, Refusal};
use crate::lifecycle::{state, whys};
use crate::moves::criterion_admissible;
use crate::plan;
use crate::reader::{text, View};

/// A hold's why in plain words; an unknown why prints its own name.
fn why_words(w: &str) -> String {
    match w {
        "planning_incomplete" => "the plan is not complete yet".into(),
        "anchor_not_found" => "the code the plan points at was not found at the lines it names".into(),
        "no_test_stack" => "no test command is known for this part of the code".into(),
        "tests_not_red" => "its tests passed at the tests-first commit; they must fail before the code is written; edit the test so it fails on the code as it is, then commit it alone again".into(),
        "not_stamped" => "code on its anchor was committed before its tests were committed, seen failing and stamped".into(),
        "tests_and_code_mixed" => "its tests and its code are in one commit; the tests must go first, alone".into(),
        "proof_tampered" => "its tests changed after they were sealed, and the changed tests have not been seen failing".into(),
        "anchor_untouched" => "its tests pass, but no commit changed the code at its anchor".into(),
        "regression" => "a test that passed before this change fails now".into(),
        "anchor_moved" => "the code at its anchor changed after the plan was made".into(),
        "two_failures" => "two attempts in a row ended with its tests failing".into(),
        "two_tries_not_red" => "two tries to go red failed; this needs the person".into(),
        "no_teeth" => "its tests still pass with the change taken back out, so they do not test the change".into(),
        "command_changed" => "the test command changed between the tests-first commit and now".into(),
        "released" => "released: it no longer blocks anything".into(),
        "red_for_another_reason" => "its test did not fail for the reason it tests: it is broken, too slow, or its command never ran it; fix the test or the command, then commit it alone again".into(),
        "two_test_broken" => "two attempts in a row ended with its test broken".into(),
        "two_crashed" => "two attempts in a row ended with its test crashing".into(),
        "two_timed_out" => "two attempts in a row ended with its test too slow".into(),
        "environment_error" => "its tests could not run here: the environment is missing something".into(),
        "run_unclassified" => "its test run ended in a way no rule reads".into(),
        "retry_spent" => "its test run was killed or its runner crashed, twice".into(),
        "crashed" => "its test crashed before any fix".into(),
        "command_broke" => "the test command worked before and fails now".into(),
        "miscoped" => "its tests passed once the dependency was fixed".into(),
        // the adapter (plan A-4, A-5; lane a5) [draft words]
        "reading_gap" => "some outcomes of the test runs here cannot be read; those runs will ask".into(),
        "setup_changed" => "the test setup changed after its tests were sealed, and under it its sealed tests no longer fail for the reason they test".into(),
        // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
        "review_disputed" => "a reviewer read the fix twice and found it does not do what was asked; this needs the person".into(),
        // an existing test's lines change only when announced (the owner, 2026-10-20) [draft words]
        "existing_test_changed" => "a test that was in the project before this plan changed; its old expectation changes only when the person's words call for it, said first".into(),
        // places before the fix (the owner's plan of 2026-10-21, step 3) [draft words]
        "places_unclassified" => "the binary listed places in the code that name what this part's place names; before its tests-first commit the agent classifies each: applies (the behavior asked for reaches it, and a test of its own shows it there) or not_affected, with a one-line reason".into(),
        // a clarifying question on a signal (the owner's plan of 2026-10-21, step 2) [draft words]
        "reading_unclear" => "the person's words for this part can be read more than one way; before its tests-first commit the agent says how it reads them (one reading, and why) or lists the readings it sees, and the person picks one".into(),
        // test first, then pick the spot (the owner's plan of 2026-10-23) [draft words]
        "try_owed" => "its tests run under pytest, so its place in the code is picked from what its failing test runs: write the test, then run familiar-ai try; the place (plan line 4) comes after a red try".into(),
        "try_not_red" => "its latest try passed, so the test does not show the problem yet: change the test so it fails on the code as it is, then run familiar-ai try again; the place (plan line 4) comes after a red try".into(),
        "repro_unknown" => "the person's words do not show how to make the problem happen (no command, code or quoted output); before the test is written the person is asked how they make it happen".into(),
        // the cases card (the owner's plan of 2026-10-25) [draft words]
        // the lens panel (the owner's plan of 2026-10-26, approved) [draft words]
        "cases_owed" => "its contract is whole, so before its test is written the lens panel reads the plan: five lenses, then their synthesis, each started with the Agent tool".into(),
        "cases_unconfirmed" => "the lens panel answered; before the test is written the person checks the cases it will cover, the ones found and the panel's questions".into(),
        other => format!("held on {}", other.replace('_', " ")),
    }
}

/// Each state, and what must come first in it, for a "not yet".
fn state_words(s: &str) -> [&'static str; 2] {
    match s {
        "unplanned" => ["planning is not confirmed yet", "the plan must be complete and confirmed"],
        "waiting" => ["it waits for the criterion before it on the string", "the criterion before it must be green or proven"],
        "testing" => ["it is its turn: the tests come first", "its tests must be committed alone, seen failing and stamped"],
        "sealed" => ["its tests are committed, failing and stamped; the code comes next", "the code on its anchor must be committed and its tests seen passing under the same stamp"],
        "green" => ["its tests pass on this laptop under the same stamp", "the pipeline must prove it"],
        "proven" => ["the pipeline proved it, the teeth run included", "nothing"],
        "held" => ["it is held", "the hold must be cleared"],
        _ => ["", ""],
    }
}

/// A `planning_incomplete` refusal that names its cause, by the plan line
/// missing: line 2 (the owner, 2026-10-11, approved 12:36).
pub const LIGHT_NOT_DOCUMENTATION: &str = "its path is light but its anchor is not documentation: claim it --path full";

/// A clearing move, by the fact it writes.
fn kind_words(k: &str) -> Option<&'static str> {
    Some(match k {
        "intent_submitted" => "submit the intent",
        "behavior" => "write the behavior",
        "promise" => "write the promise and fill its three slots",
        "criterion" => "write the criterion",
        "place_chosen" => "choose its place in the code again and have the lines checked",
        "new_work_spot_chosen" => "choose the spot for the new work",
        "new_work_no_spot" => "announce new work with no spot",
        "anchor_sha_matched" => "check the lines at its anchor",
        "test_planned" => "plan its tests",
        "test_stack_detected" => "name the command that runs the tests",
        "touching_test_declared" => "declare the tests its change touches",
        "criterion_head" | "criterion_strung_on_head" => "put it on the string",
        "criterion_assigned" => "give it to an agent conversation",
        "test_written" => "write its planned tests",
        "pass_please_pass" => "run its tests",
        "fingerprint_stamped" => "stamp its tests",
        "criterion_transfer" => "hand it to another agent conversation",
        "commit_seen" => "a new commit",
        "hold_answered" => "answer its card",
        "pipeline_run_seen" => "run the pipeline on the pull request's head",
        "test_change_announced" => "announce the test change first",
        "model_seen" => "see which model a conversation runs",
        // test first, then pick the spot [draft words]
        "trace_listed" => "write the test, then run familiar-ai try",
        "off_list_reason" => "say in one line why the fix is not in a function the try listed",
        _ => return None,
    })
}

/// An event in plain words, for a refusal that names it as what clears.
fn event_words(e: &str) -> Option<&'static str> {
    Some(match e {
        "the agent's first edit or commit" => "make the first edit or commit",
        "a commit of a criterion's tests" => "commit its tests alone, with no code",
        "an edit of a file" => "edit the file",
        "announce_test_change" => "announce the test change first",
        "the agent declares a criterion finished" => "declare it finished",
        "the agent stops" => "stop",
        "a criterion about to start" => "start the criterion",
        _ => return None,
    })
}

/// An answer choice in plain words.
fn choice_words(ch: &str) -> &str {
    match ch {
        "re_plan" => "re-plan this criterion",
        "keep_going" => "keep going as planned",
        "release" => "release it: it stops blocking the string",
        "try_again" => "try again",
        "take_over" => "take it over yourself",
        "its_the_red" => "it is the red: seal its tests on this run",
        "its_the_pass" => "it is the pass: its tests passed on this run",
        other => other,
    }
}

const TYPED_NO_TEST_STACK: &str = "type the command that runs the tests";
const TYPED_OTHER: &str = "or type your own answer";
/// Shown when the binary is not running; it is a message, not a state.
pub const RUN_ABSENT: &str = "refused · the binary is not running: start it, then try again";
const RUN_EMPTY: &str = "—";

fn or_list(xs: &[&str]) -> String {
    match xs.len() {
        0 => String::new(),
        1 => xs[0].to_string(),
        n => format!("{} or {}", xs[..n - 1].join(", "), xs[n - 1]),
    }
}

fn field<'a>(m: &'a Clear, k: &str) -> Option<&'a str> {
    m.get(k).and_then(Value::as_str)
}

/// One clearing move in words.
fn clear_words_one(x: &Clear) -> Option<String> {
    let kind = field(x, "kind")?;
    let tt = x.get("touches_tests") == Some(&Value::Bool(true));
    let tc = x.get("touches_code") == Some(&Value::Bool(true));
    let tf = x.get("touches_tests") == Some(&Value::Bool(false));
    let cf = x.get("touches_code") == Some(&Value::Bool(false));
    if kind == "commit_seen" && tt && cf {
        return Some("commit its tests alone, with no code, and see them fail (commit_seen)".into());
    }
    if kind == "commit_seen" && tc && tf {
        return Some("commit the code on its anchor (commit_seen)".into());
    }
    if kind == crate::existing::ANNOUNCED {
        let file = field(x, "file_path").unwrap_or("the file");
        return Some(format!("announce the change to {file} with the reason the person's words give for it ({kind})"));
    }
    if kind == crate::places::CLASSIFIED {
        let at = format!("{}:{}", field(x, "file").unwrap_or("the site"), x.get("line").and_then(Value::as_u64).unwrap_or(0));
        return Some(format!("classify {at}: applies, or not_affected with its reason ({kind})"));
    }
    if kind == crate::ask::ONE {
        return Some(format!("say the ask reads one way, and why ({kind})"));
    }
    if kind == crate::ask::ALTERNATIVES {
        return Some(format!("list the two or three readings you see, for the person to pick from ({kind})"));
    }
    if kind == "commit_seen" && x.len() == 1 {
        return Some("a new commit, one that puts the changed lines back when the change is not called for (commit_seen)".into());
    }
    if kind == "hold_answered" {
        if let Some(ch) = field(x, "choice") {
            return Some(format!("the person answers its card: {ch} (hold_answered)"));
        }
    }
    if kind == "criterion_transfer" {
        if let Some(r) = field(x, "rung") {
            // the agent's way out of two_failures
            return Some(format!("an investigator one rung up takes it: a fresh {r} subagent (criterion_transfer)"));
        }
    }
    let words = kind_words(kind).map(str::to_string).unwrap_or_else(|| format!("a {kind} fact"));
    Some(format!("{words} ({kind})"))
}

/// What clears a hold, in words with the move's name beside them; `None`
/// when nothing is named.
fn clear_words(x: &Clears) -> Option<String> {
    match x {
        Clears::Nothing => None,
        Clears::Name(s) => Some(match (event_words(s), kind_words(s)) {
            (Some(w), _) => format!("{w} ({s})"),
            (None, Some(w)) => format!("{w} ({s})"),
            (None, None) => s.clone(),
        }),
        Clears::Moves(ms) => {
            let is_answer = |m: &Clear| field(m, "kind") == Some("hold_answered") && field(m, "choice").is_some();
            let ans: Vec<&str> = ms.iter().filter(|m| is_answer(m)).filter_map(|m| field(m, "choice")).collect();
            let mut rest: Vec<String> = Vec::new();
            for m in ms.iter().filter(|m| !is_answer(m)) {
                if let Some(w) = clear_words_one(m) {
                    if !rest.contains(&w) {
                        rest.push(w);
                    }
                }
            }
            let mut parts: Vec<String> = Vec::new();
            if !ans.is_empty() {
                // the card is the person's way out, so it is named as the person's
                parts.push(format!("the person answers its card: {} (hold_answered)", or_list(&ans)));
            }
            parts.extend(rest);
            let s = parts.join("; or ");
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

fn held_line(v: &View, c: &str) -> String {
    whys(v, c)
        .iter()
        .map(|w| format!("why {w}: {}", why_words(w)))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// `TOLD_WORDS.own_work_moved` (the owner, 2026-10-11) [draft]: what the
/// agent is told when a criterion goes on at its lines' new place.
pub fn own_work_words(text: &str, file: &str, a: &str, b: &str) -> String {
    format!("its lines were changed by the work on \"{text}\"; they now sit at {file} {a}–{b}")
}

/// `toldLine`: the criterion goes on at the lines' new place, and the agent
/// is told which commit and criterion changed them.
fn told_line(v: &View, c: &str) -> Option<String> {
    // F7: the place whose lines the work moved, read from that place's own start check
    let (w, st) = crate::lifecycle::own_work_places(v, c).into_iter().next()?;
    let cf = v.kind("criterion").filter(|f| text(f, "criterion_id") == Some(w.criterion_id.as_str())).last();
    let words = cf.and_then(|f| text(f, "text")).unwrap_or(&w.criterion_id);
    let n = |k: &str| st.fields.get(k).map_or_else(String::new, |x| x.as_str().map_or_else(|| x.to_string(), str::to_string));
    Some(format!("  told · criterion {c} · {} (commit {}, criterion {})", own_work_words(words, &n("file_path"), &n("line_start"), &n("line_end")), w.commit, w.criterion_id))
}

/// E.g. `criterion c1 · sealed`; a held line carries its why.
fn state_line(v: &View, c: &str) -> String {
    let s = state(v, c);
    let mut line = format!("criterion {c} · {s}");
    if s == "held" {
        line.push_str(" · ");
        line.push_str(&held_line(v, c));
    }
    line
}

/// Whether this hold was answered after the criterion's latest other fact,
/// since a later fact may have raised it again.
fn answered(v: &View, c: &str, why: &str) -> bool {
    for f in v.facts.iter().rev() {
        if f.criterion_id() != Some(c) {
            continue;
        }
        if f.kind != "hold_answered" {
            return false;
        }
        if crate::dependency::ended(v, f) {
            continue; // an approval missed twice answers nothing (plan §7.3)
        }
        if text(f, "hold") == Some(why) {
            return true;
        }
    }
    false
}

/// One line per criterion whose state differs between the two logs.
pub fn lines(before: &View, after: &View) -> Vec<String> {
    let bc = plan::criteria(before);
    plan::criteria(after)
        .iter()
        .filter(|c| !bc.contains(c) || state(before, c) != state(after, c))
        .map(|c| state_line(after, c))
        .collect()
}

/// The state a refusal names when it is only too early rather than held.
fn not_yet_state(r: &Refusal) -> Option<&'static str> {
    match (&r.state, &r.hold) {
        (Some(s), None) => Some(s),
        (_, Some(h)) => CRIT_ORDER.iter().copied().find(|s| s == h),
        (None, None) => None,
    }
}

/// The criterion a refusal is about: the event's, except for the referee and
/// the stop, which name the first criterion the refusal holds.
fn about_criterion(v: &View, r: &Refusal, e: &Event, st: Option<&str>) -> Option<String> {
    if r.hold.as_deref() == Some("planning_incomplete") {
        return None;
    }
    // a refusal that names its criterion (an edit of another criterion's sealed test) is about that one
    if let Some(c) = r.criterion_id.as_deref() {
        return Some(c.to_string());
    }
    let c = e.criterion_id();
    if let Some(c) = c {
        if !matches!(e, Event::AboutToStart { .. }) {
            return Some(c.to_string());
        }
    }
    let has = |x: &String| match st {
        Some(st) => state(v, x) == st,
        None => r.hold.as_ref().is_some_and(|h| whys(v, x).contains(h)),
    };
    plan::criteria(v)
        .into_iter()
        .find(has)
        .or_else(|| c.map(str::to_string))
}

/// The refusal text for an event, or `None` when the hook allows it.
pub fn refusal(v: &View, event: &Event) -> Option<String> {
    let r = match decide(v, event) {
        Decision::Allow { .. } => return None,
        Decision::Refuse(r) => r,
    };
    let st = not_yet_state(&r);
    let c = about_criterion(v, &r, event, st);
    let who = c.as_ref().map(|c| format!("criterion {c}")).unwrap_or_else(|| "the run".into());
    let next = clear_words(&r.clears).unwrap_or_else(|| RUN_EMPTY.into());
    if let Some(st) = st {
        let p = if st == "waiting" { c.as_deref().and_then(|c| plan::string_before(v, c)) } else { None };
        let sw = state_words(st);
        let first = match p {
            Some(p) => format!("criterion {p} must be green or proven; it is {}", state(v, &p)),
            None => sw[1].to_string(),
        };
        return Some([format!("not yet · {who} · {st}"), format!("  {}", sw[0]), format!("  first: {first}"), format!("  next: {next}")].join("\n"));
    }
    let hold = r.hold.unwrap_or_default();
    // the owner (2026-10-11): line 2 missing because a light anchor is not documentation
    if hold == "planning_incomplete" && plan::PLAN_LINES.iter().position(|n| !plan::line(v, n).unwrap_or(true)) == Some(1) {
        if let Some(ld) = plan::criteria(v).into_iter().find(|x| plan::not_documentation(v, x)) {
            return Some([format!("refused · criterion {ld} · held: {hold}"), format!("  {LIGHT_NOT_DOCUMENTATION}"), format!("  to clear it: {next}")].join("\n"));
        }
    }
    // a place off the try's list [draft words]: each place off it, then another place or the reason
    if hold == "planning_incomplete" {
        if let Some(c) = plan::criteria(v).into_iter().find(|x| crate::trace::reason_owed(v, x)) {
            let mut lines = vec![format!("refused · criterion {c} · held: {hold}")];
            for p in crate::trace::off_list(v, &c) {
                lines.push(format!("  its place {} {} is not on the try's list of what the failing test ran (familiar-ai status shows it)", p.file_path, p.symbol));
            }
            lines.push(format!("  to clear it: {next}"));
            return Some(lines.join("\n"));
        }
    }
    // places before the fix [draft words]: each site left, then what clears it
    if hold == crate::places::HOLD {
        let mut lines = vec![format!("refused · {who} · held: {hold}"), format!("  {}", why_words(&hold))];
        if let Some(c) = c.as_deref() {
            lines.extend(crate::places::unclassified(v, c).iter().map(|s| format!("  unclassified: {}", s.words())));
        }
        lines.push(format!("  to clear it: {next}"));
        return Some(lines.join("\n"));
    }
    // a clarifying question on a signal [draft words]: the person's words, then what fired or the
    // readings the person picks from, then what clears it
    if hold == crate::ask::HOLD {
        let mut lines = vec![format!("refused · {who} · held: {hold}"), format!("  {}", why_words(&hold))];
        if let Some(c) = c.as_deref() {
            if let Some(w) = crate::ask::words_of(v, c) {
                lines.push(format!("  the person said: \"{}\"", w.split_whitespace().collect::<Vec<_>>().join(" ")));
            }
            if crate::ask::owed(v, c) {
                lines.extend(crate::ask::fired(v, c).iter().map(|t| format!("  it has {}", t.words())));
            } else {
                lines.extend(crate::ask::readings(v, c).iter().enumerate().map(|(i, r)| format!("  reading {}: {r}", i + 1)));
            }
        }
        lines.push(format!("  to clear it: {next}"));
        return Some(lines.join("\n"));
    }
    // an existing test's lines change only when announced (the owner, 2026-10-20) [draft words]:
    // each file, its tests and the base's lines, and the judge's no when it said one
    if hold == crate::existing::HOLD {
        let mut lines = vec![format!("refused · {who} · held: {hold}"), format!("  {}", why_words(&hold))];
        for e in crate::existing::unannounced(v) {
            let no = crate::existing::judged_no(v, e).map(|f| {
                let gap = text(f, "gap").filter(|g| !g.is_empty()).map(|g| format!(": {}", g.trim().trim_end_matches('.'))).unwrap_or_default();
                format!(" · the judge said its old expectation is not what the person asked to change{gap}")
            });
            lines.push(format!("  changed: {}{}", crate::existing::words(e), no.unwrap_or_default()));
        }
        lines.push(format!("  to clear it: {next}"));
        return Some(lines.join("\n"));
    }
    Some([format!("refused · {who} · held: {hold}"), format!("  {}", why_words(&hold)), format!("  to clear it: {next}")].join("\n"))
}

/// The holds a person answers: the lifecycle's `ANSWERS`, plus a missing
/// test command, answered by typing it.
fn person_whys() -> Vec<&'static str> {
    ANSWERS.iter().map(|(w, _)| *w).chain(std::iter::once("no_test_stack")).collect()
}

fn anchor_at(v: &View, c: &str) -> String {
    let Some(a) = plan::anchor_of(v, c) else {
        return "its anchor".into();
    };
    if a.kind == "place_chosen" {
        if let Some(p) = plan::places_of(a).first() {
            return format!("{} {}", p.file_path, p.symbol);
        }
    }
    if let Some(fp) = text(a, "file_path") {
        return fp.to_string();
    }
    "the new work".into()
}

/// Names the holder's model only when it is off the ladder, since then no
/// investigator is sent and the card is the only way out.
fn off_the_ladder(v: &View, c: &str) -> String {
    let h = plan::assigned_to(v, c);
    match crate::ladder::model_of(v, h) {
        Some(m) if crate::ladder::rung_of(v, h).is_none() => {
            format!("; it ran on {m}, a model off the ladder, so no investigator is sent")
        }
        _ => String::new(),
    }
}

/// The gap card's question (plan A-4) [draft words]: each runner and root whose choice has open
/// gaps, and the outcomes that will ask, in words (`reading::OUTCOMES`).
fn gap_question(v: &View) -> String {
    let each: Vec<String> = crate::reading::gaps_open(v)
        .iter()
        .map(|ch| {
            let root = text(ch, "root").filter(|r| !r.is_empty()).unwrap_or("the project root");
            let outcomes: Vec<&str> = crate::reader::texts(ch, "gaps").into_iter().map(crate::reading::outcome_words).collect();
            format!(
                "Familiar cannot read every outcome of {} at {root}; a run that ends in one of these will ask you: {}",
                text(ch, "runner").unwrap_or("the runner"),
                outcomes.join(", ")
            )
        })
        .collect();
    format!("the test setup: {}. What now?", each.join("; "))
}

fn question(v: &View, why: &str, c: &str, at: &str) -> String {
    match why {
        "reading_gap" => gap_question(v),
        // a clarifying question on a signal [draft words]
        "reading_unclear" => format!("criterion {c}: the person's words can be read more than one way. Which did they mean?"),
        // the one question (the owner's plan of 2026-10-23, change D), the owner's words
        "repro_unknown" => crate::trace::REPRO_QUESTION.to_string(),
        // the cases card (the owner's plan of 2026-10-25) [draft words]
        "cases_unconfirmed" => format!("Before I write the test for criterion {c}, here's what it will check:"),
        "anchor_moved" => format!("criterion {c}: the code at {at} changed after the plan was made. What now?"),
        "anchor_untouched" => format!("criterion {c}: its tests pass, but no commit changed the code at {at}. What now?"),
        "two_failures" => format!("criterion {c}: two attempts in a row ended with its tests failing{}. What now?", off_the_ladder(v, c)),
        "no_test_stack" => format!("criterion {c}: no test command is known for {at}. What command runs its tests?"),
        "crashed" => {
            let at_place = crate::outcome::card_run(v, c, "crashed")
                .zip(plan::anchor_of(v, c))
                .is_some_and(|(t, a)| crate::outcome::points_at_place(t, a, v));
            let where_ = if at_place { format!("the crash points at {at}") } else { format!("the crash is not at {at}") };
            format!("criterion {c}: its test crashed before any fix; {where_}. What now?")
        }
        "command_broke" => format!("criterion {c}: the test command worked before and fails now. What now?"),
        "review_disputed" => format!(
            "criterion {c}: its tests pass, but a reviewer read the fix against the person's words twice and found it does not do {}. What now?",
            crate::review::gap(v, c).map(|g| g.trim().trim_end_matches('.')).unwrap_or("what was asked")
        ),
        // H-4: the stage decides what the person is asked to confirm
        "run_unclassified" if crate::criterion::seal_raw(v, c, crate::criterion::Place::Laptop).is_some() => {
            format!("criterion {c}: its tests ran after the seal and Familiar could not tell whether they passed. What now?")
        }
        "run_unclassified" => {
            format!("criterion {c}: its tests ran before any fix and Familiar could not tell whether they failed for the reason they test. What now?")
        }
        "miscoped" => {
            let dep = crate::dependency::miscoped_by(v, c).unwrap_or_else(|| "the dependency".to_string());
            format!("criterion {c}: criterion {c} passed once criterion {dep} was fixed: criterion {c} was criterion {dep}. What now?")
        }
        _ => format!("criterion {c} is held: {}. What now?", why_words(why)),
    }
}

/// One card: a hold that needs a person, not yet answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub criterion_id: String,
    pub hold: String,
    pub question: String,
    /// `(choice, words)`, at most three, each an admissible `hold_answered`.
    pub choices: Vec<(String, String)>,
    pub typed: String,
    pub text: String,
    /// The can't-tell card's run in short sentences (H-4, `run_summary`); empty on other cards.
    pub summary: Vec<String>,
}

impl Card {
    pub fn to_json(&self) -> Value {
        json!({
            "criterion_id": self.criterion_id,
            "hold": self.hold,
            "question": self.question,
            "choices": self.choices.iter().map(|(c, w)| json!({ "choice": c, "words": w })).collect::<Vec<_>>(),
            "typed": self.typed,
            "writes": { "kind": "hold_answered", "criterion_id": self.criterion_id, "hold": self.hold },
            "text": self.text,
            "summary": self.summary,
        })
    }
}

/// A line of the output cut to one line of at most `SUMMARY_CUT` characters.
const SUMMARY_CUT: usize = 160;

fn one_cut_line(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= SUMMARY_CUT {
        return one;
    }
    let mut cut: String = one.chars().take(SUMMARY_CUT).collect();
    cut.push('…');
    cut
}

/// The counts a run's translation carries, in words: "1 failed, 3 passed".
fn counts_words(t: &familiar_ai_facts::Fact) -> Option<String> {
    let counts = t.fields.get("counts").and_then(Value::as_object)?;
    let said: Vec<String> = ["failed", "passed", "skipped", "deselected"]
        .iter()
        .filter_map(|k| counts.get(*k).and_then(Value::as_u64).map(|n| format!("{n} {k}")))
        .collect();
    (!said.is_empty()).then(|| said.join(", "))
}

/// Where a run's failure points, in words: `file:line, in function`.
fn where_words(t: &familiar_ai_facts::Fact) -> Option<String> {
    let l = t.fields.get("location").and_then(Value::as_object)?;
    let file = l.get("file_path").and_then(Value::as_str).filter(|f| !f.is_empty())?;
    let mut out = match l.get("line").and_then(Value::as_u64) {
        Some(n) => format!("{file}:{n}"),
        None => file.to_string(),
    };
    if let Some(f) = l.get("function").and_then(Value::as_str).filter(|f| !f.is_empty()) {
        out.push_str(&format!(", in {f}"));
    }
    Some(out)
}

/// Why no rule read the run, in plain words.
fn why_cant_tell(t: &familiar_ai_facts::Fact) -> String {
    let kind = t.fields.get("location").and_then(|l| l.get("kind")).and_then(Value::as_str);
    match (text(t, "label"), where_words(t)) {
        (Some("unclassified") | None, _) => "the output's summary wasn't recognised".into(),
        (_, None) => "the failure shows no line in your test or code".into(),
        (_, Some(w)) if kind == Some("exception") => format!("the failure is an error raised at {w}, not a check that failed"),
        (Some(label), Some(w)) => format!("no rule reads a run that ended {} at {w}", label.replace('_', " ")),
    }
}

/// H-4 (the owner, 2026-10-19): the can't-tell card's run in short sentences a person can read:
/// what ran, the counts, the deciding line, the failing tests and where, why Familiar couldn't
/// tell, and where the full output is kept. Never the log itself.
pub fn run_summary(v: &View, c: &str) -> Vec<String> {
    let Some(t) = crate::outcome::card_run(v, c, "run_unclassified") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let command = text(t, "run_id").and_then(|id| crate::outcome::command_of_run(v, id)).and_then(|r| text(r, "command"));
    out.push(match command {
        Some(cmd) => format!("It ran: {}", one_cut_line(cmd)),
        None => "The command it ran was not recorded.".into(),
    });
    out.push(match counts_words(t) {
        Some(n) => format!("Counts: {n}."),
        None => "No counts were read from the output.".into(),
    });
    out.push(match text(t, "evidence").map(one_cut_line).filter(|e| !e.is_empty()) {
        Some(e) => format!("The deciding line: {e}"),
        None => "No line of the output decided it.".into(),
    });
    let failing: Vec<&str> = t.fields.get("failing_tests").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let named = match failing.len() {
        0 => None,
        1..=3 => Some(failing.join(", ")),
        n => Some(format!("{} and {} more", failing[..3].join(", "), n - 3)),
    };
    out.push(match (named, where_words(t)) {
        (Some(n), Some(w)) => one_cut_line(&format!("Failing: {n}, at {w}.")),
        (Some(n), None) => one_cut_line(&format!("Failing: {n}.")),
        (None, Some(w)) => format!("It failed at {w}."),
        (None, None) => "No failing test or line was named.".into(),
    });
    out.push(format!("Familiar could not tell because {}.", why_cant_tell(t)));
    out.extend(call_path_lines(v, c, t));
    // the binary keeps the end of each run under the commit's name (`kept::keep_run`)
    if let Some(commit) = text(t, "commit").filter(|s| !s.is_empty() && s.chars().all(|ch| ch.is_ascii_alphanumeric())) {
        out.push(format!("The full output: .familiar/runs/{commit}.log"));
    }
    out
}

/// The code the agent planned to change for `c`: each place's file, first and last line, and name.
fn planned(v: &View, c: &str) -> Vec<(String, u64, u64, String)> {
    let Some(a) = plan::anchor_of(v, c) else { return Vec::new() };
    let one = |p: &Value| -> Option<(String, u64, u64, String)> {
        let file = p.get("file_path").and_then(Value::as_str).filter(|f| !f.is_empty())?;
        let start = p.get("line_start").and_then(Value::as_u64)?;
        let end = p.get("line_end").and_then(Value::as_u64)?;
        let name = p.get("symbol").and_then(Value::as_str).unwrap_or("");
        Some((file.to_string(), start, end, name.to_string()))
    };
    match a.fields.get("places").and_then(Value::as_array) {
        Some(ps) => ps.iter().filter_map(one).collect(),
        None => one(&Value::Object(a.fields.clone())).into_iter().collect(),
    }
}

/// The can't-tell card's call path (2026-10-20), only from the frames the reporter recorded: the
/// test's frame, every frame inside the planned code (its file, its line within the planned lines),
/// the frame the error was raised in, and the frames between counted, never guessed; then whether
/// the error passed through the planned code. No frames: said so.
fn call_path_lines(v: &View, c: &str, t: &familiar_ai_facts::Fact) -> Vec<String> {
    let frames: Vec<&serde_json::Map<String, Value>> = t.fields.get("frames").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_object).collect()).unwrap_or_default();
    if frames.is_empty() {
        return vec!["Familiar has no call path for this run.".into()];
    }
    let places = planned(v, c);
    let file = |f: &serde_json::Map<String, Value>| f.get("file_path").and_then(Value::as_str).unwrap_or("").to_string();
    let line = |f: &serde_json::Map<String, Value>| f.get("line").and_then(Value::as_u64).unwrap_or(0);
    let inside = |f: &serde_json::Map<String, Value>| places.iter().find(|(pf, a, b, _)| *pf == file(f) && (*a..=*b).contains(&line(f)));
    let last = frames.len() - 1;
    let shown: Vec<usize> = (0..frames.len()).filter(|&i| i == 0 || i == last || inside(frames[i]).is_some()).collect();
    // the record keeps the first and the last half of a longer path: across that cut the count is unknown
    let cut = frames.len() == familiar_ai_facts::FRAMES_KEPT;
    let mut out = vec!["Call path, from the test to the error:".to_string()];
    let mut prev: Option<usize> = None;
    for &i in &shown {
        if let Some(p) = prev {
            let between = i - p - 1;
            let across = cut && p < familiar_ai_facts::FRAMES_KEPT / 2 && i >= familiar_ai_facts::FRAMES_KEPT / 2;
            if across {
                out.push("  … more calls …".into());
            } else if between > 0 {
                out.push(format!("  … {between} more call{} …", if between == 1 { "" } else { "s" }));
            }
        }
        let f = frames[i];
        let name = f.get("function").and_then(Value::as_str).filter(|n| !n.is_empty()).map(|n| format!(" in {n}")).unwrap_or_default();
        let mark = if inside(f).is_some() {
            " (the code the agent planned to fix)"
        } else if i == last {
            " (the error was raised here)"
        } else {
            ""
        };
        out.push(format!("  {}", one_cut_line(&format!("{}:{}{name}{mark}", file(f), line(f)))));
        prev = Some(i);
    }
    let through = frames.iter().find_map(|f| inside(f));
    let named = |n: &str, f: &str| if n.is_empty() { f.to_string() } else { n.to_string() };
    match (through, places.first()) {
        (Some((pf, _, _, n)), _) => out.push(format!("The error passes through {}, the code the agent planned to fix.", named(n, pf))),
        (None, Some((pf, _, _, n))) => out.push(format!("The error does not pass through {}, the code the agent planned to fix.", named(n, pf))),
        (None, None) => {}
    }
    out
}

/// One card per unanswered hold that needs a person, with at most three
/// choices that are admissible at this log and always a typed input.
pub fn cards(v: &View) -> Vec<Card> {
    let people = person_whys();
    let mut out = Vec::new();
    let mut adm: Option<Vec<crate::sample::Move>> = None;
    // the reserved part `setup` after the parts (plan A-4): its holds read their own answers
    let setup = crate::reading::SETUP_PART;
    for c in plan::criteria(v).into_iter().chain(std::iter::once(setup.to_string())) {
        let mut seen: Vec<String> = Vec::new();
        for why in whys(v, &c) {
            if seen.contains(&why) {
                continue;
            }
            seen.push(why.clone());
            // a clarifying question on a signal [draft]: the person's card while readings stand,
            // one per ask, its choices the readings verbatim
            if why == crate::ask::HOLD {
                if crate::ask::carries(v, &c) {
                    out.push(reading_card(v, &c));
                }
                continue;
            }
            // the cases card [draft]: the found cases, its choices by the finding, and words typed
            if why == crate::cases::CARD {
                out.push(cases_card(v, &c));
                continue;
            }
            // the one question [draft]: how to make it happen, its two choices and words typed
            if why == crate::trace::REPRO {
                out.push(repro_card(v, &c));
                continue;
            }
            if !people.contains(&why.as_str()) || (c != setup && answered(v, &c, &why)) {
                continue;
            }
            let moves = adm.get_or_insert_with(|| criterion_admissible(v));
            let offered: Vec<&str> = moves
                .iter()
                .filter(|m| {
                    m.kind == "hold_answered"
                        && m.fields.get("criterion_id").and_then(Value::as_str) == Some(c.as_str())
                        && m.fields.get("hold").and_then(Value::as_str) == Some(why.as_str())
                })
                .filter_map(|m| m.fields.get("choice").and_then(Value::as_str))
                .collect();
            let choices: Vec<(String, String)> = answers_of(&why)
                .map(|cs| cs.iter().copied().filter(|ch| offered.contains(ch)).take(4).map(|ch| (ch.to_string(), choice_words(ch).to_string())).collect())
                .unwrap_or_default();
            let typed = if why == "no_test_stack" { TYPED_NO_TEST_STACK } else { TYPED_OTHER };
            let q = question(v, &why, &c, &anchor_at(v, &c));
            let summary = if why == "run_unclassified" { run_summary(v, &c) } else { Vec::new() };
            let mut text_lines = vec![format!("? {q}")];
            text_lines.extend(summary.iter().map(|l| format!("  {l}")));
            for (i, (ch, w)) in choices.iter().enumerate() {
                text_lines.push(format!("  {}) {ch} — {w}", i + 1));
            }
            text_lines.push(format!("  > {typed}"));
            out.push(Card {
                criterion_id: c.clone(),
                hold: why.clone(),
                question: q,
                choices,
                typed: typed.to_string(),
                text: text_lines.join("\n"),
                summary,
            });
        }
    }
    out
}

/// The card of `c`'s ask read more than one way [draft words]: the readings verbatim, each an
/// admissible `hold_answered`, and the typed answer.
fn reading_card(v: &View, c: &str) -> Card {
    let choices: Vec<(String, String)> = crate::ask::readings(v, c).into_iter().map(|r| (r, READING_PICKED.to_string())).collect();
    let q = question(v, crate::ask::HOLD, c, "");
    let mut text_lines = vec![format!("? {q}")];
    for (i, (ch, w)) in choices.iter().enumerate() {
        text_lines.push(format!("  {}) {ch} — {w}", i + 1));
    }
    text_lines.push(format!("  > {TYPED_OTHER}"));
    Card {
        criterion_id: c.to_string(),
        hold: crate::ask::HOLD.to_string(),
        question: q,
        choices,
        typed: TYPED_OTHER.to_string(),
        text: text_lines.join("\n"),
        summary: Vec::new(),
    }
}

/// The cases card's lines past its question (the owner's plan of 2026-10-25, §3, wording approved
/// by the owner): before the found checks, and in place of them when the finder found none or failed.
pub const CASES_ALSO: &str = "Also worth checking:";
pub const CASES_NONE: &str = "Nothing else seemed worth checking.";
pub const CASES_FAILED: &str = "I couldn't finish looking for more checks.";
/// A planned check's line, and a found check's (numbered from 1, as words typed pick them).
pub const CASES_PLANNED: &str = "✓ ";

/// A count in words, to ten.
fn count_words(n: usize) -> String {
    const WORDS: [&str; 11] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    WORDS.get(n).map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// `c`'s planned checks in the person's words: each case of its promise by its short line [draft
/// name], else by its given, on one line.
fn planned_lines(v: &View, c: &str) -> Vec<String> {
    let one = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(p) = plan::promise_of(v, c) else {
        return Vec::new();
    };
    let shorts: Vec<Option<String>> = p
        .fields
        .get("cases")
        .and_then(Value::as_array)
        .map(|cs| cs.iter().map(|k| k.get("short").and_then(Value::as_str).map(one).filter(|s| !s.is_empty())).collect())
        .unwrap_or_default();
    plan::cases_in(p).iter().enumerate().map(|(i, k)| shorts.get(i).cloned().flatten().unwrap_or_else(|| one(&k.given))).collect()
}

/// The found checks of `c`'s latest finding, by their short lines, in its order.
fn found_lines(v: &View, c: &str) -> Vec<String> {
    let Some(f) = crate::cases::findings(v, c).last().copied() else {
        return Vec::new();
    };
    f.fields
        .get("cases")
        .and_then(Value::as_array)
        .map(|cs| cs.iter().map(|k| k.get("short").and_then(Value::as_str).unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")).collect())
        .unwrap_or_default()
}

/// The note on `c`'s latest finding when it found cases (the owner's ruling, 2026-10-25, "Keep
/// what fits") [draft name]: its why, on one line, which says what the binary left out of the
/// finder's answer; `None` when it is blank, or the finding found none or failed.
pub fn cases_note(v: &View, c: &str) -> Option<String> {
    let f = crate::cases::findings(v, c).last().copied().filter(|f| text(f, "outcome") == Some("found"))?;
    Some(text(f, "why")?.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|n| !n.is_empty())
}

/// The cases card's lines on `c`, past its question: the planned checks, then the found ones
/// numbered and the finding's note, or what the finder said when it found none or failed.
pub fn cases_lines(v: &View, c: &str) -> Vec<String> {
    let mut out: Vec<String> = planned_lines(v, c).into_iter().map(|l| format!("{CASES_PLANNED}{l}")).collect();
    let outcome = crate::cases::findings(v, c).last().and_then(|f| text(f, "outcome")).unwrap_or("");
    let found = found_lines(v, c);
    if outcome == "found" && !found.is_empty() {
        out.push(CASES_ALSO.to_string());
        out.extend(found.iter().enumerate().map(|(i, l)| format!("{}. {l}", i + 1)));
        out.extend(cases_note(v, c));
    } else if outcome == "failed" {
        out.push(CASES_FAILED.to_string());
    } else {
        out.push(CASES_NONE.to_string());
    }
    out
}

/// The cases card's options on `c` (the owner's plan of 2026-10-25, §3, wording approved by the
/// owner; the words for one or two found checks, a failed finder's descriptions and the person's
/// own case after the last look [draft words]):
/// `(choice, label, description)`, the choice the finding recommends marked; the picker adds its
/// own "Other".
pub fn cases_options(v: &View, c: &str) -> Vec<(&'static str, String, String)> {
    let planned = planned_lines(v, c).len();
    let found = found_lines(v, c).len();
    let failed = crate::cases::findings(v, c).last().and_then(|f| text(f, "outcome")) == Some("failed");
    let recommended = " (Recommended)";
    crate::cases::offered(v, c)
        .into_iter()
        .map(|ch| match ch {
            crate::cases::ADD_ALL => {
                let add = match found {
                    1 => "Add it".to_string(),
                    2 => "Add both".to_string(),
                    n => format!("Add all {}", count_words(n)),
                };
                (ch, format!("{add}{recommended}"), format!("the test checks all {}", count_words(planned + found)))
            }
            crate::cases::AGAIN => (ch, "Look again".to_string(), "one more search".to_string()),
            // the person's own case, after the last look (the owner's ruling, 2026-10-25) [draft words]
            crate::cases::TYPED => (ch, "Add my own".to_string(), "pick Other and describe a case to add".to_string()),
            _ if found > 0 => (ch, format!("Just the first {}", count_words(planned)), format!("the test checks the {} above", count_words(planned))),
            _ if failed => (ch, "Test these".to_string(), "I'll write the test".to_string()),
            _ => (ch, format!("Looks right{recommended}"), "I'll write the test".to_string()),
        })
        .collect()
}

/// The cases card on `c` [draft]: its lines, the choices its latest finding allows, and words typed,
/// until its choice is answered; then (the lens panel, the owner's plan of 2026-10-26, approved)
/// no choice, and below, each of the synthesis's questions still open with its options numbered.
fn cases_card(v: &View, c: &str) -> Card {
    let answered = crate::cases::choice_answered(v, c);
    let choices: Vec<(String, String)> = if answered { Vec::new() } else { cases_options(v, c).into_iter().map(|(ch, label, _)| (ch.to_string(), label)).collect() };
    let q = question(v, crate::cases::CARD, c, "");
    let summary = cases_lines(v, c);
    let mut text_lines = vec![format!("? {q}")];
    text_lines.extend(summary.iter().map(|l| format!("  {l}")));
    for (i, (ch, w)) in choices.iter().enumerate() {
        text_lines.push(format!("  {}) {ch} — {w}", i + 1));
    }
    if !answered {
        text_lines.push(format!("  > {TYPED_OTHER}"));
    }
    let open = crate::cases::unanswered_questions(v, c);
    for fq in crate::cases::questions(v, c).into_iter().filter(|fq| open.contains(&fq.question_id)) {
        text_lines.push(format!("  ? {}", fq.text.split_whitespace().collect::<Vec<_>>().join(" ")));
        text_lines.extend(fq.options.iter().enumerate().map(|(i, o)| format!("    {}) {o}", i + 1)));
    }
    Card {
        criterion_id: c.to_string(),
        hold: crate::cases::CARD.to_string(),
        question: q,
        choices,
        typed: TYPED_OTHER.to_string(),
        text: text_lines.join("\n"),
        summary,
    }
}

/// The card of the one question on `c` [draft]: "How do you make it happen? What do you run, and
/// what do you see?", "I'll type it" and "I don't know — go ahead", and the typed answer.
fn repro_card(v: &View, c: &str) -> Card {
    let choices: Vec<(String, String)> =
        [(crate::trace::TYPE_IT, crate::trace::TYPE_IT_WORDS), (crate::trace::GO_AHEAD, crate::trace::GO_AHEAD_WORDS)].iter().map(|(ch, w)| (ch.to_string(), w.to_string())).collect();
    let q = question(v, crate::trace::REPRO, c, "");
    let mut text_lines = vec![format!("? {q}")];
    for (i, (ch, w)) in choices.iter().enumerate() {
        text_lines.push(format!("  {}) {ch} — {w}", i + 1));
    }
    text_lines.push(format!("  > {TYPED_OTHER}"));
    Card {
        criterion_id: c.to_string(),
        hold: crate::trace::REPRO.to_string(),
        question: q,
        choices,
        typed: TYPED_OTHER.to_string(),
        text: text_lines.join("\n"),
        summary: Vec::new(),
    }
}

/// What a reading picked on the card means [draft words].
pub const READING_PICKED: &str = "the person meant this; the agent plans the work for it";

/// The plan's state and one line per criterion, with the CI line last.
pub fn status(v: &View) -> String {
    let mut out = vec![format!("plan · {}", plan::plan_state(v))];
    for c in plan::criteria(v) {
        let path = plan::path_of(v, &c).unwrap_or("null");
        let agent = plan::assigned_to(v, &c).unwrap_or("none");
        out.push(format!("{} · path {path} · agent {agent}", state_line(v, &c)));
        if let Some(told) = told_line(v, &c) {
            out.push(told);
        }
    }
    // the CI line last
    if let Some(ci) = ci_line(v) {
        out.push(ci);
    }
    out.join("\n")
}

/// What CI said of the head, read from the record and never the network; see
/// docs/decisions.md#ci-proves-the-laptop-guides.
/// A report for another sha reads as stale; `None` when there is no head.
pub fn ci_line(v: &View) -> Option<String> {
    let head = match v.last("commit_seen") {
        Some(f) => text(f, "commit"),
        None => v.last("intent_submitted").and_then(|i| text(i, "sha")),
    }?;
    let sha7 = |s: &str| s.chars().take(7).collect::<String>();
    if let Some(r) = v.kind("ci_report_seen").filter(|f| text(f, "sha") == Some(head)).last() {
        let id = text(r, "run_id").unwrap_or("");
        return Some(match text(r, "outcome") {
            Some("proven") => format!("CI: proven at {}, run {id}", sha7(head)),
            Some("cancelled") => format!("CI: cancelled (run {id})"),
            Some("timed_out") => format!("CI: timed out (run {id})"),
            // a run that could not prove reads not proven, with its first report line as the reason
            _ => {
                let first = crate::reader::texts(r, "report").first().map(|l| format!(": {l}")).unwrap_or_default();
                format!("CI: not proven at {}, run {id}{first}", sha7(head))
            }
        });
    }
    Some(match v.last("ci_report_seen") {
        Some(any) => format!("CI: stale (ran at {})", sha7(text(any, "sha").unwrap_or(""))),
        None => format!("CI: no run for {}", sha7(head)),
    })
}

/// The words of the holds, by why, for a caller that renders its own line.
pub fn words_of_why(why: &str) -> String {
    why_words(why)
}
