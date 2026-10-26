//! Write the person's pick from Claude Code's picker. Besides the person's own
//! `familiar-ai claim hold_answered`, this is the only writer of a card's answer,
//! and it reads only the card the hook itself put in the picker.

use std::path::{Path, PathBuf};

use familiar_ai_facts::{HoldAnswered, Log, Record};
use familiar_ai_rules::View;
use serde_json::{json, Map, Value};

use crate::decide::picker::{self as decide, Elsewhere, Picked};
use crate::door::project::{open_record, other_checkouts, relayed_cards};
use crate::door::{cannot, project_plan, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::input::Surroundings;
use crate::teach::picker as words;

/// What the pick comes to: a line for the agent and one for the person.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Told {
    pub context: Option<String>,
    pub message: Option<String>,
}

/// The cases card's "Add my own" picked with nothing typed [draft]: no answer (the rules'
/// `cases::answers`), so, as "something else" picked with nothing typed, nothing is recorded and
/// the question stays open; `None` for any other pick.
fn own_case_untyped(log: &Log, picked: &Picked) -> Option<Picked> {
    use familiar_ai_rules::cases::{CARD, TYPED};
    let Picked::Answer { criterion_id, hold, choice, typed } = picked else {
        return None;
    };
    if hold != CARD || choice != TYPED || !typed.trim().is_empty() {
        return None;
    }
    let label = familiar_ai_rules::terminal::cases_options(&View::new(log), criterion_id).into_iter().find(|(ch, ..)| *ch == TYPED).map_or_else(|| TYPED.to_string(), |(_, l, _)| l);
    Some(Picked::NothingTyped { criterion_id: criterion_id.clone(), label })
}

/// Write the person's pick. `Err` only when the record cannot be written.
pub(crate) fn write(record: &Record, root: &Path, log: &mut Log, picked: &Picked, around: &Surroundings) -> Result<Told, String> {
    if let Some(untyped) = own_case_untyped(log, picked) {
        return write(record, root, log, &untyped, around);
    }
    let cannot_write = |e: familiar_ai_facts::RecordError| cannot("write its record", &e);
    match picked {
        Picked::Answer { criterion_id, hold, choice, typed } => {
            let fact = HoldAnswered { criterion_id: criterion_id.clone(), hold: hold.clone(), choice: choice.clone(), typed: typed.clone() };
            // a reading picked is the person's words, quoted as they read them [draft]
            let what = if !typed.is_empty() {
                format!("\"{typed}\"")
            } else if hold == familiar_ai_rules::ask::HOLD {
                format!("\"{choice}\"")
            } else if hold == familiar_ai_rules::trace::REPRO {
                // the one question [draft]: the choice as the person read it
                let read = if choice == familiar_ai_rules::trace::TYPE_IT { familiar_ai_rules::trace::TYPE_IT_WORDS } else { familiar_ai_rules::trace::GO_AHEAD_WORDS };
                format!("\"{read}\"")
            } else {
                crate::teach::words::choice(choice)
            };
            let before = log.clone();
            let message = words::recorded(&View::new(&before), criterion_id, &what);
            // §6.4: covered_by_dependency closes the part as covered by the dependency it was
            let dep = (hold == "miscoped" && choice == "covered_by_dependency")
                .then(|| familiar_ai_rules::dependency::miscoped_by(&View::new(&before), criterion_id))
                .flatten();
            // H-4: its_the_red and its_the_pass are bound to the can't-tell card's run
            let bound = familiar_ai_rules::criterion::CONFIRMS
                .contains(&choice.as_str())
                .then(|| familiar_ai_rules::criterion::confirmable(&View::new(&before), criterion_id, choice).map(str::to_string))
                .flatten();
            match bound {
                Some(run_id) => {
                    let fields: Map<String, Value> = json!({ "criterion_id": criterion_id, "hold": hold, "choice": choice, "typed": typed, "run_id": run_id })
                        .as_object()
                        .cloned()
                        .unwrap_or_default();
                    log.push(record.append_raw("hold_answered", fields, around.now, TENANT).map_err(cannot_write)?);
                }
                None => log.push(record.append(&fact, around.now, TENANT).map_err(cannot_write)?),
            }
            if let Some(dep) = dep {
                let fields: Map<String, Value> = json!({ "criterion_id": criterion_id, "by_criterion": dep }).as_object().cloned().unwrap_or_default();
                log.push(record.append_raw("criterion_covered", fields, around.now, TENANT).map_err(cannot_write)?);
            }
            // D13 (plan §7.3): the person's a_dependency is drafted now, the record let go meanwhile
            crate::door::dependency::after_pick(record, root, log, around)?;
            // F12: keep_going on anchor_untouched appends the place where the change landed now
            crate::door::anchor::binary_checks(record, root, log, around.now, TENANT).map_err(cannot_write)?;
            // places before the fix (the owner's plan of 2026-10-21, step 3) [draft]: the cases card's answer,
            // the plan's last step [draft], lists the places it owes, as a claim does; other picks list none
            if hold == familiar_ai_rules::cases::CARD {
                crate::door::places::write_owed(record, root, log, around.now, TENANT).map_err(cannot_write)?;
            }
            // the CTA is read on the log after the pick (§6.7)
            // a part a subagent holds: its holder is told (door::told), the orchestrator that it will be
            let elsewhere = familiar_ai_rules::plan::assigned_to(&View::new(log), criterion_id).is_some_and(|h| h.contains('/'));
            let context = if elsewhere {
                words::told_holder(&View::new(&before), criterion_id, &what)
            } else if typed.is_empty() {
                words::told_after(&View::new(&before), &View::new(log), criterion_id, hold, choice, &what)
            } else {
                words::told(&View::new(&before), criterion_id, &what)
            };
            Ok(Told { context: Some(context), message: Some(message) })
        }
        Picked::Command { criterion_id, command } => {
            let stack = command.split_whitespace().next().unwrap_or("").to_string();
            let mut fields: Map<String, Value> =
                json!({ "project_root": ".", "test_stack": stack, "test_command": command }).as_object().cloned().unwrap_or_default();
            let v = View::new(log);
            let what = format!("the tests run with `{command}`");
            if let Err(e) = crate::door::claim_cmd::fill_in("test_stack_detected", &mut fields, root, log, around) {
                // Nothing is written and the question stays open.
                let line = format!("{} · not recorded: {e}", words::recorded(&v, criterion_id, &what));
                return Ok(Told { context: Some(line.clone()), message: Some(line) });
            }
            let told = Told { context: Some(words::told(&v, criterion_id, &what)), message: Some(words::recorded(&v, criterion_id, &what)) };
            log.push(record.append_raw("test_stack_detected", fields, around.now, TENANT).map_err(cannot_write)?);
            Ok(told)
        }
        Picked::NothingTyped { criterion_id, label } => {
            let line = words::nothing_typed(&View::new(log), criterion_id, label);
            Ok(Told { context: Some(line), message: None })
        }
        Picked::Push(answer) => Ok(Told { context: Some(words::push_chosen(answer)), message: None }),
    }
}

/// Each other checkout's record as read, with `record` let go meanwhile so
/// two checkouts never wait on each other; a busy or unreadable one is left out.
pub(crate) fn read_elsewhere(record: &Record, root: &Path, around: &Surroundings) -> Vec<(PathBuf, Elsewhere)> {
    let wait = around.record_wait;
    record
        .while_released(wait, || other_checkouts(root, wait))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| elsewhere_of(&o).map(|e| (o.root, e)))
        .collect()
}

/// Another checkout as the picker reads it, with the cards it relays; `None` when unreadable.
fn elsewhere_of(o: &crate::door::project::Other) -> Option<Elsewhere> {
    let (log, _) = o.read.as_ref().ok()?;
    Some(Elsewhere { folder: o.folder.clone(), cards: relayed_cards(o, log), log: log.clone() })
}

/// The person's pick on another checkout's card, written to that checkout's
/// record (with its plan file), never this one's; `Ok(None)` when the call is
/// not one. `own` is this checkout's facts; `record` is let go meanwhile.
pub(crate) fn write_elsewhere(record: &Record, input: &HookInput, own: &Log, root: &Path, around: &Surroundings) -> Result<Option<Told>, String> {
    if input.event != HookEvent::PostToolUse || !decide::is_picker(input) || !decide::persons_session(input) {
        return Ok(None);
    }
    let wait = around.record_wait;
    let written = record.while_released(wait, || -> Result<Option<Told>, String> {
        for o in other_checkouts(root, wait) {
            let Some(e) = elsewhere_of(&o) else {
                continue;
            };
            if decide::picked_elsewhere(input, own, &e).is_none() {
                continue;
            }
            let other = match open_record(&o.root, wait) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                Err(e) => return Err(cannot("write its record", &e)),
            };
            let mut log = other.read_all().map_err(|e| cannot("read its record", &e))?;
            // Read again with the record held: the card must still be open.
            let again = Elsewhere { folder: o.folder.clone(), cards: relayed_cards(&o, &log), log: log.clone() };
            let Some(picked) = decide::picked_elsewhere(input, own, &again) else {
                return Ok(None);
            };
            let picked = own_case_untyped(&log, &picked).unwrap_or(picked);
            let told = write(&other, &o.root, &mut log, &picked, around)?;
            project_plan(&o.root, &log)?;
            // the lane's holder gets the CTA (door::told); this reply keeps today's line
            let plain = match &picked {
                Picked::Answer { criterion_id, choice, typed, .. } => {
                    let what = if typed.is_empty() { crate::teach::words::choice(choice) } else { format!("\"{typed}\"") };
                    Some(words::told(&View::new(&log), criterion_id, &what))
                }
                _ => told.context,
            };
            return Ok(Some(Told {
                context: plain.map(|c| words::told_in(&c, &o.folder)),
                message: told.message.map(|m| words::recorded_in(&m, &o.folder)),
            }));
        }
        Ok(None)
    });
    written.map_err(|e| cannot("read its record", &e))?
}

/// At the person's prompt and session start, each other checkout's open card,
/// in the words the holder is told, naming the checkout, so the agent asks it.
/// Not in a session with no one to ask (the bench, a headless lane).
pub(crate) fn told_elsewhere(record: &Record, input: &HookInput, root: &Path, around: &Surroundings) -> Option<String> {
    if !matches!(input.event, HookEvent::SessionStart | HookEvent::UserPromptSubmit) || !decide::persons_session(input) || around.bench {
        return None;
    }
    let elsewhere = read_elsewhere(record, root, around);
    let mut lines: Vec<String> = elsewhere
        .iter()
        .filter_map(|(_, e)| {
            let v = View::new(&e.log);
            decide::persons_card(e).map(|card| words::facts_in(&v, &card, &e.folder))
        })
        .collect();
    // An ended lane's answered card: how to resume it (plan §6.7), told once.
    let lanes: Vec<(PathBuf, Log)> = elsewhere.into_iter().map(|(r, e)| (r, e.log)).collect();
    lines.extend(crate::door::told::resume_lines(record, &lanes, around));
    (!lines.is_empty()).then(|| lines.join("\n"))
}
