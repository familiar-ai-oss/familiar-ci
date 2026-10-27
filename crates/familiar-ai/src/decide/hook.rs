//! The `hook` command, from Claude Code's event to the one answer it prints.
//! A call inside a project runs in stages, each taking the decision so far and returning it,
//! so a later stage may override an earlier one.

use std::path::Path;

use familiar_ai_facts::{Log, Record, RecordError};

use crate::decide::decision::{self, Decision};
use crate::decide::hook_output;
use crate::decide::{investigator, lens, picker, restrict, seal, tell};
use crate::door::investigator_run::{Launch, Returned};
use crate::door::set_up as removed;
use crate::door::{self, cannot, catch_up, catch_up_with_run, project_plan};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::input::Surroundings;
use crate::teach::{self, session_start_context};
use crate::Outcome;

/// Answer one hook call, failing closed: bad input or an unreadable record refuses the tool call.
/// See docs/decisions.md#the-hook-fails-closed.
pub(crate) fn run_hook(event: HookEvent, input: &str, around: &Surroundings) -> Outcome {
    let hook_input = match HookInput::parse(input, event) {
        // A session the binary launched is the conversation it was given.
        Ok(parsed) => parsed.launched(around.conversation.as_deref(), crate::door::investigator::INVESTIGATOR),
        Err(err) => {
            return Outcome {
                exit_code: 2,
                stdout_text: String::new(),
                stderr_text: format!("familiar-ai: the hook's input could not be read, so nothing goes ahead · {err}\n"),
            }
        }
    };
    // Plan mode allows everything and writes nothing, except that SessionStart still gives the rulebook
    // because the mode can change later with no hook; see docs/decisions.md#plan-mode-is-never-blocked.
    if hook_input.permission_mode.as_deref() == Some("plan") && event != HookEvent::SessionStart {
        return hook_output::render(&hook_input, &Decision::allow(), &[]);
    }
    let root = crate::door::project::hook_project(around.project_dir.as_deref(), hook_input.cwd.as_deref().map(Path::new));
    // A slow hook here blocks the model switch, so nothing is opened.
    if event == HookEvent::PreModelSwitch {
        return hook_output::render(&hook_input, &Decision::allow(), &[]);
    }
    // A project that was set up but lost its folder refuses until setup runs again; see
    // docs/decisions.md#a-removed-folder-refuses-until-setup.
    if root.as_deref().is_some_and(|r| removed::plan_removed(r, around.data_dir.as_deref())) {
        return hook_output::render(&hook_input, &removed::decide(&hook_input), &[]);
    }
    let record = match root
        .as_deref()
        .map(|root| crate::door::project::open_record(root, around.record_wait))
    {
        None | Some(Ok(None)) => None,
        Some(Ok(Some(record))) => Some(record),
        Some(Err(RecordError::Busy)) => return hook_output::render_busy(event),
        Some(Err(other)) => {
            let message = format!(
                "Familiar cannot read its record ({}/{}/{}): {}",
                door::FAMILIAR_FOLDER, door::RECORD_FILE, "record", other
            );
            return hook_output::render_cannot_check(event, &message);
        }
    };
    // With no `.familiar` folder only the session's start speaks, naming the setup command.
    let (Some(record), Some(root)) = (record, root) else {
        let decided = match event {
            HookEvent::SessionStart => Decision::Allow { told: Some(crate::door::setup::NOT_SET_UP.to_string()) },
            _ => Decision::allow(),
        };
        return hook_output::render(&hook_input, &decided, &[]);
    };
    // A linked worktree with no switch of its own follows main's.
    let switch = match crate::door::switch::here_holding(&record, &root, around.record_wait) {
        Ok(switch) => switch,
        Err(RecordError::Busy) => return hook_output::render_busy(event),
        Err(e) => return hook_output::render_cannot_check(event, &cannot("read its record", &e)),
    };
    // An agent's command is read by its text before anything else; the switch is the person's even while off.
    if let Some(reason) = crate::door::shell_write::agent_command_refusal(&hook_input, !switch.off) {
        return hook_output::render(&hook_input, &Decision::Refuse { reason }, &[]);
    }
    // Switched off, every hook allows and writes nothing but the person's prompt; see
    // docs/decisions.md#the-off-switch.
    if switch.off {
        return while_off(&hook_input, &record, &root, around, &switch);
    }
    match hook_in_project(&hook_input, &record, &root, around, None, None) {
        Ok(Step::Done(outcome)) => outcome,
        Ok(Step::Launch(go)) => {
            // The investigator's own hooks need the record.
            drop(record);
            after_launch(&hook_input, &go, &root, around)
        }
        Ok(Step::Diagnose(pending)) => {
            // The record is let go while the diagnosis reads the project (S7b).
            drop(record);
            after_diagnosis(&hook_input, *pending, &root, around)
        }
        Err(message) => hook_output::render_cannot_check(event, &message),
    }
}

/// A call while switched off: everything is allowed and nothing written here,
/// but another checkout's card still reaches the person's session: their
/// picker asks it and records the pick there, and their prompt and session
/// start are told it. See docs/decisions.md#the-off-switch.
fn while_off(hook_input: &HookInput, record: &Record, root: &Path, around: &Surroundings, switch: &crate::door::switch::Switch) -> Outcome {
    let event = hook_input.event;
    let log = match record.read_all() {
        Ok(log) => log,
        Err(e) => return hook_output::render_cannot_check(event, &cannot("read its record", &e)),
    };
    // A card of this checkout's own stays unasked while it is off.
    let own_open = picker::open_card(&familiar_ai_rules::View::new(&log), hook_input).is_some();
    if !own_open {
        let elsewhere = || crate::door::picker::read_elsewhere(record, root, around).into_iter().map(|(_, e)| e).collect();
        match picker::rewrite(hook_input, &log, || false, elsewhere) {
            Some(picker::Rewrite::Ask(updated)) => return hook_output::render_ask(&updated),
            Some(picker::Rewrite::Refuse(reason)) => return hook_output::render(hook_input, &Decision::Refuse { reason }, &[]),
            None => {}
        }
        match crate::door::picker::write_elsewhere(record, hook_input, &log, root, around) {
            Ok(Some(told)) => {
                let mut out = hook_output::response(hook_input, &Decision::allow(), &[]);
                if let Some(context) = told.context.as_deref() {
                    out = out.with_context(event, context);
                }
                if let Some(message) = told.message.as_deref() {
                    out = out.with_system_message(message);
                }
                return out.finish();
            }
            Ok(None) => {}
            Err(message) => return hook_output::render_cannot_check(event, &message),
        }
    }
    let mut decided = match crate::door::switch::decide(record, hook_input, around.now, switch) {
        Ok(decided) => decided,
        Err(message) => return hook_output::render_cannot_check(event, &message),
    };
    if let Some(told) = crate::door::picker::told_elsewhere(record, hook_input, root, around) {
        let before = match decided {
            Decision::Allow { told } => told,
            Decision::Refuse { .. } => None,
        };
        decided = Decision::Allow { told: Some(before.map_or_else(|| told.clone(), |b| format!("{b}\n{told}"))) };
    }
    hook_output::render(hook_input, &decided, &[])
}

/// A call's answer, or an investigator for the binary to run before answering.
enum Step {
    Done(Outcome),
    Launch(Launch),
    /// The diagnosis of a test that never went red, and what the call had come to (S7b).
    Diagnose(Box<Pending>),
}

/// A diagnosis to run, and what the call it interrupts had already done and told.
struct Pending {
    go: crate::door::diagnosis::Diagnosis,
    carried: Carried,
}

/// What a call carries across its diagnosis: the record as it began, what the judge and the
/// pick told, and each diagnosis's line.
struct Carried {
    before: Log,
    judged: crate::door::judge::Told,
    picked: crate::door::picker::Told,
    diagnosed: Vec<String>,
}

/// Run the owed diagnosis with the record let go, then write what came back; a miss tries once
/// more in the same call (`finding::MISSES`). Then answer the call with what it had come to.
fn after_diagnosis(hook_input: &HookInput, pending: Pending, root: &Path, around: &Surroundings) -> Outcome {
    let event = hook_input.event;
    let Some(i) = around.investigator.as_ref() else {
        return hook_output::render(hook_input, &Decision::allow(), &[]);
    };
    let Pending { mut go, mut carried } = pending;
    let mut now = around.now;
    for attempt in 1..=familiar_ai_rules::finding::MISSES {
        let ran = crate::door::diagnosis::wait_for(&go, root, i, now);
        // Later facts carry the hook's time plus the child's wall time, since no clock is read here.
        now += ran.wall.as_secs_f64();
        let later = Surroundings { now, ..around.clone() };
        let record = match crate::door::project::open_record(root, later.record_wait) {
            Ok(Some(record)) => record,
            Ok(None) => return hook_output::render_cannot_check(event, "Familiar cannot read its record after the investigator: the .familiar folder is gone"),
            Err(RecordError::Busy) => return hook_output::render_busy(event),
            Err(e) => return hook_output::render_cannot_check(event, &cannot("read its record", &e)),
        };
        let came = catch_up(&record, root, &later).and_then(|mut log| {
            let came = crate::door::diagnosis::back(&record, root, &mut log, &go, &ran, now)?;
            Ok((came, log))
        });
        let (came, log) = match came {
            Ok(x) => x,
            Err(message) => return hook_output::render_cannot_check(event, &message),
        };
        carried.diagnosed.push(teach::investigator::diagnosed(&log, &go.criterion_id, go.alias, &came));
        let again = matches!(came, crate::door::diagnosis::Came::Missed { .. })
            && attempt < familiar_ai_rules::finding::MISSES
            && familiar_ai_rules::finding::owed(&familiar_ai_rules::View::new(&log), &go.criterion_id);
        if !again {
            return match hook_in_project(hook_input, &record, root, &later, None, Some(carried)) {
                Ok(Step::Done(outcome)) => outcome,
                // Unreachable: the call after a diagnosis launches none.
                Ok(_) => hook_output::render(hook_input, &Decision::allow(), &[]),
                Err(message) => hook_output::render_cannot_check(event, &message),
            };
        }
        go = crate::door::diagnosis::prepare(&log, root, &go.criterion_id, now);
        drop(record);
    }
    hook_output::render(hook_input, &Decision::allow(), &[])
}

/// Run the investigator and wait, then reopen the record, write what came back, and answer the call.
fn after_launch(hook_input: &HookInput, go: &Launch, root: &Path, around: &Surroundings) -> Outcome {
    let event = hook_input.event;
    let Some(i) = around.investigator.as_ref() else {
        return hook_output::render(hook_input, &Decision::allow(), &[]);
    };
    let ran = crate::door::investigator_run::launch(go, root, i);
    crate::door::investigator_run::log_run(root, around.now, go, i, &ran);
    // Later facts carry the hook's time plus the child's wall time, since no clock is read here.
    let later = Surroundings { now: around.now + ran.wall.as_secs_f64(), ..around.clone() };
    let around = &later;
    let record = match crate::door::project::open_record(root, around.record_wait) {
        Ok(Some(record)) => record,
        Ok(None) => return hook_output::render_cannot_check(event, "Familiar cannot read its record after the investigator: the .familiar folder is gone"),
        Err(RecordError::Busy) => return hook_output::render_busy(event),
        Err(e) => return hook_output::render_cannot_check(event, &cannot("read its record", &e)),
    };
    let returned = catch_up(&record, root, around).and_then(|mut log| crate::door::investigator_run::back(&record, &mut log, go, &ran, around.now));
    match returned.and_then(|r| hook_in_project(hook_input, &record, root, around, Some(&r), None)) {
        Ok(Step::Done(outcome)) => outcome,
        // Unreachable: with an investigator returned, none is launched again.
        Ok(_) => hook_output::render(hook_input, &Decision::allow(), &[]),
        Err(message) => hook_output::render_cannot_check(event, &message),
    }
}

/// One hook call inside a project, as its stages read and add to it.
struct Call<'a> {
    input: &'a HookInput,
    record: &'a Record,
    root: &'a Path,
    around: &'a Surroundings,
    /// The investigator the binary ran, when this is the call after it.
    returned: Option<&'a Returned>,
    /// Whether this is the call after a diagnosis, which runs none again.
    after_diagnosis: bool,
    /// Each diagnosis's line, for the agent and the person.
    diagnosed: Vec<String>,
    /// The record as the call began, before anything was caught up.
    before: Log,
    /// The record after this call's own facts.
    log: Log,
    /// The OUTCOMES lines of the runs this call settled that the state lines do not tell.
    not_run: Vec<String>,
    judged: crate::door::judge::Told,
    picked: crate::door::picker::Told,
    /// What this conversation is owed and nobody told yet (plan §1.2, §6.7).
    owed: Vec<String>,
    /// B-4: the subagent's claim as it runs instead, on an allow.
    rewrite: Option<serde_json::Value>,
    /// The lines meant for the person (`systemMessage`).
    for_person: Vec<String>,
    judge_on: bool,
}

/// One hook call inside a project, stage by stage. An `Err` names what could not be read.
fn hook_in_project(
    hook_input: &HookInput,
    record: &Record,
    root: &Path,
    around: &Surroundings,
    returned: Option<&Returned>,
    carried: Option<Carried>,
) -> Result<Step, String> {
    let mut call = record_the_call(hook_input, record, root, around, returned, carried)?;
    resumed_at_start(&mut call)?;
    if let Some(answer) = picker_answer(&call) {
        return Ok(Step::Done(answer));
    }
    let idle = is_idle(&call);
    let decided = match first_decision(&call, idle) {
        Ok(decided) => decided,
        Err(answer) => return Ok(Step::Done(answer)),
    };
    let decided = binary_restrictions(&call, decided, idle);
    if let Some(go) = investigator_to_launch(&mut call)? {
        return Ok(Step::Launch(go));
    }
    if let Some(go) = diagnosis_to_run(&call) {
        let carried = Carried { before: call.before, judged: call.judged, picked: call.picked, diagnosed: Vec::new() };
        return Ok(Step::Diagnose(Box::new(Pending { go, carried })));
    }
    let decided = stop_with_cards(&mut call, decided);
    let decided = what_the_decision_tells(&mut call, decided);
    let decided = match investigator_came_back(&mut call, decided) {
        Ok(decided) => decided,
        Err(answer) => return Ok(Step::Done(answer)),
    };
    ended_at_stop(&mut call, &decided)?;
    // B-4 (plan §6.7): an allowed claim of a part by a subagent names the subagent.
    if matches!(decided, Decision::Allow { .. }) {
        call.rewrite = crate::door::shell_write::assigned_rewrite(hook_input);
    }
    tell_once(&mut call)?;
    Ok(Step::Done(respond(call, decided)))
}

/// Every `told` of this call, written last, after any early answer, so a `told` that lands is
/// always spoken in this reply and nothing is spoken that another teller told first (D11).
fn tell_once(call: &mut Call) -> Result<(), String> {
    let (record, now, before) = (call.record, call.around.now, call.before.len());
    let landed = crate::door::told::inline(record, &mut call.log, before, now)?;
    call.not_run = teach::outcome_lines(&landed, &call.log);
    if !crate::door::told::picked_here(record, &mut call.log, before, now)? {
        call.picked.context = None;
    }
    call.owed = crate::door::told::owed(record, &mut call.log, call.input, now)?;
    Ok(())
}

/// At a headless session's start whose session id is a holder that ended (`launch_lane --resume`),
/// write that it runs again: `holder_resumed` (the owner, 2026-10-16).
fn resumed_at_start(call: &mut Call) -> Result<(), String> {
    let input = call.input;
    if input.event != HookEvent::SessionStart || !call.around.bench || input.agent_id.is_some() {
        return Ok(());
    }
    let Some(id) = input.session_id.clone().filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    if !crate::door::told::has_ended(&call.log, &id, 0) {
        return Ok(());
    }
    let fields = serde_json::json!({ "agent_id": id });
    let fact = call.record.append_raw("holder_resumed", fields.as_object().cloned().unwrap_or_default(), call.around.now, door::TENANT).map_err(|e| cannot("write its record", &e))?;
    call.log.push(fact);
    Ok(())
}

/// At an allowed SubagentStop, write that the subagent ended; at a headless session's allowed
/// Stop, that the lane ended (plan §1.2, §6.7). Nothing is told for it.
fn ended_at_stop(call: &mut Call, decided: &Decision) -> Result<(), String> {
    let input = call.input;
    if !matches!(decided, Decision::Allow { .. }) {
        return Ok(());
    }
    let (agent_id, agent_type) = match input.event {
        HookEvent::SubagentStop => (input.agent_id.clone(), input.agent_type.clone().unwrap_or_default()),
        HookEvent::Stop if call.around.bench && input.agent_id.is_none() => (input.session_id.clone(), "lane".to_string()),
        _ => return Ok(()),
    };
    let Some(agent_id) = agent_id.filter(|a| !a.is_empty()) else {
        return Ok(());
    };
    let fields = serde_json::json!({ "agent_id": agent_id, "agent_type": agent_type });
    let fact = call.record.append_raw("subagent_ended", fields.as_object().cloned().unwrap_or_default(), call.around.now, door::TENANT).map_err(|e| cannot("write its record", &e))?;
    call.log.push(fact);
    Ok(())
}

/// Catch the record up, write the facts this call carries, and write the plan file. Decides nothing.
fn record_the_call<'a>(
    hook_input: &'a HookInput,
    record: &'a Record,
    root: &'a Path,
    around: &'a Surroundings,
    returned: Option<&'a Returned>,
    carried: Option<Carried>,
) -> Result<Call<'a>, String> {
    if let Some(carried) = carried {
        // The call after its diagnosis: this call's own facts are written; only the diagnosis's are new.
        let log = catch_up(record, root, around)?;
        project_plan(root, &log)?;
        let for_person = carried.picked.message.clone().into_iter().collect();
        return Ok(Call {
            input: hook_input,
            record,
            root,
            around,
            returned,
            after_diagnosis: true,
            diagnosed: carried.diagnosed,
            before: carried.before,
            log,
            not_run: Vec::new(),
            judged: carried.judged,
            picked: carried.picked,
            owed: Vec::new(),
            rewrite: None,
            for_person,
            judge_on: around.judge_dir.is_some(),
        });
    }
    let before: Log = record.read_all().map_err(|e| cannot("read its record", &e))?;
    // A commit is the agent's only when it landed during its own Bash call.
    let by = (hook_input.event == HookEvent::PostToolUse && hook_input.tool_name.as_deref() == Some("Bash")).then_some(door::commits::BY_AGENT);
    let (mut log, _) = catch_up_with_run(record, root, around, by)?;
    crate::door::investigator::hook_facts(record, hook_input, &mut log, around.now)?;
    // the lens panel [draft]: which Agent call is which lens, then each answer as it returns
    crate::door::lens::release(record, hook_input, &mut log, around.now)?;
    crate::door::lens::hook_facts(record, hook_input, &mut log, around.now)?;
    crate::door::person::heard(record, hook_input, &mut log, around.now)?;
    // The judge runs on any call for the oldest unjudged seal, but not again after the investigator's run.
    let judge_on = around.judge_dir.is_some();
    let judged = match returned {
        None => crate::door::judge::backstop(record, root, &mut log, hook_input, around)?,
        Some(_) => crate::door::judge::Told::default(),
    };
    // the lens panel's questions [draft]: the person's picks on them, before the card's own choice
    let questions = crate::door::picker::write_questions(record, root, hook_input, &before, &mut log, around)?;
    let picked = match picker::picked(hook_input, &before, &log) {
        Some(p) => crate::door::picker::write(record, root, &mut log, &p, around)?,
        // With none of its own open, the person's pick may be another checkout's card.
        None if questions.context.is_none() => crate::door::picker::write_elsewhere(record, hook_input, &log, root, around)?.unwrap_or_default(),
        None => crate::door::picker::Told::default(),
    };
    let join = |a: Option<String>, b: Option<String>| match (a, b) {
        (Some(a), Some(b)) => Some(format!("{a}\n{b}")),
        (a, b) => a.or(b),
    };
    let picked = crate::door::picker::Told { context: join(questions.context, picked.context), message: join(questions.message, picked.message) };
    let for_person = picked.message.clone().into_iter().collect();
    project_plan(root, &log)?;
    Ok(Call { input: hook_input, record, root, around, returned, after_diagnosis: false, diagnosed: Vec::new(), before, log, not_run: Vec::new(), judged, picked, owed: Vec::new(), rewrite: None, for_person, judge_on })
}

/// When this call is the agent asking the person, show the open card in Claude Code's picker,
/// or tell the agent to ask again.
fn picker_answer(call: &Call) -> Option<Outcome> {
    let elsewhere = || crate::door::picker::read_elsewhere(call.record, call.root, call.around).into_iter().map(|(_, e)| e).collect();
    match picker::rewrite(call.input, &call.log, || tell::push_to_ask(&call.log, call.root), elsewhere) {
        Some(picker::Rewrite::Ask(updated)) => Some(hook_output::render_ask(&updated)),
        Some(picker::Rewrite::Refuse(reason)) => Some(hook_output::render(call.input, &Decision::Refuse { reason }, &[])),
        None => None,
    }
}

/// True when no run is open, both as the call began and after its facts.
fn is_idle(call: &Call) -> bool {
    [&call.before, &call.log].iter().all(|l| teach::person::mode(l) == teach::person::IDLE)
}

/// The first decision, from the binary's own files, the idle check, the investigator, then the rules.
/// `Err` is the spawn rewritten for the investigator, which is the whole answer.
fn first_decision(call: &Call, idle: bool) -> Result<Decision, Outcome> {
    let (hook_input, log, root) = (call.input, &call.log, call.root);
    let events = decision::model_events(hook_input, log, root);
    let own = decision::familiars_own_file(hook_input, root)
        .or_else(|| crate::door::folder::removal_refusal(hook_input, root))
        .or_else(|| crate::door::shell_write::out_of_reach_refusal(hook_input, root));
    // The lens panel [draft]: a released spawn rewritten to its lens, and a lens's calls but a read refused.
    match own.is_none().then(|| lens::decide(hook_input, log, &lens::model())).flatten() {
        Some(investigator::Answer::Rewrite { input, reason, message }) => return Err(hook_output::render_rewrite(&input, &reason, &message)),
        Some(investigator::Answer::Decided(d)) => return Ok(d),
        None => {}
    }
    Ok(match own {
        Some(reason) => Decision::Refuse { reason },
        // D16: while another call drafts a dependency, an edit, a commit or a stop waits for it
        None if !events.is_empty()
            && matches!(hook_input.event, HookEvent::PreToolUse | HookEvent::Stop)
            && crate::door::dependency::in_flight(log, call.around.now, crate::door::dependency::drafting_for(call.around)).is_some() =>
        {
            let a = crate::door::dependency::in_flight(log, call.around.now, crate::door::dependency::drafting_for(call.around));
            Decision::Refuse { reason: crate::teach::words::drafting(&familiar_ai_rules::View::new(log), a.and_then(|a| a.criterion_id()).unwrap_or("")) }
        }
        // With no run open the rules are not asked.
        None if idle => restrict::intent_first(hook_input, log, root),
        None => match investigator::decide(hook_input, log) {
            Some(investigator::Answer::Rewrite { input, reason, message }) => {
                return Err(hook_output::render_rewrite(&input, &reason, &message));
            }
            Some(investigator::Answer::Decided(d)) => d,
            // A shell command's file writes are each decided as an edit.
            None => decision::decide(log, root, &events, call.around.now, call.judge_on).then(crate::door::shell_write::decision(hook_input, log, root, call.around.now, call.judge_on)),
        },
    })
}

/// The refusals the binary adds after the rules allowed.
fn binary_restrictions(call: &Call, decided: Decision, idle: bool) -> Decision {
    let (hook_input, log, root) = (call.input, &call.log, call.root);
    let decided = restrict::criterion_first(decided, hook_input, log, root, idle);
    let decided = seal::decide(decided, hook_input, log, root, call.judge_on);
    restrict::unabsorbed_prompt(decided, hook_input, log, root, idle)
}

/// The diagnosis the binary runs now, before answering (S7b): on a PostToolUse or a Stop, whose
/// hooks have the time, while a criterion is owed one; never on the call after one ran.
fn diagnosis_to_run(call: &Call) -> Option<crate::door::diagnosis::Diagnosis> {
    if call.after_diagnosis || call.returned.is_some() || call.around.investigator.is_none() {
        return None; // FAMILIAR_AI_INVESTIGATOR=off: the card comes at once
    }
    if !matches!(call.input.event, HookEvent::PostToolUse | HookEvent::Stop) {
        return None;
    }
    let c = crate::door::diagnosis::due(&call.log)?;
    Some(crate::door::diagnosis::prepare(&call.log, call.root, &c, call.around.now))
}

/// The investigator the binary runs now, before answering; never on the call after one returned.
fn investigator_to_launch(call: &mut Call) -> Result<Option<Launch>, String> {
    if call.returned.is_some() {
        return Ok(None);
    }
    // See docs/decisions.md#two-failures-send-an-investigator.
    let Some(go) = crate::door::investigator_run::take(call.record, &mut call.log, call.input, call.root, call.around, &call.before)? else {
        return Ok(None);
    };
    project_plan(call.root, &call.log)?;
    Ok(Some(go))
}

/// Decide a stop while the person's card is open: refused, except on the bench, where a stop
/// with only the person's cards left goes and shows them to the person. Nothing goes back to the
/// agent: Claude Code continues the turn on a stop's `additionalContext`.
fn stop_with_cards(call: &mut Call, decided: Decision) -> Decision {
    let (hook_input, log) = (call.input, &call.log);
    if call.around.bench {
        let main = crate::door::project::is_main(call.root);
        let Some(cards) = restrict::only_the_persons_cards_left(&decided, hook_input, log, main) else {
            return decided;
        };
        call.for_person.push(tell::cards_left(log, &cards));
        return Decision::allow();
    }
    match restrict::card_open_at_stop(&decided, hook_input, log) {
        Some(open) => Decision::Refuse { reason: tell::card_open_refusal(log, &open) },
        None => decided,
    }
}

/// Add what the decision tells beside itself, with the judge's line first.
/// An edit's words reach the agent before the tool runs; after it, the same words go to the
/// person instead, so each reads them once.
fn what_the_decision_tells(call: &mut Call, mut decided: Decision) -> Decision {
    let (hook_input, log) = (call.input, &call.log);
    match (hook_input.event, &mut decided) {
        (HookEvent::PostToolUse, Decision::Allow { told }) if hook_input.file_path().is_some() => call.for_person.extend(told.take()),
        (HookEvent::PostToolUse, Decision::Allow { .. }) => {
            // A shell write's words were the agent's before it ran; after it, they are the person's.
            let lane = familiar_ai_rules::lifecycle::edit_lane(&familiar_ai_rules::View::new(log), hook_input.agent_conversation_id().as_deref());
            let events: Vec<familiar_ai_rules::Event> =
                edited_files(hook_input, call.root).into_iter().map(|file_path| familiar_ai_rules::Event::FileEdit { criterion_id: lane.clone(), file_path }).collect();
            if let (false, Decision::Allow { told: Some(t) }) = (events.is_empty(), decision::decide(log, call.root, &events, call.around.now, call.judge_on)) {
                call.for_person.push(t);
            }
        }
        (HookEvent::PreToolUse, Decision::Allow { .. }) => {
            // An edit of a planned test not yet sealed, by a tool or by the shell, is told what the test is for.
            let v = familiar_ai_rules::View::new(log);
            for file in edited_files(hook_input, call.root) {
                let planned = familiar_ai_rules::plan::criteria(&v).into_iter().find(|c| {
                    familiar_ai_rules::criterion::test_files(&v, c).contains(&file) && familiar_ai_rules::criterion::sealed_at(&v, c, familiar_ai_rules::criterion::Place::Laptop).is_none()
                });
                if let Some(c) = planned {
                    decided = decided.then(Some(Decision::Allow { told: Some(teach::words::planned_test(log, &file, &c)) }));
                }
            }
        }
        _ => {}
    }
    if let Some(c) = crate::door::lens::recorded_since(log, call.before.len()) {
        // A lens's answer recorded on this call [draft]: what comes next on the panel, or the person's card.
        let told = teach::lens::after_answer(&familiar_ai_rules::View::new(log), c);
        decided = decided.then(Some(Decision::Allow { told: Some(told) }));
    }
    if hook_input.event == HookEvent::UserPromptSubmit && crate::door::person::is_the_persons(hook_input) {
        // A prompt typed during an open run is told now, not first at the refused stop.
        let since = call.before.last().map_or(0, |f| f.t + 1);
        if let Some(p) = teach::person::unabsorbed(log).into_iter().find(|p| p.t >= since) {
            let told = teach::person::heard_during_run(p, &crate::teach::example::Project { log, root: call.root });
            decided = decided.then(Some(Decision::Allow { told: Some(told) }));
        }
    }
    if hook_input.event == HookEvent::SubagentStart {
        // The investigator is told its criterion; any other subagent, while a run is open, the person's words.
        // A lens [draft] starts with the binary's brief only.
        let conv = hook_input.agent_conversation_id();
        let finder = crate::door::lens::is_lens(hook_input.agent_type.as_deref());
        let told = conv.as_deref().and_then(|conv| teach::investigator::context(log, conv)).or_else(|| teach::investigator::subagent_context(log, conv.as_deref())).filter(|_| !finder);
        decided = Decision::Allow { told };
    }
    if hook_input.event == HookEvent::SessionStart {
        // A session's start shows the whole picture.
        decided = Decision::Allow { told: Some(session_start_context(log, call.root, call.around.now, call.judge_on)) };
    }
    if matches!(hook_input.event, HookEvent::Stop | HookEvent::SubagentStop) {
        // A refused stop shows the intent next to each sealed test.
        decided = tell::refusal_then(decided, &teach::judge::intent_next_to_test(log, call.judge_on));
    }
    let decided = call.diagnosed.iter().fold(decided, |d, line| tell::judge_first(d, Some(line)));
    call.for_person.extend(call.diagnosed.iter().cloned());
    tell::judge_first(decided, call.judged.context.as_deref())
}

/// The project files a tool call edits, relative to the root: the tool's file, else those a shell command writes.
fn edited_files(input: &HookInput, root: &Path) -> Vec<String> {
    use crate::door::shell_write::{inside_project, written_paths, Written};
    if let Some(f) = input.file_path() {
        return vec![decision::relative_to(root, f)];
    }
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    input
        .command()
        .map(written_paths)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| match w {
            Written::Path(p) => inside_project(root, cwd, &p),
            Written::Unknown => None,
        })
        .collect()
}

/// Report what the investigator came to on the call after it ran.
/// A fix-phase investigator ends the holder's turn, returned as the whole answer in `Err`.
fn investigator_came_back(call: &mut Call, decided: Decision) -> Result<Decision, Outcome> {
    let Some(r) = call.returned else {
        return Ok(decided);
    };
    let (hook_input, log) = (call.input, &call.log);
    let red = teach::investigator::sent_red(&familiar_ai_rules::View::new(log), &r.criterion_id, &r.conversation);
    if hook_input.event == HookEvent::PostToolUse && !red {
        return Err(crate::door::investigator_run::ended(log, r, &call.for_person));
    }
    Ok(if hook_input.event == HookEvent::PostToolUse {
        // A red-phase investigator leaves the turn open; the agent and the person see the same line.
        let line = teach::investigator::red_returned(log, r);
        call.for_person.push(line.clone());
        tell::judge_first(decided, Some(&line))
    } else {
        // At a stop the person sees it start and end, and a refusal carries the end to the agent.
        let line = teach::investigator::returned(log, r);
        let v = familiar_ai_rules::View::new(log);
        let rung = familiar_ai_rules::ladder::rung_of(&v, Some(r.conversation.as_str())).unwrap_or(r.model.as_str());
        call.for_person.push(teach::investigator::taking_over(log, &r.criterion_id, rung));
        call.for_person.push(line.clone());
        match decided {
            Decision::Refuse { .. } => tell::judge_first(decided, Some(&line)),
            allowed => allowed,
        }
    })
}

/// Build the answer for the decision with the lines this call adds. Decides nothing.
fn respond(mut call: Call, decided: Decision) -> Outcome {
    let (hook_input, log, before, root) = (call.input, &call.log, &call.before, call.root);
    call.for_person.extend(call.judged.message.clone());
    call.for_person.extend(teach::opened_cards(before, log));
    // One line per criterion whose state this call's facts changed.
    let mut lines = teach::state_lines(before, log, call.judge_on, root);
    // These go after a refusal's words, else with the state lines.
    let after: Vec<String> = std::mem::take(&mut call.not_run).into_iter().chain(teach::unplanned_tests(log, root, before.len())).collect();
    let decided = match decided {
        Decision::Refuse { .. } => tell::refusal_then(decided, &after),
        other => {
            lines.extend(after);
            other
        }
    };
    let mut out = hook_output::response(hook_input, &decided, &lines);
    if !call.around.bench {
        out = tell::call_to_action(out, hook_input, &decided, log, root, &mut call.for_person);
    }
    out = tell::card_context(out, hook_input, &decided, before, log);
    if let Some(context) = call.picked.context.as_deref() {
        out = out.with_context(hook_input.event, context);
    }
    if !call.owed.is_empty() {
        out = out.with_context(hook_input.event, &call.owed.join("\n"));
    }
    // Another checkout's open card, for the person's session to ask.
    if let Some(told) = crate::door::picker::told_elsewhere(call.record, hook_input, root, call.around) {
        out = out.with_context(hook_input.event, &told);
    }
    if !call.for_person.is_empty() {
        out = out.with_system_message(&call.for_person.join("\n"));
    }
    if let Some(updated) = call.rewrite.as_ref() {
        out = out.with_rewrite(updated, &format!("{}the claim names this subagent as the part's holder", teach::words::LEAD));
    }
    out.finish()
}
