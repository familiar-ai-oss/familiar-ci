//! The investigator's diagnosis of a test that never went red [draft words] (the owner,
//! 2026-10-16, S7b): on `two_tries_not_red` the binary runs a read-only investigator one rung
//! above the holder, never below opus (Read, Grep and Glob; no plugin, no transfer; the record
//! let go while it runs), and writes `investigator_finding` from its structured output once it
//! has read every quote again, or `investigator_unfinished` with why. The card stays the person's.

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai_facts::{append_raw, Fact, FindingRun, Log, Record};
use familiar_ai_rules::criterion::{mine, test_files};
use familiar_ai_rules::{finding, lifecycle, plan, View};
use serde_json::{json, Map, Value};

use crate::door::child::{self, Ended, Ran};
use crate::door::{cannot, FAMILIAR_FOLDER, INVESTIGATOR_LOG, TENANT};
use crate::input::Investigating;
use crate::teach::person_words::one_line;

/// How long one diagnosis may take; two fit in a PostToolUse or Stop hook's 1800 s.
pub const WAIT: Duration = Duration::from_secs(600);
/// The read-only tools the investigator has.
pub const TOOLS: &str = "Read,Grep,Glob";
/// The run id a `behavior_exists` finding gives the binary's own run at the planning commit.
pub const AT_PLANNING: &str = "at_planning";
/// How long the planned test may run at the planning commit.
const PLANNING_RUN_WAIT: Duration = Duration::from_secs(300);

/// The `--json-schema` of the finding: a category, the reason, the quotes and each category's own field.
pub const SCHEMA: &str = r#"{"type":"object","properties":{"category":{"type":"string","enum":["wrong_test","behavior_exists","requirement_missed"]},"reason":{"type":"string"},"quotes":{"type":"array","minItems":1,"items":{"type":"object","properties":{"role":{"type":"string","enum":["test","code","person"]},"file_path":{"type":"string"},"line_start":{"type":"integer"},"line_end":{"type":"integer"},"commit":{"type":"string"},"said":{"type":"integer"},"text":{"type":"string"}},"required":["role","text"],"additionalProperties":false}},"guidance":{"type":"string"},"missed":{"type":"string"},"scope":{"type":"string"}},"required":["category","reason","quotes"],"additionalProperties":false}"#;

/// A diagnosis about to run: the facts it was built from are read, the child not yet run.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosis {
    pub criterion_id: String,
    /// `<holder's session>/<session_id>`, the conversation an `investigator_unfinished` names.
    pub conversation: String,
    pub session_id: String,
    pub alias: &'static str,
    pub prompt: String,
    /// The two runs the brief shows, as the finding records them.
    pub runs: Vec<FindingRun>,
    /// The commits the brief names; a quote at any other is unverified (review 2a).
    pub commits: Vec<String>,
}

/// The fewest words, or else characters, a person's quote must hold to be proof (review 2b).
pub const PERSON_QUOTE_WORDS: usize = 4;
pub const PERSON_QUOTE_CHARS: usize = 20;

/// The first criterion, in plan order, owed a diagnosis.
pub fn due(log: &Log) -> Option<String> {
    let v = View::new(log);
    plan::criteria(&v).into_iter().find(|c| finding::owed(&v, c))
}

/// The child's argv after the program's name.
pub fn args(alias: &str, budget: &str, session_id: &str) -> Vec<String> {
    [
        "-p", "--model", alias, "--output-format", "json", "--json-schema", SCHEMA, "--setting-sources", "", "--strict-mcp-config", "--tools", TOOLS, "--max-budget-usd", budget,
        "--session-id", session_id,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn text<'a>(f: &'a Fact, k: &str) -> Option<&'a str> {
    f.fields.get(k).and_then(Value::as_str)
}

/// The latest two laptop runs of `c`, oldest first.
fn two_runs<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    let mut rs: Vec<&Fact> = v.of("test_log_translation", c).collect();
    let from = rs.len().saturating_sub(2);
    rs.drain(..from);
    rs
}

/// The commit the planning check read `c`'s place at, else the intent's.
pub fn planning_commit(v: &View, c: &str) -> Option<String> {
    lifecycle::planning_check(v, c).and_then(|f| text(f, "sha")).or_else(|| v.last("intent_submitted").and_then(|i| text(i, "sha"))).map(str::to_string)
}

/// The commit of `c`'s tests-first try: the latest laptop stamp at a tests-only commit, else the first.
pub fn tests_commit(v: &View, c: &str) -> Option<String> {
    let stamps: Vec<&Fact> = mine(v, c, "fingerprint_stamped").into_iter().filter(|s| text(s, "ran_in") == Some("laptop")).collect();
    stamps.iter().rev().find(|s| lifecycle::tests_first_run(v, c, s)).or(stamps.first()).and_then(|s| text(s, "commit")).map(str::to_string)
}

/// `bytes` numbered from 1, each line `   n| text`.
fn numbered(bytes: &[u8], from: u64) -> String {
    String::from_utf8_lossy(bytes).lines().enumerate().map(|(i, l)| format!("{:>5}| {l}", from + i as u64)).collect::<Vec<_>>().join("\n")
}

/// The place's lines at `commit`, numbered, each place on its own.
fn place_at(v: &View, root: &Path, c: &str, commit: &str) -> String {
    let Some(a) = plan::anchor_of(v, c) else {
        return "none chosen".to_string();
    };
    match a.kind {
        "place_chosen" => plan::places_of(a)
            .iter()
            .map(|p| {
                let (s, e) = (p.line_start.unwrap_or(0), p.line_end.unwrap_or(0));
                let at = |n: i64| u64::try_from(n).unwrap_or(0);
                let lines = familiar_ai_git::file_at(root, commit, p.file_path).ok().and_then(|b| familiar_ai_git::line_bytes(&b, at(s), at(e)));
                match lines {
                    Some(l) => format!("{} · {} · lines {s}–{e}:\n{}", p.file_path, p.symbol, numbered(&l, at(s))),
                    None => format!("{} · {} · lines {s}–{e} (not readable at this commit)", p.file_path, p.symbol),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "new_work_spot_chosen" => format!("new work in {} ({}), beside {}", text(a, "file_path").unwrap_or(""), text(a, "symbol").unwrap_or(""), text(a, "beside").unwrap_or("")),
        _ => "new work with no spot in the existing code".to_string(),
    }
}

/// What the brief is built from, read from the record, git and the kept runs.
pub fn prepare(log: &Log, root: &Path, c: &str, now: f64) -> Diagnosis {
    let v = View::new(log);
    let holder = plan::assigned_to(&v, c).unwrap_or("").to_string();
    let session_id = crate::door::investigator_run::minted(&holder, c, now, log.len());
    let conversation = format!("{}/{session_id}", familiar_ai_rules::ladder::session_of(Some(&holder)).unwrap_or("none"));
    let alias = crate::door::judge::rung(log, c);
    let runs: Vec<FindingRun> = two_runs(&v, c)
        .iter()
        .map(|t| FindingRun { run_id: text(t, "run_id").unwrap_or("").to_string(), commit: text(t, "commit").unwrap_or("").to_string(), label: text(t, "label").unwrap_or("").to_string() })
        .collect();
    let planning = planning_commit(&v, c).unwrap_or_else(|| "HEAD".to_string());
    let tests = tests_commit(&v, c).unwrap_or_else(|| "HEAD".to_string());
    let crit = v.of("criterion", c).last();
    let promise = crit.and_then(|f| text(f, "promise_id")).and_then(|pid| v.kind("promise").filter(|p| text(p, "promise_id") == Some(pid)).last());
    let b = crate::teach::investigator::Diagnosing {
        criterion_id: c.to_string(),
        said: v.kind("person_said").map(|p| (p.t, text(p, "text").unwrap_or("").to_string())).collect(),
        criterion: crit.and_then(|f| text(f, "text")).unwrap_or("").to_string(),
        promise: promise.map(|p| [text(p, "accepts"), text(p, "gives_back"), text(p, "refuses")].map(|s| s.unwrap_or("").to_string())),
        planning: (planning.clone(), place_at(&v, root, c, &planning)),
        head: ("HEAD".to_string(), place_at(&v, root, c, "HEAD")),
        tests: test_files(&v, c)
            .into_iter()
            .map(|fp| {
                let shown = familiar_ai_git::file_at(root, &tests, &fp).map(|b| numbered(&b, 1)).unwrap_or_else(|_| "(not readable at this commit)".to_string());
                (fp, shown)
            })
            .collect(),
        tests_commit: tests,
        command: lifecycle::command_at(&v, c, i64::MAX).unwrap_or("none recorded").to_string(),
        runs: two_runs(&v, c)
            .iter()
            .map(|t| {
                let commit = text(t, "commit").unwrap_or("");
                let output = crate::door::kept::kept_run(root, commit);
                (
                    format!("run {} at commit {commit}: {} · {}", text(t, "run_id").unwrap_or("?"), text(t, "label").unwrap_or("?"), text(t, "evidence").unwrap_or("").trim()),
                    output,
                )
            })
            .collect(),
    };
    let mut commits: Vec<String> = vec![b.planning.0.clone(), b.tests_commit.clone(), "HEAD".to_string()];
    commits.extend(familiar_ai_git::head_commit(root).ok().flatten());
    commits.extend(runs.iter().map(|r| r.commit.clone()));
    commits.retain(|x| !x.is_empty());
    Diagnosis { criterion_id: c.to_string(), conversation, session_id, alias, prompt: crate::teach::investigator::diagnosis_brief(&b), runs, commits }
}

/// Run the investigator in the project, read-only, with the prompt on stdin, for at most `wait`.
pub fn ask(go: &Diagnosis, root: &Path, i: &Investigating, wait: Duration) -> Ran {
    let set = [("FAMILIAR_AI_INVESTIGATOR", "off"), ("FAMILIAR_AI_JUDGE", "off")];
    child::run(&i.program.to_string_lossy(), &args(go.alias, &i.budget, &go.session_id), root, &set, &go.prompt, wait)
}

/// Append the call to `.familiar/investigator.log`.
pub fn log_call(root: &Path, now: f64, go: &Diagnosis, i: &Investigating, ran: &Ran) {
    let argv: Vec<String> = std::iter::once(i.program.to_string_lossy().into_owned()).chain(args(go.alias, &i.budget, &go.session_id)).collect();
    let path = root.join(FAMILIAR_FOLDER).join(INVESTIGATOR_LOG);
    child::log(&path, &format!("at {now:.0} · diagnosis of criterion {} · {}", go.criterion_id, child::shown(&argv)), ran, "");
}

fn result_of(stdout: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(stdout.trim()).ok()? {
        Value::Array(xs) => xs.into_iter().rev().find(|x| x.get("type").and_then(Value::as_str) == Some("result")),
        one => Some(one),
    }
}

/// The answer as the investigator gave it, before its quotes are read again.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub model: String,
    pub fields: Map<String, Value>,
}

/// The answer, or why there is none: the `investigator_unfinished` word and a few words on how.
pub fn read(ran: &Ran, alias: &str) -> Result<Answer, (&'static str, String)> {
    let result = result_of(&ran.stdout);
    let subtype = result.as_ref().and_then(|r| r.get("subtype")).and_then(Value::as_str).unwrap_or("").to_string();
    let Some(out) = ran.answer() else {
        return Err(match ran.ended {
            Ended::TimedOut => ("timeout", ran.how()),
            _ if subtype.contains("budget") => ("budget", format!("{}, {subtype}", ran.how())),
            _ => ("error", ran.how()),
        });
    };
    let outer = result_of(out).ok_or(("unparseable", "exit 0, but no result object".to_string()))?;
    if subtype.contains("budget") {
        return Err(("budget", format!("exit 0, {subtype}")));
    }
    if outer.get("is_error").and_then(Value::as_bool) == Some(true) || subtype.starts_with("error_during") {
        return Err(("error", "exit 0, but Claude Code reported an error".to_string()));
    }
    let structured = outer.get("structured_output").and_then(Value::as_object).cloned().or_else(|| {
        let t = outer.get("result")?.as_str()?.trim();
        let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).map(|x| x.trim_end_matches("```")).unwrap_or(t);
        serde_json::from_str::<Value>(t.trim()).ok()?.as_object().cloned()
    });
    let Some(fields) = structured else {
        return Err(("unparseable", "exit 0, but its answer was not the one JSON object".to_string()));
    };
    let used: Vec<String> = outer.get("modelUsage").and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let model = used.iter().find(|k| familiar_ai_rules::ladder::family_of(Some(k)) == familiar_ai_rules::ladder::family_of(Some(alias))).or(used.first()).cloned().unwrap_or_else(|| alias.to_string());
    Ok(Answer { model, fields })
}

/// Lines `s..=e` of `path` at `commit` as git holds them, without the last line's ending.
pub(crate) fn lines_at(root: &Path, commit: &str, path: &str, s: u64, e: u64) -> Option<String> {
    let b = familiar_ai_git::file_at(root, commit, path).ok()?;
    let l = familiar_ai_git::line_bytes(&b, s, e)?;
    Some(String::from_utf8_lossy(&l).trim_end_matches(['\n', '\r']).to_string())
}

/// Each quote as the binary read it again, or why one was not found where it said.
pub fn verify(log: &Log, root: &Path, quotes: &[Value], named: &[String]) -> Result<Vec<Value>, String> {
    if quotes.is_empty() {
        return Err("no quote".to_string());
    }
    quotes
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let n = i + 1;
            let said = q.get("text").and_then(Value::as_str).unwrap_or("");
            match q.get("role").and_then(Value::as_str) {
                Some(role @ ("test" | "code")) => {
                    let (Some(path), Some(s), Some(e), Some(commit)) = (
                        q.get("file_path").and_then(Value::as_str),
                        q.get("line_start").and_then(Value::as_u64),
                        q.get("line_end").and_then(Value::as_u64),
                        q.get("commit").and_then(Value::as_str),
                    ) else {
                        return Err(format!("quote {n} names no file, lines and commit"));
                    };
                    // review 2a: only a commit the brief named
                    if !named.iter().any(|x| x == commit || (commit.len() >= 7 && x.starts_with(commit))) {
                        return Err(format!("quote {n}: {commit} is no commit the brief named"));
                    }
                    let read = lines_at(root, commit, path, s, e).ok_or_else(|| format!("quote {n}: {path} lines {s}–{e} not readable at {commit}"))?;
                    if !reads_as(&read, said) {
                        return Err(format!("quote {n}: {path} lines {s}–{e} at {commit} do not read as quoted"));
                    }
                    Ok(json!({ "role": role, "file_path": path, "line_start": s, "line_end": e, "commit": commit, "text": read }))
                }
                Some("person") => {
                    let t = q.get("said").and_then(Value::as_u64).ok_or_else(|| format!("quote {n} names no turn"))?;
                    let words = log.iter().find(|f| f.kind == "person_said" && f.t == t).and_then(|f| text(f, "text")).ok_or_else(|| format!("quote {n}: the person said nothing at turn {t}"))?;
                    // review 2b: long enough to be proof, and kept as the bytes the person typed
                    let want: Vec<&str> = said.split_whitespace().collect();
                    if want.len() < PERSON_QUOTE_WORDS && said.trim().chars().count() < PERSON_QUOTE_CHARS {
                        return Err(format!("quote {n}: too short to be the person's proof"));
                    }
                    let text = own_bytes(words, &want).ok_or_else(|| format!("quote {n}: the person's words at turn {t} do not hold it"))?;
                    Ok(json!({ "role": "person", "said": t, "text": text }))
                }
                _ => Err(format!("quote {n} has no role")),
            }
        })
        .collect()
}

/// Whether lines read from git are the quoted text, each whitespace run read as one space; an
/// empty quote is none. The investigator's finding and the reviewer's "no" are checked by it.
pub(crate) fn reads_as(read: &str, said: &str) -> bool {
    !said.trim().is_empty() && one_line(read) == one_line(said)
}

/// The span of `words` whose whitespace-separated words are `want`, as the bytes of `words`.
fn own_bytes(words: &str, want: &[&str]) -> Option<String> {
    if want.is_empty() {
        return None;
    }
    let spans: Vec<(usize, &str)> = words.split_whitespace().map(|w| (w.as_ptr() as usize - words.as_ptr() as usize, w)).collect();
    spans.windows(want.len()).find(|ws| ws.iter().map(|(_, w)| *w).eq(want.iter().copied())).map(|ws| {
        let (start, _) = ws[0];
        let (last, w) = ws[ws.len() - 1];
        words[start..last + w.len()].to_string()
    })
}

/// The planned test at the planning commit, in a scratch copy: the test files as the tests-first
/// commit holds them, over the code as planned.
pub fn run_at_planning(log: &Log, root: &Path, c: &str) -> Option<FindingRun> {
    let v = View::new(log);
    let planning = planning_commit(&v, c)?;
    let tests = tests_commit(&v, c)?;
    let command = lifecycle::command_at(&v, c, i64::MAX)?.to_string();
    let dir: PathBuf = std::env::temp_dir().join(format!("familiar-ai-at-planning-{}-{}", std::process::id(), log.len()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let ran = (|| {
        familiar_ai_git::scratch_at(root, &dir, &planning).ok()?;
        for fp in test_files(&v, c) {
            familiar_ai_git::file_put_back(root, &dir, &tests, &fp).ok()?;
        }
        Some(child::run("sh", &["-c".to_string(), command.clone()], &dir, &[("FAMILIAR_AI_INVESTIGATOR", "off"), ("FAMILIAR_AI_JUDGE", "off")], "", PLANNING_RUN_WAIT))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    let label = match ran.map(|r| r.ended) {
        Some(Ended::Exited(Some(0))) => "passed",
        Some(Ended::Exited(_)) => "failed",
        _ => "did_not_run",
    };
    Some(FindingRun { run_id: AT_PLANNING.to_string(), commit: planning, label: label.to_string() })
}

/// What one diagnosis came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Came {
    Found { category: String },
    Missed { why: &'static str, how: String },
    /// Another call wrote first, or nothing is owed any more.
    Nothing,
}

/// After the child: write the finding from the answer once every quote is read again, else
/// `investigator_unfinished` with why. Nothing is written when the diagnosis is no longer owed.
pub fn back(record: &Record, root: &Path, log: &mut Log, go: &Diagnosis, ran: &Ran, now: f64) -> Result<Came, String> {
    if !finding::owed(&View::new(log), &go.criterion_id) {
        return Ok(Came::Nothing);
    }
    let fact = match read(ran, go.alias).and_then(|a| to_finding(log, root, go, a)) {
        Ok(fields) => fields,
        Err((why, how)) => {
            let fields = json!({ "criterion_id": go.criterion_id, "conversation": go.conversation, "why": why });
            log.push(record.append_raw("investigator_unfinished", fields.as_object().cloned().unwrap_or_default(), now, TENANT).map_err(|e| cannot("write its record", &e))?);
            return Ok(Came::Missed { why, how });
        }
    };
    let category = fact.get("category").and_then(Value::as_str).unwrap_or("").to_string();
    log.push(record.append_raw("investigator_finding", fact, now, TENANT).map_err(|e| cannot("write its record", &e))?);
    Ok(Came::Found { category })
}

/// The finding's fields from the answer, its quotes read again; `unparseable` when they do not
/// make the kind's form, `unverified` when a quote is not where it says.
fn to_finding(log: &Log, root: &Path, go: &Diagnosis, a: Answer) -> Result<Map<String, Value>, (&'static str, String)> {
    let quotes = a.fields.get("quotes").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut fields = Map::new();
    fields.insert("criterion_id".into(), Value::from(go.criterion_id.as_str()));
    fields.insert("model".into(), Value::from(a.model));
    for k in ["category", "reason", "guidance", "missed", "scope"] {
        if let Some(x) = a.fields.get(k) {
            fields.insert(k.into(), x.clone());
        }
    }
    let mut runs: Vec<Value> = go.runs.iter().map(|r| json!({ "run_id": r.run_id, "commit": r.commit, "label": r.label })).collect();
    if a.fields.get("category").and_then(Value::as_str) == Some("behavior_exists") {
        if let Some(r) = run_at_planning(log, root, &go.criterion_id) {
            runs.push(json!({ "run_id": r.run_id, "commit": r.commit, "label": r.label }));
        }
    }
    fields.insert("runs".into(), Value::Array(runs));
    // the form first, with the quotes as given: a shape it refuses is no finding
    let mut shaped = fields.clone();
    shaped.insert("quotes".into(), Value::Array(quotes.clone()));
    append_raw(&Vec::new(), "investigator_finding", shaped, 0.0, TENANT).map_err(|e| ("unparseable", e.to_string()))?;
    let read = verify(log, root, &quotes, &go.commits).map_err(|e| ("unverified", e))?;
    fields.insert("quotes".into(), Value::Array(read));
    Ok(fields)
}

/// Run one diagnosis and log it; the caller has let the record go.
pub fn wait_for(go: &Diagnosis, root: &Path, i: &Investigating, now: f64) -> Ran {
    let ran = ask(go, root, i, WAIT.min(i.wait));
    log_call(root, now, go, i, &ran);
    ran
}
