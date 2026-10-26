//! Every hold on a criterion and the moves that clear it.
//! A hold's `clears` names each admissible move by its `kind` and the fields that must match.

use familiar_ai_facts::Fact;
use serde_json::{Map, Value};

use crate::criterion::{
    answers_of, epoch, mine, overlaps, own_tests, ran_in, result_at, results,
    stamp_at, tests_first_commit, tname, touches_tests_of, touching_tests, Clear, Hold, Place,
    PLAN_KINDS,
};
use crate::lifecycle::{
    attempts, command_changed, green_conditions, later, planning_check, released, stack_missing,
    tries_not_red, untouched,
};
use crate::outcome::{agents_failure, is, Family};
use crate::plan;
use crate::reader::{flag, last_of, text, t_of, View, INF};

fn clear(kind: &str) -> Clear {
    let mut m = Map::new();
    m.insert("kind".into(), Value::from(kind));
    m
}

/// A tests-only `commit_seen`.
pub fn tests_only_clear() -> Clear {
    let mut m = clear("commit_seen");
    m.insert("touches_tests".into(), Value::Bool(true));
    m.insert("touches_code".into(), Value::Bool(false));
    m
}

/// A code-only `commit_seen`.
pub fn code_clear() -> Clear {
    let mut m = clear("commit_seen");
    m.insert("touches_tests".into(), Value::Bool(false));
    m.insert("touches_code".into(), Value::Bool(true));
    m
}

/// A new `place_chosen`.
pub fn reanchor_clear() -> Clear {
    clear("place_chosen")
}

/// The `hold_answered` moves of a hold a person answers.
fn answer_clears(why: &str) -> Vec<Clear> {
    answers_of(why)
        .map(|choices| {
            choices
                .iter()
                .map(|choice| {
                    let mut m = clear("hold_answered");
                    m.insert("hold".into(), Value::from(why));
                    m.insert("choice".into(), Value::from(*choice));
                    m
                })
                .collect()
        })
        .unwrap_or_default()
}

fn hold(why: &str, clears: Vec<Clear>) -> Hold {
    Hold {
        why: why.to_string(),
        clears,
    }
}

/// A `place_classified` of each listed site of `c` left unclassified.
pub fn place_clears(v: &View, c: &str) -> Vec<Clear> {
    crate::places::unclassified(v, c)
        .into_iter()
        .map(|s| {
            let mut m = clear(crate::places::CLASSIFIED);
            m.insert("criterion_id".into(), Value::from(c));
            m.insert("file".into(), Value::from(s.file));
            m.insert("line".into(), Value::from(s.line));
            m
        })
        .collect()
}

/// What clears `reading_unclear` on `c` [draft]: the agent's word on the ask while it is owed, else
/// the person's pick of each reading on the card.
pub fn reading_clears(v: &View, c: &str) -> Vec<Clear> {
    if crate::ask::owed(v, c) {
        return vec![clear(crate::ask::ONE), clear(crate::ask::ALTERNATIVES)];
    }
    crate::ask::readings(v, c)
        .into_iter()
        .map(|r| {
            let mut m = clear("hold_answered");
            m.insert("hold".into(), Value::from(crate::ask::HOLD));
            m.insert("choice".into(), Value::from(r));
            m
        })
        .collect()
}

/// What clears `trace::REPRO` on `c` [draft]: the person's answer, either choice or words typed.
pub fn repro_clears(c: &str) -> Vec<Clear> {
    [crate::trace::TYPE_IT, crate::trace::GO_AHEAD]
        .iter()
        .map(|ch| {
            let mut m = clear("hold_answered");
            m.insert("criterion_id".into(), Value::from(c));
            m.insert("hold".into(), Value::from(crate::trace::REPRO));
            m.insert("choice".into(), Value::from(*ch));
            m
        })
        .collect()
}

/// What clears `cases::OWED` on `c` [draft]: the binary's record of the case-finder's answer.
pub fn cases_owed_clears(c: &str) -> Vec<Clear> {
    let mut m = clear(crate::cases::FOUND);
    m.insert("criterion_id".into(), Value::from(c));
    vec![m]
}

/// What clears `cases::CARD` on `c` [draft]: the person's answer, one of the card's choices or words typed.
pub fn cases_card_clears(v: &View, c: &str) -> Vec<Clear> {
    crate::cases::offered(v, c)
        .into_iter()
        .filter(|ch| *ch != crate::cases::TYPED)
        .chain(std::iter::once(crate::cases::TYPED))
        .map(|ch| {
            let mut m = clear("hold_answered");
            m.insert("criterion_id".into(), Value::from(c));
            m.insert("hold".into(), Value::from(crate::cases::CARD));
            m.insert("choice".into(), Value::from(ch));
            m
        })
        .collect()
}

/// What clears the try's holds on `c` [draft]: the binary's listing after `familiar-ai try`.
pub fn try_clears(c: &str) -> Vec<Clear> {
    let mut m = clear(crate::trace::LISTED);
    m.insert("criterion_id".into(), Value::from(c));
    vec![m]
}

/// The holds of one criterion, in the reference model's order; the hooks read the first.
pub fn holds_of(v: &View, c: &str) -> Vec<Hold> {
    if c == crate::reading::SETUP_PART {
        return setup_holds(v);
    }
    if released(v, c) {
        // A released criterion is held for good with why `released`, which blocks nothing.
        return vec![hold("released", Vec::new())];
    }
    let mut out: Vec<Hold> = Vec::new();
    let a = plan::anchor_of(v, c);
    let ix = v.idx();
    let e = epoch(v, c);
    let rep = |k: &str| ix.rep.get(k).and_then(|&i| v.at(i));
    if !crate::criterion::confirmed(v) {
        out.push(hold("planning_incomplete", PLAN_KINDS.iter().map(|k| clear(k)).collect()));
    }
    // test first, then pick the spot (the owner's plan of 2026-10-23) [draft]: the person's card on
    // how to make it happen, before the first try and before reading_unclear; then the agent's try
    if crate::trace::repro_unknown(v, c) {
        out.push(hold(crate::trace::REPRO, repro_clears(c)));
    }
    // the cases card (the owner's plan of 2026-10-25) [draft]: the case-finder, then the person's card
    if crate::cases::owed(v, c) {
        out.push(hold(crate::cases::OWED, cases_owed_clears(c)));
    }
    if crate::cases::card_open(v, c) {
        out.push(hold(crate::cases::CARD, cases_card_clears(v, c)));
    }
    if let Some(why) = crate::trace::try_hold(v, c) {
        out.push(hold(why, try_clears(c)));
    }
    let pc = planning_check(v, c);
    if a.is_some_and(|a| a.kind == "place_chosen")
        && pc.is_some_and(|pc| pc.fields.get("matched") == Some(&Value::Bool(false)))
    {
        out.push(hold("anchor_not_found", vec![reanchor_clear()]));
    }
    if stack_missing(v, c) {
        out.push(hold("no_test_stack", vec![clear("test_stack_detected")]));
    }
    // places before the fix (the owner's plan of 2026-10-21, step 3) [draft]: the agent's hold
    // while a listed site is unclassified, cleared by classifying each
    if crate::places::holds(v, c) {
        out.push(hold(crate::places::HOLD, place_clears(v, c)));
    }
    let tf = tests_first_commit(v, c);
    let own = own_tests(v, c);
    let lat = |t: &str| -> Option<&Fact> {
        tf.as_ref()
            .and_then(|tf| result_at(v, c, Place::Laptop, false, t, Some(&tf.key)))
    };
    if tf.is_some() && own.iter().any(|t| is(v, c, lat(&t.file_path), Family::Green)) {
        out.push(hold("tests_not_red", vec![tests_only_clear()]));
    }
    // The run's own holds (plan §6.1, §6.3): read from each own test's latest laptop result.
    let unsealed = crate::criterion::seal_raw(v, c, Place::Laptop).is_none();
    let newest: Vec<&Fact> = {
        let laptop = results(v, c, Place::Laptop, false);
        own.iter().filter_map(|t| laptop.iter().rev().find(|r| tname(r) == t.file_path).copied()).collect()
    };
    // an approval the binary missed twice answers nothing: its card opens again (plan §7.3)
    let answered_after = |why: &str, r: &Fact| {
        v.of("hold_answered", c).any(|f| f.t > r.t && text(f, "hold") == Some(why) && !crate::dependency::ended(v, f))
    };
    // Test is broken or too slow at the tests-first commit, or a command never valid: back to step 3.
    let broken_first = tf.is_some()
        && unsealed
        && own.iter().any(|t| is(v, c, lat(&t.file_path), Family::TestBroken) || is(v, c, lat(&t.file_path), Family::TooSlow));
    let never_valid = newest.iter().any(|r| {
        crate::outcome::result_family(v, c, r) == Family::CommandBroken
            && text(r, "run_id").is_some_and(|id| crate::outcome::good_run_before(v, id).is_none())
    });
    if broken_first || never_valid {
        out.push(hold("red_for_another_reason", vec![tests_only_clear(), clear("test_stack_detected")]));
    }
    // A crash before the seal is the person's card (§6.1).
    if let Some(r) = tf.as_ref().and(unsealed.then_some(())).and_then(|_| {
        own.iter().filter_map(|t| lat(&t.file_path)).find(|r| is(v, c, Some(r), Family::Crashed))
    }) {
        if !answered_after("crashed", r) {
            out.push(hold("crashed", answer_clears("crashed")));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for r in &newest {
        if let Some(why) = crate::outcome::card_hold_of(v, c, r) {
            // a run that carries the person's earlier its_the_red raises no card [draft]
            let carried = why == "run_unclassified" && crate::criterion::confirmed_as(v, c, r, "its_the_red");
            if !seen.contains(&why) && !answered_after(why, r) && !carried {
                seen.push(why);
                out.push(hold(why, answer_clears(why)));
            }
        }
    }
    // A can't-tell red at the tests-first commit stays the person's card until it is answered or a
    // seal stands, whatever ran after it (the bench's 13363; the owner, 2026-10-20).
    if unsealed && !seen.contains(&"run_unclassified") {
        let first = own.iter().filter_map(|t| lat(&t.file_path)).find(|r| crate::outcome::result_family(v, c, r) == Family::Unknown);
        if first.is_some_and(|r| !answered_after("run_unclassified", r) && !crate::criterion::confirmed_as(v, c, r, "its_the_red")) {
            seen.push("run_unclassified");
            out.push(hold("run_unclassified", answer_clears("run_unclassified")));
        }
    }
    // §7.3 step 6: the dependency's lines left the plan incomplete (a place the stack does not
    // cover, in `.venv`): the environment_error card, until the person answers it
    if !seen.contains(&"environment_error") && crate::dependency::plan_refused(v, c).is_some() {
        out.push(hold("environment_error", answer_clears("environment_error")));
    }
    // §6.4: read from S9a's re-earn, fixed at t4; it stands until the person answers it
    if let Some(r) = crate::dependency::re_earn(v, c).filter(|_| crate::dependency::miscoped(v, c)) {
        if !v.of("hold_answered", c).any(|f| f.t > r.approval && text(f, "hold") == Some("miscoped")) {
            out.push(hold("miscoped", answer_clears("miscoped")));
        }
    }
    let stamps: Vec<&Fact> = mine(v, c, "fingerprint_stamped")
        .into_iter()
        .filter(|f| ran_in(f, Place::Laptop))
        .collect();
    let stamped_before = |k: &str| stamps.iter().any(|s| t_of(s) < ix.first_at(Some(k)));
    // `keys` is in the order contents first landed, so the latest match is the last one
    let latest_key = |p: &dyn Fn(&Fact) -> bool| -> Option<&String> {
        ix.keys
            .iter()
            .rfind(|k| ix.first_at(Some(k)) > e && rep(k).is_some_and(p))
    };
    let code_only = |f: &Fact| !flag(f, "touches_tests") && overlaps(v, c, f);
    let tests_only = |f: &Fact| touches_tests_of(v, c, f) && !flag(f, "touches_code");
    if let Some(kc) = latest_key(&code_only) {
        if !stamped_before(kc) {
            // The latest code on its anchor came before any stamp.
            let by = if stamps.is_empty() { tests_only_clear() } else { code_clear() };
            out.push(hold("not_stamped", vec![by]));
        }
    }
    if let Some(km) = latest_key(&|f: &Fact| touches_tests_of(v, c, f) && overlaps(v, c, f)) {
        if !stamped_before(km) {
            let kt = ix
                .keys
                .iter()
                .find(|k| ix.first_at(Some(k)) > ix.first_at(Some(km)) && rep(k).is_some_and(tests_only));
            let fixed = kt.is_some_and(|kt| {
                ix.keys
                    .iter()
                    .any(|k| ix.first_at(Some(k)) > ix.first_at(Some(kt)) && rep(k).is_some_and(code_only))
            });
            if !fixed {
                // Tests and code were mixed: commit tests only, then code.
                let by = if kt.is_some() { code_clear() } else { tests_only_clear() };
                out.push(hold("tests_and_code_mixed", vec![by]));
            }
        }
    }
    // Only a move away from a proof that a red tests-first run sealed counts: a seal whose run
    // passed proves nothing, so editing those tests is a fresh seal attempt. A red the person
    // confirmed on its can't-tell card counts as one read (the owner, 2026-10-20).
    let now = crate::criterion::proof(v, c);
    let away = crate::lifecycle::sealing_runs(v, c)
        .iter()
        .any(|s| text(s, "proof_not_tampered") != now);
    let all_red = tf.is_some()
        && !own.is_empty()
        && own.iter().all(|t| {
            let r = lat(&t.file_path);
            is(v, c, r, Family::ValidRed) || r.is_some_and(|r| crate::criterion::confirmed_as(v, c, r, "its_the_red"))
        });
    if away && !all_red {
        // The proof changed and the new tests were not seen red, announced or not.
        out.push(hold("proof_tampered", vec![tests_only_clear()]));
    }
    // an existing test's lines change only when announced (the owner, 2026-10-20) [draft]: the
    // agent's hold, cleared by announcing each file or by putting its lines back
    if crate::existing::holds(v, c) {
        let mut clears: Vec<Clear> = crate::existing::unannounced(v)
            .iter()
            .map(|e| {
                let mut m = clear(crate::existing::ANNOUNCED);
                m.insert("file_path".into(), e.get("file_path").cloned().unwrap_or(Value::Null));
                m
            })
            .collect();
        clears.push(clear("commit_seen"));
        out.push(hold(crate::existing::HOLD, clears));
    }
    if command_changed(v, c) {
        out.push(hold("command_changed", vec![tests_only_clear()])); // Not a pausing hold.
    }
    if untouched(v, c) {
        // The person's answer clears it. The card is offered only while it stands, so an answer
        // since the epoch came after the stamp that raised it. A criterion_transfer does not clear it.
        let answered = mine(v, c, "hold_answered")
            .iter()
            .any(|f| text(f, "hold") == Some("anchor_untouched"));
        if !answered {
            let mut clears = vec![reanchor_clear()];
            clears.extend(answer_clears("anchor_untouched"));
            out.push(hold("anchor_untouched", clears));
        }
    }
    // This proof attempt starts at the latest stamp at its tests-first commit.
    let seal_t = match &tf {
        Some(tf) => stamp_at(v, c, Place::Laptop, Some(&tf.key)).map(t_of).unwrap_or(-1),
        None => INF,
    };
    if tf.is_some() {
        let laptop = results(v, c, Place::Laptop, false);
        let regressed = touching_tests(v, c).iter().any(|t| {
            let at = lat(&t.file_path);
            let tip = last_of(
                laptop
                    .iter()
                    .filter(|r| {
                        tname(r) == t.file_path
                            && t_of(r) > seal_t
                            && later(v, c, text(r, "commit").and_then(|id| ix.key_of(id)))
                    })
                    .copied()
                    .collect(),
            );
            is(v, c, at, Family::Green) && agents_failure(v, c, tip)
        });
        if regressed {
            out.push(hold("regression", vec![code_clear()]));
        }
    }
    // A-5 (lane a5): a re-check of the seal under a later setup read anything but an honest red;
    // the person's re_plan or release clears it, never a later setup
    if crate::reading::setup_changed(v, c) {
        out.push(hold(crate::reading::SETUP_CHANGED, answer_clears(crate::reading::SETUP_CHANGED)));
    }
    // F7: each place against its own planning check; the owner (2026-10-11): not when only
    // this plan's own recorded work moved its lines
    if crate::lifecycle::anchor_moved(v, c) {
        out.push(hold("anchor_moved", answer_clears("anchor_moved")));
    }
    if attempts(v, c) >= 2 && !green_conditions(v, c, Place::Laptop) {
        // The person is always told, or an investigator one rung up takes it.
        let mut clears = answer_clears("two_failures");
        clears.extend(crate::ladder::investigate_clears(v, c));
        out.push(hold("two_failures", clears));
    }
    for key in crate::lifecycle::OTHER_ATTEMPTS {
        // D4 by family (the owner, 2026-10-16): two attempts of one family are its card
        if crate::lifecycle::attempts_of(v, c, key) >= 2 && !green_conditions(v, c, Place::Laptop) {
            let why = crate::lifecycle::two_of(key);
            let clears = answer_clears(&why);
            out.push(hold(&why, clears));
        }
    }
    if tries_not_red(v, c) >= 2 && crate::criterion::sealed_at(v, c, Place::Laptop).is_none() {
        // Two tries to go red failed: the person's card (D5); no investigator is sent.
        out.push(hold("two_tries_not_red", answer_clears("two_tries_not_red")));
    }
    // The reviewer at green (the owner's plan of 2026-10-20, step 3) [draft]: two verified "no"s on
    // one fix, or one since the person's try_again, are the person's card
    if crate::review::disputed(v, c) {
        out.push(hold(crate::review::HOLD, answer_clears(crate::review::HOLD)));
    }
    let teeth = results(v, c, Place::Pipeline, true);
    let no_teeth = own.iter().any(|t| {
        let r = teeth.iter().rev().find(|x| tname(x) == t.file_path).copied();
        is(v, c, r, Family::Green) && r.is_some_and(|r| t_of(r) > seal_t)
    });
    if no_teeth {
        // Read against the current proof only.
        out.push(hold("no_teeth", vec![tests_only_clear()]));
    }
    if let Some(fl) = crate::lifecycle::fix_location(v, c) {
        // The latest claim is not this_repository and the person has not answered since it;
        // read only once planning is confirmed.
        let answered = v
            .of("hold_answered", c)
            .any(|f| text(f, "hold") == Some("fix_elsewhere") && f.t > fl.t);
        if crate::criterion::confirmed(v) && text(fl, "category") != Some("this_repository") && !answered {
            out.push(hold("fix_elsewhere", answer_clears("fix_elsewhere")));
        }
    }
    // a clarifying question on a signal (the owner's plan of 2026-10-21, step 2) [draft]: the
    // agent's word on how the ask reads, then the person's pick of the readings it lists; last, so
    // a card the person answers is named first
    if crate::ask::holds(v, c) {
        out.push(hold(crate::ask::HOLD, reading_clears(v, c)));
    }
    out
}

/// The holds of the reserved part `setup` (plan A-4, lane a5): the gap card while the latest
/// choices carry gaps with no answer after them. It never pauses: no part reads it.
pub fn setup_holds(v: &View) -> Vec<Hold> {
    if crate::reading::gaps_open(v).is_empty() {
        return Vec::new();
    }
    vec![hold(crate::reading::READING_GAP, answer_clears(crate::reading::READING_GAP))]
}

/// An approval the binary verified against the allowed signers available to
/// it: its `t` and the card it signs. The rules read no file and no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approved {
    pub t: String,
    pub criterion_id: String,
    pub hold: String,
    pub choice: String,
}

/// One `hold_answered` of a criterion: its card's hold and choice, the
/// approval it names (`approval`), and `verified`, that approval's `t` when
/// it is among `approved` for the same card; `None` reads "not verified".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub hold: String,
    pub choice: String,
    pub approval: Option<String>,
    pub verified: Option<String>,
}

/// Every answer to a card of `c`, oldest first, each read against the
/// approvals the binary verified (the owner, 2026-10-18: verified only by a
/// signature, never by a file's presence).
pub fn answers(v: &View, c: &str, approved: &[Approved]) -> Vec<Answer> {
    v.of("hold_answered", c)
        .map(|f| {
            let hold = text(f, "hold").unwrap_or("").to_string();
            let choice = text(f, "choice").unwrap_or("").to_string();
            let approval = text(f, "approval").filter(|t| !t.is_empty()).map(str::to_string);
            let verified = approval
                .as_ref()
                .filter(|t| approved.iter().any(|a| &&a.t == t && a.criterion_id == c && a.hold == hold && a.choice == choice))
                .cloned();
            Answer { hold, choice, approval, verified }
        })
        .collect()
}
