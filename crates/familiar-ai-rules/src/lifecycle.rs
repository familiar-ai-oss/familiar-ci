//! The criterion's lifecycle from the seal on: green, proven, the readings the holds need, and
//! the states, which are exclusive by construction.

use std::rc::Rc;

use familiar_ai_facts::Fact;

use crate::criterion::{
    commit_of, confirmed, epoch, mine, overlaps, own_tests, ran_in, result_at, results,
    sealed_at, stamp_at, tests_first_commit, tname, touches, touches_tests_of, touching_tests, Hold,
    Place, CRIT_ORDER, PAUSING,
};
use crate::outcome::{agents_failure, is, result_family, Family};
use crate::plan;
use crate::reader::{flag, last_of, t_of, text, View, INF};

/// Whether `x` is a later commit than the tests-first one, as green requires.
pub fn later(v: &View, c: &str, x: Option<&str>) -> bool {
    let Some(tf) = tests_first_commit(v, c) else {
        return false;
    };
    x != Some(tf.key.as_str()) && v.idx().first_at(x) > tf.at
}

/// Whether every own test passed at `x`.
pub fn passed_there(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    let own = own_tests(v, c);
    !own.is_empty()
        && own
            .iter()
            .all(|t| passed_or_confirmed(v, c, result_at(v, c, place, false, &t.file_path, x)))
}

/// Whether the result is green, or its run reads can't-tell and the person confirmed it as the
/// pass on its card (H-4); the teeth never read this.
fn passed_or_confirmed(v: &View, c: &str, r: Option<&Fact>) -> bool {
    is(v, c, r, Family::Green) || r.is_some_and(|r| crate::criterion::confirmed_as(v, c, r, "its_the_pass"))
}

/// Whether the latest proof was stamped at `x` after the seal and after every own test's result.
pub fn same_stamp_there(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    let (Some(seal), Some(s)) = (sealed_at(v, c, place), stamp_at(v, c, place, x)) else {
        return false;
    };
    text(s, "proof_not_tampered") == crate::criterion::proof(v, c)
        && s.t > seal.t
        && own_tests(v, c).iter().all(|t| {
            result_at(v, c, place, false, &t.file_path, x).is_some_and(|r| r.t < s.t)
        })
}

/// Whether a commit after the tests-first one, up to `x`, overlaps the anchor. When that commit
/// follows an announcement of the epoch, code counts from the epoch's first red seal instead.
pub fn anchor_touched(v: &View, c: &str, x: Option<&str>) -> bool {
    let fx = v.idx().first_at(x);
    fixes(v, c).iter().any(|&(at, _)| at <= fx)
}

/// The first commit after the tests-first one that lands code on the anchor, as
/// `anchor_touched` counts them: where the journey's fix step was done.
pub fn fix_commit<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    fixes(v, c).first().map(|&(_, f)| f)
}

/// The commits that overlap the anchor after the tests-first one (from the epoch's first red
/// seal when an announcement came before it), each with where it first landed, in landing order.
fn fixes<'a>(v: &View<'a>, c: &str) -> Vec<(i64, &'a Fact)> {
    let Some(tf) = tests_first_commit(v, c) else {
        return Vec::new();
    };
    let ix = v.idx();
    let from = if mine(v, c, "test_change_announced").iter().any(|a| t_of(a) < tf.at) {
        red_runs(v, c)
            .first()
            .map_or(tf.at, |s| ix.first_at(text(s, "commit").and_then(|id| ix.key_of(id))).min(tf.at))
    } else {
        tf.at
    };
    ix.keys
        .iter()
        .filter_map(|k| {
            let at = ix.first_at(Some(k));
            let f = ix.rep.get(k).and_then(|&i| v.at(i))?;
            (at > from && crate::criterion::overlaps(v, c, f)).then_some((at, f))
        })
        .collect()
}

/// Whether every touching test passed at `x`.
pub fn touching_passed_there(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    touching_tests(v, c)
        .iter()
        .all(|t| passed_or_confirmed(v, c, result_at(v, c, place, false, &t.file_path, x)))
}

/// Whether `c` is green at one commit content.
pub fn green_at(v: &View, c: &str, place: Place, x: Option<&str>) -> bool {
    later(v, c, x)
        && passed_there(v, c, place, x)
        && same_stamp_there(v, c, place, x)
        && anchor_touched(v, c, x)
        && touching_passed_there(v, c, place, x)
        && crate::criterion::contract_green_at(v, c, place, x)
}

/// The part's green point, by its content key: the latest commit after the tests-first one
/// that is relevant to the part, which is one that overlaps its anchor, as `overlaps` reads it
/// (a greenfield spot, a new-work spot's file and a kept place included), or that changes its
/// own test files; `None` when none has landed.
fn green_point(v: &View, c: &str) -> Option<String> {
    let tf = tests_first_commit(v, c)?;
    let ix = v.idx();
    ix.keys
        .iter()
        .rev()
        .find(|k| {
            let k = *k;
            ix.rep.get(k).and_then(|&i| v.at(i)).is_some_and(|f| {
                ix.first_at(Some(k)) > tf.at && (overlaps(v, c, f) || touches_tests_of(v, c, f))
            })
        })
        .cloned()
}

/// After the part's last change to its place or its own tests (a revert included), it is green
/// only when shown green at that commit or a later one; before any such change, a commit green
/// at its content stands, as before.
pub fn green_conditions(v: &View, c: &str, place: Place) -> bool {
    let ix = v.idx();
    sealed_at(v, c, place).is_some()
        && match green_point(v, c) {
            Some(p) => {
                let p_at = ix.first_at(Some(p.as_str()));
                ix.keys.iter().any(|x| ix.first_at(Some(x)) >= p_at && green_at(v, c, place, Some(x)))
            }
            None => ix.keys.iter().any(|x| green_at(v, c, place, Some(x))),
        }
}

/// Whether every own test failed at `x` with the anchor put back, after the pipeline seal.
/// For new work the integration point, or the spot's file, is what is put back.
pub fn teeth_failed_there(v: &View, c: &str, x: Option<&str>) -> bool {
    let Some(seal) = sealed_at(v, c, Place::Pipeline) else {
        return false;
    };
    let own = own_tests(v, c);
    !own.is_empty()
        && own.iter().all(|t| {
            result_at(v, c, Place::Pipeline, true, &t.file_path, x)
                .is_some_and(|r| result_family(v, c, r) == Family::ValidRed && r.t > seal.t)
        })
}

/// The `test_command` in force for the criterion's root before `before`; new work with no
/// integration point takes any root. Plan it again keeps the command of the place
/// it is leaving until the new place is chosen (the last anchor exit, not the chosen one).
pub fn command_at<'a>(v: &View<'a>, c: &str, before: i64) -> Option<&'a str> {
    let a = plan::last_exit(v, c)?;
    let fps: Option<Vec<String>> = plan::exit_files(a);
    let r = plan::stack_covering(v, fps.as_deref(), before)?;
    text(r, "test_command")
}

/// Whether the seal at that place was stamped under the `test_command` in force now.
pub fn same_command(v: &View, c: &str, place: Place) -> bool {
    sealed_at(v, c, place).is_some_and(|s| command_at(v, c, t_of(s)) == command_at(v, c, INF))
}

/// Whether CI's latest stamp carries the proof the laptop claims.
/// A pipeline that has since stamped other tests proves nothing of these.
pub fn same_proof(v: &View, c: &str) -> bool {
    last_of(
        mine(v, c, "fingerprint_stamped")
            .into_iter()
            .filter(|f| ran_in(f, Place::Pipeline))
            .collect(),
    )
    .is_some_and(|s| text(s, "proof_not_tampered") == crate::criterion::proof(v, c))
}

/// Whether CI proved `c`: green and teeth at one commit, under the command in force and CI's
/// latest proof.
pub fn proven_conditions(v: &View, c: &str) -> bool {
    sealed_at(v, c, Place::Pipeline).is_some()
        && same_command(v, c, Place::Pipeline)
        && same_proof(v, c)
        && v.idx().keys.iter().any(|x| {
            green_at(v, c, Place::Pipeline, Some(x)) && teeth_failed_there(v, c, Some(x))
        })
        // A-5 (lane a5): CI proves only after its own re-check of each later setup
        && crate::reading::rechecks_owed_at(v, c, Place::Pipeline).is_empty()
}

/// Whether the person released the criterion, which ends it for good.
pub fn released(v: &View, c: &str) -> bool {
    // covered_by_dependency on `miscoped` releases it too (plan §6.4), beside its criterion_covered
    v.of("hold_answered", c).any(|f| matches!(text(f, "choice"), Some("release" | "covered_by_dependency")))
        || v.of("criterion_covered", c).next().is_some()
}

/// The latest `fix_location_category` of the criterion.
pub fn fix_location<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts.iter().rev().find(|f| f.kind == "fix_location_category" && crate::reader::is_of(f, c))
}

/// The latest fix-location claim written while the current holder held the criterion.
/// It is derived, so after a hand-back the earlier holder's claim stands again with nothing appended.
pub fn standing_claim<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let now = plan::assigned_to(v, c);
    let mut holder: Option<&str> = None;
    let mut out: Option<&'a Fact> = None;
    for f in v.facts.iter().filter(|f| crate::reader::is_of(f, c)) {
        match f.kind {
            "criterion_assigned" => holder = text(f, "agent_conversation_id"),
            "criterion_transfer" => holder = text(f, "to_conversation"),
            "fix_location_category" if holder == now => out = Some(f),
            _ => {}
        }
    }
    out
}

/// Whether the current holder's first act must be its fix-location claim: planning confirmed,
/// not released, and no standing claim.
pub fn first_act(v: &View, c: &str) -> bool {
    confirmed(v) && !released(v, c) && standing_claim(v, c).is_none()
}

/// Whether the person answered `fix_elsewhere` with `re_plan` since the latest claim, so the
/// holder may claim again.
pub fn replanned(v: &View, c: &str) -> bool {
    fix_location(v, c).is_some_and(|fl| {
        v.of("hold_answered", c)
            .any(|f| text(f, "hold") == Some("fix_elsewhere") && text(f, "choice") == Some("re_plan") && f.t > fl.t)
    })
}

/// Whether it is the criterion's turn: it heads its string, or its predecessor is green, proven
/// or released.
pub fn turn(v: &View, c: &str) -> bool {
    if !confirmed(v) || !plan::on_string(v, c) {
        return false;
    }
    match plan::string_before(v, c) {
        None => true,
        Some(b) => released(v, &b) || matches!(state(v, &b), "green" | "proven"),
    }
}

/// The latest planning check since the epoch.
pub fn planning_check<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_of(
        mine(v, c, "anchor_sha_matched")
            .into_iter()
            .filter(|f| text(f, "checked_at") == Some("planning"))
            .collect(),
    )
}

/// The latest start check since the epoch, made before the criterion starts.
pub fn start_check<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_of(
        mine(v, c, "anchor_sha_matched")
            .into_iter()
            .filter(|f| text(f, "checked_at") == Some("start"))
            .collect(),
    )
}

/// The latest start check since the epoch of one place of the criterion's
/// anchor, made after the anchor was chosen: the same file and symbol, since
/// the start check carries the lines where it found them, which a shift moves.
pub fn start_check_of<'a>(v: &View<'a>, c: &str, p: &plan::PlaceRef) -> Option<&'a Fact> {
    let a = plan::anchor_of(v, c)?;
    last_of(
        mine(v, c, "anchor_sha_matched")
            .into_iter()
            .filter(|f| {
                text(f, "checked_at") == Some("start")
                    && t_of(f) > t_of(a)
                    && text(f, "file_path") == Some(p.file_path)
                    && text(f, "symbol") == Some(p.symbol)
            })
            .collect(),
    )
}

/// One place of a chosen anchor, with its own planning check and its own start check.
#[derive(Debug, Clone)]
pub struct PlaceCheck<'a> {
    pub place: plan::PlaceRef<'a>,
    pub planning: Option<&'a Fact>,
    pub start: Option<&'a Fact>,
}

/// Each place of the criterion's `place_chosen`, in the plan's order, with its
/// planning check and its start check (lane two-places, F7: each place is
/// checked at the start and compared with its own planning check); empty for
/// any other anchor.
pub fn place_checks<'a>(v: &View<'a>, c: &str) -> Vec<PlaceCheck<'a>> {
    let Some(a) = plan::anchor_of(v, c).filter(|a| a.kind == "place_chosen") else {
        return Vec::new();
    };
    plan::places_of(a)
        .into_iter()
        .map(|p| PlaceCheck { planning: plan::check_of(v, c, &p), start: start_check_of(v, c, &p), place: p })
        .collect()
}

/// Whether a place of the criterion's anchor still lacks its start check.
pub fn start_owed(v: &View, c: &str) -> bool {
    place_checks(v, c).iter().any(|p| p.start.is_none())
}

/// `anchor_moved` (F7): a place whose start check's lines differ from its own
/// planning check's, not answered since that start check, and not moved by
/// this plan's own recorded work.
pub fn anchor_moved(v: &View, c: &str) -> bool {
    place_checks(v, c).iter().any(|p| {
        let (Some(st), Some(pl)) = (p.start, p.planning) else {
            return false;
        };
        let answered = v.of("hold_answered", c).any(|f| text(f, "hold") == Some("anchor_moved") && f.t > st.t);
        text(st, "lines_hash") != text(pl, "lines_hash") && !answered && own_work_at(v, c, st, pl).is_none()
    })
}

/// Another criterion's work that changed the anchor's lines: its commit and
/// the criterion the record stamps it for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnWork {
    pub commit: String,
    pub criterion_id: String,
}

/// `own_work_moved` (`ownWorkOf`, the owner, 2026-10-11): "The anchor is
/// moved when its lines at the start differ from its lines at planning,
/// unless the working tree matches the last commit there and every commit
/// that touched those lines is in the record as another criterion's work in
/// this plan." Read per place (F7): the work of the first place whose lines
/// this plan's own work moved, or `None`.
pub fn own_work_moved(v: &View, c: &str) -> Option<OwnWork> {
    own_work_places(v, c).into_iter().next().map(|(w, _)| w)
}

/// Each place whose lines this plan's own work moved: the work and that place's start check.
pub fn own_work_places<'a>(v: &View<'a>, c: &str) -> Vec<(OwnWork, &'a Fact)> {
    place_checks(v, c)
        .into_iter()
        .filter_map(|p| {
            let st = p.start?;
            own_work_at(v, c, st, p.planning?).map(|w| (w, st))
        })
        .collect()
}

/// One place's reading: the commits after its planning check whose
/// `anchor_overlap` names c, each stamped on the laptop before its start
/// check for another criterion of the plan that the commit's `anchor_overlap`
/// also names; the latest of them, or `None`.
pub fn own_work_at(v: &View, c: &str, st: &Fact, pl: &Fact) -> Option<OwnWork> {
    if text(st, "lines_hash") == text(pl, "lines_hash") || st.fields.get("tip_matched").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let (pt, stt) = (t_of(pl), t_of(st));
    let crits = plan::criteria(v);
    let mut last = None;
    for k in v.kind("commit_seen").filter(|f| t_of(f) > pt && t_of(f) < stt && crate::reader::texts(f, "anchor_overlap").contains(&c)) {
        let overlap = crate::reader::texts(k, "anchor_overlap");
        let s = v.kind("fingerprint_stamped").find(|x| {
            text(x, "ran_in") == Some("laptop")
                && text(x, "commit") == text(k, "commit")
                && t_of(x) < stt
                && text(x, "criterion_id").is_some_and(|x| x != c && overlap.contains(&x) && crits.iter().any(|y| y == x))
        })?;
        last = Some(OwnWork { commit: text(k, "commit")?.to_string(), criterion_id: text(s, "criterion_id")?.to_string() });
    }
    last
}

/// Whether no detected stack with a command covers the criterion's anchor files.
pub fn stack_missing(v: &View, c: &str) -> bool {
    let Some(a) = plan::anchor_of(v, c) else {
        return false;
    };
    let stacks = plan::roots_with_command(v);
    let Some(fps) = plan::exit_files(a) else {
        return stacks.is_empty();
    };
    !fps.iter()
        .all(|fp| stacks.iter().any(|s| fp.starts_with(plan::root_of(s))))
}

/// Whether a stamp is at a tests-only commit of the criterion's tests.
pub fn tests_first_run(v: &View, c: &str, s: &Fact) -> bool {
    let ix = v.idx();
    text(s, "commit")
        .and_then(|id| ix.key_of(id))
        .and_then(|k| ix.rep.get(k))
        .and_then(|&i| v.at(i))
        .is_some_and(|f| touches_tests_of(v, c, f) && !flag(f, "touches_code"))
}

/// The latest laptop result for `fp` at the stamp's commit from before the stamp.
pub fn ran_at<'a>(laptop: &[&'a Fact], s: &Fact, fp: &str) -> Option<&'a Fact> {
    laptop
        .iter()
        .rev()
        .find(|x| text(x, "commit") == text(s, "commit") && tname(x) == fp && x.t < s.t)
        .copied()
}

/// The laptop tests-first stamps whose every own test failed, oldest first.
pub fn red_runs<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    red_runs_by(v, c, false)
}

/// [`red_runs`], a run the person confirmed as the red on its can't-tell card counting as one: the
/// tamper check's reading (H-4; the owner, 2026-10-20).
pub fn sealing_runs<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    red_runs_by(v, c, true)
}

fn red_runs_by<'a>(v: &View<'a>, c: &str, confirmed: bool) -> Vec<&'a Fact> {
    let own = own_tests(v, c);
    let laptop = results(v, c, Place::Laptop, false);
    mine(v, c, "fingerprint_stamped")
        .into_iter()
        .filter(|s| {
            ran_in(s, Place::Laptop)
                && tests_first_run(v, c, s)
                && !own.is_empty()
                && own.iter().all(|t| {
                    let r = ran_at(&laptop, s, &t.file_path);
                    is(v, c, r, Family::ValidRed) || (confirmed && r.is_some_and(|r| crate::criterion::confirmed_as(v, c, r, "its_the_red")))
                })
        })
        .collect()
}

/// Count failed code runs at the criterion's own commits since the seal, the last red run, the
/// last answer, or the last investigator transfer, which gets its own two unless it ended
/// unfinished: the attempts of `tests_failed`, which `two_failures` reads.
pub fn attempts(v: &View, c: &str) -> usize {
    attempts_skipping(v, c, None)
}

/// The two-attempt hold of an outcome (D4): `two_failures` for `tests_failed`, else `two_<outcome>`.
pub fn two_of(outcome: &str) -> String {
    if outcome == "tests_failed" {
        "two_failures".to_string()
    } else {
        format!("two_{outcome}")
    }
}

/// What an attempt is counted under (the owner, 2026-10-16: by the family, not the label): a valid
/// red under `tests_failed` (`two_failures`), a broken test under `test_broken` [draft], a crash
/// under `crashed`, a run too slow under `timed_out`. Any other family is no attempt.
pub fn attempt_key(v: &View, c: &str, r: &Fact) -> Option<&'static str> {
    match crate::outcome::result_family(v, c, r) {
        Family::ValidRed => Some("tests_failed"),
        Family::TestBroken => Some("test_broken"),
        Family::Crashed => Some("crashed"),
        Family::TooSlow => Some("timed_out"),
        _ => None,
    }
}

/// The attempt keys other than `tests_failed`, each with its own `two_<key>` hold.
pub const OTHER_ATTEMPTS: [&str; 3] = ["test_broken", "crashed", "timed_out"];

/// The attempts of one outcome (plan §6.1, D4): each laptop stamp at a code commit since the
/// seal counts under the outcome of the run behind its failing result (`run_id` → the
/// translation; a legacy `failed` is `tests_failed`), bounded by the person's last answer to that
/// outcome's hold (§6.7: `try_again` resets its own count). Only `tests_failed` climbs the ladder.
pub fn attempts_of(v: &View, c: &str, outcome: &str) -> usize {
    attempts_of_skipping(v, c, outcome, None)
}

/// `attempts`, reading the investigator transfer `skip` as if it had not been sent: it is left out
/// of the last-investigator reading, so the attempts it restarted count again. `None` is `attempts`.
pub fn attempts_skipping(v: &View, c: &str, skip: Option<&Fact>) -> usize {
    attempts_of_skipping(v, c, "tests_failed", skip)
}

fn attempts_of_skipping(v: &View, c: &str, outcome: &str, skip: Option<&Fact>) -> usize {
    let Some(seal) = crate::criterion::seal_raw(v, c, Place::Laptop) else {
        return 0;
    };
    let an = last_answer(v, c, &two_of(outcome));
    let iv = if outcome == "tests_failed" { last_investigator_skipping(v, c, skip) } else { None };
    let red = red_runs(v, c).last().copied();
    let since = t_of(seal)
        .max(red.map(t_of).unwrap_or(-1))
        .max(an.map(t_of).unwrap_or(-1))
        .max(iv.map(t_of).unwrap_or(-1));
    let own = own_tests(v, c);
    let ix = v.idx();
    let laptop = results(v, c, Place::Laptop, false);
    mine(v, c, "fingerprint_stamped")
        .into_iter()
        .filter(|s| {
            ran_in(s, Place::Laptop)
                && t_of(s) > since
                && !tests_first_run(v, c, s)
                && stamp_touches(v, c, s)
                && later(v, c, text(s, "commit").and_then(|id| ix.key_of(id)))
                && own
                    .iter()
                    .filter_map(|t| ran_at(&laptop, s, &t.file_path))
                    .find(|r| !is(v, c, Some(r), Family::Green))
                    .is_some_and(|r| attempt_key(v, c, r) == Some(outcome))
        })
        .count()
}

/// The command gate (plan §6.3): while the person's `fix_what_broke_it` on `command_broke`
/// stands, the criterion's command is locked; a later result of `c` whose run is not the
/// command's own breakage frees it.
pub fn command_locked(v: &View, c: &str) -> bool {
    let Some(a) = v
        .of("hold_answered", c)
        .filter(|f| text(f, "hold") == Some("command_broke") && text(f, "choice") == Some("fix_what_broke_it"))
        .last()
    else {
        return false;
    };
    !v.of("pass_please_pass", c).any(|r| r.t > a.t && result_family(v, c, r) != Family::CommandBroken)
}

/// The command gate for one stack root (plan §6.3, "for this criterion"): the locked criterion
/// whose stack, the one covering its anchor's files, has `root` as its project root.
pub fn command_locked_at(v: &View, root: &str) -> Option<String> {
    plan::criteria(v).into_iter().find(|c| {
        command_locked(v, c) && crate::criterion::stack_of(v, c).and_then(|s| text(s, "project_root")) == Some(root)
    })
}

/// The latest answer to `hold` other than a release.
pub fn last_answer<'a>(v: &View<'a>, c: &str, hold: &str) -> Option<&'a Fact> {
    v.of("hold_answered", c)
        .filter(|f| text(f, "hold") == Some(hold) && text(f, "choice") != Some("release") && !crate::dependency::ended(v, f))
        .last()
}

/// The latest investigator transfer of `c` whose child did not end unfinished.
pub fn last_investigator<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_investigator_skipping(v, c, None)
}

/// `last_investigator`, leaving out the transfer `skip` (matched by its position); `None` leaves out none.
pub fn last_investigator_skipping<'a>(v: &View<'a>, c: &str, skip: Option<&Fact>) -> Option<&'a Fact> {
    v.of("criterion_transfer", c)
        .filter(|f| {
            crate::ladder::investigates(v, f) && !crate::ladder::unfinished(v, f) && skip.is_none_or(|s| s.t != f.t)
        })
        .last()
}

/// Count tries that should have gone red and did not while no laptop seal stands, at the
/// criterion's own commits since the first stamp, the last red run's end, the last answer, or
/// the last investigator transfer. A try is a tests-only run that passed, a code run that failed
/// or touched code, or new code not yet run.
pub fn tries_not_red(v: &View, c: &str) -> usize {
    if let Some(x) = v.memo.borrow().tries.get(c) {
        return *x;
    }
    let x = tries_uncached(v, c, None);
    v.memo.borrow_mut().tries.insert(c.to_string(), x);
    x
}

/// `tries_not_red`, reading the investigator transfer `skip` as if it had not been sent. The plain
/// reader memoises its answer per `View`; this form computes afresh and neither reads nor writes
/// that cache, because a cached answer is for no skip. `None` computes the plain answer, uncached.
pub fn tries_not_red_skipping(v: &View, c: &str, skip: Option<&Fact>) -> usize {
    tries_uncached(v, c, skip)
}

fn tries_uncached(v: &View, c: &str, skip: Option<&Fact>) -> usize {
    if crate::criterion::seal_raw(v, c, Place::Laptop).is_some() {
        return 0;
    }
    let own = own_tests(v, c);
    let laptop = results(v, c, Place::Laptop, false);
    // A laptop stamp with no laptop result for an own test behind it (one the seed copied from
    // the plan) is no try (the owner, 2026-10-15).
    let stamps: Vec<&Fact> = mine(v, c, "fingerprint_stamped")
        .into_iter()
        .filter(|s| ran_in(s, Place::Laptop) && own.iter().any(|t| ran_at(&laptop, s, &t.file_path).is_some()))
        .collect();
    let Some(first) = stamps.first() else {
        return 0;
    };
    let red = red_runs(v, c).last().map(|r| t_of(r));
    let broke = red.map(|r| v.facts.iter().find(|f| t_of(f) > r && touches_tests_of(v, c, f)).map(t_of).unwrap_or(INF));
    let since = t_of(first)
        .max(broke.unwrap_or(-1))
        .max(last_answer(v, c, "two_tries_not_red").map(t_of).unwrap_or(-1))
        .max(last_investigator_skipping(v, c, skip).map(t_of).unwrap_or(-1));
    let ix = v.idx();
    let content = |id: Option<&str>| id.and_then(|id| ix.key_of(id));
    let passed = |s: &Fact| own.iter().any(|t| is(v, c, ran_at(&laptop, s, &t.file_path), Family::Green));
    let failed = |s: &Fact| own.iter().any(|t| agents_failure(v, c, ran_at(&laptop, s, &t.file_path)));
    let code = |s: &Fact| {
        content(text(s, "commit")).and_then(|k| ix.rep.get(k)).and_then(|&i| v.at(i)).is_some_and(|f| flag(f, "touches_code"))
    };
    let wrong = stamps
        .iter()
        .filter(|s| t_of(s) >= since && stamp_touches(v, c, s))
        .filter(|s| if tests_first_run(v, c, s) { passed(s) } else { failed(s) || code(s) })
        .count();
    let ran: Vec<Option<&str>> = stamps.iter().map(|s| content(text(s, "commit"))).collect();
    // Code content first landed since and not yet run; a rebase lands no new content.
    let unrun = ix
        .keys
        .iter()
        .filter(|k| {
            ix.first_at(Some(k)) >= since
                && ix.rep.get(k.as_str()).and_then(|&i| v.at(i)).is_some_and(|f| flag(f, "touches_code") && touches(v, c, f))
                && !ran.contains(&Some(k.as_str()))
        })
        .count();
    wrong + unrun
}

/// Whether a laptop stamp ever passed under the same proof with no change on the anchor.
/// It stays true until a re-anchor.
pub fn untouched(v: &View, c: &str) -> bool {
    if let Some(x) = v.memo.borrow().untouched.get(c) {
        return *x;
    }
    let x = mine(v, c, "fingerprint_stamped").into_iter().any(|s| {
        if !ran_in(s, Place::Laptop) {
            return false;
        }
        let l = v.prefix(s.t as usize + 1);
        let id = text(s, "commit").unwrap_or("");
        let ix = l.idx();
        let x = ix.key_of(id);
        crate::criterion::seal_raw(&l, c, Place::Laptop).is_some()
            && later(&l, c, x)
            && passed_there(&l, c, Place::Laptop, x)
            && same_stamp_there(&l, c, Place::Laptop, x)
            && !anchor_touched(&l, c, x)
    });
    v.memo.borrow_mut().untouched.insert(c.to_string(), x);
    x
}

/// The criterion's holds, memoized per view.
pub fn holds(v: &View, c: &str) -> Rc<Vec<Hold>> {
    if let Some(h) = v.memo.borrow().holds.get(c) {
        return h.clone();
    }
    let h = Rc::new(crate::holds::holds_of(v, c));
    v.memo.borrow_mut().holds.insert(c.to_string(), h.clone());
    h
}

/// The whys of a criterion's holds.
pub fn whys(v: &View, c: &str) -> Vec<String> {
    holds(v, c).iter().map(|h| h.why.clone()).collect()
}

/// Whether the binary runs the criterion and an agent works on it: its turn, not released, and
/// testing, sealed or held. An announcement made while green opens it again until the edit it
/// covers is run and stamped.
pub fn runnable(v: &View, c: &str) -> bool {
    turn(v, c)
        && !released(v, c)
        && (matches!(state(v, c), "testing" | "sealed" | "held")
            || (matches!(state(v, c), "green" | "proven")
                && announced(v, c)
                && crate::criterion::green_at_announcement(v, c)))
}

/// Every runnable criterion, in plan order.
pub fn runnable_criteria(v: &View) -> Vec<String> {
    if let Some(r) = v.memo.borrow().runnable.as_ref() {
        return r.clone();
    }
    let r: Vec<String> = plan::criteria(v).into_iter().filter(|c| runnable(v, c)).collect();
    v.memo.borrow_mut().runnable = Some(r.clone());
    r
}

/// The runnable criterion the conversation holds, with no fallback to another conversation's.
pub fn lane(v: &View, conversation: Option<&str>) -> Option<String> {
    let conv = conversation?;
    runnable_criteria(v).into_iter().find(|c| plan::assigned_to(v, c) == Some(conv))
}

/// The part an edit of the conversation is for: its lane, else a waiting part it holds, whose
/// edits are refused as its commits are (S9a's two, the owner, 2026-10-16).
pub fn edit_lane(v: &View, conversation: Option<&str>) -> Option<String> {
    lane(v, conversation).or_else(|| {
        let conv = conversation?;
        plan::criteria(v).into_iter().find(|c| plan::assigned_to(v, c) == Some(conv) && state(v, c) == "waiting")
    })
}

/// Whether `c` was touched in this checkout (the owner, 2026-10-15): a fact after the latest
/// `record_seeded` names it and is an agent's claim, or a laptop result, which the runner writes
/// when this checkout's commits touched it. With no mark every part is: the record is main's, or
/// older than the mark. The seed's own facts, the binary's (`test_written`, `commit_seen`, stamps)
/// and the person's `hold_answered` touch nothing. Memoised per view.
pub fn touched_here(v: &View, c: &str) -> bool {
    if let Some(x) = v.memo.borrow().touched.get(c) {
        return *x;
    }
    // the agent's claims (plan §6.8)
    const CLAIMS: [&str; 12] = [
        "criterion",
        "criterion_assigned",
        "criterion_transfer",
        "place_chosen",
        "new_work_spot_chosen",
        "new_work_no_spot",
        "criterion_head",
        "criterion_strung_on_head",
        "test_planned",
        "touching_test_declared",
        "test_change_announced",
        "fix_location_category",
    ];
    let x = match v.facts.iter().rposition(|f| f.kind == "record_seeded") {
        None => true,
        Some(m) => v.facts[m + 1..].iter().any(|f| {
            crate::reader::is_of(f, c)
                && (CLAIMS.contains(&f.kind) || (f.kind == "pass_please_pass" && ran_in(f, Place::Laptop)))
        }),
    };
    v.memo.borrow_mut().touched.insert(c.to_string(), x);
    x
}

/// Whether the stamp's commit is `c`'s, by its tests or its anchor.
pub fn stamp_touches(v: &View, c: &str, s: &Fact) -> bool {
    text(s, "commit").and_then(|id| commit_of(v, id)).is_some_and(|f| touches(v, c, f))
}

/// Whether the criterion is held on a pausing hold.
pub fn paused(v: &View, c: &str) -> bool {
    holds(v, c).iter().any(|h| PAUSING.contains(&h.why.as_str()))
}

/// Whether the command in force at the tests-first stamp differs from the one in force now.
pub fn command_changed(v: &View, c: &str) -> bool {
    let Some(tf) = tests_first_commit(v, c) else {
        return false;
    };
    stamp_at(v, c, Place::Laptop, Some(&tf.key))
        .is_some_and(|sl| command_at(v, c, t_of(sl)) != command_at(v, c, INF))
}

/// Whether the agent announced a change to a sealed test since the seal and no stamp of another
/// proof has landed since, so each announcement covers one edit.
pub fn announced(v: &View, c: &str) -> bool {
    let (Some(s), Some(a)) = (crate::criterion::seal_raw(v, c, Place::Laptop), crate::criterion::announcement(v, c)) else {
        return false;
    };
    a.t > s.t && crate::criterion::proof(v, c) == crate::criterion::proof(&v.prefix(a.t as usize), c)
}

/// Whether the latest tests commit changed sealed tests `announced` or `unannounced`, or `None`.
pub fn test_change(v: &View, c: &str) -> Option<&'static str> {
    if let Some(x) = v.memo.borrow().test_change.get(c) {
        return *x;
    }
    let x = test_change_uncached(v, c);
    v.memo.borrow_mut().test_change.insert(c.to_string(), x);
    x
}

fn test_change_uncached(v: &View, c: &str) -> Option<&'static str> {
    let e = epoch(v, c);
    let f = v
        .facts
        .iter()
        .rev()
        .find(|x| t_of(x) > e && touches_tests_of(v, c, x))?;
    let l = v.prefix(f.t as usize);
    crate::criterion::seal_raw(&l, c, Place::Laptop)?;
    // Compare with the proof in force before the commit, and read the announcement there.
    if text(f, "commit").and_then(|id| crate::criterion::tests_hash_at(v, c, id)) == crate::criterion::proof(&l, c) {
        return None;
    }
    if announced(&l, c) {
        Some("announced")
    } else {
        Some("unannounced")
    }
}

/// Whether the criterion is in state `s`; the states are exclusive by construction.
pub fn state_is(v: &View, c: &str, s: &str) -> bool {
    let conf = confirmed(v);
    let held = || conf && !holds(v, c).is_empty();
    let proven = || conf && !held() && turn(v, c) && proven_conditions(v, c);
    let green = || conf && !held() && !proven() && turn(v, c) && green_conditions(v, c, Place::Laptop);
    // A-5 (lane a5): green waits while a re-check of the seal under a later setup is owed
    let green = || green() && crate::reading::setup_rechecks_owed(v, c).is_empty();
    let sealed = || {
        conf && !held() && !proven() && !green() && turn(v, c) && sealed_at(v, c, Place::Laptop).is_some()
    };
    match s {
        "held" => held(),
        "proven" => proven(),
        "green" => green(),
        "sealed" => sealed(),
        "testing" => conf && !held() && !proven() && !green() && !sealed() && turn(v, c),
        "waiting" => conf && !held() && !turn(v, c),
        "unplanned" => !conf,
        _ => false,
    }
}

/// The first state in order that holds, memoized.
pub fn state(v: &View, c: &str) -> &'static str {
    if let Some(s) = v.memo.borrow().state.get(c) {
        return s;
    }
    let s = CRIT_ORDER
        .iter()
        .copied()
        .find(|s| state_is(v, c, s))
        .unwrap_or("?");
    v.memo.borrow_mut().state.insert(c.to_string(), s);
    s
}
