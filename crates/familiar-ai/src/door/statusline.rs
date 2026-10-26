//! `familiar-ai statusline`, the project's `statusLine` command: one line with
//! each of the person's asks in their words and a map of its parts by number
//! and state, never ids or CI, then how the test runs are read. It only reads the record; nothing is written
//! and no judge, investigator or `gh` is asked.

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai_facts::{Log, Record, RecordError};
use familiar_ai_rules::{lifecycle, plan, terminal, View};
use serde_json::Value;

use crate::Outcome;
use crate::door::project::{find_project, plain, start_dir};
use crate::door::{FAMILIAR_FOLDER, RECORD_FILE};
use crate::input::Surroundings;
use crate::teach::judge::{self, Said};
use crate::teach::person_words::{self as pw, Ask};

/// The longest the line may be, in characters.
pub const MOST: usize = 200;
/// The setting `setup` adds.
pub const COMMAND: &str = "familiar-ai statusline";

/// Each state's words on the map, for one part and for a run of them.
const WORDS: [(&str, &str, &str); 9] = [
    ("needs you", "needs you", "need you"),
    ("running", "running", "running"),
    ("waiting", "waits", "wait"),
    ("unplanned", "planning", "planning"),
    ("testing", "testing", "testing"),
    ("sealed", "sealed", "sealed"),
    ("green", "green", "green"),
    ("proven", "proven", "proven"),
    ("held", "held", "held"),
];

/// Print the line, or nothing outside a repository. It always exits 0, so the
/// person's bar never shows an error.
pub fn run_statusline(input: &str, around: &Surroundings) -> Outcome {
    let start = from_input(input).unwrap_or_else(|| start_dir(around));
    plain(0, shown(&start, around.judge_dir.is_some(), around.record_wait).map(|l| format!("{l}\n")).unwrap_or_default(), String::new())
}

/// The session's project folder from Claude Code's JSON, else its current one.
fn from_input(input: &str) -> Option<PathBuf> {
    let v: Value = serde_json::from_str(input).ok()?;
    ["/workspace/project_dir", "/workspace/current_dir", "/cwd"]
        .iter()
        .find_map(|p| v.pointer(p).and_then(Value::as_str).filter(|s| !s.is_empty()))
        .map(PathBuf::from)
}

/// The line for the folder `start`, or `None` outside any repository. It
/// waits its turn at the record like every command.
pub fn shown(start: &Path, judge_on: bool, wait: Duration) -> Option<String> {
    let Some(root) = find_project(start) else {
        return familiar_ai_git::worktree_root(start).ok().flatten().map(|top| format!("{} | Familiar | not set up", codebase(&top)));
    };
    let path = root.join(FAMILIAR_FOLDER).join(RECORD_FILE);
    let log = if path.is_file() {
        match Record::open_waiting(&path, wait).and_then(|r| r.read_all()) {
            Ok(log) => log,
            Err(RecordError::Busy) => return Some(format!("{} | its record is in use by another step", on(&root))),
            Err(_) => return Some(format!("{} | its record cannot be read", on(&root))),
        }
    } else {
        Vec::new()
    };
    let switch = match crate::door::switch::here(&root, &log, wait) {
        Ok(s) => s,
        Err(RecordError::Busy) => return Some(format!("{} | main's record is in use by another step", on(&root))),
        Err(_) => return Some(format!("{} | main's record cannot be read", on(&root))),
    };
    // Each other checkout's record is read with none held here.
    let others = crate::door::project::other_checkouts(&root, wait);
    Some(line_with(&log, &root, judge_on, &switch, &elsewhere(&others, judge_on)))
}

/// One entry per other checkout with a run open, each ` · <name>: <counts>`:
/// how many of its parts stand in each state, `needs you` first, then in
/// the order its parts first reach each state.
pub fn elsewhere(others: &[crate::door::project::Other], judge_on: bool) -> String {
    let mut out = String::new();
    for o in others {
        let Ok((log, _)) = &o.read else {
            continue;
        };
        if crate::teach::person::mode(log) == crate::teach::person::IDLE {
            continue;
        }
        let cards: Vec<String> = crate::door::project::relayed_cards(o, log).into_iter().map(|c| c.criterion_id).collect();
        let states = part_states(log, &cards, judge_on);
        let mut counted: Vec<(&str, usize)> = Vec::new();
        for st in states.iter().filter(|s| **s == "needs you").chain(states.iter().filter(|s| **s != "needs you")) {
            match counted.iter_mut().find(|(s, _)| s == st) {
                Some((_, n)) => *n += 1,
                None => counted.push((st, 1)),
            }
        }
        let words: Vec<String> = counted
            .iter()
            .map(|(st, n)| {
                let row = WORDS.iter().find(|r| r.0 == *st);
                format!("{n} {}", row.map_or_else(|| st.replace('_', " "), |r| if *n == 1 { r.1 } else { r.2 }.to_string()))
            })
            .collect();
        let words = if words.is_empty() { "planning".to_string() } else { words.join(" ") };
        out.push_str(&format!(" · {}: {words}", o.name));
    }
    out
}

/// Each criterion's state on the map, in plan order: a card relayed from
/// there (`cards`, its criteria) first, then a run in flight, then the rules'
/// state as the judge leaves it.
fn part_states(log: &Log, cards: &[String], judge_on: bool) -> Vec<&'static str> {
    let v = View::new(log);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    let running: Vec<String> = crate::door::runs::runs(log).into_iter().filter(|r| r.live() && !r.void(now) && !r.expired(now)).map(|r| r.criterion_id).collect();
    plan::criteria(&v)
        .iter()
        .map(|c| {
            if cards.iter().any(|x| x == c) {
                "needs you"
            } else if running.iter().any(|x| x == c) {
                "running"
            } else {
                let st = lifecycle::state(&v, c);
                // A seal the judge has not read waits, and after its no the test comes first again.
                match (st, judge::said(&v, c)) {
                    ("sealed" | "green" | "proven", Some(Said::DoesNotCover { .. })) => "testing",
                    ("sealed" | "green" | "proven", Some(Said::Nothing)) if judge_on => "waiting",
                    _ => WORDS.iter().find(|r| r.0 == st).map_or("planning", |r| r.0),
                }
            }
        })
        .collect()
}

fn codebase(root: &Path) -> String {
    root.file_name().map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The start of every line that is set up and on.
fn on(root: &Path) -> String {
    format!("{} | Familiar | ON", codebase(root))
}

/// The line for a record: each ask by its words, then its parts' map. The
/// ask's words shorten to the first sentence, then to the ask's time alone,
/// before the map gives way from the end; words are never cut mid-way.
pub fn line(log: &Log, root: &Path, judge_on: bool) -> String {
    let own = crate::door::switch::Switch { off: crate::door::switch::off_in(log), from_main: false, main_off: None };
    line_with(log, root, judge_on, &own, "")
}

/// [`line`] with the checkout's switch as it decides: in a worktree it says
/// whose switch it is (`OFF · as main`, `ON · here · main OFF`); `tail`,
/// the other checkouts' entries, goes last and first gives way.
pub fn line_with(log: &Log, root: &Path, judge_on: bool, switch: &crate::door::switch::Switch, tail: &str) -> String {
    let head = format!("{} | Familiar | {}", codebase(root), switch.bar());
    let tail = if head.chars().count() + tail.chars().count() + 40 > MOST { "" } else { tail };
    if switch.off {
        return format!("{head}{tail}");
    }
    let v = View::new(log);
    // how the runners are read (plan A), a short segment before the other checkouts
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    let reading = crate::teach::reading_segment(log, now).map(|r| format!(" · {r}")).unwrap_or_default();
    let mut asks = pw::asks(&v);
    if asks.is_empty() {
        let intent = (crate::teach::person::mode(log) != crate::teach::person::IDLE).then(|| crate::teach::person::run_intent(log)).flatten();
        let Some(intent) = intent else {
            return format!("{head} | {}{reading}{tail}", crate::teach::person::IDLE);
        };
        asks.push(pw::ask_for(&v, intent));
    }
    let known = plan::criteria(&v);
    let cards: Vec<String> = terminal::cards(&v).into_iter().map(|c| c.criterion_id).collect();
    let states = part_states(log, &cards, judge_on);
    let state = |c: &str| known.iter().position(|k| k == c).map_or("planning", |i| states[i]);
    let maps: Vec<Vec<(usize, usize, String)>> = asks
        .iter()
        .map(|a| {
            let states: Vec<&str> = a.parts.iter().filter(|c| known.contains(c)).map(|c| state(c)).collect();
            if states.is_empty() {
                return vec![(0, 0, "planning".to_string())];
            }
            fold(&states)
        })
        .collect();
    // the answers no verified approval stands behind, always shown
    let flag = crate::teach::not_verified_count(&v, &crate::teach::approved_if_answered(&v, root));
    let flagged = if flag == 0 { String::new() } else { format!(" · {flag} {}", crate::teach::ANSWERED_NOT_VERIFIED) };
    let tail = format!("{flagged}{reading}{tail}");
    let tail = tail.as_str();
    let start = format!("{head} | ");
    let room = MOST.saturating_sub(start.chars().count() + tail.chars().count());
    for words in [Words::Whole, Words::FirstSentence, Words::None] {
        let l = join(&asks, &maps, words, usize::MAX);
        if l.chars().count() <= room {
            return format!("{start}{l}{tail}");
        }
    }
    // The map gives way from the end.
    let items: usize = maps.iter().map(Vec::len).sum();
    (0..items).rev().map(|keep| join(&asks, &maps, Words::None, keep)).find(|l| l.chars().count() <= room).map(|l| format!("{start}{l}{tail}")).unwrap_or_default()
}

/// How much of an ask's words the line shows.
#[derive(Clone, Copy)]
enum Words {
    Whole,
    FirstSentence,
    None,
}

/// The ask's words through the end of its first sentence, else its first line.
fn first_sentence(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let end = (0..chars.len()).find(|&i| matches!(chars[i], '.' | '?' | '!') && chars.get(i + 1).is_none_or(|c| c.is_whitespace()));
    match end {
        Some(i) => pw::one_line(&chars[..=i].iter().collect::<String>()),
        None => pw::one_line(text.lines().next().unwrap_or("")),
    }
}

/// Consecutive parts in the same state as one run: `(first, last, words)`.
fn fold(states: &[&str]) -> Vec<(usize, usize, String)> {
    let mut out: Vec<(usize, usize, &str)> = Vec::new();
    for (i, st) in states.iter().enumerate() {
        match out.last_mut() {
            Some(last) if last.2 == *st => last.1 = i + 1,
            _ => out.push((i + 1, i + 1, st)),
        }
    }
    out.into_iter()
        .map(|(a, b, st)| {
            let row = WORDS.iter().find(|r| r.0 == st);
            let words = row.map_or_else(|| st.replace('_', " "), |r| if a == b { r.1 } else { r.2 }.to_string());
            (a, b, words)
        })
        .collect()
}

/// The asks and their maps on one line, the first `keep` map items shown
/// and the parts past them counted as `N more`.
fn join(asks: &[Ask], maps: &[Vec<(usize, usize, String)>], words: Words, keep: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut shown = 0;
    let mut more = 0;
    for (a, map) in asks.iter().zip(maps) {
        let said = match words {
            Words::Whole => pw::one_line(&a.text),
            Words::FirstSentence => first_sentence(&a.text),
            Words::None => String::new(),
        };
        parts.push(if said.is_empty() { a.heading() } else { format!("{}: {said}", a.heading()) });
        for (first, last, w) in map {
            if shown >= keep {
                more += if *first == 0 { 0 } else { last - first + 1 };
                continue;
            }
            shown += 1;
            parts.push(match (first, last) {
                (0, _) => w.clone(),
                (f, l) if f == l => format!("{f} {w}"),
                (f, l) => format!("{f}–{l} {w}"),
            });
        }
    }
    if more > 0 {
        parts.push(format!("{more} more"));
    }
    parts.join(" · ")
}

/// Add the `statusLine` setting to `.claude/settings.json` when none is set,
/// and say so in one line. The person's settings are never re-ordered, so the
/// file is written only when it reads back as the same settings plus this one.
pub fn offer(root: &Path) -> Result<String, String> {
    let folder = root.join(".claude");
    let path = folder.join("settings.json");
    let setting = format!("\"statusLine\": {{\"type\": \"command\", \"command\": \"{COMMAND}\"}}");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let new = if text.trim().is_empty() {
        format!("{{\n  {setting}\n}}\n")
    } else {
        let Ok(Value::Object(before)) = serde_json::from_str::<Value>(&text) else {
            return Ok(format!("{} is not a JSON object, so no status line was added to it", path.display()));
        };
        if let Some(there) = before.get("statusLine") {
            let ours = there.get("command").and_then(Value::as_str) == Some(COMMAND);
            return Ok(if ours {
                format!("the familiar-ai status line is already set in {}", path.display())
            } else {
                format!("a status line of your own is already set in {}; it was left as it is", path.display())
            });
        }
        let Some(i) = text.find('{') else {
            return Ok(format!("{} is not a JSON object, so no status line was added to it", path.display()));
        };
        let comma = if before.is_empty() { "" } else { "," };
        let (head, tail) = text.split_at(i + 1);
        let new = format!("{head}\n  {setting}{comma}{tail}");
        let mut expected = before.clone();
        expected.insert("statusLine".into(), serde_json::json!({ "type": "command", "command": COMMAND }));
        if serde_json::from_str::<Value>(&new).ok() != Some(Value::Object(expected)) {
            return Ok(format!("{} could not take the status line safely; it was left as it is", path.display()));
        }
        new
    };
    std::fs::create_dir_all(&folder).map_err(|e| format!("cannot make {}: {e}", folder.display()))?;
    std::fs::write(&path, new).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(format!("added the familiar-ai status line to {}", path.display()))
}

#[cfg(test)]
#[path = "statusline_tests.rs"]
mod tests;
