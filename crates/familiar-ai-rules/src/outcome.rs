//! What a run's outcome means, read from its `test_log_translation` (plan §6.1, §6.2, §6.5):
//! the family decides the route; the place rule decides whether a red is valid. Pure: the
//! function a location sits in was read by the binary at classification (`location.function`,
//! from git's hunk header), so nothing here touches git.

use familiar_ai_facts::Fact;
use serde_json::{Map, Value};

use crate::criterion::test_files;
use crate::plan;
use crate::reader::{flag, t_of, text, View};

/// The families of §6.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Green,
    ValidRed,
    TestBroken,
    CommandBroken,
    NotTheAgents,
    Crashed,
    Transient,
    TooSlow,
    Unknown,
}

/// Where in the proof a run lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// At the tests-first commit, before the seal.
    BeforeSeal,
    /// At a code commit after the seal.
    AfterFix,
}

/// What a family's run does at a stage (§6.1's table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Refused: back to writing the test.
    Refuse,
    Seal,
    Green,
    /// An attempt, counted per outcome.
    Attempt,
    /// The person's card.
    Card,
    /// Retried once, then a card.
    Retry,
    /// §6.3: split by the command's history.
    Command,
}

/// The route of a family at a stage.
pub fn route(f: Family, stage: Stage) -> Route {
    let before = stage == Stage::BeforeSeal;
    match f {
        Family::Green => if before { Route::Refuse } else { Route::Green },
        Family::ValidRed => if before { Route::Seal } else { Route::Attempt },
        Family::TestBroken | Family::TooSlow => if before { Route::Refuse } else { Route::Attempt },
        Family::CommandBroken => Route::Command,
        Family::NotTheAgents | Family::Unknown => Route::Card,
        Family::Crashed => if before { Route::Card } else { Route::Attempt },
        Family::Transient => Route::Retry,
    }
}

/// The translation of a run.
pub fn translation_of<'a>(v: &View<'a>, run_id: &str) -> Option<&'a Fact> {
    v.facts.iter().rev().find(|f| f.kind == "test_log_translation" && text(f, "run_id") == Some(run_id))
}

/// The family of a run, judged against its own criterion's place.
pub fn family(v: &View, run_id: &str) -> Family {
    match translation_of(v, run_id) {
        Some(t) => family_of(v, t, t.criterion_id().unwrap_or("")),
        None => Family::Unknown,
    }
}

/// D7 (lane H6): a translation without `counts` is legacy, written before the binary kept a red's
/// kind and a run's counts, and reads exactly as it always did; one with `counts` (`{}` when
/// nothing was counted) is read by the family table of the plan of record (2026-10-19).
pub fn legacy_translation(t: &Fact) -> bool {
    !t.fields.contains_key("counts")
}

/// The family of a translation read for criterion `c` (a reused run is another criterion's).
/// A new-style red is honest only on positive evidence, where it points and its kind; a new-style
/// pass with a test skipped beside it can't tell (D4) until a seal run says the skip was there.
pub fn family_of(v: &View, t: &Fact, c: &str) -> Family {
    if !legacy_translation(t) {
        match text(t, "label") {
            Some("tests_failed") => return red_family(v, t, c),
            Some("passed") => return if count(t, "failed") == 0 && count(t, "skipped") == 0 { Family::Green } else { Family::Unknown },
            _ => {}
        }
    }
    match text(t, "label").unwrap_or("") {
        "passed" => Family::Green,
        "tests_failed" | "missing_target_symbol" => {
            if text(t, "label") == Some("missing_target_symbol") && (location(t).is_none() || module_elsewhere(t, v, c)) {
                return Family::Unknown;
            }
            match plan::anchor_of(v, c) {
                Some(a) if points_at(t, a, v, c) => Family::ValidRed,
                None if text(t, "label") == Some("tests_failed") && !exception(t, v, c) => Family::ValidRed,
                _ => Family::TestBroken,
            }
        }
        "collection_error" | "no_tests_collected" | "setup_error" => Family::TestBroken,
        "usage_error" => Family::CommandBroken,
        "environment_error" => Family::NotTheAgents,
        "crashed" => Family::Crashed,
        "runner_crash" | "killed" => Family::Transient,
        "timed_out" => Family::TooSlow,
        _ => Family::Unknown,
    }
}

/// The outcome word a result is counted under (§6.1, D4): its run's label; a legacy result's
/// `failed` is `tests_failed`.
pub fn outcome_of<'a>(v: &View<'a>, r: &'a Fact) -> &'a str {
    match text(r, "run_id") {
        Some(id) => translation_of(v, id).and_then(|t| text(t, "label")).unwrap_or("unclassified"),
        None if legacy(v, r) => match text(r, "outcome") {
            Some("passed") => "passed",
            Some("failed") => "tests_failed",
            _ => "unclassified",
        },
        None => "unclassified",
    }
}

/// The grandfather rule (§6.5): a result without `run_id` is legacy when it precedes the
/// record's first translation, the pipeline's as the laptop's (S9d: verify's runs have ids).
pub fn legacy(v: &View, r: &Fact) -> bool {
    if text(r, "run_id").is_some() {
        return false;
    }
    v.facts.iter().find(|f| f.kind == "test_log_translation").is_none_or(|first| t_of(r) < t_of(first))
}

/// The family of a result of `c`: its run's, through `run_id`; a legacy result's by its word;
/// any other result without `run_id` is Unknown, never failed. A new-style pass is judged
/// against the seal run (`green_against_the_seal`); a new-style red whose reason isn't shown, at
/// a code commit after the seal run, is still the agent's failed try (D3); a teeth run stays
/// strict.
pub fn result_family(v: &View, c: &str, r: &Fact) -> Family {
    if let Some(id) = text(r, "run_id") {
        return match translation_of(v, id) {
            Some(t) if !legacy_translation(t) && crate::criterion::is_touching(v, c, text(r, "file_path").unwrap_or("")) => touching_family(v, r, t, c),
            Some(t) if !legacy_translation(t) && text(t, "label") == Some("passed") => green_against_the_seal(v, c, r, t),
            // the person's its_the_bug on the crashed card makes the crash before it a valid red (§6.1)
            Some(t) => match family_of(v, t, c) {
                Family::Crashed if its_the_bug(v, c, r) => Family::ValidRed,
                Family::Unknown if !legacy_translation(t) && text(t, "label") == Some("tests_failed") && at_a_fix(v, c, r) => Family::ValidRed,
                f => f,
            },
            None => Family::Unknown,
        };
    }
    if !legacy(v, r) {
        return Family::Unknown;
    }
    match text(r, "outcome") {
        Some("passed") => Family::Green,
        Some("failed") => Family::ValidRed,
        _ => Family::Unknown,
    }
}

/// A touching test's result `r` of the run translated as `t` (the owner's plan of 2026-10-21,
/// step 0) [draft]: an existing test the change may break, read on its own and never against a
/// seal run, since it was never red. A pass is green when nothing failed beside it: the table's
/// `passed` with no failure counted, or a run that exited 0 (its result's word is `passed`) whose
/// counts show tests and no failure, whatever the table called it (pytest's reads a pass beside a
/// skip as unclassified, an all-skipped file as no tests collected). A skip or a deselect in a
/// touching file is the project's, never the part's to explain, so it never makes the part
/// can't-tell. A run that failed a test is a failure of the change wherever it points (the place
/// rule is for the part's own reds), a regression when the file passed at the tests-first commit.
/// A touching failure is the change's only when the test was not already failing at the
/// tests-first commit.
/// Anything else reads as its family: with no counts, or a non-zero exit, no pass.
fn touching_family(v: &View, r: &Fact, t: &Fact, c: &str) -> Family {
    let nothing_failed = counts(t).is_some() && count(t, "failed") == 0;
    match text(t, "label") {
        Some("passed") if count(t, "failed") == 0 => Family::Green,
        Some("passed") => Family::Unknown,
        Some("tests_failed") => {
            // without per-test ids, or with none in the file, it reads as it always did, green
            // only as the file's own result says
            let ids = match failing_ids(t) {
                Some(ids) if !ids.is_empty() => ids,
                _ => return Family::ValidRed,
            };
            let Some(file) = text(r, "file_path") else {
                return Family::ValidRed;
            };
            let now: Vec<String> = ids.into_iter().filter(|id| id_path(id) == file).collect();
            if now.is_empty() {
                // the run failed tests the file did not: green only as the file's own result says
                return if text(r, "outcome") == Some("passed") { Family::Green } else { Family::ValidRed };
            }
            match tests_first_failures(v, c, r) {
                // every test failing now failed at the tests-first commit: pre-existing, not the change's
                Some(before) if now.iter().all(|id| before.iter().any(|b| b == id)) => Family::Green,
                _ => Family::ValidRed,
            }
        }
        Some("unclassified" | "no_tests_collected") if nothing_failed && text(r, "outcome") == Some("passed") => Family::Green,
        _ => family_of(v, t, c),
    }
}

/// A run's failing test ids, `None` when it counted none: the translation's `failing_tests`,
/// each an id `path::Class::test`.
fn failing_ids(t: &Fact) -> Option<Vec<String>> {
    t.fields.get("failing_tests").and_then(Value::as_array).map(|ids| {
        ids.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    })
}

/// An id's path part, before the first `::` (a leading `./` stripped).
fn id_path(id: &str) -> &str {
    id.split("::").next().unwrap_or(id).trim_start_matches("./")
}

/// The file's failing ids at its tests-first run (the latest result of the file at the
/// criterion's tests-first commit): the run's failing ids in the file, none when the run
/// passed with nothing failed; `None` when the run counted no per-test ids or the file was
/// never run there.
fn tests_first_failures(v: &View, c: &str, r: &Fact) -> Option<Vec<String>> {
    let file = text(r, "file_path")?;
    let tf = crate::criterion::tests_first_commit(v, c)?;
    let ix = v.idx();
    let base = v
        .of("pass_please_pass", c)
        .filter(|f| !flag(f, "with_anchor_reverted"))
        .filter(|f| text(f, "file_path") == Some(file) && text(f, "commit").and_then(|id| ix.key_of(id)) == Some(tf.key.as_str()))
        .last()?;
    let t = text(base, "run_id").and_then(|id| translation_of(v, id))?;
    let ids = failing_ids(t)?;
    match text(t, "label") {
        Some("tests_failed") if !ids.is_empty() => Some(ids.into_iter().filter(|id| id_path(id) == file).collect()),
        Some("passed") if count(t, "failed") == 0 => Some(Vec::new()),
        _ => None,
    }
}

/// A run's `counts` (lane H3, D7), when it counted anything.
fn counts(t: &Fact) -> Option<&Map<String, Value>> {
    t.fields.get("counts").and_then(Value::as_object).filter(|m| !m.is_empty())
}

/// A run's count of `field`. An absent field is 0: the count rows write a field only where the
/// runner's summary printed it, and a summary that leaves a field out leaves it out at zero
/// (pytest's `1 failed` prints no `0 passed`).
fn count(t: &Fact, field: &str) -> u64 {
    counts(t).and_then(|m| m.get(field)).and_then(Value::as_u64).unwrap_or(0)
}

/// Whether the result `f` of `c` is an honest red: its run's family is a valid red, or a legacy
/// result's word is `failed`.
fn honest_red(v: &View, c: &str, f: &Fact) -> bool {
    match text(f, "run_id") {
        Some(id) => translation_of(v, id).is_some_and(|t| family_of(v, t, c) == Family::ValidRed),
        None => legacy(v, f) && text(f, "outcome") == Some("failed"),
    }
}

/// The seal run's result for `r` (H-3, D6): the first honest red of the same test at the same
/// place, not with the anchor put back, at an earlier commit than `r`'s.
pub fn seal_run_of<'a>(v: &View<'a>, c: &str, r: &Fact) -> Option<&'a Fact> {
    v.of("pass_please_pass", c).find(|f| {
        f.t < r.t
            && text(f, "file_path") == text(r, "file_path")
            && text(f, "ran_in") == text(r, "ran_in")
            && text(f, "commit") != text(r, "commit")
            && !flag(f, "with_anchor_reverted")
            && honest_red(v, c, f)
    })
}

/// After the seal (H-3, D6): a new-style pass is green only if no test failed, no more were
/// skipped or deselected than at the seal run, and at least as many passed as ran there
/// (passed + failed). With no seal run that counted, green only with nothing skipped; a seal run
/// that counted and a pass that counted nothing can't tell.
fn green_against_the_seal(v: &View, c: &str, r: &Fact, t: &Fact) -> Family {
    let seal = seal_run_of(v, c, r).and_then(|s| translation_of(v, text(s, "run_id")?)).filter(|s| counts(s).is_some());
    let green = match seal {
        None => count(t, "failed") == 0 && count(t, "skipped") == 0,
        Some(_) if counts(t).is_none() => false,
        Some(s) => {
            count(t, "failed") == 0
                && count(t, "skipped") + count(t, "deselected") <= count(s, "skipped") + count(s, "deselected")
                && count(t, "passed") >= count(s, "passed") + count(s, "failed")
        }
    };
    if green {
        Family::Green
    } else {
        Family::Unknown
    }
}

/// D3: `r` ran at a code commit after the seal run of its test, not with the anchor put back.
fn at_a_fix(v: &View, c: &str, r: &Fact) -> bool {
    let ix = v.idx();
    !flag(r, "with_anchor_reverted")
        && text(r, "commit").and_then(|id| ix.key_of(id)).and_then(|k| ix.rep.get(k)).and_then(|&i| v.at(i)).is_some_and(|f| flag(f, "touches_code"))
        && seal_run_of(v, c, r).is_some()
}

/// Whether the person answered `its_the_bug` on `c`'s crashed card after the result `r`.
fn its_the_bug(v: &View, c: &str, r: &Fact) -> bool {
    v.of("hold_answered", c)
        .any(|f| f.t > r.t && text(f, "hold") == Some("crashed") && text(f, "choice") == Some("its_the_bug"))
}

/// The command a run was reserved with.
pub fn command_of_run<'a>(v: &View<'a>, run_id: &str) -> Option<&'a Fact> {
    v.facts.iter().find(|f| f.kind == "run_reserved" && text(f, "run_id") == Some(run_id))
}

/// §6.3: the last run before `run_id` with the same command whose family is not Command is
/// broken (`{good run}`); `None` when the command was never valid.
pub fn good_run_before<'a>(v: &View<'a>, run_id: &str) -> Option<&'a Fact> {
    let bad = command_of_run(v, run_id)?;
    let cmd = text(bad, "command");
    v.facts
        .iter()
        .filter(|f| f.kind == "run_reserved" && f.t < bad.t && text(f, "command") == cmd)
        .filter(|f| {
            let Some(id) = text(f, "run_id") else { return false };
            translation_of(v, id).is_some() && family(v, id) != Family::CommandBroken
        })
        .last()
}

/// §6.3: the first run after the good one with the same command whose family is Command is
/// broken (`{bad run}`).
pub fn bad_run_after<'a>(v: &View<'a>, good: &Fact) -> Option<&'a Fact> {
    let cmd = text(good, "command");
    v.facts.iter().find(|f| {
        f.kind == "run_reserved"
            && f.t > good.t
            && text(f, "command") == cmd
            && text(f, "run_id").is_some_and(|id| translation_of(v, id).is_some() && family(v, id) == Family::CommandBroken)
    })
}

/// The hold a result of `c` raises by its family alone, after the place rule: the card holds of
/// the run (plan §6.1). `None` for a family whose route is a seal, a refusal or an attempt.
pub fn card_hold_of(v: &View, c: &str, r: &Fact) -> Option<&'static str> {
    let id = text(r, "run_id")?;
    translation_of(v, id)?;
    match result_family(v, c, r) {
        Family::NotTheAgents => Some("environment_error"),
        Family::Unknown => Some("run_unclassified"),
        Family::Transient => Some("retry_spent"),
        Family::CommandBroken if good_run_before(v, id).is_some() => Some("command_broke"),
        _ => None,
    }
}

/// The card's run (§6.7): the latest translated laptop result of `c` whose hold is `hold`.
pub fn card_run<'a>(v: &View<'a>, c: &str, hold: &str) -> Option<&'a Fact> {
    // before the seal, the can't-tell card is the tests-first run's, whatever ran after it (13363)
    if hold == "run_unclassified" && crate::criterion::seal_raw(v, c, crate::criterion::Place::Laptop).is_none() {
        if let Some(tf) = crate::criterion::tests_first_commit(v, c) {
            let first = crate::criterion::own_tests(v, c)
                .iter()
                .filter_map(|t| crate::criterion::result_at(v, c, crate::criterion::Place::Laptop, false, &t.file_path, Some(&tf.key)))
                .find(|r| result_family(v, c, r) == Family::Unknown)
                // answered since (try_again, ...): no longer the card's run; the newer one is
                .filter(|r| !v.of("hold_answered", c).any(|f| f.t > r.t && text(f, "hold") == Some("run_unclassified")));
            if let Some(t) = first.and_then(|r| translation_of(v, text(r, "run_id")?)) {
                return Some(t);
            }
        }
    }
    let rs: Vec<&Fact> = v.of("pass_please_pass", c).filter(|r| text(r, "ran_in") == Some("laptop")).collect();
    rs.into_iter()
        .rev()
        .filter_map(|r| Some((r, translation_of(v, text(r, "run_id")?)?)))
        .find(|&(r, t)| match hold {
            "crashed" | "two_crashed" => text(t, "label") == Some("crashed"),
            "environment_error" => family_of(v, t, c) == Family::NotTheAgents,
            // read as the hold was raised: a pass the seal run cannot vouch for is the card's run (H6)
            "run_unclassified" => result_family(v, c, r) == Family::Unknown,
            "retry_spent" => family_of(v, t, c) == Family::Transient,
            "command_broke" | "red_for_another_reason" => true,
            "two_test_broken" => family_of(v, t, c) == Family::TestBroken,
            "two_timed_out" => family_of(v, t, c) == Family::TooSlow,
            _ => family_of(v, t, c) == Family::ValidRed,
        })
        .map(|(_, t)| t)
}

/// Whether the result is present and of this family.
pub fn is(v: &View, c: &str, r: Option<&Fact>, f: Family) -> bool {
    r.is_some_and(|r| result_family(v, c, r) == f)
}

/// Whether the result ran and did not pass in a way that is the agent's to fix: a valid red,
/// a broken test, a crash or a run too slow. The person's (environment, unknown), the command's
/// and a transient run are no try.
pub fn agents_failure(v: &View, c: &str, r: Option<&Fact>) -> bool {
    r.is_some_and(|r| matches!(result_family(v, c, r), Family::ValidRed | Family::TestBroken | Family::Crashed | Family::TooSlow))
}

fn location(t: &Fact) -> Option<&serde_json::Map<String, Value>> {
    t.fields.get("location").and_then(Value::as_object)
}

fn loc_text<'a>(t: &'a Fact, k: &str) -> Option<&'a str> {
    location(t).and_then(|l| l.get(k)).and_then(Value::as_str)
}

/// The last segment of a path name: `auth::parse_token`, `auth.parse_token` → `parse_token`.
fn last_segment(name: &str) -> &str {
    name.rsplit([':', '.', '/', '\\']).next().unwrap_or(name)
}

/// Whether `header` names `symbol` as a whole word.
fn names(header: &str, symbol: &str) -> bool {
    let s = last_segment(symbol);
    !s.is_empty() && header.split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$')).any(|w| w == s)
}

/// The places of an anchor: (file, symbol) pairs. `new_work_no_spot` has none.
/// A missing name the output qualifies with a module (`os.mul`, `calc::mul`) whose module is not
/// the file of any of the place's spots: the name matches the place's by chance, so the run
/// can't be read (H-1). An unqualified name, or no place yet, is not this case.
fn module_elsewhere(t: &Fact, v: &View, c: &str) -> bool {
    let Some(module) = text(t, "missing_name").and_then(module_path) else {
        return false;
    };
    let Some(a) = plan::anchor_of(v, c) else {
        return false;
    };
    let name = text(t, "missing_name").unwrap_or("");
    let ps = places(a);
    // only a name that IS the place's can match by chance; any other name is plainly not it
    ps.iter().any(|(_, s)| last_segment(s) == last_segment(name)) && !ps.iter().any(|(f, _)| file_is_module(f, &module))
}

/// The module path a missing name is qualified with, one component each: `["os"]` for `os.mul`,
/// `["crate", "calc"]` for `crate::calc::mul`; none for a bare name.
fn module_path(name: &str) -> Option<Vec<&str>> {
    let cut = name.rfind(['.', ':'])?;
    let parts: Vec<&str> = name[..cut].split(['.', ':']).filter(|p| !p.is_empty()).collect();
    (!parts.is_empty()).then_some(parts)
}

/// Whether a source file is the module `path` names, compared whole: the file's folders and stem
/// (a package's `__init__` stands for its folder) end with every component of the path, so
/// `pkg.login` is `src/pkg/login.py` but not `src/login.py`, and `crate` stands for any root.
fn file_is_module(file: &str, path: &[&str]) -> bool {
    let mut parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    if let Some(last) = parts.last_mut() {
        *last = last.split('.').next().unwrap_or(last);
    }
    if parts.last() == Some(&"__init__") {
        parts.pop();
    }
    let path: Vec<&str> = path.iter().copied().filter(|p| !matches!(*p, "crate" | "self" | "super")).collect();
    !path.is_empty() && parts.len() >= path.len() && parts[parts.len() - path.len()..] == path[..]
}

fn places(a: &Fact) -> Vec<(String, String)> {
    match a.kind {
        "place_chosen" => plan::places_of(a).into_iter().map(|p| (p.file_path.to_string(), p.symbol.to_string())).collect(),
        "new_work_spot_chosen" => vec![(text(a, "file_path").unwrap_or("").to_string(), text(a, "symbol").unwrap_or("").to_string())],
        _ => Vec::new(),
    }
}

/// The family of a new-style `tests_failed` (the plan of record's table, 2026-10-19): no location
/// can't tell (§6.2); inside the place's function a valid red, whatever its kind (§6.2); in a
/// planned test file a valid red only as an assertion, an exception there can't tell (D1);
/// elsewhere an exception is a broken test (§6.2) and an assertion can't tell (D2). A location
/// with no kind is no evidence either way.
fn red_family(v: &View, t: &Fact, c: &str) -> Family {
    let Some(file) = loc_text(t, "file_path") else {
        return Family::Unknown;
    };
    let inside = |a: &Fact| loc_text(t, "function").is_some_and(|function| places(a).iter().any(|(f, s)| f == file && names(function, s)));
    if plan::anchor_of(v, c).is_some_and(inside) {
        return Family::ValidRed;
    }
    // an assertion in a planned test is the red; anything else outside the place, an exception
    // in the project's own code included, is can't tell and the person confirms (the owner, 2026-10-19)
    if test_files(v, c).iter().any(|x| x == file) && loc_text(t, "kind") == Some("assertion") {
        Family::ValidRed
    } else {
        Family::Unknown
    }
}

/// Whether a failed test's location is an exception outside the criterion's planned tests;
/// a failure with no location, or one in a planned test, is an assertion (the legacy reading).
fn exception(t: &Fact, v: &View, c: &str) -> bool {
    loc_text(t, "file_path").is_some_and(|f| !test_files(v, c).iter().any(|x| x == f))
}

/// The place rule (§6.2): whether the translation `t` points at the anchor `place`. An
/// assertion always does; a missing name when it is the place's symbol (on the last path
/// segment) and its location is one of the criterion's planned test files; an exception when
/// its deepest project frame is in the place's file and inside the place's function.
pub fn points_at_place(t: &Fact, place: &Fact, v: &View) -> bool {
    points_at(t, place, v, t.criterion_id().unwrap_or(""))
}

fn points_at(t: &Fact, place: &Fact, v: &View, c: &str) -> bool {
    let ps = places(place);
    match text(t, "label") {
        Some("missing_target_symbol") => {
            let (Some(name), Some(file)) = (text(t, "missing_name"), loc_text(t, "file_path")) else {
                return false;
            };
            test_files(v, c).iter().any(|x| x == file) && ps.iter().any(|(_, s)| last_segment(s) == last_segment(name))
        }
        Some("tests_failed" | "crashed") => {
            if !exception(t, v, c) {
                return text(t, "label") == Some("tests_failed");
            }
            let (Some(file), Some(function)) = (loc_text(t, "file_path"), loc_text(t, "function")) else {
                return false;
            };
            ps.iter().any(|(f, s)| f == file && names(function, s))
        }
        _ => false,
    }
}
