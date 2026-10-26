//! How the runs are read here (the adapter, plan A step 2, lane a5) [draft]: when a dry run is
//! owed, which choice reads a run at a commit, the gap card on the reserved part `setup`, and the
//! re-check of a seal after its setup changed. Pure: the binary passes its own version in.
//!
//! A choice is a `reader_chosen` fact. The runner of a stack is the binary's word: the rules never
//! derive it from the command (that table is the run crate's `details_for`), so a choice is found
//! by its root and compared by its command; one stack is in force per root, so the command names
//! the runner (lane a5's decision, noted in `.sprint/lanes/a5.notes.md`).

use familiar_ai_facts::Fact;

use crate::criterion::{sealed_at, stack_of, tests_first_commit, Place};
use crate::outcome::{family_of, translation_of, Family};
use crate::plan::{has_command, root_of};
use crate::reader::{t_of, text, texts, View, INF};

/// The reserved part that carries the project's own cards [draft].
pub const SETUP_PART: &str = "setup";
/// The gap card's hold [draft].
pub const READING_GAP: &str = "reading_gap";
/// The hold of a seal whose code no longer reads an honest red under the current setup [draft].
pub const SETUP_CHANGED: &str = "setup_changed";

/// Each probe of the dry run as the outcome it stands for, in a person's words [draft words, lane
/// a-surf]: a gap names the outcome that will ask, never the probe's id.
pub const OUTCOMES: [(&str, &str); 8] = [
    ("P1", "a plain pass"),
    ("P2", "a failed assertion"),
    ("P3", "a missing function"),
    ("P4", "a test file that cannot load"),
    ("P5", "a pass beside a skipped test"),
    ("P6", "no test selected"),
    ("P3b", "a pass once the missing function exists"),
    ("P7", "an error raised in the code"),
];

/// The outcome a probe stands for, in words; a probe the table does not know is its own id.
pub fn outcome_words(probe: &str) -> &str {
    OUTCOMES.iter().find(|(p, _)| *p == probe).map_or(probe, |(_, w)| w)
}

/// One dry run owed: the stack's root and command, and why [draft words]: `no_choice`,
/// `command_changed`, `familiar_version`, `setup_files`, `unclassified`, `versions_drift`,
/// `try_again`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunOwed {
    pub root: String,
    pub command: String,
    pub why: &'static str,
}

/// The stacks in force: for each root, its latest `test_stack_detected` with a command, in the
/// order the roots first appeared.
pub fn stacks_in_force<'a>(v: &View<'a>) -> Vec<&'a Fact> {
    let mut out: Vec<&'a Fact> = Vec::new();
    for f in v.kind("test_stack_detected").filter(|f| has_command(f)) {
        match out.iter_mut().find(|x| root_of(x) == root_of(f)) {
            Some(x) => *x = f,
            None => out.push(f),
        }
    }
    out
}

/// Whether a choice was made on this laptop: one CI made (`ran_in: pipeline`) is CI's own.
fn laptops(f: &Fact) -> bool {
    text(f, "ran_in") != Some("pipeline")
}

/// The latest laptop choice for a root.
pub fn choice_of<'a>(v: &View<'a>, root: &str) -> Option<&'a Fact> {
    v.kind("reader_chosen").filter(|f| laptops(f) && text(f, "root") == Some(root)).last()
}

/// Whether one watched pattern matches a path relative to the root: `*` and `?` stay inside one
/// folder, a `**/` matches any number of folders, none included.
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    fn seg(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => seg(&p[1..], s) || (!s.is_empty() && s[0] != b'/' && seg(p, &s[1..])),
            (Some(b'?'), Some(c)) if *c != b'/' => seg(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => seg(&p[1..], &s[1..]),
            _ => false,
        }
    }
    fn walk(p: &str, s: &str) -> bool {
        match p.strip_prefix("**/") {
            Some(rest) => walk(rest, s) || s.find('/').is_some_and(|i| walk(p, &s[i + 1..])),
            None => seg(p.as_bytes(), s.as_bytes()),
        }
    }
    walk(pattern, path)
}

/// Whether any of the files lies under the root and matches one of the watched patterns.
pub fn hits_watched(watched: &[&str], root: &str, files: &[&str]) -> bool {
    files.iter().any(|f| f.strip_prefix(root).is_some_and(|rel| watched.iter().any(|p| pattern_matches(p, rel))))
}

/// The commit facts whose content first landed after position `after` and that change a file the
/// choice watches under its root; a rebase lands no new content.
fn watched_commits_after<'a>(v: &View<'a>, choice: &Fact, after: i64) -> Vec<&'a Fact> {
    let ix = v.idx();
    let root = text(choice, "root").unwrap_or("");
    let watched = texts(choice, "watched");
    ix.keys
        .iter()
        .filter(|k| ix.first_at(Some(k)) > after)
        .filter_map(|k| ix.rep.get(k).and_then(|&i| v.at(i)))
        .filter(|f| hits_watched(&watched, root, &texts(f, "files")))
        .collect()
}

/// Whether a run was the dry run's own.
fn dry_run(v: &View, run_id: &str) -> bool {
    crate::outcome::command_of_run(v, run_id).is_some_and(|r| text(r, "by") == Some("dry_run"))
}

/// The root of the part a translation is of, through the part's stack.
fn root_of_part<'a>(v: &View<'a>, t: &Fact) -> Option<&'a str> {
    let c = t.criterion_id().filter(|c| !c.is_empty())?;
    stack_of(v, c).map(root_of)
}

/// A pass beside a skip read as a pass of a touching test (a skip is the project's, never the
/// part's to explain), so it owes no dry run, whatever the table called it.
fn reads_as_a_pass(v: &View, t: &Fact) -> bool {
    let Some(rid) = text(t, "run_id") else { return false };
    v.kind("pass_please_pass")
        .filter(|r| text(r, "run_id") == Some(rid))
        .any(|r| {
            let Some(c) = r.criterion_id().filter(|c| !c.is_empty()) else { return false };
            crate::criterion::is_touching(v, c, text(r, "file_path").unwrap_or(""))
                && crate::outcome::result_family(v, c, r) == Family::Green
        })
}

/// The dry runs owed now, one per stack in force, by its first trigger (plan A-3).
pub fn dry_run_owed(v: &View, familiar: &str) -> Vec<DryRunOwed> {
    let mut out = Vec::new();
    for st in stacks_in_force(v) {
        let root = root_of(st);
        let command = text(st, "test_command").unwrap_or("");
        let owe = |why: &'static str| DryRunOwed { root: root.to_string(), command: command.to_string(), why };
        let Some(ch) = choice_of(v, root) else {
            out.push(owe("no_choice"));
            continue;
        };
        if text(ch, "command") != Some(command) {
            out.push(owe("command_changed"));
            continue;
        }
        if text(ch, "familiar") != Some(familiar) {
            out.push(owe("familiar_version"));
            continue;
        }
        if !watched_commits_after(v, ch, t_of(ch)).is_empty() {
            out.push(owe("setup_files"));
            continue;
        }
        let later: Vec<&Fact> = v
            .kind("test_log_translation")
            .filter(|t| t_of(t) > t_of(ch) && !text(t, "run_id").is_some_and(|id| dry_run(v, id)) && root_of_part(v, t) == Some(root))
            .collect();
        if later.iter().any(|t| text(t, "label") == Some("unclassified") && !reads_as_a_pass(v, t)) {
            out.push(owe("unclassified"));
            continue;
        }
        if let Some(seen) = text(ch, "versions") {
            if later.iter().any(|t| text(t, "versions").is_some_and(|x| x != seen)) {
                out.push(owe("versions_drift"));
                continue;
            }
        }
        let again = !texts(ch, "gaps").is_empty()
            && v.of("hold_answered", SETUP_PART).any(|a| {
                t_of(a) > t_of(ch) && text(a, "hold") == Some(READING_GAP) && text(a, "choice") == Some("try_again")
            });
        if again {
            out.push(owe("try_again"));
        }
    }
    out
}

/// The choice that reads a run of `runner` under `root` at `commit`: the latest whose own commit
/// landed at or before it. A choice at a commit the record never saw (the seed's) is before every
/// commit; a commit the record never saw reads the latest choice.
pub fn reader_at<'a>(v: &View<'a>, root: &str, runner: &str, commit: &str) -> Option<&'a Fact> {
    let ix = v.idx();
    let at = ix.at_of_id.get(commit).copied().unwrap_or(INF);
    v.kind("reader_chosen")
        .filter(|f| text(f, "root") == Some(root) && text(f, "runner") == Some(runner))
        .filter(|f| text(f, "commit").and_then(|c| ix.at_of_id.get(c).copied()).unwrap_or(-1) <= at)
        .last()
}

/// The latest choices, one per stack in force, that carry gaps no answer on the gap card came
/// after (plan A-4).
pub fn gaps_open<'a>(v: &View<'a>) -> Vec<&'a Fact> {
    stacks_in_force(v)
        .into_iter()
        .filter_map(|st| choice_of(v, root_of(st)))
        .filter(|ch| !texts(ch, "gaps").is_empty())
        .filter(|ch| !v.of("hold_answered", SETUP_PART).any(|a| t_of(a) > t_of(ch) && text(a, "hold") == Some(READING_GAP)))
        .collect()
}

/// The commits after `c`'s tests-first commit that change a file its stack's choice watches: each
/// owes one re-check of the seal (plan A-5).
pub fn setup_commits<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    if v.kind("reader_chosen").next().is_none() {
        return Vec::new();
    }
    let (Some(tf), Some(st)) = (tests_first_commit(v, c), stack_of(v, c)) else {
        return Vec::new();
    };
    let Some(ch) = choice_of(v, root_of(st)) else {
        return Vec::new();
    };
    watched_commits_after(v, ch, tf.at)
}

/// Whether a reserved run is a re-check (it names the setup it puts back), and where it ran.
pub fn recheck_place(r: &Fact) -> Option<Place> {
    text(r, "setup_from")?;
    Some(if text(r, "by") == Some("pipeline") { Place::Pipeline } else { Place::Laptop })
}

/// The family of the latest re-check of `c`'s tests-first commit with the setup of commit `x` at
/// that place, or `None` while none was read.
pub fn recheck_family(v: &View, c: &str, x: &Fact, place: Place) -> Option<Family> {
    let ix = v.idx();
    let tf = tests_first_commit(v, c)?;
    let xk = text(x, "commit").and_then(|id| ix.key_of(id))?;
    v.of("run_reserved", c)
        .filter(|r| {
            recheck_place(r) == Some(place)
                && text(r, "setup_from").and_then(|id| ix.key_of(id)) == Some(xk)
                && text(r, "commit").and_then(|id| ix.key_of(id)) == Some(tf.key.as_str())
        })
        .filter_map(|r| translation_of(v, text(r, "run_id")?))
        .last()
        .map(|t| family_of(v, t, c))
}

/// The commit id to put back for one setup commit: its latest landing.
fn id_of(v: &View, x: &Fact) -> String {
    let ix = v.idx();
    let id = text(x, "commit").unwrap_or("");
    ix.key_of(id).and_then(|k| ix.ids.get(k)).and_then(|ids| ids.last()).cloned().unwrap_or_else(|| id.to_string())
}

/// The setup commits whose re-check at that place is owed: none before the seal there.
pub fn rechecks_owed_at(v: &View, c: &str, place: Place) -> Vec<String> {
    if sealed_at(v, c, place).is_none() {
        return Vec::new();
    }
    setup_commits(v, c).into_iter().filter(|x| recheck_family(v, c, x, place).is_none()).map(|x| id_of(v, x)).collect()
}

/// The setup commits whose laptop re-check is owed (plan A-5): green waits while any is.
pub fn setup_rechecks_owed(v: &View, c: &str) -> Vec<String> {
    rechecks_owed_at(v, c, Place::Laptop)
}

/// Whether a re-check at either place read anything but an honest red: the seal does not stand
/// under the setup it was re-read under, until the person answers.
pub fn setup_changed(v: &View, c: &str) -> bool {
    setup_commits(v, c).iter().any(|x| {
        [Place::Laptop, Place::Pipeline]
            .iter()
            .any(|p| recheck_family(v, c, x, *p).is_some_and(|f| f != Family::ValidRed))
    })
}
