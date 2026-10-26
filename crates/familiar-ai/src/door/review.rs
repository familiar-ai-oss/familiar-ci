//! The reviewer at green [draft names] (the owner's plan of 2026-10-20, step 3). At a stop with no
//! seal left to judge, a part whose seal the judge covered and whose tests pass at a commit after
//! the tests-first one is read by the judge's model (one rung above the holder, never below opus,
//! the judge's cap), once per green commit and at most `judge::TWICE` times there with no verdict.
//! It sees the person's words, the criterion, the sealed test, the fix from the tests-first
//! commit to the green one and the code around each change, and it may Read and Grep a scratch
//! copy of the green commit. Its "no" is written as `fix_verdict` only once the binary found every
//! quote at its file and line (±`WINDOW`) at the green commit, through the investigator's quote
//! check; any other "no" is written not judged and never blocks. The reviewer only blocks: it
//! marks nothing done.

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai_facts::{Fact, FixCovers, FixNotJudgedWhy, FixQuote, FixVerdict, JudgeUnansweredWhy, Log, Record};
use familiar_ai_rules::criterion::{sealed_at, tests_first_commit, Place};
use familiar_ai_rules::{lifecycle, plan, review, View};
use serde_json::{Map, Value};

use crate::door::child::{self, Ran};
use crate::door::judge::{self, Asking, Told};
use crate::door::{FAMILIAR_FOLDER, JUDGE_LOG};
use crate::input::hook_input::HookInput;
use crate::input::Surroundings;
use crate::teach::judge::Said;
use crate::teach::review as words;

/// The read-only tools the reviewer has.
pub const TOOLS: &str = "Read,Grep";
/// How long one review may take; it runs at a stop, whose hook has 1800 s.
pub const WAIT: Duration = Duration::from_secs(300);
/// The lines a quote may be off by and still be found.
pub const WINDOW: u64 = 2;
/// The fewest characters other than whitespace a quote must hold to be proof.
pub const LEAST_QUOTE_CHARS: usize = 6;

/// The `--json-schema` of the answer: covers, the gap, the quotes and the reason, nothing else.
pub const SCHEMA: &str = r#"{"type":"object","properties":{"covers":{"type":"boolean"},"gap":{"type":"string"},"quotes":{"type":"array","items":{"type":"object","properties":{"file":{"type":"string"},"line":{"type":"integer"},"text":{"type":"string"}},"required":["file","line","text"],"additionalProperties":false}},"reason":{"type":"string"}},"required":["covers","gap","quotes","reason"],"additionalProperties":false}"#;

/// The child's argv after the program's name: the judge's, with read-only tools.
pub fn args(alias: &str) -> Vec<String> {
    [
        "-p", "--model", alias, "--output-format", "json", "--json-schema", SCHEMA, "--setting-sources", "", "--strict-mcp-config", "--tools", TOOLS, "--max-budget-usd",
        judge::BUDGET,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn text<'a>(f: &'a Fact, k: &str) -> Option<&'a str> {
    f.fields.get(k).and_then(Value::as_str)
}

/// A part owed a review, and the two commits the fix lies between.
#[derive(Debug, Clone, PartialEq)]
pub struct Owed {
    pub criterion_id: String,
    pub tests_first: String,
    pub green: String,
}

/// The latest commit at which `c` is green: the last landed commit whose content the rules read
/// green. Two commits of one content are each green; the later one is the fix as it stands.
pub fn green_commit(v: &View, c: &str) -> Option<String> {
    let ix = v.idx();
    v.kind("commit_seen")
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter_map(|f| text(f, "commit"))
        .find(|id| lifecycle::green_at(v, c, Place::Laptop, ix.key_of(id)))
        .map(str::to_string)
}

/// The first part, in plan order, owed a review: unreleased, green or proven, its seal covered by
/// the judge, the fix not kept by the person, and its green commit with no yes or no and fewer
/// than `judge::TWICE` reviews not judged.
pub fn owed(log: &Log) -> Option<Owed> {
    let v = View::new(log);
    plan::criteria(&v).into_iter().filter(|c| !lifecycle::released(&v, c)).find_map(|c| {
        if !matches!(lifecycle::state(&v, &c), "green" | "proven") || crate::teach::judge::said(&v, &c) != Some(Said::Covers) || review::accepted(&v, &c) {
            return None;
        }
        let green = green_commit(&v, &c)?;
        let tests_first = tests_first_commit(&v, &c)?.ids.last()?.clone();
        let here: Vec<&Fact> = review::verdicts(&v, &c).into_iter().filter(|f| text(f, "commit") == Some(green.as_str())).collect();
        let decided = here.iter().any(|f| matches!(text(f, "covers"), Some("yes" | "no")));
        (!decided && here.len() < crate::teach::judge::TWICE).then_some(Owed { criterion_id: c, tests_first, green })
    })
}

/// At a stop, review the first green fix owed a review, write the verdict, and return what to tell.
pub fn backstop(record: &Record, root: &Path, log: &mut Log, input: &HookInput, around: &Surroundings, dir: &Path) -> Result<Told, String> {
    let Some(go) = owed(log) else {
        return Ok(Told::default());
    };
    crate::door::investigator::holder_model_for_the_judge(record, input, log, around.now, std::slice::from_ref(&go.criterion_id))?;
    let asking = Asking { program: judge::PROGRAM, alias: judge::rung(log, &go.criterion_id), dir, wait: WAIT, now: around.now };
    review_once(record, root, log, &go, &asking)
}

/// What the reviewer is shown for `go`.
pub fn inputs(log: &Log, root: &Path, go: &Owed) -> Option<words::Inputs> {
    let v = View::new(log);
    let seal = sealed_at(&v, &go.criterion_id, Place::Laptop)?.t as usize;
    let judged = judge::inputs(log, root, &go.criterion_id, seal)?;
    Some(words::Inputs {
        criterion_id: judged.criterion_id,
        intent: judged.intent,
        reading: judged.reading,
        criterion: judged.criterion,
        tests: judged.tests,
        changed: changed(root, &go.tests_first, &go.green),
    })
}

/// The files changed from `from` to `to`, Familiar's own aside, each as it was and as it is.
fn changed(root: &Path, from: &str, to: &str) -> Vec<words::Changed> {
    let mut paths: Vec<String> = Vec::new();
    for id in familiar_ai_git::commits_between(root, from, to).unwrap_or_default() {
        for f in familiar_ai_git::changed(root, &id).unwrap_or_default() {
            if !paths.contains(&f.path) && !crate::door::commits::is_familiars_own(&f.path) {
                paths.push(f.path);
            }
        }
    }
    paths
        .into_iter()
        .filter_map(|path| {
            let before = familiar_ai_git::file_at(root, from, &path).ok();
            let after = familiar_ai_git::file_at(root, to, &path).ok();
            (before != after).then_some(words::Changed { path, before, after })
        })
        .collect()
}

/// Review `go` once: the child in a scratch copy of the green commit, then the verdict.
fn review_once(record: &Record, root: &Path, log: &mut Log, go: &Owed, asking: &Asking) -> Result<Told, String> {
    let Some(inputs) = inputs(log, root, go) else {
        return Ok(Told::default());
    };
    let scratch = asking.dir.join(format!("review-{}-{}", std::process::id(), log.len()));
    let ran = ask(asking, root, &go.green, &scratch, &words::prompt(&inputs));
    log_call(root, asking, go, &ran);
    let verdict = verdict_of(root, go, asking.alias, &scratch, &ran);
    let _ = std::fs::remove_dir_all(&scratch);
    let told = match (verdict.covers, &verdict.why) {
        (FixCovers::Yes, _) => words::verdict(&verdict.model, true, ""),
        (FixCovers::No, _) => words::verdict(&verdict.model, false, verdict.gap.as_deref().unwrap_or("")),
        (FixCovers::NotJudged, Some(FixNotJudgedWhy::Unverified)) => words::not_judged(verdict.detail.as_deref().unwrap_or("")),
        (FixCovers::NotJudged, _) => words::unanswered(verdict.detail.as_deref().unwrap_or("")),
    };
    log.push(record.append(&verdict, asking.now, crate::TENANT).map_err(|e| crate::door::cannot("write its record", &e))?);
    // The person sees the verdict; a "no" reaches the agent in the stop's refusal, which names it.
    Ok(Told { context: None, message: Some(told) })
}

/// Run the reviewer in a scratch copy of `green` with the prompt on stdin, for at most the wait.
fn ask(asking: &Asking, root: &Path, green: &str, scratch: &Path, prompt: &str) -> Ran {
    let _ = std::fs::remove_dir_all(scratch);
    let made = std::fs::create_dir_all(scratch).map_err(|e| format!("{}: {e}", scratch.display())).and_then(|()| familiar_ai_git::scratch_at(root, scratch, green).map_err(|e| e.to_string()));
    if let Err(e) = made {
        return Ran { ended: child::Ended::NotStarted(e), wall: Duration::ZERO, stdout: String::new(), stderr: String::new() };
    }
    child::run(asking.program, &args(asking.alias), scratch, &[("FAMILIAR_AI_JUDGE", "off"), ("FAMILIAR_AI_INVESTIGATOR", "off")], prompt, asking.wait)
}

/// Append the call to `.familiar/judge.log`.
fn log_call(root: &Path, asking: &Asking, go: &Owed, ran: &Ran) {
    let argv: Vec<String> = std::iter::once(judge::PROGRAM.to_string()).chain(args(asking.alias)).collect();
    let path = root.join(FAMILIAR_FOLDER).join(JUDGE_LOG);
    let at: String = go.green.chars().take(7).collect();
    child::log(&path, &format!("at {:.0} · review of criterion {} at {at} · {}", asking.now, go.criterion_id, child::shown(&argv)), ran, "");
}

/// The reviewer's answer as it gave it, before its quotes are read again.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub model: String,
    pub covers: bool,
    pub gap: String,
    pub quotes: Vec<Value>,
    pub reason: String,
}

/// The answer, or why there is none with a few words on how the call ended.
pub fn read(ran: &Ran, alias: &str) -> Result<Answer, (FixNotJudgedWhy, String)> {
    let why = |w: JudgeUnansweredWhy| match w {
        JudgeUnansweredWhy::Error => FixNotJudgedWhy::Error,
        JudgeUnansweredWhy::Timeout => FixNotJudgedWhy::Timeout,
        JudgeUnansweredWhy::Unparseable | JudgeUnansweredWhy::CannotSee => FixNotJudgedWhy::Unparseable,
    };
    let out = crate::door::judge_answer::answered(ran).map_err(|(w, how)| (why(w), how))?;
    let not_the_form = || (FixNotJudgedWhy::Unparseable, "exit 0, but its answer was not the one JSON object".to_string());
    let outer = crate::door::judge_answer::result_of(out).ok_or_else(not_the_form)?;
    let a: Map<String, Value> = crate::door::judge_answer::answer_object(&outer).ok_or_else(not_the_form)?;
    let (Some(covers), Some(gap), Some(quotes), Some(reason)) =
        (a.get("covers").and_then(Value::as_bool), a.get("gap").and_then(Value::as_str), a.get("quotes").and_then(Value::as_array), a.get("reason").and_then(Value::as_str))
    else {
        return Err(not_the_form());
    };
    if a.len() != 4 {
        return Err(not_the_form());
    }
    Ok(Answer { model: crate::door::judge_answer::model_of(&outer, alias), covers, gap: gap.trim().to_string(), quotes: quotes.clone(), reason: reason.trim().to_string() })
}

/// The verdict to write for the call: yes, a no whose quotes were all found, or not judged with why.
pub fn verdict_of(root: &Path, go: &Owed, alias: &str, scratch: &Path, ran: &Ran) -> FixVerdict {
    let base = |model: String, covers: FixCovers, reason: String| FixVerdict {
        criterion_id: go.criterion_id.clone(),
        commit: go.green.clone(),
        model,
        covers,
        reason,
        gap: None,
        quotes: Vec::new(),
        why: None,
        detail: None,
    };
    let a = match read(ran, alias) {
        Ok(a) => a,
        Err((why, how)) => return FixVerdict { why: Some(why), detail: Some(how), ..base(alias.to_string(), FixCovers::NotJudged, String::new()) },
    };
    let gap = Some(a.gap.clone()).filter(|g| !g.is_empty());
    if a.covers {
        return FixVerdict { gap, ..base(a.model, FixCovers::Yes, a.reason) };
    }
    let Some(gap) = gap else {
        let detail = "a no that names no gap".to_string();
        return FixVerdict { why: Some(FixNotJudgedWhy::Unparseable), detail: Some(detail), ..base(a.model, FixCovers::NotJudged, a.reason) };
    };
    match check(root, &go.green, scratch, &a.quotes) {
        Ok(quotes) => FixVerdict { gap: Some(gap), quotes, ..base(a.model, FixCovers::No, a.reason) },
        Err(detail) => FixVerdict {
            gap: Some(gap),
            quotes: a.quotes.iter().filter_map(as_given).collect(),
            why: Some(FixNotJudgedWhy::Unverified),
            detail: Some(detail),
            ..base(a.model, FixCovers::NotJudged, a.reason)
        },
    }
}

/// A quote as the reviewer gave it, for the record of an unverified "no".
fn as_given(q: &Value) -> Option<FixQuote> {
    Some(FixQuote { file_path: q.get("file")?.as_str()?.to_string(), line: q.get("line")?.as_u64()?, text: q.get("text")?.as_str()?.to_string() })
}

/// `file` as a path from the repository's root: the scratch copy's own folder and a leading `./`
/// taken off.
fn from_root(file: &str, scratch: &Path) -> String {
    let mut f = file.trim();
    let copies: Vec<PathBuf> = std::iter::once(scratch.to_path_buf()).chain(scratch.canonicalize().ok()).collect();
    for c in &copies {
        if let Some(rest) = Path::new(f).strip_prefix(c).ok().and_then(Path::to_str) {
            f = rest;
            break;
        }
    }
    f.trim_start_matches("./").to_string()
}

/// `said` without a line number a reader put before each line (`12| `, `12→`, `12<tab>`).
fn unnumbered(said: &str) -> String {
    said.lines()
        .map(|l| {
            let t = l.trim_start();
            let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let rest = &t[digits..];
            match rest.chars().next() {
                Some(c @ ('|' | '\t' | '→')) if digits > 0 => {
                    let r = &rest[c.len_utf8()..];
                    r.strip_prefix(' ').unwrap_or(r)
                }
                _ => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each quote found at its file and line, or up to `WINDOW` lines from it, at `commit`, as git
/// holds it there; or why one was not. A "no" with no quote is not proof.
pub fn check(root: &Path, commit: &str, scratch: &Path, quotes: &[Value]) -> Result<Vec<FixQuote>, String> {
    if quotes.is_empty() {
        return Err("a no with no quote".to_string());
    }
    quotes
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let n = i + 1;
            let (Some(file), Some(line), Some(said)) = (q.get("file").and_then(Value::as_str), q.get("line").and_then(Value::as_u64), q.get("text").and_then(Value::as_str)) else {
                return Err(format!("quote {n} names no file, line and text"));
            };
            let path = from_root(file, scratch);
            let said = unnumbered(said.trim_matches(['\n', '\r']));
            if said.chars().filter(|c| !c.is_whitespace()).count() < LEAST_QUOTE_CHARS {
                return Err(format!("quote {n}: {path} line {line} is too short to be proof"));
            }
            let lines = said.lines().count().max(1) as u64;
            let near = std::iter::once(line).chain((1..=WINDOW).flat_map(|d| [line.checked_sub(d), Some(line + d)]).flatten()).filter(|s| *s >= 1);
            for start in near {
                if let Some(read) = crate::door::diagnosis::lines_at(root, commit, &path, start, start + lines - 1) {
                    if crate::door::diagnosis::reads_as(&read, &said) {
                        return Ok(FixQuote { file_path: path, line: start, text: read });
                    }
                }
            }
            Err(format!("quote {n}: {path} line {line} does not read as quoted"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A repository with one commit of `src/app.py`.
    fn repo(name: &str) -> (PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("familiar-ai-review-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).expect("dir");
        std::fs::write(dir.join("src/app.py"), "import os\n\n\ndef greet(name):\n    return \"hello\"\n\n\ndef bye():\n    return \"bye\"\n").expect("write");
        let git = |args: &[&str]| {
            std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(&dir).output().expect("git")
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "one"]);
        let head = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout).trim().to_string();
        (dir, head)
    }

    fn q(file: &str, line: u64, text: &str) -> Value {
        json!({ "file": file, "line": line, "text": text })
    }

    #[test]
    fn a_quote_is_found_at_its_line_or_two_off_and_kept_as_git_holds_it() {
        let (dir, head) = repo("found");
        let scratch = dir.join("copy");
        let want = |line| Ok(vec![FixQuote { file_path: "src/app.py".into(), line, text: "def greet(name):".into() }]);
        assert_eq!(check(&dir, &head, &scratch, &[q("src/app.py", 4, "def greet(name):")]), want(4));
        assert_eq!(check(&dir, &head, &scratch, &[q("./src/app.py", 6, "  def   greet(name):  ")]), want(4), "two off, whitespace as one space");
        assert_eq!(check(&dir, &head, &scratch, &[q("src/app.py", 2, "def greet(name):")]), want(4));
        let abs = scratch.join("src/app.py");
        assert_eq!(check(&dir, &head, &scratch, &[q(&abs.to_string_lossy(), 4, "     4| def greet(name):")]), want(4), "the copy's path and a line number taken off");
        let two = check(&dir, &head, &scratch, &[q("src/app.py", 4, "def greet(name):\n    return \"hello\"")]);
        assert_eq!(two.map(|v| v[0].text.clone()), Ok("def greet(name):\n    return \"hello\"".to_string()), "a quote of two lines");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_quote_not_there_or_none_is_not_proof() {
        let (dir, head) = repo("missing");
        let scratch = dir.join("copy");
        assert_eq!(check(&dir, &head, &scratch, &[]), Err("a no with no quote".to_string()));
        assert_eq!(check(&dir, &head, &scratch, &[q("src/app.py", 7, "def greet(name):")]), Err("quote 1: src/app.py line 7 does not read as quoted".to_string()), "three off");
        assert_eq!(check(&dir, &head, &scratch, &[q("src/app.py", 4, "def greet(name):"), q("src/app.py", 5, "return \"hi\"")]), Err("quote 2: src/app.py line 5 does not read as quoted".to_string()), "every quote must be found");
        assert_eq!(check(&dir, &head, &scratch, &[q("src/nope.py", 1, "import os here")]), Err("quote 1: src/nope.py line 1 does not read as quoted".to_string()));
        assert!(check(&dir, &head, &scratch, &[q("../outside.py", 1, "import os here")]).is_err());
        assert_eq!(check(&dir, &head, &scratch, &[q("src/app.py", 1, "os")]), Err("quote 1: src/app.py line 1 is too short to be proof".to_string()));
        assert_eq!(check(&dir, &head, &scratch, &[json!({ "file": "src/app.py", "text": "import os" })]), Err("quote 1 names no file, line and text".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_argv_is_the_judges_with_read_only_tools() {
        assert_eq!(
            args("opus").join(" "),
            format!("-p --model opus --output-format json --json-schema {SCHEMA} --setting-sources  --strict-mcp-config --tools Read,Grep --max-budget-usd 0.50")
        );
    }

    #[test]
    fn an_answer_is_read_in_its_one_form() {
        let ran = |out: &str| Ran { ended: child::Ended::Exited(Some(0)), wall: Duration::ZERO, stdout: out.to_string(), stderr: String::new() };
        let wrap = |a: Value| json!({ "type": "result", "is_error": false, "result": a.to_string(), "modelUsage": { "claude-opus-4-6": {} } }).to_string();
        let a = read(&ran(&wrap(json!({ "covers": false, "gap": " g ", "quotes": [], "reason": "r" }))), "opus").expect("read");
        assert_eq!((a.model.as_str(), a.covers, a.gap.as_str()), ("claude-opus-4-6", false, "g"));
        let judge_form = wrap(json!({ "answers_intent": true, "gap": "", "reason": "r" }));
        assert_eq!(read(&ran(&judge_form), "opus").map(|_| ()), Err((FixNotJudgedWhy::Unparseable, "exit 0, but its answer was not the one JSON object".to_string())));
        let extra = wrap(json!({ "covers": true, "gap": "", "quotes": [], "reason": "r", "more": 1 }));
        assert!(read(&ran(&extra), "opus").is_err());
        let timed_out = Ran { ended: child::Ended::TimedOut, wall: Duration::ZERO, stdout: String::new(), stderr: String::new() };
        assert_eq!(read(&timed_out, "opus").map_err(|e| e.0), Err(FixNotJudgedWhy::Timeout));
    }
}
