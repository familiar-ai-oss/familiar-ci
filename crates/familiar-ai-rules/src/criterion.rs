//! The types and reads of one criterion's lifecycle up to the seal, derived from the facts.
//! The same predicates run on the laptop and in CI, told apart by `ran_in`.
//! See docs/decisions.md#seven-states-in-a-fixed-order.

use familiar_ai_facts::Fact;
use serde_json::{Map, Value};

use crate::plan;
use crate::reader::{flag, last_of, t_of, text, texts, View};

/// The criterion states in the order they are checked.
pub const CRIT_ORDER: [&str; 7] = ["held", "proven", "green", "sealed", "testing", "waiting", "unplanned"];

/// The holds a person answers, and their choices.
/// Decided B (the owner, 2026-10-15): `a_dependency` joins `two_failures` and `two_<outcome>`.
pub const ANSWERS: [(&str, &[&str]); 18] = [
    ("anchor_moved", &["re_plan", "keep_going", "release"]),
    ("two_failures", &["try_again", "take_over", "a_dependency", "release"]),
    ("two_tries_not_red", &["try_again", "re_plan", "release"]),
    ("fix_elsewhere", &["release", "re_plan", "take_over"]),
    ("anchor_untouched", &["re_plan", "keep_going", "release"]),
    // the run's cards (plan §6.1, §6.3, §6.4; lane S7)
    ("two_test_broken", &["try_again", "re_plan", "a_dependency", "release"]),
    ("two_crashed", &["try_again", "re_plan", "a_dependency", "release"]),
    ("two_timed_out", &["try_again", "re_plan", "a_dependency", "release"]),
    ("environment_error", &["try_again", "re_plan", "release"]),
    // H-4 (the owner, 2026-10-19): the person confirms what Familiar can't read, each choice
    // offered only at its stage (`confirmable`) [draft names]
    ("run_unclassified", &["try_again", "its_the_red", "its_the_pass", "re_plan", "release"]),
    ("retry_spent", &["try_again", "re_plan", "release"]),
    ("crashed", &["its_the_bug", "a_dependency", "release"]),
    ("command_broke", &["fix_what_broke_it", "a_dependency", "release"]),
    ("miscoped", &["covered_by_dependency", "re_plan", "release"]),
    // the adapter (plan A-4, A-5; lane a5) [draft]: the gap card on the setup part, and a seal
    // that no longer reads an honest red under a changed setup
    ("reading_gap", &["keep_going", "try_again"]),
    ("setup_changed", &["re_plan", "release"]),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft]: two verified "no"s on
    // one fix; keep_going keeps the fix as it is, try_again is one more review after another fix
    ("review_disputed", &["keep_going", "try_again", "release"]),
    // the cases card (the owner's plan of 2026-10-25) [draft names]: each choice is offered only
    // as the finding allows (`cases::offered`)
    (crate::cases::CARD, &[crate::cases::ADD_ALL, crate::cases::KEEP, crate::cases::AGAIN]),
];

/// The holds that pause further commits on their criterion.
pub const PAUSING: [&str; 16] = [
    "anchor_moved",
    "two_failures",
    "two_tries_not_red",
    "anchor_untouched",
    "fix_elsewhere",
    "two_test_broken",
    "two_crashed",
    "two_timed_out",
    "environment_error",
    "run_unclassified",
    "retry_spent",
    "crashed",
    "command_broke",
    "miscoped",
    "setup_changed",
    "review_disputed",
];

/// The plan facts that clear `planning_incomplete`, line 11's claim included.
pub const PLAN_KINDS: [&str; 16] = [
    "intent_submitted",
    "behavior",
    "promise",
    "criterion",
    "place_chosen",
    "new_work_spot_chosen",
    "new_work_no_spot",
    "anchor_sha_matched",
    "test_planned",
    "test_stack_detected",
    "touching_test_declared",
    "criterion_head",
    "criterion_strung_on_head",
    "criterion_assigned",
    "fix_location_category",
    "anchor_is_documentation", // the owner (2026-10-11): line 2's documentation check
];

/// The choices of a hold a person answers, by its why.
pub fn answers_of(why: &str) -> Option<&'static [&'static str]> {
    ANSWERS.iter().find(|(w, _)| *w == why).map(|(_, a)| *a)
}

/// The can't-tell card's choices that confirm its run (H-4): `its_the_red` before the seal,
/// `its_the_pass` after it.
pub const CONFIRMS: [&str; 2] = ["its_the_red", "its_the_pass"];

/// The run `choice` would confirm on `c`'s can't-tell card, when the card offers it: the card's
/// run (`outcome::card_run`), with `its_the_red` before the seal and only on a run at the
/// tests-first commit, with `its_the_pass` once sealed. The answer is bound to this run.
pub fn confirmable<'a>(v: &View<'a>, c: &str, choice: &str) -> Option<&'a str> {
    let t = crate::outcome::card_run(v, c, "run_unclassified")?;
    let sealed = seal_raw(v, c, Place::Laptop).is_some();
    let offered = match choice {
        "its_the_red" => {
            !sealed
                && tests_first_commit(v, c).is_some_and(|tf| {
                    let ix = v.idx();
                    text(t, "commit").and_then(|id| ix.key_of(id)) == Some(tf.key.as_str())
                })
        }
        "its_the_pass" => sealed,
        _ => false,
    };
    if offered {
        text(t, "run_id")
    } else {
        None
    }
}

/// Whether the person confirmed the run of the result `r`, which reads can't-tell, as `choice`
/// on `c`'s card, bound to that run (H-4). Only the seal's and green's readers take the person's
/// word; the teeth and every other reader keep the run's own family.
///
/// Confirmed once per part (the owner, 2026-10-25) [draft]: an its_the_red also carries to a later
/// can't-tell run of the same part's test that fails with the same kind of error
/// (`red_carried`). An its_the_pass never carries.
pub fn confirmed_as(v: &View, c: &str, r: &Fact, choice: &str) -> bool {
    crate::outcome::result_family(v, c, r) == crate::outcome::Family::Unknown
        && (confirmed_by_the_person(v, c, r, choice) || (choice == "its_the_red" && red_carried(v, c, r)))
}

/// The person's own answer `choice` on `c`'s can't-tell card, bound to the run of `r`.
fn confirmed_by_the_person(v: &View, c: &str, r: &Fact, choice: &str) -> bool {
    let Some(id) = text(r, "run_id") else {
        return false;
    };
    v.of("hold_answered", c).any(|f| {
        f.t > r.t && text(f, "hold") == Some("run_unclassified") && text(f, "choice") == Some(choice) && text(f, "run_id") == Some(id)
    })
}

/// [draft] Whether the can't-tell result `r` of `c` inherits an its_the_red the person gave on an
/// earlier can't-tell run of the same part's test (the owner, 2026-10-25, "Confirmed once per
/// part"): `r`'s run is at the tests-first commit (its_the_red's stage, as `confirmable` offers
/// it), and an earlier laptop result of the same test file of `c`, confirmed its_the_red by the
/// person and bound to its own run, failed with the same kind of error (`error_kind`). Never
/// across parts: only `c`'s own results and answers are read.
fn red_carried(v: &View, c: &str, r: &Fact) -> bool {
    let Some(id) = text(r, "run_id") else {
        return false;
    };
    let ix = v.idx();
    // While an announcement stands, the way back reads this very question (did the edited tests come
    // out red), so the tests-first commit is read without it [draft reading, the checks crate's].
    let tf = if announcement(v, c).is_some() { tests_first_now(v, c) } else { tests_first_commit(v, c) };
    let at_tests_first = tf.is_some_and(|tf| text(r, "commit").and_then(|x| ix.key_of(x)) == Some(tf.key.as_str()));
    if !at_tests_first {
        return false;
    }
    let Some(kind) = error_kind(v, id) else {
        return false;
    };
    v.of("pass_please_pass", c).any(|s| {
        s.t < r.t
            && text(s, "ran_in") == Some("laptop")
            && text(s, "file_path") == text(r, "file_path")
            && text(s, "run_id").is_some_and(|sid| sid != id && error_kind(v, sid).as_ref() == Some(&kind))
            && crate::outcome::result_family(v, c, s) == crate::outcome::Family::Unknown
            && confirmed_by_the_person(v, c, s, "its_the_red")
    })
}

/// [draft] The kind of error a run failed with, read from its `test_log_translation`: the
/// exception's name from the deciding line (`evidence`, `exception_name`) and the place it was
/// raised (`location`: its file and function, or its file and line when no function is recorded).
/// `None` unless both are there.
pub fn error_kind(v: &View, run_id: &str) -> Option<(String, String, String)> {
    let t = crate::outcome::translation_of(v, run_id)?;
    let name = exception_name(text(t, "evidence")?)?;
    let at = t.fields.get("location")?;
    let file = at.get("file_path").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let place = match at.get("function").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        Some(f) => format!("function {f}"),
        None => format!("line {}", at.get("line").filter(|l| !l.is_null())?),
    };
    Some((name.to_string(), file.to_string(), place))
}

/// [draft] The first token of `line` that looks like a Python exception class: a capitalised
/// identifier ending in Error, Exception, Interrupt, Exit or Warning (`IndexError` in
/// "FAILED t.py::test - IndexError: list index out of range").
pub fn exception_name(line: &str) -> Option<&str> {
    const ENDS: [&str; 5] = ["Error", "Exception", "Interrupt", "Exit", "Warning"];
    line.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .find(|w| w.starts_with(|ch: char| ch.is_ascii_uppercase()) && ENDS.iter().any(|e| w.ends_with(e)))
}

/// The `pass_please_pass` outcome of a run that never ran the tests: no result.
pub const DID_NOT_RUN: &str = "did_not_run";

/// Where a fact was written, read from `ran_in`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    Laptop,
    Pipeline,
}

/// Both places, for a caller that reads each.
pub const PLACES: [Place; 2] = [Place::Laptop, Place::Pipeline];

impl Place {
    pub fn as_str(self) -> &'static str {
        match self {
            Place::Laptop => "laptop",
            Place::Pipeline => "pipeline",
        }
    }
}

/// A test as the lifecycle reads it.
/// Results are read per planned test file; `test_name` is carried but never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test {
    pub file_path: String,
    pub test_name: String,
}

impl Test {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("file_path".into(), Value::from(self.file_path.as_str()));
        m.insert("test_name".into(), Value::from(self.test_name.as_str()));
        Value::Object(m)
    }
}

/// The move that clears a hold: its `kind` and the fields that must match.
pub type Clear = Map<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct Hold {
    pub why: String,
    pub clears: Vec<Clear>,
}

impl Hold {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("why".into(), Value::from(self.why.as_str()));
        m.insert(
            "clears".into(),
            Value::Array(self.clears.iter().cloned().map(Value::Object).collect()),
        );
        Value::Object(m)
    }
}

/// The tests-first commit, found by content: the tests-only commit whose tests hash is the
/// stamp's `proof_not_tampered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestsFirst {
    pub key: String,
    pub at: i64,
    pub ids: Vec<String>,
}

impl TestsFirst {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("key".into(), Value::from(self.key.as_str()));
        m.insert("at".into(), Value::from(self.at));
        m.insert(
            "ids".into(),
            Value::Array(self.ids.iter().map(|s| Value::from(s.as_str())).collect()),
        );
        Value::Object(m)
    }
}

pub(crate) type SealKey = (String, Place);

pub(crate) fn ran_in(f: &Fact, place: Place) -> bool {
    text(f, "ran_in") == Some(place.as_str())
}

/// The test file a result is for; results are read per planned test file.
pub fn tname(f: &Fact) -> &str {
    text(f, "file_path").unwrap_or("")
}

/// Whether the result is present and has this outcome.
pub fn outcome_is(f: Option<&Fact>, outcome: &str) -> bool {
    f.is_some_and(|r| text(r, "outcome") == Some(outcome))
}

/// Whether planning is confirmed complete.
pub fn confirmed(v: &View) -> bool {
    if let Some(x) = v.memo.borrow().confirmed {
        return x;
    }
    let x = plan::planning_confirm_complete(v);
    v.memo.borrow_mut().confirmed = Some(x);
    x
}

/// The time proof is judged from: the criterion's last anchor exit, so a re-anchor starts the
/// proof again, or, while a re-plan has unchosen the place, that answer. The place the binary
/// appends for the person's keep_going on `anchor_untouched` does not move it: the seal, the
/// stamps and the answers before it still count (the owner, 2026-10-13, F12).
pub fn epoch(v: &View, c: &str) -> i64 {
    plan::anchor_of(v, c)
        .map(|a| plan::epoch_exit(v, c).unwrap_or(a))
        .or_else(|| plan::unchosen(v, c))
        .map(t_of)
        .unwrap_or(-1)
}

/// The criterion's facts of a kind since its epoch.
pub fn mine<'a>(v: &View<'a>, c: &str, kind: &str) -> Vec<&'a Fact> {
    let e = epoch(v, c);
    v.of(kind, c).filter(|f| t_of(f) > e).collect()
}

/// Whether the anchor is `new_work_no_spot`.
pub fn greenfield(v: &View, c: &str) -> bool {
    plan::anchor_of(v, c).is_some_and(|a| a.kind == "new_work_no_spot")
}

/// The anchor's first place, when the anchor is `place_chosen`.
pub fn place<'a>(v: &View<'a>, c: &str) -> Option<&'a Value> {
    let a = plan::anchor_of(v, c)?;
    if a.kind != "place_chosen" {
        return None;
    }
    a.fields.get("places")?.as_array()?.first()
}

/// Whether a code change overlaps the criterion's anchor; with no spot any code change counts,
/// and a new-work spot counts its file because the new symbol has no lines yet.
pub fn overlaps(v: &View, c: &str, f: &Fact) -> bool {
    flag(f, "touches_code")
        && !sibling_before_start(v, c, f)
        && (greenfield(v, c) || texts(f, "anchor_overlap").contains(&c) || spot_file_changed(v, c, f) || kept_from(v, c, f))
}

/// `f` is code the kept place was made from (F12): the anchor is the place the binary
/// appended for the person's keep_going, and `f`'s content first landed after the
/// tests-first commit and before that place, in a file it names. Such a commit was read
/// against the place chosen before; the kept place names exactly where it landed.
fn kept_from(v: &View, c: &str, f: &Fact) -> bool {
    let Some(a) = plan::anchor_of(v, c).filter(|a| plan::kept(a)) else {
        return false;
    };
    let Some(tf) = tests_first_commit(v, c) else {
        return false;
    };
    let ix = v.idx();
    let at = ix.first_at(text(f, "commit").and_then(|id| ix.key_of(id)));
    let files = plan::file_paths_of(a);
    at > tf.at && at < t_of(a) && texts(f, "files").iter().any(|p| files.iter().any(|x| x == p))
}

/// The commits whose code landed since the seal, before `before`: each content that touched
/// code, first landed after the tests-first commit, oldest first (F12: what keep_going keeps).
pub fn landed<'a>(v: &View<'a>, c: &str, before: i64) -> Vec<&'a Fact> {
    let Some(tf) = tests_first_commit(v, c) else {
        return Vec::new();
    };
    let ix = v.idx();
    let mut out: Vec<(i64, &'a Fact)> = ix
        .keys
        .iter()
        .filter_map(|k| {
            let at = ix.first_at(Some(k));
            let f = ix.rep.get(k).and_then(|&i| v.at(i))?;
            (at > tf.at && at < before && flag(f, "touches_code")).then_some((at, f))
        })
        .collect();
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, f)| f).collect()
}

/// The files the change landed in since the seal: the landed commits' files, in order, none
/// of them a planned test (F12: the places keep_going appends).
pub fn landed_files(v: &View, c: &str) -> Vec<String> {
    let tests: Vec<String> = plan::criteria(v).iter().flat_map(|x| test_files(v, x)).collect();
    let mut out: Vec<String> = Vec::new();
    for f in landed(v, c, crate::reader::INF) {
        for p in texts(f, "files") {
            if !tests.iter().any(|t| t == p) && !out.iter().any(|x| x == p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

/// `sibling_before_start` (the owner, 2026-10-11) [reading]: another
/// criterion's commit that also names c's lines, landing before c's start
/// check, is not c's code (c has not started; its start check maps the
/// lines or raises anchor_moved).
pub fn sibling_before_start(v: &View, c: &str, f: &Fact) -> bool {
    let ov = texts(f, "anchor_overlap");
    if !(ov.len() > 1 && ov.contains(&c)) {
        return false;
    }
    crate::lifecycle::start_check(v, c).is_none_or(|st| t_of(f) < t_of(st))
}

/// The anchor is a new-work spot and `f` changed the file it names.
fn spot_file_changed(v: &View, c: &str, f: &Fact) -> bool {
    plan::anchor_of(v, c)
        .filter(|a| a.kind == "new_work_spot_chosen")
        .and_then(|a| text(a, "file_path"))
        .is_some_and(|p| texts(f, "files").contains(&p))
}

/// The criterion's planned test files.
pub fn test_files(v: &View, c: &str) -> Vec<String> {
    v.of("test_planned", c)
        .filter_map(|f| text(f, "file_path"))
        .map(str::to_string)
        .collect()
}

/// The tests written, distinct by file, first wins.
pub fn own_tests(v: &View, c: &str) -> Vec<Test> {
    let mut out: Vec<Test> = Vec::new();
    for f in v.of("test_written", c) {
        let fp = tname(f);
        if !out.iter().any(|t| t.file_path == fp) {
            out.push(Test {
                file_path: fp.to_string(),
                test_name: text(f, "test_name").unwrap_or("").to_string(),
            });
        }
    }
    out
}

/// The existing tests touching the change: those declared, then those the binary derived from
/// the fix's imports [draft] (the owner, 2026-10-20), each file once.
pub fn touching_tests(v: &View, c: &str) -> Vec<Test> {
    let mut out: Vec<Test> = Vec::new();
    for f in v.of("touching_test_declared", c).chain(v.of(TOUCHING_DERIVED, c)) {
        let fp = tname(f);
        if f.kind == TOUCHING_DERIVED && out.iter().any(|t| t.file_path == fp) {
            continue;
        }
        out.push(Test { file_path: fp.to_string(), test_name: text(f, "test_name").unwrap_or("").to_string() });
    }
    out
}

/// The binary's touching tests, found by the fix's imports [draft name].
pub const TOUCHING_DERIVED: &str = "touching_test_derived";

/// Whether a `commit_seen` touches one of the criterion's planned test files.
pub fn touches_tests_of(v: &View, c: &str, f: &Fact) -> bool {
    if f.kind != "commit_seen" || !flag(f, "touches_tests") {
        return false;
    }
    let files = test_files(v, c);
    texts(f, "files").iter().any(|p| files.iter().any(|x| x == p))
}

/// Whether the commit is `c`'s, by its planned tests or by its anchor, regardless of how many
/// criteria may run.
pub fn touches(v: &View, c: &str, f: &Fact) -> bool {
    touches_tests_of(v, c, f) || overlaps(v, c, f)
}

/// The `commit_seen` that first landed the content of commit `id`.
pub fn commit_of<'a>(v: &View<'a>, id: &str) -> Option<&'a Fact> {
    let ix = v.idx();
    ix.key_of(id).and_then(|k| ix.rep.get(k)).and_then(|&i| v.at(i))
}

/// The latest `test_command` the criterion's own `test_planned` names.
pub fn own_command<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    v.of("test_planned", c).filter_map(|f| text(f, "test_command")).filter(|x| !x.is_empty()).last()
}

/// Whether the criterion has a command of its own or one derived from its files.
pub fn scoped(v: &View, c: &str) -> bool {
    own_command(v, c).is_some() || derived_command(v, c).is_some()
}

/// The command that runs the criterion's tests: its own, else derived, else the stack's.
/// See docs/decisions.md#the-command-is-claimed-derived-or-unscoped.
pub fn command(v: &View, c: &str) -> Option<String> {
    own_command(v, c)
        .map(str::to_string)
        .or_else(|| derived_command(v, c))
        .or_else(|| crate::lifecycle::command_at(v, c, crate::reader::INF).map(str::to_string))
}

/// Apply the runner table's form for a stack to the files, or `None` when it cannot.
/// Every derived command goes through here so the table is read in one place.
pub(crate) fn derive_from(stack: &Fact, files: &[String]) -> Option<String> {
    let r = runner_of(text(stack, "test_stack").unwrap_or(""))?;
    if files.is_empty() {
        return None;
    }
    let cmd = text(stack, "test_command")?;
    runner_form(r, cmd, files)
}

/// The `test_stack_detected` with a command whose root covers the anchor's files: the longest root, else the latest.
/// New work with no integration point has no files, so any root covers it.
/// Plan it again keeps the command of the place it is leaving until the new
/// place is chosen, so this reads the last anchor exit, not the chosen anchor.
pub fn stack_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = plan::last_exit(v, c)?;
    let fps: Option<Vec<String>> = plan::exit_files(a);
    plan::stack_covering(v, fps.as_deref(), i64::MAX)
}

/// Match a known runner by the whole stack name or its suffix after the last `_`.
fn runner_of(stack: &str) -> Option<&'static str> {
    const KNOWN: [&str; 5] = ["pytest", "cargo", "jest", "vitest", "go"];
    let bare = match stack.rfind('_') {
        Some(j) => &stack[j + 1..],
        None => stack,
    };
    KNOWN.iter().find(|k| **k == stack || **k == bare).copied()
}

/// Build the derived command for a runner from the stack's command and the files.
/// The table must match the reference model's.
fn runner_form(r: &str, cmd: &str, files: &[String]) -> Option<String> {
    match r {
        "pytest" | "jest" => Some(format!("{cmd} {}", files.join(" "))),
        "vitest" => Some(format!("{cmd} run {}", files.join(" "))),
        "cargo" => {
            let mut a: Vec<String> = Vec::new();
            for f in files {
                let n = cargo_test_name(f)?;
                a.push(format!("--test {n}"));
            }
            Some(format!("{cmd} {}", a.join(" ")))
        }
        "go" => {
            let mut a: Vec<String> = Vec::new();
            for f in files {
                let d = f.rfind('/').map(|i| &f[..i]).unwrap_or(".");
                if !a.iter().any(|x| x == d) {
                    a.push(d.to_string());
                }
            }
            Some(format!("{cmd} {}", a.join(" ")))
        }
        _ => None,
    }
}

/// The cargo integration-test name of a file whose parent directory is named `tests`.
/// Any other file, such as a unit test in `src/`, cannot be expressed.
fn cargo_test_name(f: &str) -> Option<&str> {
    let i = f.rfind('/')?;
    let dir = &f[..i];
    if dir.rsplit('/').next()? != "tests" {
        return None;
    }
    f[i + 1..].strip_suffix(".rs")
}

/// The stack's runner applied to all of `c`'s test files in plan order, or `None` when the table
/// cannot express them. The touching tests are never in it: they have a run of their own
/// ([`touching_command`]), so the own run's counts are what H6 compares with the seal run's (the
/// owner's plan of 2026-10-21, step 0).
pub fn derived_command(v: &View, c: &str) -> Option<String> {
    let st = stack_of(v, c)?;
    let mut files: Vec<String> = Vec::new();
    for f in test_files(v, c) {
        if !files.contains(&f) {
            files.push(f);
        }
    }
    derive_from(st, &files)
}

/// The command of `c`'s touching run (the owner's plan of 2026-10-21, step 0) [draft]: the
/// stack's runner applied to the touching tests, the declared then the derived, each file once
/// and none of the part's own (those run in the own run);
/// `None` with no touching test, no stack, or a table that cannot express them, and then the
/// touching results are the part's own run's, as before.
pub fn touching_command(v: &View, c: &str) -> Option<String> {
    let st = stack_of(v, c)?;
    let mut files: Vec<String> = Vec::new();
    for t in touching_tests(v, c) {
        if !files.contains(&t.file_path) && is_touching(v, c, &t.file_path) {
            files.push(t.file_path);
        }
    }
    derive_from(st, &files)
}

/// Whether `file` is one of `c`'s touching tests and not one of its own: a touching result is
/// read on its own, without a seal run ([`crate::outcome::result_family`]).
pub fn is_touching(v: &View, c: &str, file: &str) -> bool {
    v.of("touching_test_declared", c).chain(v.of(TOUCHING_DERIVED, c)).any(|f| tname(f) == file)
        && !v.of("test_planned", c).chain(v.of("test_written", c)).any(|f| tname(f) == file)
}

/// A content hash of what a result of `c` is a result of: the anchor, the declared tests and
/// the command, read from the plan.
pub fn criterion_rev(v: &View, c: &str) -> String {
    use sha2::{Digest, Sha256};
    let anchor: Value = match plan::anchor_of(v, c) {
        None => Value::Null,
        Some(a) if a.kind == "place_chosen" => Value::Array(
            plan::places_of(a)
                .iter()
                .map(|p| {
                    let sha = plan::check_of(v, c, p).and_then(|ch| text(ch, "sha"));
                    serde_json::json!([p.file_path, p.line_start, p.line_end, sha])
                })
                .collect(),
        ),
        Some(a) => serde_json::json!([a.kind, text(a, "file_path")]),
    };
    let mut tests: Vec<String> = test_files(v, c);
    tests.extend(touching_tests(v, c).into_iter().map(|t| t.file_path));
    tests.sort();
    tests.dedup();
    let body = serde_json::json!([anchor, tests, command(v, c)]).to_string();
    Sha256::digest(body.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The latest `tests_observed` of `c` at that commit: the fingerprint of its own files.
pub fn observed<'a>(v: &View<'a>, c: &str, id: &str) -> Option<&'a Fact> {
    let i = *v.idx().obs.get(id)?.get(c)?;
    v.at(i)
}

/// The fingerprint of `c`'s own tests at that commit, falling back to a legacy log's
/// per-commit hash.
pub fn tests_hash_at<'a>(v: &View<'a>, c: &str, id: &str) -> Option<&'a str> {
    match observed(v, c, id) {
        Some(o) => text(o, "tests_hash"),
        None => legacy_hash_at(v, c, id),
    }
}

/// Read an old log's one `commit_seen.tests_hash` per commit, unverified; it is no longer written.
fn legacy_hash_at<'a>(v: &View<'a>, c: &str, id: &str) -> Option<&'a str> {
    let at = *v.idx().at_of_id.get(id)?;
    let r = v
        .facts
        .iter()
        .rev()
        .find(|f| t_of(f) <= at && touches_tests_of(v, c, f))?;
    text(r, "tests_hash")
}

/// The latest fingerprint of `c`'s tests at any commit, falling back to a legacy log's hash.
pub fn current_tests<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    if let Some(&i) = v.idx().last_obs.get(c) {
        return v.at(i).and_then(|o| text(o, "tests_hash"));
    }
    let r = v.facts.iter().rev().find(|f| touches_tests_of(v, c, f))?;
    text(r, "tests_hash")
}

/// A commit fact's fingerprint for `c`.
fn hash_of<'a>(v: &View<'a>, c: &str, f: &Fact) -> Option<&'a str> {
    text(f, "commit").and_then(|id| tests_hash_at(v, c, id))
}

/// The criterion's results at a place, from teeth runs or not, excluding runs that did not run.
pub fn results<'a>(v: &View<'a>, c: &str, place: Place, rev: bool) -> Vec<&'a Fact> {
    mine(v, c, "pass_please_pass")
        .into_iter()
        .filter(|f| ran_in(f, place) && flag(f, "with_anchor_reverted") == rev && text(f, "outcome") != Some(DID_NOT_RUN))
        .collect()
}

/// The latest result for a test file at a content key.
pub fn result_at<'a>(
    v: &View<'a>,
    c: &str,
    place: Place,
    rev: bool,
    test: &str,
    key: Option<&str>,
) -> Option<&'a Fact> {
    let ix = v.idx();
    last_of(
        results(v, c, place, rev)
            .into_iter()
            .filter(|f| tname(f) == test && text(f, "commit").and_then(|id| ix.key_of(id)) == key)
            .collect(),
    )
}

/// The latest stamp of a place at a content key.
pub fn stamp_at<'a>(v: &View<'a>, c: &str, place: Place, key: Option<&str>) -> Option<&'a Fact> {
    let ix = v.idx();
    last_of(
        mine(v, c, "fingerprint_stamped")
            .into_iter()
            .filter(|f| ran_in(f, place) && text(f, "commit").and_then(|id| ix.key_of(id)) == key)
            .collect(),
    )
}

/// The latest laptop stamp, which is the claim the pipeline rechecks.
pub fn stamp<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_of(
        mine(v, c, "fingerprint_stamped")
            .into_iter()
            .filter(|f| ran_in(f, Place::Laptop))
            .collect(),
    )
}

/// The latest laptop stamp's `proof_not_tampered`.
pub fn proof<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    stamp(v, c).and_then(|s| text(s, "proof_not_tampered"))
}

/// The first tests-only content whose tests hash is the stamp's proof, cached per view.
pub fn tests_first_commit(v: &View, c: &str) -> Option<TestsFirst> {
    if let Some(x) = v.memo.borrow().tests_first.get(c) {
        return x.clone();
    }
    let out = tests_first_uncached(v, c);
    v.memo.borrow_mut().tests_first.insert(c.to_string(), out.clone());
    out
}

fn tests_first_uncached(v: &View, c: &str) -> Option<TestsFirst> {
    if mark(v, c).is_none() {
        // c_dep (plan §7.1): after the person's a_dependency, the red is re-earned at B-or-G
        if let Some(r) = crate::dependency::re_earn(v, c) {
            return Some(r.tests_first(v));
        }
        return tests_first_now(v, c);
    }
    // After the way back, the tests-first commit is the red seal's, read before the announcement.
    let a = announcement(v, c)?;
    let t = tests_first_commit(&v.prefix(a.t as usize), c)?;
    let ids = v.idx().ids.get(&t.key).cloned().unwrap_or_default();
    Some(TestsFirst { key: t.key, at: t.at, ids })
}

/// The first-landed tests-only content whose tests hash is the latest proof, ignoring the way back.
fn tests_first_now(v: &View, c: &str) -> Option<TestsFirst> {
    let p = proof(v, c)?;
    let ix = v.idx();
    let mut ks: Vec<&String> = ix
        .keys
        .iter()
        .filter(|k| {
            ix.rep.get(*k).and_then(|&i| v.at(i)).is_some_and(|f| {
                touches_tests_of(v, c, f) && !flag(f, "touches_code") && hash_of(v, c, f) == Some(p)
            })
        })
        .collect();
    ks.sort_by_key(|k| ix.first_at(Some(k)));
    let k = ks.first()?;
    Some(TestsFirst {
        key: (*k).clone(),
        at: ix.first_at(Some(k)),
        ids: ix.ids.get(*k).cloned().unwrap_or_default(),
    })
}

/// The latest `test_change_announced` of `c`.
pub fn announcement<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_of(mine(v, c, "test_change_announced"))
}

/// Whether `c` was green when its announcement landed, read on the log before it.
pub fn green_at_announcement(v: &View, c: &str) -> bool {
    announcement(v, c).is_some_and(|a| green_raw(&v.prefix(a.t as usize), c, Place::Laptop))
}

/// The way back: after an announcement that landed while green, the first change of `c`'s
/// tests, run, stamped and not red, which the green carries from then on. See docs/decisions.md#the-way-back.
pub fn mark(v: &View, c: &str) -> Option<TestsFirst> {
    if let Some(x) = v.memo.borrow().mark.get(c) {
        return x.clone();
    }
    let out = mark_uncached(v, c);
    v.memo.borrow_mut().mark.insert(c.to_string(), out.clone());
    out
}

fn mark_uncached(v: &View, c: &str) -> Option<TestsFirst> {
    let a = announcement(v, c)?;
    let l = v.prefix(a.t as usize);
    // Only an announcement that landed while green opens the way back.
    if seal_raw(&l, c, Place::Laptop).is_none() || !green_raw(&l, c, Place::Laptop) {
        return None;
    }
    let (p, p0) = (proof(v, c), proof(&l, c));
    if p == p0 {
        // No stamp of another proof since the announcement: the way back as it stood before it.
        let m = mark(&l, c)?;
        let ids = v.idx().ids.get(&m.key).cloned().unwrap_or_default();
        return Some(TestsFirst { key: m.key, at: m.at, ids });
    }
    // The edited tests came out red, so a new seal applies instead.
    if tests_first_now(v, c).is_some_and(|tf| seal_at(v, c, Place::Laptop, &tf).is_some()) {
        return None;
    }
    // A red run after the announcement spends the way back.
    if crate::lifecycle::red_runs(v, c).iter().any(|s| s.t > a.t) {
        return None;
    }
    // The first change of its tests after the announcement is the one it covers.
    let e = v
        .facts
        .iter()
        .find(|f| f.t > a.t && touches_tests_of(v, c, f) && hash_of(v, c, f) != p0)?;
    if hash_of(v, c, e) != p {
        return None;
    }
    let ix = v.idx();
    let k = ix.key_of(text(e, "commit")?)?.to_string();
    let at = ix.first_at(Some(&k));
    (at > t_of(a)).then(|| TestsFirst { ids: ix.ids.get(&k).cloned().unwrap_or_default(), key: k, at })
}

/// The seal before the stack gate: the stamp at the tests-first commit once every planned test
/// is written and red there. `sealed_at` decides whether it stands.
pub fn seal_raw<'a>(v: &View<'a>, c: &str, place: Place) -> Option<&'a Fact> {
    let key: SealKey = (c.to_string(), place);
    if let Some(i) = v.memo.borrow().seal_raw.get(&key) {
        return i.and_then(|i| v.at(i));
    }
    let s = seal_raw_uncached(v, c, place);
    v.memo
        .borrow_mut()
        .seal_raw
        .insert(key, s.map(|f| f.t as usize));
    s
}

/// The seal, which stands unless the criterion is unscoped and another criterion of its stack
/// has an outstanding red.
pub fn sealed_at<'a>(v: &View<'a>, c: &str, place: Place) -> Option<&'a Fact> {
    let key: SealKey = (c.to_string(), place);
    if let Some(i) = v.memo.borrow().sealed_at.get(&key) {
        return i.and_then(|i| v.at(i));
    }
    let s = seal_raw(v, c, place).and_then(|s| (scoped(v, c) || !red_outstanding(v, c)).then_some(s));
    v.memo
        .borrow_mut()
        .sealed_at
        .insert(key, s.map(|f| f.t as usize));
    s
}

/// Whether the criterion has an intentional red that stands: sealed, not released, not green
/// and not proven.
pub fn outstanding(v: &View, c: &str) -> bool {
    seal_raw(v, c, Place::Laptop).is_some()
        && !crate::lifecycle::released(v, c)
        && !green_raw(v, c, Place::Laptop)
        && !proven_raw(v, c)
}

/// Whether another criterion of the same stack has an outstanding red.
pub fn red_outstanding(v: &View, c: &str) -> bool {
    plan::criteria(v)
        .iter()
        .any(|y| y != c && same_stack(v, c, y) && outstanding(v, y))
}

/// Whether `c` and `y` resolve to the same stack.
pub fn same_stack<'a>(v: &View<'a>, c: &str, y: &str) -> bool {
    match (stack_of(v, c), stack_of(v, y)) {
        (Some(s), Some(t)) => text(s, "project_root") == text(t, "project_root") && text(s, "test_stack") == text(t, "test_stack"),
        _ => false,
    }
}

/// Whether the same `proof_not_tampered` was stamped at `x` after the raw seal and after every
/// own test's result there.
pub fn same_stamp_raw<'a>(v: &View<'a>, c: &str, place: Place, x: Option<&'a str>) -> bool {
    let (Some(seal), Some(s)) = (seal_raw(v, c, place), stamp_at(v, c, place, x)) else {
        return false;
    };
    text(s, "proof_not_tampered") == proof(v, c)
        && s.t > seal.t
        && own_tests(v, c).iter().all(|t| {
            result_at(v, c, place, false, &t.file_path, x)
                .is_some_and(|r| r.t < s.t)
        })
}

/// Whether `c` is green at `x`, judged on the raw seal.
pub fn green_at_raw(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    crate::lifecycle::later(v, c, x)
        && crate::lifecycle::passed_there(v, c, place, x)
        && same_stamp_raw(v, c, place, x)
        && crate::lifecycle::anchor_touched(v, c, x)
        && crate::lifecycle::touching_passed_there(v, c, place, x)
        && contract_green_at(v, c, place, x)
}

/// Whether some later commit is green for `c`, judged on the raw seal.
pub fn green_raw(v: &View, c: &str, place: Place) -> bool {
    seal_raw(v, c, place).is_some()
        && v.idx().keys.iter().any(|x| green_at_raw(v, c, place, Some(x)))
}

/// Whether the raw seal was stamped under the command in force now.
pub fn same_command_raw(v: &View, c: &str, place: Place) -> bool {
    seal_raw(v, c, place)
        .is_some_and(|s| crate::lifecycle::command_at(v, c, t_of(s)) == crate::lifecycle::command_at(v, c, crate::reader::INF))
}

/// Whether every own test failed at `x` with the anchor put back, after the pipeline's raw seal.
/// For new work the integration point, or the spot's file, is what is put back.
pub fn teeth_failed_raw(v: &View, c: &str, x: Option<&str>) -> bool {
    let own = own_tests(v, c);
    seal_raw(v, c, Place::Pipeline).is_some()
        && !own.is_empty()
        && own.iter().all(|t| {
            result_at(v, c, Place::Pipeline, true, &t.file_path, x)
                .is_some_and(|r| crate::outcome::result_family(v, c, r) == crate::outcome::Family::ValidRed && r.t > seal_raw(v, c, Place::Pipeline).unwrap().t)
        })
}

/// Whether CI proved `c` on the raw seal: same command, same proof, green and teeth at one commit.
pub fn proven_raw(v: &View, c: &str) -> bool {
    seal_raw(v, c, Place::Pipeline).is_some()
        && same_command_raw(v, c, Place::Pipeline)
        && crate::lifecycle::same_proof(v, c)
        && v.idx().keys.iter().any(|x| green_at_raw(v, c, Place::Pipeline, Some(x)) && teeth_failed_raw(v, c, Some(x)))
}

fn seal_raw_uncached<'a>(v: &View<'a>, c: &str, place: Place) -> Option<&'a Fact> {
    seal_at(v, c, place, &tests_first_commit(v, c)?)
}

/// The stamp at `tf` once every planned test is written and red there, stamped with `tf`'s own
/// tests hash.
fn seal_at<'a>(v: &View<'a>, c: &str, place: Place, tf: &TestsFirst) -> Option<&'a Fact> {
    let own = own_tests(v, c);
    if own.is_empty() {
        return None;
    }
    if !test_files(v, c).iter().all(|fp| own.iter().any(|t| &t.file_path == fp)) {
        return None;
    }
    let mut rs = Vec::with_capacity(own.len());
    for t in &own {
        let r = result_at(v, c, place, false, &t.file_path, Some(&tf.key))?;
        // a seal needs a valid red, one that points at the place (plan §6.2), never a bare failed;
        // or a run the person confirmed as the red on its can't-tell card (H-4)
        if crate::outcome::result_family(v, c, r) != crate::outcome::Family::ValidRed && !confirmed_as(v, c, r, "its_the_red") {
            return None;
        }
        rs.push(r);
    }
    let s = stamp_at(v, c, place, Some(&tf.key))?;
    let ix = v.idx();
    let hash = ix.rep.get(&tf.key).and_then(|&i| v.at(i)).and_then(|f| hash_of(v, c, f));
    let same = text(s, "proof_not_tampered") == hash;
    if same && rs.iter().all(|r| r.t < s.t) {
        Some(s)
    } else {
        None
    }
}

// The contract, confirmed by the runner where it reads each test (the owner's plan of 2026-10-21,
// step 1) [draft names].

/// Whether a test the judge named matches a runner's test id. Both are cut into segments at each
/// `::` and each `.`, after the name loses a trailing `()` and the id a trailing parameter in
/// square brackets (`test_x[1]`); they match when the name's segments are the id's last segments,
/// one for one. So `test_x`, `TestA::test_x` and `TestA.test_x` all match
/// `tests/test_a.py::TestA::test_x` and `tests.test_a.TestA.test_x`; `x` and `test_x_too` do not.
/// The rule knows no runner: an id it cannot cut this way is matched only whole.
pub fn test_id_matches(name: &str, id: &str) -> bool {
    let name = name.trim();
    let name = name.strip_suffix("()").unwrap_or(name).trim();
    if name.is_empty() {
        return false;
    }
    let id = id.trim();
    let id = match (id.ends_with(']'), id.rfind('[')) {
        (true, Some(i)) if i > 0 => &id[..i],
        _ => id,
    };
    let cut = |s: &str| -> Vec<String> { s.split("::").flat_map(|p| p.split('.')).map(str::to_string).collect() };
    let (n, i) = (cut(name), cut(id));
    n.len() <= i.len() && n.iter().all(|s| !s.is_empty()) && i[i.len() - n.len()..] == n[..]
}

/// The judge's latest word on `c`'s laptop seal, when it names tests for the contract's cases.
fn case_word<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let seal = seal_raw(v, c, Place::Laptop)?;
    let word = v.of("judge_verdict", c).filter(|f| f.t > seal.t).last()?;
    word.fields.get("cases").and_then(Value::as_array).is_some_and(|a| !a.is_empty()).then_some(word)
}

/// Each case of `c`'s promise with the test the judge's latest word names for it: `None` when the
/// word names none (or leaves the case out). Empty when the promise gives no cases or the word
/// names no tests, so the judge's mapping, or its absence, stands as before.
pub fn case_tests(v: &View, c: &str) -> Vec<(plan::Case, Option<String>)> {
    let Some(word) = case_word(v, c) else {
        return Vec::new();
    };
    let named: Vec<&Value> = word.fields.get("cases").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    plan::cases_of(v, c)
        .into_iter()
        .map(|k| {
            let t = named.iter().find(|m| m.get("case_id").and_then(Value::as_str) == Some(k.case_id.as_str()));
            let test = t.filter(|m| m.get("ok").and_then(Value::as_bool) != Some(false)).and_then(|m| m.get("test").and_then(Value::as_str)).map(str::trim).filter(|s| {
                let s = s.trim_end_matches('.');
                !s.is_empty() && !s.eq_ignore_ascii_case("none")
            });
            (k, test.map(str::to_string))
        })
        .collect()
}

/// A run's per-test lists, read by Familiar's reporter: the tests that ran, failed and were
/// skipped.
struct PerTest<'a> {
    ran: Vec<&'a str>,
    failing: Vec<&'a str>,
    skipped: Vec<&'a str>,
}

/// The per-test lists of `c`'s runs at `x` at `place`: every own test's result there names a run
/// whose translation is the reporter's with `ran_tests`; `None` otherwise.
fn per_test_at<'a>(v: &View<'a>, c: &str, place: Place, x: Option<&str>) -> Option<PerTest<'a>> {
    per_test_of(v, own_tests(v, c).iter().map(|t| result_at(v, c, place, false, &t.file_path, x)).collect())
}

/// The per-test lists of the runs behind `rs`, one result for each own test: each names a run
/// whose translation is the reporter's with `ran_tests`; `None` otherwise, or with no results.
fn per_test_of<'a>(v: &View<'a>, rs: Vec<Option<&'a Fact>>) -> Option<PerTest<'a>> {
    if rs.is_empty() {
        return None;
    }
    let mut out = PerTest { ran: Vec::new(), failing: Vec::new(), skipped: Vec::new() };
    for r in rs {
        let tr = crate::outcome::translation_of(v, text(r?, "run_id")?)?;
        if text(tr, "by") != Some("reporter") || !tr.fields.contains_key("ran_tests") {
            return None;
        }
        out.ran.extend(texts(tr, "ran_tests"));
        out.failing.extend(texts(tr, "failing_tests"));
        out.skipped.extend(texts(tr, "skipped_tests"));
    }
    Some(out)
}

/// The ids of a run's lists that `name` matches; empty when the run holds none of them, and then
/// the judge's mapping stands alone.
fn matching<'a>(p: &PerTest<'a>, name: &str) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = Vec::new();
    for id in p.ran.iter().chain(&p.failing).chain(&p.skipped) {
        if test_id_matches(name, id) && !out.contains(id) {
            out.push(id);
        }
    }
    out
}

fn case_label(k: &plan::Case) -> String {
    format!("case {} ({})", k.case_id, k.kind)
}

/// Red once counts (the owner's ruling, 2026-10-25) [draft names]: whether the test the judge
/// named `name` failed at a seal run of `c` at a tests-first commit before the part's first fix
/// commit, where that run reads each test, while the latest tests-first commit `tf` landed after
/// that fix, so the seal run there is on code that already holds it. A seal run before any fix is
/// its own witness [draft reading].
fn failed_before_the_fix(v: &View, c: &str, tf: &TestsFirst, name: &str) -> bool {
    let ix = v.idx();
    let landed = |f: &Fact| ix.first_at(text(f, "commit").and_then(|id| ix.key_of(id)));
    let Some(fix) = crate::lifecycle::fix_commit(v, c).map(landed).filter(|&at| at < tf.at) else {
        return false;
    };
    let own = own_tests(v, c);
    let laptop = results(v, c, Place::Laptop, false);
    crate::lifecycle::sealing_runs(v, c).into_iter().filter(|s| landed(s) < fix).any(|s| {
        per_test_of(v, own.iter().map(|t| crate::lifecycle::ran_at(&laptop, s, &t.file_path)).collect())
            .is_some_and(|p| matching(&p, name).iter().any(|id| p.failing.contains(id)))
    })
}

/// What keeps `c`'s seal from carrying its contract, in the agent's words: a case the judge named
/// no test for; or, where the seal run reads each test, a normal case's named test that did not
/// fail there, nor at a seal run before the fix when the seal came after it (red once counts).
/// An opposite or edge case may pass at the seal: what stays the same passes before the fix too.
pub fn contract_unmet_at_seal(v: &View, c: &str) -> Option<String> {
    let named = case_tests(v, c);
    if named.is_empty() {
        return None;
    }
    if let Some((k, _)) = named.iter().find(|(_, t)| t.is_none()) {
        return Some(format!("{}: no test exercises it, as the judge read the tests", case_label(k)));
    }
    let tf = tests_first_commit(v, c)?;
    let seal = per_test_at(v, c, Place::Laptop, Some(&tf.key))?;
    // a listed place that applies is confirmed as a normal case is: it fails before the fix
    named.iter().filter(|(k, _)| k.kind == "normal" || k.kind == crate::places::KIND).find_map(|(k, t)| {
        let t = t.as_deref()?;
        let ids = matching(&seal, t);
        (!ids.is_empty() && !ids.iter().any(|id| seal.failing.contains(id)) && !failed_before_the_fix(v, c, &tf, t))
            .then(|| format!("{}: the judge named {t}, but it did not fail at the seal run; a normal case's test fails before the fix", case_label(k)))
    })
}

/// The first case whose named test did not run and pass at `x`, where the run there reads each
/// test: in its ran tests and not among its failing ones.
fn case_not_green_at(v: &View, c: &str, place: Place, x: Option<&str>) -> Option<(plan::Case, String)> {
    let named = case_tests(v, c);
    if named.is_empty() {
        return None;
    }
    let run = per_test_at(v, c, place, x)?;
    named.into_iter().find_map(|(k, t)| {
        let t = t?;
        let ids = matching(&run, &t);
        let ok = ids.is_empty() || (ids.iter().any(|id| run.ran.contains(id)) && ids.iter().all(|id| run.ran.contains(id) && !run.failing.contains(id)));
        (!ok).then_some((k, t))
    })
}

/// Whether `c`'s contract holds at `x`: nothing keeps the seal from carrying it, and every case's
/// named test ran and passed there where the run reads each test. With no cases named, it holds.
pub fn contract_green_at(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    case_tests(v, c).is_empty() || (contract_unmet_at_seal(v, c).is_none() && case_not_green_at(v, c, place, x).is_none())
}

/// What keeps `c`'s contract from holding, in the agent's words: at the seal, else at the latest
/// commit whose own tests passed; `None` when nothing does.
pub fn contract_unmet(v: &View, c: &str) -> Option<String> {
    case_word(v, c)?;
    if let Some(w) = contract_unmet_at_seal(v, c) {
        return Some(w);
    }
    let ix = v.idx();
    let mut keys: Vec<&String> = ix.keys.iter().filter(|k| crate::lifecycle::later(v, c, Some(k)) && crate::lifecycle::passed_there(v, c, Place::Laptop, Some(k))).collect();
    keys.sort_by_key(|k| ix.first_at(Some(k)));
    let x = keys.last()?;
    let (k, t) = case_not_green_at(v, c, Place::Laptop, Some(x))?;
    let commit: String = ix.ids.get(*x).and_then(|ids| ids.first()).map(|id| id.chars().take(7).collect()).unwrap_or_default();
    Some(format!("{}: the judge named {t}, but it did not run and pass at {commit}", case_label(&k)))
}
