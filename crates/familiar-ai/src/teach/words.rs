//! The words the person and the agent read. Every line begins with `familiar-ai:`, a
//! refusal states what comes next rather than saying refused or error, and a criterion
//! is named by its own text in quotes. The rules crate's terminal words
//! (`familiar_ai_rules::terminal`) are not changed; this module re-maps them for display,
//! keeping the binary's own lines after them in order and every claim line unchanged.

use familiar_ai_facts::Log;
use familiar_ai_rules::terminal::Card;
use familiar_ai_rules::{criterion, ladder, lifecycle, plan, View};
use serde_json::Value;

use crate::teach::judge;

/// How every line the binary emits begins.
pub const LEAD: &str = "familiar-ai: ";

/// The holds in the person's words, by the model's name.
const HOLDS: [(&str, &str); 32] = [
    ("planning_incomplete", "the plan comes first"),
    ("proof_tampered", "the sealed test was changed; fix the code without editing the test, or commit the changed test alone and see it fail"),
    ("tests_not_red", "the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count"),
    ("not_stamped", "the code landed before its test was sealed"),
    ("tests_and_code_mixed", "tests and code are in one commit; the tests go first, alone"),
    ("two_failures", "two tries failed; a stronger model is taking over"),
    ("two_tries_not_red", "two tries to go red failed; this needs you"),
    ("anchor_untouched", "the change landed outside the place the plan named"),
    ("anchor_moved", "the code at the plan's place changed after the plan was made"),
    ("command_changed", "the test command changed after the test was sealed"),
    ("no_teeth", "the test still passes with the change taken back out"),
    ("regression", "a test that passed before fails now"),
    ("released", "released by you"),
    ("anchor_not_found", "the code the plan points at is not at the lines it names"),
    ("no_test_stack", "the command that runs the tests is not known yet"),
    // Placeholder words until the card's text is settled.
    ("fix_elsewhere", "held on fix elsewhere"),
    ("red_for_another_reason", "the test did not fail for the reason it tests: it is broken, too slow, or its command never ran it"),
    ("two_test_broken", "two tries ended with the test broken; this needs you"),
    ("two_crashed", "two tries ended with the test crashing; this needs you"),
    ("two_timed_out", "two tries ended with the test too slow; this needs you"),
    ("environment_error", "the tests could not run here; this needs you"),
    ("run_unclassified", "the run ended in a way no rule reads; this needs you"),
    ("retry_spent", "the run was killed or its runner crashed, twice; this needs you"),
    ("crashed", "the test crashed before any fix; this needs you"),
    ("command_broke", "the test command worked before and fails now; this needs you"),
    ("miscoped", "its tests passed once its dependency was fixed; this needs you"),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
    ("review_disputed", "a reviewer read the fix twice and found it does not do what was asked; this needs you"),
    // test first, then pick the spot (the owner's plan of 2026-10-23) [draft words]
    ("try_owed", "the test comes before the place: write the planned test, then run familiar-ai try; the place in the code is picked from what the failing test runs"),
    ("try_not_red", "the latest try passed, so the test does not show the problem yet: change the test so it fails, then run familiar-ai try again"),
    ("repro_unknown", "before the test, the person is asked: How do you make it happen? What do you run, and what do you see?"),
    // the cases card (the owner's plan of 2026-10-25); the lens panel (the owner's plan of 2026-10-26, approved) [draft words]
    ("cases_owed", "before the test, the lens panel reads the plan for the checks the planned cases miss: five lenses, one Agent call each, in parallel, then the synthesis; each runs as familiar-ai:lens, and the binary gives each its brief"),
    ("cases_unconfirmed", "before the test, the person checks the cases it will cover, the ones the lens panel found and its questions: ask them with AskUserQuestion"),
];

/// An agent's `claude -p`, refused: a lane is started with `launch_lane`.
pub const LANE_BY_HAND: &str = "familiar-ai: a lane is started with familiar-ai launch_lane, so the person's words travel with it and one session runs per worktree · next: familiar-ai launch_lane --brief <the brief's file>";

/// A commit refused because the holder has not yet claimed where the fix lives (`lifecycle::first_act`).
pub const CLAIM_FIRST: &str = "where its fix lives is claimed first, before any commit";

/// What clears `CLAIM_FIRST`, in words.
pub const CLAIM_NEXT: &str = "claim where its fix lives";

/// Shown after the claim's command: the categories of a fix outside this repository.
pub const CLAIM_ELSEWHERE: &str = "(the fix cannot be made in this repository: --category dependency, environment or request, --where names where it lives, --needs what it needs; the person is asked, no test and no code)";

/// The labels of a `fix_elsewhere` card's claim, by field.
pub const CARD_WHERE: &str = "where: ";
pub const CARD_NEEDS: &str = "needs: ";

/// `two_failures` when no rung is left above the holder.
pub const TWO_FAILURES_NEEDS_YOU: &str = "two tries failed; this needs you";

/// `two_tries_not_red` with no rung left above the holder; its second half matches `TWO_FAILURES_NEEDS_YOU`.
pub const TWO_TRIES_NOT_RED_NEEDS_YOU: &str = "two tries to go red failed; this needs you";

/// Shown after `tests_not_red`'s teaching: a passing tests-first run counts as a failed try.
pub const TESTS_NOT_RED_TRIES: &str = "a second run that passes counts as a failed try; two in a row and the person decides";

/// The command the binary runs for `c`'s tests after each commit, in words.
pub fn command_of(v: &View, c: Option<&str>) -> String {
    c.and_then(|c| familiar_ai_rules::criterion::command(v, c)).unwrap_or_else(|| "the test command".to_string())
}

/// What `tests_not_red` teaches: what to write, the command the binary runs after each commit,
/// and what a second passing run means.
pub fn tests_not_red_teaching(v: &View, c: Option<&str>) -> Vec<String> {
    vec![
        format!("  write a test that fails on the code as it is now; familiar-ai runs {} after each commit", command_of(v, c)),
        format!("  {TESTS_NOT_RED_TRIES}"),
    ]
}

/// The first failed try since the seal: the tests still fail, what the person asked for, where
/// the code goes, and the last lines of the kept run (`run`, when one was kept).
pub fn failed_try(log: &Log, c: &str, run: Option<&str>) -> Vec<String> {
    let v = View::new(log);
    let mut out = vec![format!("{LEAD}ran the tests of {}: they still fail · try 1 of 2; two failed tries in a row send a stronger model", quoted(&v, c))];
    out.extend(judge::asked_for(log));
    out.push(format!("  the code goes on {}; the test stays as it is", anchor_words(&v, c)));
    match run.map(str::trim_end).filter(|r| !r.trim().is_empty()) {
        Some(r) => {
            out.push("  the last lines of the run:".to_string());
            let lines: Vec<&str> = r.lines().collect();
            let from = lines.len().saturating_sub(crate::teach::investigator::OUTPUT_LINES);
            out.extend(lines.iter().skip(from).map(|l| format!("    {l}")));
        }
        None => out.push("  the run's output was not kept".to_string()),
    }
    out
}

/// The line for a landed test file the plan does not name, with the claim that plans it
/// or the planned file to move it into.
pub fn unplanned_test(file: &str, planned: &[String], c: &str, into: &str) -> String {
    let known: serde_json::Map<String, serde_json::Value> =
        [("criterion_id", c), ("file_path", file), ("will", "create")].iter().map(|(k, v)| (k.to_string(), serde_json::Value::from(*v))).collect();
    let claim = crate::door::claim::command_form("test_planned", &known, None).unwrap_or_default();
    format!("{LEAD}{file} is not a planned test file; the plan names {}; plan it: {claim}, or move the test into {into}", planned.join(", "))
}

/// A card's choices in the person's words, by the model's name.
const CHOICES: [(&str, &str); 13] = [
    ("re_plan", "plan it again"),
    ("keep_going", "keep going as planned"),
    ("release", "release it"),
    ("try_again", "try again"),
    ("take_over", "take it over yourself"),
    ("its_the_bug", "the crash is the bug"),
    ("a_dependency", "a dependency comes first"),
    ("fix_what_broke_it", "fix what broke the command"),
    ("covered_by_dependency", "covered by its dependency"),
    // H-4 (the owner, 2026-10-19) [draft words]
    ("its_the_red", "it is the red"),
    ("its_the_pass", "it is the pass"),
    // the one question [draft words]
    ("type_it", "I'll type it"),
    ("go_ahead", "I don't know — go ahead"),
];

/// `s` with `LEAD` in front, once.
pub fn lead(s: &str) -> String {
    if s.starts_with(LEAD) {
        s.to_string()
    } else {
        format!("{LEAD}{s}")
    }
}

/// A model by its family's word (`sonnet` for `claude-sonnet-5-5`); a model off the ladder as it is.
pub fn model(m: &str) -> &str {
    ladder::family_of(Some(m)).unwrap_or(m)
}

pub fn text_of<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    v.of("criterion", c).last().and_then(|f| f.fields.get("text")).and_then(Value::as_str).filter(|t| !t.trim().is_empty())
}

/// The criterion's own text in quotes, or its id when it has no text; the reserved part `setup`
/// is the test setup (plan A-4).
pub fn quoted(v: &View, c: &str) -> String {
    if c == familiar_ai_rules::reading::SETUP_PART {
        return crate::teach::person_words::THE_TEST_SETUP.to_string();
    }
    text_of(v, c).map_or_else(|| c.to_string(), |t| format!("\"{}\"", t.trim()))
}

/// Every `criterion <id>` or `Criterion <id>` in `s` replaced by the criterion's quoted text.
/// Longest ids go first and only whole ids match, so c1 does not match c10.
pub fn named(v: &View, s: &str) -> String {
    let mut ids = plan::criteria(v);
    ids.sort_by_key(|c| std::cmp::Reverse(c.len()));
    let mut out = s.to_string();
    for c in ids {
        for word in ["criterion", "Criterion"] {
            out = replace_whole(&out, &format!("{word} {c}"), &quoted(v, &c));
        }
    }
    out
}

/// `from` replaced by `to` wherever the next character does not continue an id.
fn replace_whole(s: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(from) {
        let after = rest.get(i + from.len()..).unwrap_or("");
        let carries_on = after.chars().next().is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-');
        out.push_str(rest.get(..i).unwrap_or(""));
        out.push_str(if carries_on { from } else { to });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A hold in the person's words; `two_failures` says who takes it next.
pub fn hold(v: &View, c: Option<&str>, why: &str) -> String {
    let spent = c.is_some_and(|c| crate::door::investigator::rung(v, c).is_none());
    if why == "two_failures" && spent {
        return TWO_FAILURES_NEEDS_YOU.to_string();
    }
    if why == "two_tries_not_red" && spent {
        return TWO_TRIES_NOT_RED_NEEDS_YOU.to_string();
    }
    HOLDS.iter().find(|(w, _)| *w == why).map_or_else(|| why.replace('_', " "), |(_, words)| (*words).to_string())
}

/// A card's choice in the person's words.
pub fn choice(ch: &str) -> String {
    CHOICES.iter().find(|(c, _)| *c == ch).map_or_else(|| ch.replace('_', " "), |(_, w)| (*w).to_string())
}

/// What a criterion is waiting on, for "not done yet".
fn not_yet_words(v: &View, c: Option<&str>, st: &str) -> String {
    match st {
        "unplanned" => "the plan comes first".into(),
        "waiting" => match c.and_then(|c| plan::string_before(v, c)) {
            Some(p) => format!("{} comes first", quoted(v, &p)),
            None => "the one before it comes first".into(),
        },
        "testing" => match c.and_then(|c| lines_now_sit(v, c)) {
            Some(at) => format!("the test comes first · {at}"),
            None => "the test comes first".into(),
        },
        // The judge's facts, when it has spoken, read from the log alone.
        "sealed" => c.and_then(|c| judge::shown(v, c, st, false)).unwrap_or_else(|| "the code comes next".into()),
        "green" => "locally green; the pipeline proves it next".into(),
        "proven" => "hosted CI-proven".into(),
        // the owner (2026-10-11): held on something else, the agent is still told whose work moved its lines
        "held" => c.map_or_else(|| "it is held".into(), |c| match lifecycle::own_work_moved(v, c).and(lines_now_sit(v, c)) {
            Some(at) => format!("{} · {at}", holds_of(v, c)),
            None => holds_of(v, c),
        }),
        other => other.replace('_', " "),
    }
}

/// Where the planned lines now sit, when the start check found the same lines at other numbers.
/// A pure shift is not a move, so the agent is only told where they are; see docs/decisions.md#the-anchor-is-a-whole-function-a-shift-is-not-a-move
/// When this plan's own recorded work changed them (`lifecycle::own_work_moved`), which work and
/// which commit (the owner, 2026-10-11: "the agent is told which commit and criterion changed them").
/// Each place is read against its own planning check, and each shifted place is named (F7).
fn lines_now_sit(v: &View, c: &str) -> Option<String> {
    let said: Vec<String> = lifecycle::place_checks(v, c)
        .into_iter()
        .filter_map(|p| place_now_sits(v, c, p.start?, p.planning?))
        .collect();
    (!said.is_empty()).then(|| said.join(" · "))
}

/// Where one place's planned lines now sit, read from its start check against its own planning check.
fn place_now_sits(v: &View, c: &str, start: &familiar_ai_facts::Fact, planning: &familiar_ai_facts::Fact) -> Option<String> {
    let n = |k: &str| start.fields.get(k).and_then(Value::as_u64);
    let file = start.fields.get("file_path").and_then(Value::as_str)?;
    if let Some(w) = lifecycle::own_work_at(v, c, start, planning) {
        let (a, b) = (n("line_start")?.to_string(), n("line_end")?.to_string());
        let words = familiar_ai_rules::terminal::own_work_words(text_of(v, &w.criterion_id).unwrap_or(&w.criterion_id), file, &a, &b);
        return Some(format!("{words} (commit {})", w.commit.chars().take(7).collect::<String>()));
    }
    let field = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).cloned();
    let lines = |f: &familiar_ai_facts::Fact| (field(f, "line_start"), field(f, "line_end"));
    let same = field(start, "matched") == Some(Value::Bool(true)) && field(start, "lines_hash") == field(planning, "lines_hash");
    if !same || lines(start) == lines(planning) {
        return None;
    }
    Some(format!("its lines now sit at {file} {}–{}", n("line_start")?, n("line_end")?))
}

/// A held criterion's holds in words, joined.
pub fn holds_of(v: &View, c: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    for w in lifecycle::whys(v, c) {
        let words = hold(v, Some(c), &w);
        if !seen.contains(&words) {
            seen.push(words);
        }
    }
    seen.join(" · ")
}

/// Where a criterion stands, in words, from the facts alone; an unjudged seal is not called waiting.
pub fn standing(v: &View, c: &str) -> String {
    standing_on(v, c, false)
}

/// Where a criterion stands, in words, the judge's word first.
/// With `judge_on` a judge is asked here, so a seal it has not read is waiting on the judge.
pub fn standing_on(v: &View, c: &str, judge_on: bool) -> String {
    let st = lifecycle::state(v, c);
    if let Some(w) = judge::shown(v, c, st, judge_on) {
        return w;
    }
    match st {
        "sealed" => "the test fails, as it should before the fix · the code comes next".into(),
        // After the way back, green carries the new stamp and the pipeline's teeth are owed.
        "green" if criterion::mark(v, c).is_some() => "green again under the new stamp · teeth on the pull request".into(),
        "green" => "locally green".into(),
        // Verified here until a hosted CI run is seen; never "proven" bare.
        "proven" if v.kind("ci_report_seen").any(|f| f.fields.get("outcome").and_then(serde_json::Value::as_str) == Some("proven")) => {
            "hosted CI-proven".into()
        }
        "proven" => "locally verified".into(),
        "waiting" => match plan::string_before(v, c) {
            Some(p) => format!("waits for {}", quoted(v, &p)),
            None => "waits its turn".into(),
        },
        st => not_yet_words(v, Some(c), st),
    }
}

/// One line per criterion whose state or judge's word changed in this call: the test result
/// when it sealed or went green, else where it stands.
pub fn state_lines(before: &Log, after: &Log, judge_on: bool) -> Vec<String> {
    let (b, v) = (View::new(before), View::new(after));
    let known = plan::criteria(&b);
    let shown = |v: &View, c: &str| {
        let st = lifecycle::state(v, c);
        (st, judge::shown(v, c, st, judge_on))
    };
    let not_red = |v: &View, c: &str| lifecycle::whys(v, c).iter().any(|w| w == "tests_not_red");
    plan::criteria(&v)
        .into_iter()
        .filter(|c| !known.contains(c) || shown(&b, c) != shown(&v, c) || (criterion::mark(&v, c).is_some() && criterion::mark(&b, c) != criterion::mark(&v, c)))
        .flat_map(|c| {
            let ran = known.contains(&c).then(|| lifecycle::state(&b, &c)) != Some(lifecycle::state(&v, &c));
            let st = lifecycle::state(&v, &c);
            // The judge's no is said on its own line, before this one.
            let tail = match judge::said(&v, &c) {
                Some(judge::Said::Covers) if st == "sealed" => judge::COVERED.to_string(),
                Some(judge::Said::Nothing) if judge_on => format!("stamped · {}", judge::WAITING),
                _ => "stamped".to_string(),
            };
            let mut lines = vec![match st {
                "sealed" if ran => format!("{LEAD}ran the tests of {}: they fail, as they should before the fix · {tail}", quoted(&v, &c)),
                "green" if ran => format!("{LEAD}ran the tests of {}: they pass · {tail}", quoted(&v, &c)),
                _ => format!("{LEAD}{} · {}", quoted(&v, &c), standing_on(&v, &c, judge_on)),
            }];
            // A seal says where the code goes, unless the judge's no means the test changes first.
            if st == "sealed" && ran && !judge::not_covered(&v, &c) {
                lines.push(format!("  the code goes on {}; the test stays as it is", anchor_words(&v, &c)));
            }
            // Tests that did not go red are told so now, with the person's words and what to write.
            if not_red(&v, &c) && !not_red(&b, &c) {
                lines.extend(judge::asked_for(after));
                lines.extend(tests_not_red_teaching(&v, Some(&c)));
            }
            lines
        })
        .collect()
}

/// The plan's state, then one line per criterion saying where it stands; `judge_on` as in `standing_on`.
pub fn status_lines(log: &Log, judge_on: bool) -> Vec<String> {
    let v = View::new(log);
    let mut out = vec![if plan::plan_state(&v) == "confirmed" {
        format!("{LEAD}the plan is confirmed")
    } else {
        format!("{LEAD}the plan is still being written")
    }];
    // a part the person cleared is left out (the owner, 2026-10-28)
    for c in plan::criteria(&v).into_iter().filter(|c| !crate::door::switch::cleared(&v, c)) {
        let sub = if plan::assigned_to(&v, &c).is_some_and(|a| a.contains('/')) { " · with a subagent" } else { "" };
        // A criterion whose runner the table cannot scope, and that names no command of its own, is unscoped.
        let scope = if !criterion::scoped(&v, &c) { " · unscoped · scope the command to prove in parallel" } else { "" };
        out.push(format!("{LEAD}{} · {}{sub}{scope}", quoted(&v, &c), standing_on(&v, &c, judge_on)));
    }
    out
}

/// A card as the person reads it in the terminal.
pub fn card(v: &View, card: &Card) -> String {
    let mut lines = vec![format!("{LEAD}a question for you · {}", named(v, &card.question))];
    lines.extend(elsewhere(v, card).into_iter().map(|l| format!("  {l}")));
    // The card says the agent may detect the stack itself, since an agent that read the card
    // as the person's alone waited for the command and never wrote the fix.
    if card.hold == "no_test_stack" {
        lines.push("  or the agent detects it: test_stack_detected is its own claim (the plan's line 7, the command in the claim it: line); no answer from the person is needed".into());
    }
    // Below the question come the person's words (`teach/person_words.rs`).
    lines.extend(crate::teach::picker::terminal(v, card));
    lines.join("\n")
}

/// A `fix_elsewhere` card's claim of where the fix lives and what it needs, from the latest claim.
pub fn elsewhere(v: &View, card: &Card) -> Vec<String> {
    if card.hold != "fix_elsewhere" {
        return Vec::new();
    }
    let Some(fl) = lifecycle::fix_location(v, &card.criterion_id) else {
        return Vec::new();
    };
    let field = |k: &str| fl.fields.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    let mut out = Vec::new();
    if let Some(w) = field("where") {
        out.push(format!("{CARD_WHERE}{w}"));
    }
    if let Some(n) = field("needs") {
        out.push(format!("{CARD_NEEDS}{n}"));
    }
    out
}

/// The model's clear words made plain: the fact kind dropped from each way out,
/// and a card's choices and the investigator's rung put in words.
pub fn clear(v: &View, words: &str) -> String {
    words
        .split("; or ")
        .map(|part| {
            let part = match part.rfind(" (") {
                Some(i) if part.ends_with(')') => part.get(..i).unwrap_or(part),
                _ => part,
            };
            if let Some(list) = part.strip_prefix("the person answers its card: ") {
                let choices: Vec<String> = list.replace(" or ", ", ").split(", ").map(choice).collect();
                return format!("the person answers its card: {}", or_list(&choices));
            }
            if let Some(rest) = part.strip_prefix("an investigator one rung up takes it: a fresh ") {
                return format!("a stronger model ({}) takes it over", rest.trim_end_matches(" subagent"));
            }
            named(v, part)
        })
        .collect::<Vec<_>>()
        .join("; or ")
}

/// The model's ways out, without the investigator's once `c`'s launches are spent.
fn without_spent_rung(v: &View, c: Option<&str>, words: &str) -> String {
    if !c.is_some_and(|c| crate::door::investigator::launches_spent(v, c)) {
        return words.to_string();
    }
    words.split("; or ").filter(|p| !p.starts_with("an investigator one rung up takes it")).collect::<Vec<_>>().join("; or ")
}

fn or_list(xs: &[String]) -> String {
    match xs.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// The model's refusal, with the binary's lines after it, in the person's words.
/// Text that is not the terminal's is led and named, its other lines kept.
pub fn refusal(log: &Log, text: &str) -> String {
    let v = View::new(log);
    let mut lines = text.lines();
    let head = lines.next().unwrap_or("");
    let parsed = head
        .strip_prefix("refused · ")
        .and_then(|r| r.split_once(" · held: ").map(|(who, h)| (who, h, true)))
        .or_else(|| head.strip_prefix("not yet · ").and_then(|r| r.rsplit_once(" · ").map(|(who, s)| (who, s, false))));
    let Some((who, what, held)) = parsed else {
        let rest: Vec<String> = lines.map(|l| kept(&v, l)).collect();
        return std::iter::once(lead(&named(&v, head))).chain(rest).collect::<Vec<_>>().join("\n");
    };
    let c = who.strip_prefix("criterion ");
    let mut next: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    for (i, l) in lines.enumerate() {
        if let Some(w) = l.strip_prefix("  to clear it: ").or_else(|| l.strip_prefix("  next: ")) {
            next = Some(w.to_string()).filter(|w| w != "—");
        } else if i == 0 || l.starts_with("  first: ") {
            // The model's words for the hold or the state, which are re-mapped above.
        } else {
            rest.push(kept(&v, l));
        }
    }
    let name = c.map(|c| format!("{} · ", quoted(&v, c))).unwrap_or_default();
    // On a seal the judge said no to, the test is what changes next.
    let next = match c {
        Some(c) if !held && what == "sealed" && judge::not_covered(&v, c) => Some(judge::CHANGE_THE_TEST.to_string()),
        _ => next.map(|n| clear(&v, &without_spent_rung(&v, c, &n))),
    };
    // A run of the part reserved and not ended: its result is told when it ends, never run again.
    let live = c.and_then(|c| crate::door::runs::runs(log).into_iter().rev().find(|r| r.criterion_id == c && r.live() && !r.cancelled()));
    let next = match (next, live) {
        (Some(n), Some(r)) if !held && n == "run its tests" => Some(format!("the tests are running (run {}); the result is told when they end: wait for it, do not run them again", r.run_id)),
        (n, _) => n,
    };
    let mut out = if held {
        let mut h = format!("{LEAD}{name}{}", hold(&v, c, what));
        if let Some(n) = &next {
            h.push_str(&format!(" · next: {n}"));
        }
        vec![h]
    } else {
        let mut h = vec![format!("{LEAD}not done yet · {name}{}", not_yet_words(&v, c, what))];
        h.extend(next.map(|n| format!("  next: {n}")));
        h
    };
    out.extend(rest);
    out.join("\n")
}

/// A line after the model's words: a claim line exactly as it is, any other with its criteria named.
fn kept(v: &View, l: &str) -> String {
    let t = l.trim_start();
    if t.starts_with("claim it:") || t.starts_with("or:") || t.starts_with("then:") {
        l.to_string()
    } else {
        named(v, l)
    }
}

/// The anchor in words: each place's file, symbol and lines, or the new work's spot.
pub fn anchor_words(v: &View, c: &str) -> String {
    let Some(a) = plan::anchor_of(v, c) else {
        return "none recorded".into();
    };
    let text = |k: &str| a.fields.get(k).and_then(Value::as_str);
    match a.kind {
        "place_chosen" => plan::places_of(a)
            .iter()
            .map(|p| match (p.line_start, p.line_end) {
                (Some(s), Some(e)) => format!("{} {} lines {s}–{e}", p.file_path, p.symbol),
                _ => format!("{} {}", p.file_path, p.symbol),
            })
            .collect::<Vec<_>>()
            .join("; "),
        "new_work_spot_chosen" => format!("new work in {} beside {}", text("file_path").unwrap_or("?"), text("beside").unwrap_or("?")),
        _ => "new work with no spot in the code yet".into(),
    }
}

/// The plan's lines, by number and name, as the rulebook lists them.
const PLAN_LINE_NAMES: [&str; 11] = [
    "the intent",
    "a criterion",
    "full path only",
    "its anchor",
    "the check",
    "its tests",
    "the command",
    "touching tests",
    "the string",
    "who holds it",
    "where it lives",
];

/// Line 3's row when the promise `c` names gives cases that are not whole: what they still want,
/// and the promise claimed again with its own slots kept.
fn contract_row(v: &View, c: &str, wants: &str, p: &crate::teach::example::Project) -> String {
    let mut known = serde_json::Map::new();
    if let Some(f) = plan::promise_of(v, c) {
        for k in ["promise_id", "behavior_id", "accepts", "gives_back", "refuses"] {
            if let Some(x) = f.fields.get(k) {
                known.insert(k.to_string(), x.clone());
            }
        }
    }
    let id = known.get("promise_id").and_then(Value::as_str).unwrap_or("").to_string();
    let form = crate::door::claim::command_form("promise", &known, Some(p)).unwrap_or_default();
    format!("promise {id}'s cases want {wants} · claim it again: {form}")
}

/// The line after the checklist: every missing line in one call, and where to read more.
pub const PLAN_ALL_AT_ONCE: &str = "  all at once: familiar-ai plan takes the missing lines as a JSON array on stdin (familiar-ai plan with nothing on stdin prints the array's form); the familiar-ai:plan skill says more";

/// The plan's eleven lines as a checklist derived from the record: each line that holds with
/// the values the facts give it, each that does not with the claim that completes it (line 5,
/// the binary's, with what it waits for). With no criterion, the lines about criteria wait for one.
pub fn plan_checklist(p: &crate::teach::example::Project) -> Vec<String> {
    use familiar_ai_rules::criterion::test_files;
    let v = View::new(p.log);
    // the parts in the plan, as its lines read them: a released part is not listed (the owner, 2026-10-28)
    let crits = plan::planned(&v);
    let any = !crits.is_empty();
    let claim = |kind: &str| crate::door::claim::command_form(kind, &serde_json::Map::new(), Some(p)).map(|c| format!("claim it: {c}")).unwrap_or_default();
    // One criterion's value as it is; several, each named.
    let per = |f: &dyn Fn(&str) -> String| match crits.as_slice() {
        [c] => f(c),
        cs => cs.iter().map(|c| format!("{}: {}", quoted(&v, c), f(c))).collect::<Vec<_>>().join("; "),
    };
    let field = |f: &familiar_ai_facts::Fact, k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let distinct = |xs: Vec<String>| {
        let mut out: Vec<String> = Vec::new();
        for x in xs {
            if !x.is_empty() && !out.contains(&x) {
                out.push(x);
            }
        }
        out
    };
    let waits = "waits for a criterion (line 2)".to_string();
    let mut rows: Vec<(bool, String)> = Vec::new();
    // 1 the intent
    rows.push(if plan::line_1(&v) {
        let text = v.last("intent_submitted").map(|i| field(i, "text")).unwrap_or_default();
        (true, format!("\"{}\"", crate::teach::person::one_line(&text)))
    } else {
        (false, crate::teach::person::intent_claim(p.log, p).map(|c| format!("claim it: {c}")).unwrap_or_default())
    });
    // 2 a criterion
    rows.push(if plan::line_2(&v) {
        (true, crits.iter().map(|c| format!("{} ({})", quoted(&v, c), plan::path_of(&v, c).unwrap_or("light"))).collect::<Vec<_>>().join(", "))
    } else if let Some(c) = crits.iter().find(|c| familiar_ai_rules::finding::criterion_owed(&v, c)) {
        // S7b: after the person's re_plan on a requirement_missed finding, the part's new reading
        (false, rescope_row(&v, c, "criterion", p))
    } else if let Some(reclaim) = crate::teach::full_path::light_not_documentation(p.log, p) {
        // the owner (2026-10-11): light is for documentation only
        (false, format!("{} · claim it: {reclaim}", familiar_ai_rules::terminal::LIGHT_NOT_DOCUMENTATION))
    } else {
        (false, claim("criterion"))
    });
    // 3 full path only
    let full: Vec<&String> = crits.iter().filter(|c| plan::path_of(&v, c) == Some("full")).collect();
    rows.push(if any && plan::line_3(&v) {
        let linked = |c: &str| {
            let crit = v.of("criterion", c).last();
            let cases = plan::cases_of(&v, c).into_iter().map(|k| k.case_id).collect::<Vec<_>>();
            let cases = if cases.is_empty() { String::new() } else { format!(" (cases {})", cases.join(", ")) };
            format!("{}: behavior {}, promise {}{cases}", quoted(&v, c), crit.map(|f| field(f, "behavior_id")).unwrap_or_default(), crit.map(|f| field(f, "promise_id")).unwrap_or_default())
        };
        (true, if full.is_empty() { "not needed on the light path".to_string() } else { full.iter().map(|c| linked(c)).collect::<Vec<_>>().join("; ") })
    } else if !any {
        (false, waits.clone())
    } else if let Some(c) = crits.iter().find(|c| familiar_ai_rules::finding::promise_owed(&v, c)) {
        (false, rescope_row(&v, c, "promise", p))
    } else if let Some((c, wants)) = full.iter().find_map(|c| plan::contract_short(&v, c).map(|w| (c.as_str(), w))) {
        // the contract [draft names]: a promise that gives cases wants a normal and an opposite one
        (false, contract_row(&v, c, &wants, p))
    } else if let Some(row) = full.iter().find_map(|c| crate::teach::cases_card::line_3(&v, c)) {
        // the cases card [draft]: the lens panel, the person's card, then the cases they asked for
        (false, row)
    } else {
        let gap = crate::teach::full_path::gap(p.log).and_then(|g| crate::teach::full_path::claim_lines(&g, p).into_iter().next());
        (false, gap.map(|l| l.trim_start().to_string()).unwrap_or_else(|| claim("promise")))
    });
    // 4 its anchor
    // Line 4 has three ways to anchor; the other two follow the first, under its value.
    let or = |kind: &str| crate::door::claim::command_form(kind, &serde_json::Map::new(), Some(p)).map(|c| format!("\n{}or:       {c}", " ".repeat(22))).unwrap_or_default();
    rows.push(if any && plan::line_4(&v) {
        (true, per(&|c| anchor_words(&v, c)))
    } else {
        (false, format!("{}{}{}", claim("place_chosen"), or("new_work_spot_chosen"), or("new_work_no_spot")))
    });
    // 5 the check, the binary's
    let check = |c: &str| {
        let Some(a) = plan::anchor_of(&v, c) else {
            return "no anchor yet".to_string();
        };
        if a.kind != "place_chosen" {
            return "no lines to check (new work)".to_string();
        }
        let shas = distinct(plan::places_of(a).iter().filter_map(|pl| plan::check_of(&v, c, pl)).map(|f| field(f, "sha").chars().take(7).collect()).collect());
        if shas.is_empty() {
            "no lines named".to_string()
        } else {
            format!("the lines matched at {}", shas.join(", "))
        }
    };
    // Lines 5 and 7 read the anchor, so they hold only once line 4 does.
    let anchored = any && plan::line_4(&v);
    rows.push(if anchored && plan::line_5(&v) {
        (true, per(&check))
    } else if !anchored {
        (false, "waits for line 4: the binary checks the anchor's lines once they are claimed".to_string())
    } else {
        let unmatched = crits.iter().find_map(|c| {
            let a = plan::anchor_of(&v, c)?;
            plan::places_of(a).iter().filter_map(|pl| plan::check_of(&v, c, pl)).find(|f| f.fields.get("matched") != Some(&Value::Bool(true))).map(|f| (field(f, "file_path"), field(f, "symbol"), field(f, "sha")))
        });
        (false, match unmatched {
            Some((file, symbol, sha)) => format!("the binary did not find {symbol} at the lines named in {file} at {}; the anchor names the lines where the code is", sha.chars().take(7).collect::<String>()),
            None => "waits for the binary's check of the anchor's lines at the next commit seen".to_string(),
        })
    });
    // 6 its tests
    rows.push(if any && plan::line_6(&v) { (true, per(&|c| test_files(&v, c).join(", "))) } else { (false, claim("test_planned")) });
    // 7 the command; before the anchor a command claimed is all the test needs, so it waits for line 4
    let waits = !anchored && v.kind("test_stack_detected").any(plan::has_command);
    rows.push(if anchored && plan::line_7(&v) {
        (true, distinct(v.kind("test_stack_detected").filter(|f| plan::has_command(f)).map(|f| field(f, "test_command")).collect()).join("; "))
    } else { (false, if waits { "waits for line 4: a command is claimed; it must cover the anchor's files once they are claimed".to_string() } else { claim("test_stack_detected") }) });
    // 8 touching tests
    let touching = distinct(v.kind("touching_test_declared").map(|f| format!("{} {}", field(f, "file_path"), field(f, "test_name")).trim().to_string()).collect());
    let touching = if touching.is_empty() { "none declared; none is allowed".to_string() } else { touching.join(", ") };
    rows.push((any && plan::line_8(&v), touching));
    // 9 the string
    rows.push(if any && plan::line_9(&v) {
        (true, crits.iter().map(|c| quoted(&v, c)).collect::<Vec<_>>().join(" → "))
    } else {
        (false, claim(if crits.iter().any(|c| plan::on_string(&v, c)) { "criterion_strung_on_head" } else { "criterion_head" }))
    });
    // 10 who holds it
    rows.push(if any && plan::line_10(&v) {
        (true, format!("held by {}", distinct(crits.iter().map(|c| plan::assigned_to(&v, c).unwrap_or("").to_string()).collect()).join(", ")))
    } else {
        (false, claim("criterion_assigned"))
    });
    // 11 where it lives
    let lives = |c: &str| match lifecycle::fix_location(&v, c) {
        Some(f) if field(f, "category") == "this_repository" => "this repository".to_string(),
        Some(f) => format!("{}: {}", field(f, "category"), field(f, "where")),
        None => String::new(),
    };
    rows.push(if any && plan::line_11(&v) { (true, per(&lives)) } else { (false, claim("fix_location_category")) });
    let held = rows.iter().filter(|(h, _)| *h).count();
    let mut out = vec![format!("  the plan's eleven lines, {held} of 11 hold:")];
    for (i, ((h, value), name)) in rows.iter().zip(PLAN_LINE_NAMES).enumerate() {
        out.push(format!("  {}{:>2} {name:<16}{value}", if *h { "✓" } else { " " }, i + 1));
    }
    out.push(PLAN_ALL_AT_ONCE.to_string());
    out
}

/// `text` with the lines of the plan's checklist that hold left out: the header (with "N of 11
/// hold"), each line still missing with its claim, and the all-at-once line stay. A repeat reply
/// of one planning (F15, finding 35) is this; the first, and the hook's refusal, keep every line.
pub fn only_missing(text: &str) -> String {
    let mut inside = false;
    let mut kept: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with("  the plan's eleven lines, ") {
            inside = true;
        } else if line == PLAN_ALL_AT_ONCE {
            inside = false;
        } else if inside && line.starts_with("  ✓") {
            continue;
        }
        kept.push(line);
    }
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// What the rules tell without refusing, in the person's words. An edit outside the anchor
/// names the place and the way to move it: the person is asked, and on a yes the new place,
/// `file` (the edited file, relative to `root`), is claimed.
pub fn told(log: &Log, root: &std::path::Path, t: &str, file: Option<&str>) -> String {
    let v = View::new(log);
    if let Some(rest) = t.strip_prefix("this edit is outside the anchor of ") {
        let c = rest.split([' ', ';']).next().unwrap_or("");
        let q = quoted(&v, c);
        let head = format!("{LEAD}this edit is outside the place the plan named for {q}, {}; it goes ahead", anchor_words(&v, c));
        let Some(claim) = file.and_then(|f| new_place_claim(&v, root, c, f)) else {
            return head;
        };
        return format!("{head}\n  if the fix belongs elsewhere: ask the person with AskUserQuestion, and on a yes claim the new place, said first: {claim}; the proof of {q} starts over at the new place");
    }
    lead(&named(&v, t))
}

/// The claim of `file` as `c`'s place: its first top-level symbol and lines when it has one,
/// else the symbol and lines of the place the plan named.
fn new_place_claim(v: &View, root: &std::path::Path, c: &str, file: &str) -> Option<String> {
    let named = plan::anchor_of(v, c).filter(|a| a.kind == "place_chosen").and_then(|a| {
        plan::places_of(a).first().map(|p| (p.symbol.to_string(), p.line_start.unwrap_or(1).max(1) as u64, p.line_end.unwrap_or(1).max(1) as u64))
    });
    let places = crate::teach::example::places_of_file(root, file, named.unwrap_or_else(|| ("greet".to_string(), 10, 20)));
    let known: serde_json::Map<String, Value> = [("criterion_id".to_string(), Value::from(c)), ("places".to_string(), Value::from(places))].into_iter().collect();
    crate::door::claim::command_form("place_chosen", &known, None)
}

/// What an edit of a planned test file of a criterion not yet sealed is told before it lands:
/// what the test is for, and how it must fail first.
pub fn planned_test(log: &Log, file: &str, c: &str) -> String {
    let v = View::new(log);
    let mut out = vec![format!("{LEAD}{file} is the planned test of {}", quoted(&v, c))];
    out.extend(judge::asked_for(log));
    out.push(format!("  it must fail on the code as it is, committed alone, before any fix; familiar-ai runs {} after each commit", command_of(&v, Some(c))));
    out.join("\n")
}

/// What each outcome of a run says, by its label (plan §6.1, S8): what happened, then what
/// comes next. The words are drafts until the owner settles them.
pub const OUTCOMES: [(&str, &str, &str); 13] = [
    ("passed", "the tests pass", ""),
    ("tests_failed", "the tests fail", "the code goes on the plan's place; the test stays as it is"),
    ("missing_target_symbol", "the test command did not run", "fix the command or the environment, or the name the tests use"),
    ("collection_error", "the tests could not be collected", "fix the test file so the runner can load it"),
    ("no_tests_collected", "the command ran no tests", "point the test command at the planned test"),
    ("setup_error", "the tests' setup failed", "fix the tests' setup"),
    ("usage_error", "the test command is wrong", "fix the command"),
    ("environment_error", "the test command did not run", "fix the command or the environment"),
    ("crashed", "the tests crashed", "the person is asked whether the crash is the bug"),
    ("runner_crash", "the test runner crashed", "it runs once more"),
    ("killed", "the run was killed", "it runs once more"),
    ("timed_out", "the run took too long", "make the tests faster or narrower"),
    ("unclassified", "the run ended in a way Familiar cannot read", "the person is asked"),
];

/// The line a run's translation is told with: its outcome's words, the evidence, the failing
/// tests, what comes next, and the run's id. An unknown label reads in plain words.
pub fn outcome_line(t: &familiar_ai_facts::Fact) -> Option<String> {
    let text = |k: &str| t.fields.get(k).and_then(Value::as_str).unwrap_or("");
    let (label, run_id) = (text("label"), text("run_id"));
    if label.is_empty() || run_id.is_empty() {
        return None;
    }
    let (what, next) = OUTCOMES.iter().find(|(l, _, _)| *l == label).map_or_else(|| (label.replace('_', " "), ""), |(_, w, n)| (w.to_string(), *n));
    let mut line = format!("{LEAD}{what}");
    if !text("evidence").is_empty() {
        line.push_str(&format!(": {}", text("evidence")));
    }
    let failing: Vec<&str> = t.fields.get("failing_tests").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !failing.is_empty() {
        line.push_str(&format!(" · failing: {}", failing.join(", ")));
    }
    if !next.is_empty() {
        line.push_str(&format!(" · next: {next}"));
    }
    line.push_str(&format!(" · run {run_id}"));
    Some(line)
}

/// The line for a dry run still going when the hook answers (plan A, lane a-py) [draft words].
pub fn dry_run_started(run_id: &str, runner: Option<&str>) -> String {
    let what = runner.map_or("the dry run".to_string(), |r| format!("the dry run for {r}"));
    format!("{LEAD}{what} started (run {run_id}); its choice of how the runner is read goes in .familiar/adapter")
}

/// The line for runs still going when the hook answers (plan §1.2).
pub fn started(run_ids: &[String]) -> String {
    let runs = if run_ids.len() == 1 { format!("run {}", run_ids[0]) } else { format!("runs {}", run_ids.join(", ")) };
    format!("{LEAD}tests started — {runs}; the result is told when it ends")
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;

/// The refusal of a new test command while the person's `fix_what_broke_it` stands (plan §6.3).
pub fn command_locked(v: &View, c: &str) -> String {
    format!(
        "the test command is locked: the person chose to fix what broke it for {}; fix the code your change broke and keep the command as it is · next: commit the fix",
        quoted(v, c)
    )
}

/// What the holder does after the person's pick, by hold and choice (plan §6.7); a pick with no
/// row keeps today's line. Placeholders are filled from the log after the pick.
pub const AFTER_PICK: [(&str, &str, &str); 46] = [
    ("anchor_moved", "release", RELEASED),
    ("crashed", "its_the_bug", "{c} is sealed on the crash at {location}. Fix the code at {place}; the test stays as it is."),
    ("crashed", "a_dependency", HAND_OFF),
    ("crashed", "release", RELEASED),
    ("command_broke", "fix_what_broke_it", "`{cmd}` worked at {good commit} (run {good run}) and fails since {bad commit} (run {bad run}). Fix what your change broke; the command is locked."),
    ("command_broke", "a_dependency", HAND_OFF),
    ("command_broke", "release", RELEASED),
    ("two_failures", "try_again", TRY_AGAIN),
    ("two_failures", "a_dependency", HAND_OFF),
    ("two_failures", "release", RELEASED),
    ("two_tries_not_red", "try_again", "Try again on {c}: {after fix}.{finding} Two more tries before this returns to the person."),
    ("two_tries_not_red", "re_plan", "Plan {c} again: choose the place again. The proof starts over there.{finding}"),
    ("two_tries_not_red", "release", RELEASED),
    ("two_test_broken", "try_again", TRY_AGAIN),
    ("two_test_broken", "re_plan", RE_PLAN),
    ("two_test_broken", "a_dependency", HAND_OFF),
    ("two_test_broken", "release", RELEASED),
    ("two_crashed", "try_again", TRY_AGAIN),
    ("two_crashed", "re_plan", RE_PLAN),
    ("two_crashed", "a_dependency", HAND_OFF),
    ("two_crashed", "release", RELEASED),
    ("two_timed_out", "try_again", TRY_AGAIN),
    ("two_timed_out", "re_plan", RE_PLAN),
    ("two_timed_out", "a_dependency", HAND_OFF),
    ("two_timed_out", "release", RELEASED),
    ("environment_error", "try_again", RUN_AGAIN),
    ("environment_error", "re_plan", RE_PLAN),
    ("environment_error", "release", RELEASED),
    ("run_unclassified", "try_again", RUN_AGAIN),
    // H-4 (the owner, 2026-10-19) [draft words]: the person's word on a run Familiar couldn't read
    ("run_unclassified", "its_the_red", "{c} is sealed on the run the person read as the red, at {commit}. Fix the code at {place}; the test stays as it is."),
    ("run_unclassified", "its_the_pass", "{c} is green on the run the person read as the pass, at {commit}. Next: {next part}."),
    ("run_unclassified", "re_plan", RE_PLAN),
    ("run_unclassified", "release", RELEASED),
    ("retry_spent", "try_again", RUN_AGAIN),
    ("retry_spent", "re_plan", RE_PLAN),
    ("retry_spent", "release", RELEASED),
    ("miscoped", "covered_by_dependency", "{c} is closed as covered by {c_dep}. Next: {next part}."),
    ("miscoped", "re_plan", "Re-plan {c}: its behavior turned out to be {c_dep}'s. Choose what {c} still has to do."),
    ("miscoped", "release", RELEASED),
    ("fix_elsewhere", "release", RELEASED),
    ("anchor_untouched", "release", RELEASED),
    // the setup part's card (plan A-4) [draft words, lane a-surf]
    ("reading_gap", "keep_going", "The person chose to go on with {c} as it is: a run Familiar cannot read asks them on its own card. Carry on with the work."),
    (
        "reading_gap",
        "try_again",
        "The person changed how the tests run: a new dry run reads the runner again and writes its choice to .familiar/adapter, which is committed with the work. Carry on with the work.",
    ),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
    ("review_disputed", "keep_going", "The person keeps the fix for {c} as it is: the reviewer's no no longer stands. Next: {next part}."),
    (
        "review_disputed",
        "try_again",
        "One more review of {c}: change the code so it does what the reviewer found missing, commit it, and stop; the new commit is reviewed again, and one more no returns this to the person.",
    ),
    ("review_disputed", "release", RELEASED),
];
const RELEASED: &str = "{c} is released. Next: {next part}.";
const TRY_AGAIN: &str = "Try again on {c}: {after fix}. Two more tries before this returns to the person.";
const RE_PLAN: &str = "Plan {c} again: choose the place again. The proof starts over there.";
const RUN_AGAIN: &str = "The person cleared it: {evidence}. The run starts again at {commit}; change nothing.";
/// The line while the dependency is not drafted yet (the one-shot missed; the backstop drafts it
/// again): the pick is recorded and the dependency's plan is owed.
const HAND_OFF: &str = "A dependency comes first for {c}: its plan is owed. Plan the dependency, then {c} goes on after it.";
/// The hand-off line (plan §7.4) once the binary's batch landed.
pub const HANDED_OFF: &str = "{c_dep} was added for {c}: {text}. Its plan is complete. Do it yourself, or send it to a subagent.";

/// D16: while a dependency is drafted for `c`, another conversation's edit, commit or stop waits
/// [draft words].
pub fn drafting(v: &View, c: &str) -> String {
    format!("familiar-ai: wait · the person approved a dependency for {}, and its plan is being drafted now · next: try again in a minute; nothing else to do until it lands", quoted(v, c))
}

/// §7.4: the hand-off line for `c`'s latest approval whose dependency's lines landed.
pub fn handed_off(v: &View, c: &str) -> Option<String> {
    let a = familiar_ai_rules::dependency::approvals(v, c).into_iter().last()?;
    let d = familiar_ai_rules::dependency::draft_of(v, a.t).filter(|_| familiar_ai_rules::dependency::landed(v, a.t))?;
    let dep = d.criterion_id()?;
    let text = d.fields.get("text").and_then(Value::as_str).unwrap_or("");
    Some(HANDED_OFF.replace("{c_dep}", &quoted(v, dep)).replace("{c}", &quoted(v, c)).replace("{text}", text))
}

/// The CTA for `(hold, choice)`, read on `after` (the log after the pick); `before` is the log
/// before it, for `{next part}`. `None` when the table has no row.
pub fn after_pick(before: &View, after: &View, c: &str, hold: &str, choice: &str) -> Option<String> {
    if hold == familiar_ai_rules::cases::CARD {
        return crate::teach::cases_card::after_pick(after, c, choice);
    }
    let row = AFTER_PICK.iter().find(|(h, ch, _)| *h == hold && *ch == choice)?.2;
    if row == HAND_OFF {
        if let Some(line) = handed_off(after, c) {
            return Some(line);
        }
    }
    let run = familiar_ai_rules::outcome::card_run(after, c, hold);
    let tr = |k: &str| run.and_then(|t| t.fields.get(k)).and_then(Value::as_str).map(str::to_string);
    let location = run
        .and_then(|t| t.fields.get("location"))
        .map(|l| {
            let f = l.get("file_path").and_then(Value::as_str).unwrap_or("?");
            match l.get("line").and_then(Value::as_u64) {
                Some(n) => format!("{f}:{n}"),
                None => f.to_string(),
            }
        })
        .unwrap_or_else(|| "an unknown place".to_string());
    let evidence = tr("evidence").filter(|e| !e.trim().is_empty()).unwrap_or_else(|| "the run".to_string());
    let label = tr("label").unwrap_or_else(|| "a failure".to_string());
    let after_fix = match tr("evidence").filter(|e| !e.trim().is_empty()) {
        // the tries that did not go red passed: their wording, never a failure's
        _ if hold == "two_tries_not_red" => "its tests passed on the code as it is, before any fix; write a test that fails there".to_string(),
        Some(e) => format!("the last run read {}: {e}", label.replace('_', " ")),
        None => format!("the last run read {}", label.replace('_', " ")),
    };
    let short = |s: Option<&str>| s.map(|s| s.chars().take(7).collect::<String>()).unwrap_or_else(|| "?".to_string());
    let rid = run.and_then(|t| t.fields.get("run_id")).and_then(Value::as_str);
    let reserved = rid.and_then(|id| familiar_ai_rules::outcome::command_of_run(after, id));
    let good = rid.and_then(|id| familiar_ai_rules::outcome::good_run_before(after, id));
    let bad = good.and_then(|g| familiar_ai_rules::outcome::bad_run_after(after, g));
    fn field<'a>(f: Option<&'a familiar_ai_facts::Fact>, k: &str) -> Option<&'a str> {
        f.and_then(|f| f.fields.get(k)).and_then(Value::as_str)
    }
    let dep = familiar_ai_rules::dependency::miscoped_by(before, c).or_else(|| familiar_ai_rules::dependency::miscoped_by(after, c)).map(|d| quoted(after, &d));
    // S7b: the finding the person's answer read, from the log before it
    let finding = (hold == familiar_ai_rules::finding::HOLD)
        .then(|| familiar_ai_rules::finding::finding(before, c))
        .flatten()
        .map(|f| finding_cta(f, choice))
        .unwrap_or_default();
    let line = row
        .replace("{finding}", &finding)
        .replace("{c_dep}", &dep.unwrap_or_else(|| "its dependency".to_string()))
        .replace("{c}", &quoted(after, c))
        .replace("{location}", &location)
        .replace("{place}", &anchor_words(after, c))
        .replace("{cmd}", field(reserved, "command").unwrap_or("the test command"))
        .replace("{good commit}", &short(field(good, "commit")))
        .replace("{good run}", field(good, "run_id").unwrap_or("?"))
        .replace("{bad commit}", &short(field(bad, "commit")))
        .replace("{bad run}", field(bad, "run_id").unwrap_or("?"))
        .replace("{after fix}", &after_fix)
        .replace("{evidence}", &evidence)
        .replace("{commit}", &short(field(reserved, "commit")))
        .replace("{next part}", &next_part(before, after));
    Some(line)
}

/// A finding's category in the words the agent and the person read [draft words] (S7b).
pub fn found_words(category: &str) -> &'static str {
    match category {
        "wrong_test" => "the test checks the wrong thing",
        "behavior_exists" => "the code already does it",
        "requirement_missed" => "a requirement was missed",
        _ => "no category",
    }
}

fn field<'a>(f: &'a familiar_ai_facts::Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("").trim().trim_end_matches('.')
}

/// The finding inside a two_tries_not_red CTA, a leading space included [draft words] (S7b).
pub fn finding_cta(f: &familiar_ai_facts::Fact, choice: &str) -> String {
    match familiar_ai_rules::finding::category(f).unwrap_or("") {
        "wrong_test" => format!(" A stronger model found the test checks the wrong thing: {}.", field(f, "guidance")),
        "behavior_exists" => format!(" A stronger model found the code already does it: {}.", field(f, "reason")),
        "requirement_missed" if choice == "re_plan" => format!(
            " A stronger model found a requirement missed: {}. Before the place, claim it again in words that include it, and its promise on the full path: the plan waits for that new reading.",
            field(f, "missed")
        ),
        "requirement_missed" => format!(" A stronger model found a requirement missed: {}.", field(f, "missed")),
        _ => String::new(),
    }
}

/// The finding as the judge reads it at the next seal [draft words] (S7b).
pub fn finding_for_the_judge(f: &familiar_ai_facts::Fact) -> String {
    let reason = field(f, "reason");
    match familiar_ai_rules::finding::category(f).unwrap_or("") {
        "wrong_test" => format!("the test checked the wrong thing ({reason}); it should {}", field(f, "guidance")),
        "behavior_exists" => format!("the code already did what was asked ({reason})"),
        "requirement_missed" => format!("a requirement was missed ({reason}): {}; the part should cover {}", field(f, "missed"), field(f, "scope")),
        other => format!("{other} ({reason})"),
    }
}

/// `{next part}` (plan §6.7): the parts whose turn the pick started; else what is still in play;
/// else that the plan is complete.
pub fn next_part(before: &View, after: &View) -> String {
    let all = plan::criteria(after);
    let started: Vec<String> = all
        .iter()
        .filter(|c| !lifecycle::turn(before, c) && lifecycle::turn(after, c) && !lifecycle::released(after, c))
        .map(|c| quoted(after, c))
        .collect();
    if !started.is_empty() {
        return format!("{} {}", started.join(" and "), if started.len() == 1 { "starts" } else { "start" });
    }
    let open: Vec<String> = all
        .iter()
        .filter(|c| !lifecycle::released(after, c) && !matches!(lifecycle::state(after, c), "green" | "proven"))
        .map(|c| format!("{} ({})", quoted(after, c), lifecycle::state(after, c)))
        .collect();
    if open.is_empty() {
        "no part is left; the plan is complete".to_string()
    } else {
        format!("still in play: {}", open.join(", "))
    }
}

/// The plan row while the rescope gate holds (S7b): what was missed, and the claim that completes it.
fn rescope_row(v: &View, c: &str, kind: &str, p: &crate::teach::example::Project) -> String {
    let missed = familiar_ai_rules::finding::rescoped(v, c)
        .and_then(|a| familiar_ai_rules::finding::read_by(v, c, a))
        .map(|f| field(f, "missed").to_string())
        .unwrap_or_default();
    let mut known = serde_json::Map::new();
    let crit = v.of("criterion", c).last();
    match kind {
        "criterion" => {
            known.insert("criterion_id".into(), Value::from(c));
            for k in ["path", "behavior_id", "promise_id"] {
                if let Some(x) = crit.and_then(|f| f.fields.get(k)) {
                    known.insert(k.into(), x.clone());
                }
            }
        }
        _ => {
            if let Some(x) = crit.and_then(|f| f.fields.get("promise_id")) {
                known.insert("promise_id".into(), x.clone());
            }
            if let Some(x) = crit.and_then(|f| f.fields.get("behavior_id")) {
                known.insert("behavior_id".into(), x.clone());
            }
        }
    }
    let form = crate::door::claim::command_form(kind, &known, Some(p)).unwrap_or_default();
    format!("the person chose to plan {} again: a stronger model found a requirement missed ({missed}); claim its {kind} again in words that include it · claim it: {form}", quoted(v, c))
}
