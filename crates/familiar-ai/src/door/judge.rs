//! Ask a judge, one rung above the holder and never below Opus, whether a red
//! seal's tests answer the person's words. Each hook call asks about the oldest
//! unjudged seal, so a seal missed by its own call is still judged; after two
//! calls with no verdict the seal stands unjudged. See
//! docs/decisions.md#a-judge-one-rung-up

use std::path::Path;
use std::time::Duration;

use familiar_ai_facts::{AnswersIntent, Fact, JudgeUnanswered, JudgeUnansweredWhy, JudgeVerdict, Log, Record, TestChangeAnnounced};
use familiar_ai_rules::criterion::{sealed_at, test_files, Place};
use familiar_ai_rules::ladder::{rung_of, LADDER};
use familiar_ai_rules::{lifecycle, plan, View};
use serde_json::Value;

use crate::door::child::{self, Ended, Ran};
use crate::door::judge_answer::{read, Answer, CONTRACT_SCHEMA, PLACES_SCHEMA, SCHEMA};
use crate::door::{FAMILIAR_FOLDER, JUDGE_LOG};
use crate::input::hook_input::HookInput;
use crate::input::Surroundings;
use crate::teach::judge::{self as words, Inputs};

/// The program asked: Claude Code, as the person is logged in to it.
pub const PROGRAM: &str = "claude";

/// The judge's cap per call, in dollars; the reviewer at green has the same.
pub const BUDGET: &str = "0.50";

/// Argv after `--model <alias>`: a JSON schema Claude Code validates, no
/// settings, MCP servers or tools, and a 50 cent cap.
pub const LEAN: [&str; 11] =
    ["--output-format", "json", "--json-schema", SCHEMA, "--setting-sources", "", "--strict-mcp-config", "--tools", "", "--max-budget-usd", BUDGET];

pub fn args(alias: &str) -> Vec<&str> {
    let mut a = vec!["-p", "--model", alias];
    a.extend(LEAN);
    a
}

/// `args` with `schema` in place of `SCHEMA`: the contract's when the criterion's promise gives
/// cases [draft names].
pub fn args_with<'a>(alias: &'a str, schema: &'a str) -> Vec<&'a str> {
    args(alias).into_iter().map(|a| if a == SCHEMA { schema } else { a }).collect()
}

/// What to tell after the judge: a line for the agent and one for the person.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Told {
    pub context: Option<String>,
    pub message: Option<String>,
}

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

/// The judge never runs below Opus.
pub const FLOOR: &str = "opus";

/// The judge's model for `c`: one rung above the holder, at least `FLOOR`; an
/// unknown or off-ladder holder gets Fable.
pub fn rung(log: &Log, c: &str) -> &'static str {
    let v = View::new(log);
    let at = |w: &str| LADDER.iter().position(|x| *x == w);
    let up = rung_of(&v, plan::assigned_to(&v, c))
        .and_then(at)
        .map(|i| LADDER.get(i + 1).copied().unwrap_or("fable"))
        .unwrap_or("fable");
    match (at(up), at(FLOOR)) {
        (Some(u), Some(f)) if u < f => FLOOR,
        _ => up,
    }
}

/// The oldest latest red seal of an unreleased criterion with no verdict and
/// fewer than `words::TWICE` misses.
pub fn unjudged(log: &Log) -> Option<(String, usize)> {
    let v = View::new(log);
    plan::criteria(&v)
        .into_iter()
        .filter(|c| !lifecycle::released(&v, c))
        .filter_map(|c| {
            let seal = sealed_at(&v, &c, Place::Laptop)?;
            let asked = words::verdict_after(&v, &c, seal).is_none() && words::unanswered_for(&v, &c, seal).len() < words::TWICE;
            asked.then_some((c, seal.t as usize))
        })
        .min_by_key(|(_, seal)| *seal)
}

/// On any hook call, judge the oldest unjudged seal, write the result, and
/// return what to tell. `Err` only when the record cannot be written.
pub fn backstop(record: &Record, root: &Path, log: &mut Log, input: &HookInput, around: &Surroundings) -> Result<Told, String> {
    let Some(dir) = around.judge_dir.as_deref() else {
        return Ok(Told::default()); // FAMILIAR_AI_JUDGE=off, or no `claude` to ask
    };
    let Some((c, seal)) = unjudged(log) else {
        // With no seal to judge, an announced change to an existing test the judge owes a word
        // (the owner, 2026-10-20) [draft]: one model call per hook call
        if let Some(told) = crate::door::existing::backstop(record, root, log, around, dir, PROGRAM)? {
            return Ok(told);
        }
        // With no seal to judge, a stop asks the reviewer about a green fix (step 3 of the
        // owner's plan of 2026-10-20): one model call per hook call, the judge's first.
        if input.event == crate::input::hook_input::HookEvent::Stop {
            return crate::door::review::backstop(record, root, log, input, around, dir);
        }
        return Ok(Told::default());
    };
    // When no model is seen for the holder, read it from the transcript.
    crate::door::investigator::holder_model_for_the_judge(record, input, log, around.now, std::slice::from_ref(&c))?;
    let asking = Asking { program: PROGRAM, alias: rung(log, &c), dir, wait: around.judge_wait, now: around.now };
    judge_once(record, root, log, &c, seal, &asking)
}

/// One judge call: who is asked, where, for how long, and when.
pub(crate) struct Asking<'a> {
    pub(crate) program: &'a str,
    pub(crate) alias: &'static str,
    pub(crate) dir: &'a Path,
    pub(crate) wait: Duration,
    pub(crate) now: f64,
}

/// Ask the judge once about `c`'s seal and write a verdict or a `judge_unanswered`.
fn judge_once(record: &Record, root: &Path, log: &mut Log, c: &str, seal: usize, asking: &Asking) -> Result<Told, String> {
    let (alias, now) = (asking.alias, asking.now);
    let (Some(inputs), Some(commit)) = (inputs(log, root, c, seal), commit_of(log, seal).map(str::to_string)) else {
        return Ok(Told::default());
    };
    let schema = match &inputs.contract {
        Some(k) if !k.places.is_empty() => PLACES_SCHEMA,
        Some(_) => CONTRACT_SCHEMA,
        None => SCHEMA,
    };
    let ran = ask_with(asking.program, alias, schema, &words::prompt(&inputs), asking.dir, asking.wait);
    log_call_with(root, now, c, alias, schema, &ran);
    let cannot = |e: familiar_ai_facts::RecordError| crate::door::cannot("write its record", &e);
    // Every call with no verdict is recorded with why.
    let unanswered = |why: JudgeUnansweredWhy, log: &mut Log| -> Result<(), String> {
        let fact = JudgeUnanswered { criterion_id: c.to_string(), seal: commit.clone(), why };
        log.push(record.append(&fact, now, crate::TENANT).map_err(cannot)?);
        Ok(())
    };
    let Answer { model, answers_intent, gap, reason, cases, places } = match read(&ran, alias) {
        Ok(a) => a,
        Err((why, how)) => {
            unanswered(why, log)?;
            return Ok(Told { context: None, message: Some(words::unanswered(&how)) });
        }
    };
    // the contract [draft names]: a case with no test, or one that does not check it, is a no
    let (answers_intent, gap) = words::weigh(inputs.contract.as_ref(), answers_intent, &gap, &cases);
    // places before the fix [draft names]: a not_affected reason the judge rejects is a no
    let (answers_intent, gap) = words::weigh_places(inputs.contract.as_ref(), answers_intent, &gap, &places);
    let Some(answers_intent) = answers_intent else {
        // A judge that cannot see the test is not a no.
        unanswered(JudgeUnansweredWhy::CannotSee, log)?;
        return Ok(Told { context: None, message: Some(words::unseen(&model, &reason)) });
    };
    let mut verdict = JudgeVerdict {
        criterion_id: c.to_string(),
        model: model.clone(),
        answers_intent,
        reason: reason.clone(),
        gap: Some(gap.clone()).filter(|g| !g.is_empty()),
        cases,
        places,
    };
    // the runner confirms the contract where the seal run reads each test [draft names]: a normal
    // case's named test that did not fail there is a no, sent back as the judge's own
    let mut gap = gap;
    if let Some(w) = runner_word(log, &verdict, now).filter(|_| verdict.answers_intent == AnswersIntent::Yes) {
        gap = w;
        verdict.answers_intent = AnswersIntent::No;
        verdict.gap = Some(gap.clone());
    }
    let answers_intent = verdict.answers_intent;
    log.push(record.append(&verdict, now, crate::TENANT).map_err(cannot)?);
    if answers_intent == AnswersIntent::No {
        // The judge's reason stands as the announced test change.
        let announced = TestChangeAnnounced { criterion_id: c.to_string(), reason: format!("judge: not exercised: {gap}; {reason}") };
        log.push(record.append(&announced, now, crate::TENANT).map_err(cannot)?);
    }
    // The person sees the verdict; the agent also reads the reason.
    Ok(Told { message: Some(words::verdict(&model, answers_intent, &gap)), context: Some(words::told(&model, answers_intent, &gap, &reason)) })
}

/// What the seal run says against the verdict's cases, read by the rules on the record with the
/// verdict in it: a normal case's named test that did not fail at the seal run, where that run
/// reads each test.
fn runner_word(log: &Log, verdict: &JudgeVerdict, now: f64) -> Option<String> {
    if verdict.cases.is_empty() {
        return None;
    }
    let with = familiar_ai_facts::append(log, verdict, now, crate::TENANT).ok()?;
    familiar_ai_rules::criterion::contract_unmet_at_seal(&View::new(&with), &verdict.criterion_id)
}

/// `familiar-ai judge-prompt <criterion_id>`: print the prompt the judge would
/// get for `c`'s latest seal, or exit 1 when it has none.
pub fn prompt_command(c: &str, around: &Surroundings) -> crate::Outcome {
    use crate::door::project::{open_project, plain};
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let log = match record.read_all() {
        Ok(log) => log,
        Err(e) => return plain(1, String::new(), format!("familiar-ai: {}\n", crate::door::cannot("read its record", &e))),
    };
    let v = View::new(&log);
    let seal = sealed_at(&v, c, Place::Laptop).map(|s| s.t as usize);
    match seal.and_then(|seal| inputs(&log, &root, c, seal)) {
        Some(i) => plain(0, words::prompt(&i), String::new()),
        None => plain(1, String::new(), format!("familiar-ai: criterion {c} has no seal in this record: the judge is asked at a seal\n")),
    }
}

fn commit_of(log: &Log, seal: usize) -> Option<&str> {
    View::new(log).at(seal).and_then(words::seal_of)
}

/// What the judge is shown for `c`: the tests at the seal commit and the
/// anchor's lines at its checked commit.
pub(crate) fn inputs(log: &Log, root: &Path, c: &str, seal: usize) -> Option<Inputs> {
    let v = View::new(log);
    let commit = v.at(seal).and_then(|s| text(s, "commit"))?;
    let read = |at: &str, path: &str| familiar_ai_git::file_at(root, at, path).ok();
    // Show each planned test file's test functions at the seal, not a diff that may hold none.
    let parent = format!("{commit}^");
    let tests = test_files(&v, c)
        .into_iter()
        .map(|fp| {
            let after = read(commit, &fp).unwrap_or_default();
            let before = read(&parent, &fp);
            let shown = crate::teach::sealed_test::shown(&fp, before.as_deref(), &after);
            (fp, shown)
        })
        .collect();
    let mut i = Inputs {
        criterion_id: c.to_string(),
        // The person's words are the intent; the agent's claim is only its reading.
        intent: crate::teach::person::words_and_picks(log),
        reading: crate::teach::person::agents_reading(log).map(str::to_string),
        criterion: v.of("criterion", c).last().and_then(|f| text(f, "text")).unwrap_or("").to_string(),
        anchor: anchor(&v, root, c),
        tests,
        // S7b: the latest finding before the seal, in the judge's words
        finding: v.of("investigator_finding", c).filter(|f| f.t < seal as u64).last().map(crate::teach::words::finding_for_the_judge),
        asked_before: asked_before(&v, c, seal),
        contract: words::contract_of(&v, c).map(|mut k| {
            for p in &mut k.places {
                p.code = read(commit, &p.file).map(|b| place_code(&b, p.line)).unwrap_or_default();
            }
            k
        }),
        precedents: Vec::new(),
    };
    // precedents (ports 2026-10-24, piece 4) [draft names]: at the seal commit, against the cut the judge reads
    i.precedents = crate::door::precedents::listed(&v, root, c, commit, &words::reading(&i));
    Some(i)
}

/// The most lines of a place's code shown to the judge.
const MOST_PLACE_LINES: usize = 30;

/// A place's code as the judge reads it, numbered: the definition the line is in, from its own
/// line to its end, or the lines around it outside any, at most `MOST_PLACE_LINES` around it.
pub fn place_code(bytes: &[u8], line: u64) -> String {
    let lines: Vec<String> = familiar_ai_git::split_lines(bytes).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    let Some(at) = usize::try_from(line).ok().and_then(|n| n.checked_sub(1)).filter(|&i| i < lines.len()) else {
        return String::new();
    };
    let (from, to) = match crate::door::siblings::def_line_of(&lines, at) {
        Some(d) => (d, crate::teach::review::block_end(&lines, d).max(at)),
        None => (at.saturating_sub(3), (at + 3).min(lines.len() - 1)),
    };
    let half = MOST_PLACE_LINES / 2;
    let from = from.max(at.saturating_sub(half));
    let to = to.min(from + MOST_PLACE_LINES - 1);
    let mut to = to;
    while to > at && lines[to].trim().is_empty() {
        to -= 1;
    }
    (from..=to).map(|i| format!("{:>5}| {}", i + 1, lines[i])).collect::<Vec<_>>().join("\n") + "\n"
}

/// Each gap an earlier "no" for `c` named before the seal at `seal`, once, oldest first.
fn asked_before(v: &View, c: &str, seal: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in v.of("judge_verdict", c).filter(|f| f.t < seal as u64 && text(f, "answers_intent") == Some("no")) {
        let gap = text(f, "gap").map(|g| g.trim().trim_end_matches('.').to_string()).unwrap_or_default();
        if !gap.is_empty() && !out.contains(&gap) {
            out.push(gap);
        }
    }
    out
}

/// The anchor in words, each place followed by its lines as checked.
fn anchor(v: &View, root: &Path, c: &str) -> String {
    let Some(a) = plan::anchor_of(v, c) else {
        return "none chosen".to_string();
    };
    match a.kind {
        "place_chosen" => plan::places_of(a)
            .iter()
            .map(|p| {
                let (s, e) = (p.line_start.unwrap_or(0), p.line_end.unwrap_or(0));
                let sha = plan::check_of(v, c, p)
                    .and_then(|ch| text(ch, "sha"))
                    .or_else(|| v.last("intent_submitted").and_then(|i| text(i, "sha")))
                    .unwrap_or("");
                let lines = familiar_ai_git::file_at(root, sha, p.file_path)
                    .ok()
                    .and_then(|b| familiar_ai_git::line_bytes(&b, u64::try_from(s).unwrap_or(0), u64::try_from(e).unwrap_or(0)))
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                // a pytest part's place was picked from its try's list (the owner's plan of 2026-10-23) [draft]
                let listed = match familiar_ai_rules::trace::red_try(v, c) {
                    None => String::new(),
                    Some(_) => match familiar_ai_rules::trace::rank_of(v, c, p) {
                        Some(r) => format!(" · rank {r} on the try's list"),
                        None => " · not on the try's list".to_string(),
                    },
                };
                format!("{} · {} · lines {s}–{e}{listed}\n{lines}", p.file_path, p.symbol)
            })
            .chain(familiar_ai_rules::trace::off_list_reason(v, c).and_then(|f| text(f, "reason")).map(|r| format!("the agent's reason the fix lies off the try's list: {}", r.trim())))
            .collect::<Vec<_>>()
            .join("\n"),
        "new_work_spot_chosen" => format!(
            "new work in {} ({}), beside {}",
            text(a, "file_path").unwrap_or(""),
            text(a, "symbol").unwrap_or(""),
            text(a, "beside").unwrap_or("")
        ),
        _ => "new work with no spot in the existing code".to_string(),
    }
}

/// Run the judge outside the project with the prompt on stdin, for at most
/// `wait`. `FAMILIAR_AI_JUDGE=off` stops its own hooks from asking a judge.
pub(crate) fn ask(program: &str, alias: &str, prompt: &str, dir: &Path, wait: Duration) -> Ran {
    ask_with(program, alias, SCHEMA, prompt, dir, wait)
}

/// `ask` under `schema`.
pub(crate) fn ask_with(program: &str, alias: &str, schema: &str, prompt: &str, dir: &Path, wait: Duration) -> Ran {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Ran { ended: Ended::NotStarted(format!("{}: {e}", dir.display())), wall: Duration::ZERO, stdout: String::new(), stderr: String::new() };
    }
    let argv: Vec<String> = args_with(alias, schema).into_iter().map(str::to_string).collect();
    child::run(program, &argv, dir, &[("FAMILIAR_AI_JUDGE", "off")], prompt, wait)
}

/// Append the call to `.familiar/judge.log`.
pub(crate) fn log_call(root: &Path, now: f64, c: &str, alias: &str, ran: &Ran) {
    log_call_with(root, now, c, alias, SCHEMA, ran);
}

/// `log_call` for a call under `schema`.
pub(crate) fn log_call_with(root: &Path, now: f64, c: &str, alias: &str, schema: &str, ran: &Ran) {
    let argv: Vec<String> = std::iter::once(PROGRAM).chain(args_with(alias, schema)).map(str::to_string).collect();
    let path = root.join(FAMILIAR_FOLDER).join(JUDGE_LOG);
    child::log(&path, &format!("at {now:.0} · criterion {c} · {}", child::shown(&argv)), ran, "");
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use familiar_ai_rules::sample::criterion_fixture;
    use serde_json::json;

    /// The fixture's c1 after a red try listing `top`, then `then` (kind, fields) appended.
    fn tried(top: serde_json::Value, then: &[(&str, serde_json::Value)]) -> Log {
        let mut log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
        let listed = json!({ "criterion_id": "c1", "run_id": "try-c1-1", "sha": "base", "red": true, "path_size": 2, "ranker": "words", "query": "q", "top": top });
        log = append_raw(&log, "trace_listed", listed.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default();
        for (k, f) in then {
            log = append_raw(&log, k, f.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default();
        }
        log
    }

    /// The place as the fixture chose it: (file, symbol, start, end).
    fn the_place(log: &Log) -> (String, String, i64, i64) {
        let v = View::new(log);
        let a = plan::anchor_of(&v, "c1").expect("the fixture's place");
        let p = &plan::places_of(a)[0];
        (p.file_path.to_string(), p.symbol.to_string(), p.line_start.unwrap_or(0), p.line_end.unwrap_or(0))
    }

    /// The judge reads where a pytest part's place sits on its try's list, and the agent's reason
    /// when it lies off it (the owner's plan of 2026-10-23, change E) [draft names].
    #[test]
    fn the_judge_reads_the_places_rank_on_the_try_and_the_reason_off_it() {
        let root = std::env::temp_dir();
        let (file, symbol, s, e) = the_place(&criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default());
        let on = tried(json!([{ "rank": 1, "file_path": file, "symbol": symbol, "line_start": s, "line_end": e }]), &[]);
        let shown = anchor(&View::new(&on), &root, "c1");
        assert!(shown.contains(" · rank 1 on the try's list"), "{shown}");
        assert!(!shown.contains("reason the fix lies off"), "{shown}");

        let off = tried(
            json!([{ "rank": 1, "file_path": "src/elsewhere.py", "symbol": "other", "line_start": 1, "line_end": 2 }]),
            &[("off_list_reason", json!({ "criterion_id": "c1", "reason": "the writer the test never reaches holds the bug" }))],
        );
        let shown = anchor(&View::new(&off), &root, "c1");
        assert!(shown.contains(" · not on the try's list"), "{shown}");
        assert!(shown.contains("the agent's reason the fix lies off the try's list: the writer the test never reaches holds the bug"), "{shown}");

        let untried = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
        let shown = anchor(&View::new(&untried), &root, "c1");
        assert!(!shown.contains("try's list"), "no try, nothing said of one: {shown}");
    }

    fn with_model(model: Option<&str>) -> Log {
        let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
        let Some(m) = model else { return log };
        let seen = json!({ "agent_conversation_id": "a1", "model": m });
        append_raw(&log, "model_seen", seen.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default()
    }

    /// The person's pick on the card of an ask read more than one way (the owner's plan of
    /// 2026-10-21, step 2) [draft names] reaches the judge as the person's words, after their prompt.
    #[test]
    fn the_persons_pick_of_a_reading_reaches_the_judge_as_their_words() {
        let add = |log: &Log, kind: &str, fields: serde_json::Value| append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
        let mut log = add(&Vec::new(), "person_said", json!({ "text": "make the report shorter" }));
        for f in criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default() {
            let mut m = f.fields.clone();
            if f.kind == "intent_submitted" {
                m.insert("for".into(), json!(0));
            }
            log = add(&log, f.kind, serde_json::Value::Object(m));
        }
        log = add(&log, "reading_alternatives", json!({ "readings": ["the report lists fewer rows", "the report's file is smaller on disk"] }));
        log = add(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "reading_unclear", "choice": "the report lists fewer rows", "typed": "" }));
        log = add(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": "4f1c2ab", "proof_not_tampered": "h", "ran_in": "laptop" }));
        let seal = log.len() - 1;
        let dir = std::env::temp_dir().join(format!("familiar-ai-judge-reading-{}", std::process::id()));
        let i = inputs(&log, &dir, "c1", seal).unwrap_or_else(|| panic!("inputs"));
        assert_eq!(i.intent, ["make the report shorter".to_string(), crate::teach::person::picked_words("the report lists fewer rows")]);
        assert_eq!(crate::teach::person::picked_words("x"), "asked which way they meant it, the person answered: x");
    }

    #[test]
    fn the_rung_is_one_up_from_the_holder_never_below_opus_and_fable_stays_fable() {
        // max(one above the holder, opus); the argv asks that alias
        for (holder, judge) in [
            (Some("claude-haiku-4-5-20251001"), "opus"),
            (Some("claude-sonnet-4-5"), "opus"),
            (Some("claude-opus-4-1"), "fable"),
            (Some("claude-fable-5"), "fable"),
            (None, "fable"),
            (Some("gpt-5"), "fable"),
        ] {
            let alias = rung(&with_model(holder), "c1");
            assert_eq!(alias, judge, "{holder:?}");
            assert_eq!(args(alias)[..3], ["-p", "--model", judge], "{holder:?}");
        }
    }

    #[test]
    fn a_judge_that_outlives_its_time_is_no_answer_and_every_call_is_logged() {
        let dir = std::env::temp_dir().join(format!("familiar-ai-judge-unit-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join(FAMILIAR_FOLDER));
        let slow = dir.join("slow-claude");
        let fast = dir.join("fast-claude");
        let _ = std::fs::write(&slow, "#!/bin/sh\nsleep 30\n");
        let _ = std::fs::write(&fast, "#!/bin/sh\ncat > /dev/null; echo \"$*\"; echo \"judge=$FAMILIAR_AI_JUDGE\" >&2\n");
        for f in [&slow, &fast] {
            let _ = std::process::Command::new("chmod").arg("+x").arg(f).status();
        }
        let started = std::time::Instant::now();
        let ran = ask(&slow.to_string_lossy(), "sonnet", "p", &dir, Duration::from_millis(300));
        assert_eq!((ran.ended.clone(), ran.answer()), (Ended::TimedOut, None));
        assert!(started.elapsed() < Duration::from_secs(5), "the wait is kept");
        log_call(&dir, 7.0, "c1", "sonnet", &ran);
        let ran = ask(&fast.to_string_lossy(), "sonnet", "p", &dir, Duration::from_secs(10));
        assert_eq!(
            ran.answer(),
            Some(format!("-p --model sonnet --output-format json --json-schema {SCHEMA} --setting-sources  --strict-mcp-config --tools  --max-budget-usd 0.50\n").as_str())
        );
        assert_eq!(ran.stderr, "judge=off\n", "the judge's own hooks never ask a judge");
        log_call(&dir, 8.0, "c1", "sonnet", &ran);
        let log = std::fs::read_to_string(dir.join(FAMILIAR_FOLDER).join(JUDGE_LOG)).unwrap_or_default();
        assert!(log.starts_with(&format!("--- at 7 · criterion c1 · claude -p --model sonnet --output-format json --json-schema {SCHEMA} --setting-sources '' --strict-mcp-config --tools '' --max-budget-usd 0.50 · stopped after 0 s · 0.")), "{log}");
        assert!(log.contains("--- at 8 · criterion c1 · claude -p --model sonnet") && log.contains(" · exit 0 · "), "{log}");
        assert!(log.contains("stdout (first 2000 characters):\n-p --model sonnet") && log.contains("stderr (first 2000 characters):\njudge=off\n"), "{log}");
        let gone = ask("no-such-program-familiar-ai", "sonnet", "p", &dir, Duration::from_secs(1));
        assert!(gone.how().starts_with("could not start: "), "{}", gone.how());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_call_that_yields_no_verdict_writes_judge_unanswered_with_why() {
        // a fake `claude` per why, with a short wait standing in for the limit
        let dir = std::env::temp_dir().join(format!("familiar-ai-judge-why-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join(FAMILIAR_FOLDER));
        let record = Record::open(&dir.join("record")).unwrap_or_else(|e| panic!("{e}"));
        for f in criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default() {
            let _ = record.append_raw(f.kind, f.fields, 0.0, crate::TENANT);
        }
        let stamp = json!({ "criterion_id": "c1", "commit": "4f1c2ab", "proof_not_tampered": "h", "ran_in": "laptop" });
        let seal = record.append_raw("fingerprint_stamped", stamp.as_object().cloned().unwrap_or_default(), 0.0, crate::TENANT).map(|f| f.t as usize).unwrap_or(0);
        let mut log = record.read_all().unwrap_or_default();
        let unseen = r#"{"structured_output":{"answers_intent":false,"gap":"","reason":"the test is not visible"}}"#;
        for (body, wait, why) in [
            ("sleep 30".to_string(), Duration::from_millis(300), "timeout"),
            ("cat > /dev/null; echo 'API Error: 500' >&2; exit 1".to_string(), Duration::from_secs(10), "error"),
            ("cat > /dev/null; echo 'Looking at it: {}'".to_string(), Duration::from_secs(10), "unparseable"),
            (format!("cat > /dev/null; echo '{unseen}'"), Duration::from_secs(10), "cannot_see"),
        ] {
            let fake = dir.join(format!("claude-{why}"));
            let _ = std::fs::write(&fake, format!("#!/bin/sh\n{body}\n"));
            let _ = std::process::Command::new("chmod").arg("+x").arg(&fake).status();
            let program = fake.to_string_lossy().into_owned();
            let asking = Asking { program: &program, alias: "opus", dir: &dir, wait, now: 5.0 };
            let started = std::time::Instant::now();
            let told = judge_once(&record, &dir, &mut log, "c1", seal, &asking).unwrap_or_default();
            assert!(started.elapsed() < Duration::from_secs(5), "{why}: the wait is kept");
            let last = log.last().map(|f| (f.kind, f.fields.clone()));
            let want = json!({ "criterion_id": "c1", "seal": "4f1c2ab", "why": why });
            assert_eq!(last, Some(("judge_unanswered", want.as_object().cloned().unwrap_or_default())), "{why}");
            assert!(told.message.is_some_and(|m| m.contains("not judged")), "{why}");
        }
        let written = record.read_all().unwrap_or_default().iter().filter(|f| f.kind == "judge_unanswered").count();
        assert_eq!(written, 4, "each in the record, not only in memory");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
