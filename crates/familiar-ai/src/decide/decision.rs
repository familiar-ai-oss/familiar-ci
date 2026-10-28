//! Map a Claude Code hook event to the rules' events and decide them in order; the first refusal is
//! the answer. A refusal is reworded for the person and ends with the `familiar-ai claim` command that
//! clears it when the door can write one.

use std::path::Path;

use familiar_ai_facts::Log;
use familiar_ai_rules::{criterion, lifecycle, plan, terminal, Event, View};

use crate::door::claim;
use crate::input::hook_input::{HookEvent, HookInput};
use crate::teach::example::Project;
use crate::teach::full_path;

/// One hook call's decision, returned and never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead under the normal permission rules, telling the agent `told`.
    Allow { told: Option<String> },
    /// Refused. `reason` names the hold or state and what clears it.
    Refuse { reason: String },
}

impl Decision {
    /// An allow that tells the model nothing.
    pub fn allow() -> Decision {
        Decision::Allow { told: None }
    }

    /// This decision followed by `next`: a refusal stands, and two allows carry both `told`s.
    pub fn then(self, next: Option<Decision>) -> Decision {
        match (self, next) {
            (refused @ Decision::Refuse { .. }, _) => refused,
            (allowed, None) => allowed,
            (Decision::Allow { .. }, Some(refused @ Decision::Refuse { .. })) => refused,
            (Decision::Allow { told: a }, Some(Decision::Allow { told: b })) => {
                let both: Vec<String> = a.into_iter().chain(b).collect();
                Decision::Allow { told: if both.is_empty() { None } else { Some(both.join("\n")) } }
            }
        }
    }
}

/// True when the command plainly runs `git commit`.
/// This is an early courtesy only; the commit itself is read from git after it lands.
pub fn mentions_git_commit(command: &str) -> bool {
    command.contains("git commit")
}

/// `file_path` relative to the project root, as facts name files; a path outside the root is kept.
pub fn relative_to(root: &Path, file_path: &str) -> String {
    Path::new(file_path)
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file_path.to_string())
}

/// The refusal of an agent's edit or shell write of `.familiar/adapter` (plan A-1) [draft words]:
/// the binary writes it beside the plan, from the dry run's choice.
pub const ADAPTER_OWN: &str = "familiar-ai: .familiar/adapter is written by familiar-ai itself from the dry run's choice of how each test runner is read, never by hand; it is committed with the work like the plan · next: familiar-ai status shows the reading; a change to the test setup's files, or the person's try_again on the test setup's card, starts a new dry run";

/// `.familiar/adapter`, relative to the project.
pub fn adapter_rel() -> String {
    format!("{}/{}", crate::FAMILIAR_FOLDER, crate::door::ADAPTER_FILE)
}

/// The refusal for an edit of `.familiar/plan`, `.familiar/adapter` or `.familiar/record`, which the binary writes itself.
/// It lives here because the rules see an edit only against an anchor and the door knows which paths are its own.
pub fn familiars_own_file(input: &HookInput, root: &Path) -> Option<String> {
    if !matches!(input.event, HookEvent::PreToolUse | HookEvent::PostToolUse) {
        return None;
    }
    let file_path = input.file_path()?;
    let rel = relative_to(root, file_path);
    // `.github` is the repository's: a project nested in a subfolder reaches it through `..`.
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    let named = Path::new(file_path);
    let full = lexically_clean(&if named.is_absolute() { named.to_path_buf() } else { cwd.join(named) });
    if let Some(reason) = repository_relative(root, &full).and_then(|r| out_of_reach(&r)).or_else(|| out_of_reach(&rel)) {
        return Some(reason);
    }
    if !crate::door::commits::is_familiars_own(&rel) {
        return None;
    }
    let plan = format!("{}/{}", crate::FAMILIAR_FOLDER, crate::PLAN_FILE);
    if rel == adapter_rel() {
        return Some(ADAPTER_OWN.to_string());
    }
    Some(if rel == plan {
        format!(
            "familiar-ai: {plan} is written by familiar-ai itself from its record, never by hand · next: familiar-ai status shows the line that is missing and the familiar-ai claim command that claims it; familiar-ai claim alone lists the kinds"
        )
    } else {
        format!("familiar-ai: {rel} is familiar-ai's own and stays as it is; it is never edited by hand")
    })
}

/// `path` with `.` and `..` resolved by their text, as the shell resolves a path to write.
pub fn lexically_clean(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut clean = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    clean
}

/// `full` (absolute, clean) relative to the top of the git repository that
/// holds `root`: the nearest folder at or above it with a `.git`. `None`
/// outside it, or with no repository.
pub fn repository_relative(root: &Path, full: &Path) -> Option<String> {
    let top = root.ancestors().find(|d| d.join(".git").exists())?;
    let rel = full.strip_prefix(top).ok()?.to_string_lossy().into_owned();
    (!rel.is_empty()).then_some(rel)
}

/// The refusal of the agent's write of CI's wiring, `.github/` (plan §6.9,
/// B-5: CI's configuration decides which approvals it trusts); `None` for any
/// other path. `.familiar/approvals/` is familiar-ai's own folder, refused
/// with the rest of it.
pub fn out_of_reach(rel: &str) -> Option<String> {
    (rel == ".github" || rel.starts_with(".github/"))
        .then(|| format!("familiar-ai: {rel} is the person's: CI's configuration decides which approvals it trusts, so the agent never edits .github/"))
}

/// The model's events for one hook call, in the order they are decided; empty when there are none.
pub fn model_events(input: &HookInput, log: &Log, root: &Path) -> Vec<Event> {
    let v = View::new(log);
    match input.event {
        HookEvent::SessionStart
        | HookEvent::UserPromptSubmit
        | HookEvent::SubagentStart
        | HookEvent::PostToolUseFailure
        | HookEvent::PreModelSwitch
        | HookEvent::PostModelSwitch
        | HookEvent::SessionEnd => Vec::new(),
        HookEvent::PreToolUse | HookEvent::PostToolUse => {
            if let Some(file_path) = input.file_path() {
                // A file outside the project's folder is not this project's code: no event.
                let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
                if crate::door::person::outside_the_project(root, cwd, file_path) {
                    return Vec::new();
                }
                // An outside edit is told against the anchor of the criterion this conversation works on.
                return vec![Event::FileEdit {
                    criterion_id: lifecycle::edit_lane(&v, input.agent_conversation_id().as_deref()),
                    file_path: relative_to(root, file_path),
                }];
            }
            if input.event == HookEvent::PreToolUse && input.command().is_some_and(mentions_git_commit)
            {
                return vec![Event::FirstEditOrCommit];
            }
            Vec::new()
        }
        // main's checkout reads every part; any other the parts touched there (the owner, 2026-10-15)
        HookEvent::Stop => vec![Event::Stops { main: crate::door::project::is_main(root) }],
        HookEvent::SubagentStop => {
            // A subagent that stops declares finished the criteria its conversation holds.
            let Some(me) = input.agent_conversation_id() else {
                return Vec::new();
            };
            plan::criteria(&v)
                .into_iter()
                .filter(|c| plan::assigned_to(&v, c) == Some(me.as_str()))
                .map(|c| Event::DeclaresFinished { criterion_id: c })
                .collect()
        }
    }
}

/// Decide each event in turn: the first refusal is the answer, else an allow carrying every `told`.
pub fn decide(log: &Log, root: &Path, events: &[Event], now: f64, judge_on: bool) -> Decision {
    let v = View::new(log);
    let mut told: Vec<String> = Vec::new();
    if let Some(refused) = first_act_refusal(log, root, events) {
        return refused;
    }
    if let Some(refused) = places_refusal(log, root, events, now, judge_on) {
        return refused;
    }
    for event in events {
        match familiar_ai_rules::decide(log, event, now) {
            familiar_ai_rules::Decision::Allow { told: t } => {
                let file = match event {
                    Event::FileEdit { file_path, .. } => Some(file_path.as_str()),
                    _ => None,
                };
                told.extend(t.map(|t| crate::teach::words::told(log, root, &t, file)));
            }
            familiar_ai_rules::Decision::Refuse(r) => return Decision::Refuse { reason: refusal_words(log, root, &v, &r, event, judge_on) },
        }
    }
    Decision::Allow {
        told: if told.is_empty() { None } else { Some(told.join("\n")) },
    }
}

/// Refuse a commit while a part at work holds `places_unclassified` [draft] (the owner's plan of
/// 2026-10-21, step 3): the rules refuse its tests-first commit until every listed site is
/// classified, and the binary sees a commit only as `git commit` is run, so any commit waits. The
/// refusal names each site left and the claim that classifies it. So with `reading_unclear`
/// [draft] (the owner's plan of 2026-10-21, step 2): any commit waits on the agent's word on how
/// the person's ask reads, then on the person's pick of the readings it lists.
fn places_refusal(log: &Log, root: &Path, events: &[Event], now: f64, judge_on: bool) -> Option<Decision> {
    if !events.contains(&Event::FirstEditOrCommit) {
        return None;
    }
    let v = View::new(log);
    let held = |c: &String| familiar_ai_rules::places::holds(&v, c) || familiar_ai_rules::ask::holds(&v, c);
    let c = lifecycle::runnable_criteria(&v).into_iter().find(held)?;
    let event = Event::TestsCommit { criterion_id: c, touches_code: false };
    match familiar_ai_rules::decide(log, &event, now) {
        familiar_ai_rules::Decision::Refuse(r) if [familiar_ai_rules::places::HOLD, familiar_ai_rules::ask::HOLD].contains(&r.hold.as_deref().unwrap_or("")) => {
            Some(Decision::Refuse { reason: refusal_words(log, root, &v, &r, &event, judge_on) })
        }
        _ => None,
    }
}

/// Refuse a commit until the holder claims where the fix lives, naming that claim.
/// The reference model allows the first edit or commit, so this refusal is the binary's; see
/// docs/decisions.md#a-fix-outside-the-repository-is-a-card.
fn first_act_refusal(log: &Log, root: &Path, events: &[Event]) -> Option<Decision> {
    if !events.contains(&Event::FirstEditOrCommit) {
        return None;
    }
    let v = View::new(log);
    if !plan::planning_confirm_complete(&v) {
        return None;
    }
    let c = lifecycle::runnable_criteria(&v).into_iter().find(|c| lifecycle::first_act(&v, c))?;
    let mut known = serde_json::Map::new();
    known.insert("criterion_id".into(), serde_json::Value::from(c.as_str()));
    let project = Project { log, root };
    let mut lines = vec![format!(
        "{}{} · {} · next: {}",
        crate::teach::words::LEAD,
        crate::teach::words::quoted(&v, &c),
        crate::teach::words::CLAIM_FIRST,
        crate::teach::words::CLAIM_NEXT
    )];
    if let Some(form) = claim::command_form("fix_location_category", &known, Some(&project)) {
        lines.push(format!("  claim it: {form}"));
    }
    if let Some(note) = crate::teach::example::note("fix_location_category", &project) {
        lines.push(format!("            {note}"));
    }
    Some(Decision::Refuse { reason: lines.join("\n") })
}

/// A rules refusal held in parts until it is worded once.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RulesRefusal {
    lines: Vec<String>,
    before_clear: Vec<String>,
    clear: Option<String>,
    after: Vec<String>,
}

impl RulesRefusal {
    /// Split the terminal's text, whose last line for a hold is `  to clear it: <words>`.
    /// This is the one place that line is read back.
    fn from_terminal(text: &str) -> RulesRefusal {
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let clear = lines.last().and_then(|l| l.strip_prefix(TO_CLEAR)).map(str::to_string);
        if clear.is_some() {
            lines.pop();
        }
        RulesRefusal { lines, before_clear: Vec::new(), clear, after: Vec::new() }
    }

    /// Join the parts back into one text, in order.
    fn words(self) -> String {
        let clear = self.clear.map(|w| format!("{TO_CLEAR}{w}"));
        self.lines.into_iter().chain(self.before_clear).chain(clear).chain(self.after).collect::<Vec<_>>().join("\n")
    }
}

const TO_CLEAR: &str = "  to clear it: ";

/// Word the rules' refusal `r` of `event`, adding the teaching for its hold and the claim commands that clear it.
/// Without the terminal's words the model's own object is used, so a refusal is never wordless.
/// `judge_on`: a judge is asked here, so a seal it has not read is not judged yet.
fn refusal_words(log: &Log, root: &Path, v: &View, r: &familiar_ai_rules::Refusal, event: &Event, judge_on: bool) -> String {
    let text = terminal::refusal(v, event).unwrap_or_else(|| familiar_ai_rules::Decision::Refuse(r.clone()).to_json().to_string());
    let mut refusal = RulesRefusal::from_terminal(&text);
    let mut lines = claim::clearing_commands(log, root, r, event);
    match r.hold.as_deref() {
        // The whole plan follows, each missing line with its claim; on the full path, when line 3 is
        // missing because nothing links, its words and claims come first.
        Some("planning_incomplete") => {
            let project = Project { log, root };
            lines = Vec::new();
            if let Some(gap) = full_path::gap(log) {
                if refusal.clear.is_some() {
                    refusal.clear = Some(full_path::to_clear(&gap));
                }
                lines = full_path::claim_lines(&gap, &project);
            }
            // the owner (2026-10-11): line 2 missing because a light anchor is not documentation
            if let Some(reclaim) = full_path::light_not_documentation(log, &project) {
                if refusal.clear.is_some() {
                    refusal.clear = Some(terminal::LIGHT_NOT_DOCUMENTATION.to_string());
                }
                lines = vec![format!("  claim it: {reclaim}")];
            }
            lines.extend(crate::teach::words::plan_checklist(&project));
        }
        Some("tests_not_red") => refusal.after.push(tests_not_red_teaching(log, r, event)),
        // The person's words and the judge's word on the seal before the claim, so the test is
        // changed to the definition and never loosened in the dark.
        Some("proof_tampered") => {
            refusal.after.extend(crate::teach::judge::asked_for(log));
            if let Some(c) = claim::criterion_of(log, r, event) {
                if let Some(seal) = criterion::sealed_at(v, &c, criterion::Place::Laptop) {
                    refusal.after.push(format!("  judge: {}", crate::teach::judge::word_on(v, &c, seal, judge_on)));
                }
            }
        }
        // The person's words and the judge's word on each announced change, so an existing test is
        // changed to the person's words and never loosened (the owner, 2026-10-20) [draft]
        Some(h) if h == familiar_ai_rules::existing::HOLD => {
            refusal.after.extend(crate::teach::judge::asked_for(log));
        }
        // What the code commits did change goes before the clear line.
        Some("anchor_untouched") => {
            if let Some(line) = claim::criterion_of(log, r, event).and_then(|c| crate::teach::untouched::teaching(log, &c)) {
                refusal.before_clear.push(line);
            }
        }
        _ => {}
    }
    refusal.after.extend(lines);
    // Reword for the person, keeping the claim lines.
    crate::teach::words::refusal(log, &refusal.words())
}

/// The teaching after `tests_not_red`: what to write and the command the binary runs after each commit.
fn tests_not_red_teaching(log: &Log, r: &familiar_ai_rules::Refusal, event: &Event) -> String {
    crate::teach::words::tests_not_red_teaching(&View::new(log), claim::criterion_of(log, r, event).as_deref()).join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_rules::sample::criterion_fixture;
    use std::path::PathBuf;

    fn input(json: &str, event: HookEvent) -> HookInput {
        HookInput::parse(json, event).expect("parse")
    }

    fn fixture() -> Log {
        criterion_fixture(&["c1", "c2"], true, false, 0.0, "_default_").expect("fixture")
    }

    #[test]
    fn an_edit_is_the_file_edit_event_with_the_conversations_criterion_and_a_relative_path() {
        let log = fixture();
        let edit = |session: &str| {
            let i = input(
                &format!(r#"{{"hook_event_name":"PreToolUse","session_id":"{session}","tool_name":"Edit","tool_input":{{"file_path":"/w/src/login.py"}}}}"#),
                HookEvent::PreToolUse,
            );
            model_events(&i, &log, &PathBuf::from("/w"))
        };
        // the fixture's a1 holds c1, the runnable criterion: its lane
        assert_eq!(edit("a1"), vec![Event::FileEdit { criterion_id: Some("c1".into()), file_path: "src/login.py".into() }]);
        // a conversation that holds no runnable criterion has no lane
        assert_eq!(edit("a2"), vec![Event::FileEdit { criterion_id: None, file_path: "src/login.py".into() }]);
    }

    #[test]
    fn a_bash_commit_is_the_first_edit_or_commit_only_before_the_tool_runs() {
        let log = fixture();
        let pre = input(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"git add -A && git commit -m x"}}"#,
            HookEvent::PreToolUse,
        );
        assert_eq!(model_events(&pre, &log, Path::new("/w")), vec![Event::FirstEditOrCommit]);
        let post = input(
            r#"{"hook_event_name":"PostToolUse","tool_name":"Bash","tool_input":{"command":"git commit -m x"}}"#,
            HookEvent::PostToolUse,
        );
        assert!(model_events(&post, &log, Path::new("/w")).is_empty());
        let other = input(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls"}}"#,
            HookEvent::PreToolUse,
        );
        assert!(model_events(&other, &log, Path::new("/w")).is_empty());
    }

    #[test]
    fn session_start_and_a_prompt_have_no_event() {
        let log = fixture();
        for (json, ev) in [
            (r#"{"hook_event_name":"SessionStart"}"#, HookEvent::SessionStart),
            (r#"{"hook_event_name":"UserPromptSubmit","prompt":"build it"}"#, HookEvent::UserPromptSubmit),
        ] {
            assert!(model_events(&input(json, ev), &log, Path::new("/w")).is_empty(), "{json}");
        }
    }

    #[test]
    fn stop_is_the_agent_stops_and_a_subagent_declares_its_own_criteria() {
        let log = fixture();
        let stop = input(r#"{"hook_event_name":"Stop"}"#, HookEvent::Stop);
        assert_eq!(model_events(&stop, &log, Path::new("/w")), vec![Event::Stops { main: true }]);
        // the fixture assigns c1 and c2 to conversation "a1"
        let sub = input(
            r#"{"hook_event_name":"SubagentStop","session_id":"a1"}"#,
            HookEvent::SubagentStop,
        );
        assert_eq!(
            model_events(&sub, &log, Path::new("/w")),
            vec![
                Event::DeclaresFinished { criterion_id: "c1".into() },
                Event::DeclaresFinished { criterion_id: "c2".into() },
            ]
        );
        let other = input(
            r#"{"hook_event_name":"SubagentStop","session_id":"s","agent_id":"x"}"#,
            HookEvent::SubagentStop,
        );
        assert!(model_events(&other, &log, Path::new("/w")).is_empty());
    }

    #[test]
    fn an_empty_log_refuses_every_event_with_the_planning_words_and_the_claim_command() {
        let log: Log = Vec::new();
        let d = decide(&log, Path::new("/w"), &[Event::Stops { main: true }], 0.0, false);
        match d {
            Decision::Refuse { reason } => {
                assert!(reason.starts_with("familiar-ai: the plan comes first · next: submit the intent\n"), "{reason}");
                assert!(!reason.contains("planning_incomplete") && !reason.contains("refused"), "the person's words: {reason}");
                // the whole plan follows, each line with its claim
                assert!(reason.contains("\n  the plan's eleven lines, 0 of 11 hold:\n    1 the intent      claim it: familiar-ai claim intent_submitted --text 'greet with hello'\n"), "{reason}");
                assert!(reason.ends_with(&format!("\n{}", crate::teach::words::PLAN_ALL_AT_ONCE)), "{reason}");
                assert!(!reason.contains('<'), "no placeholder: {reason}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_edit_of_the_plan_or_the_record_is_refused_naming_the_door() {
        let plan = input(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"/w/.familiar/plan"}}"#,
            HookEvent::PreToolUse,
        );
        let reason = familiars_own_file(&plan, Path::new("/w")).expect("refused");
        assert!(reason.contains("familiar-ai claim"), "{reason}");
        let record = input(
            r#"{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{"file_path":"/w/.familiar/record"}}"#,
            HookEvent::PostToolUse,
        );
        assert!(familiars_own_file(&record, Path::new("/w")).is_some());
        let other = input(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"/w/src/familiar/plan"}}"#,
            HookEvent::PreToolUse,
        );
        assert!(familiars_own_file(&other, Path::new("/w")).is_none());
        let bash = input(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"cat .familiar/plan"}}"#,
            HookEvent::PreToolUse,
        );
        assert!(familiars_own_file(&bash, Path::new("/w")).is_none(), "reading it is fine");
    }

    #[test]
    fn a_confirmed_plan_allows_the_first_edit_and_tells_an_outside_edit() {
        let log = fixture();
        assert_eq!(decide(&log, Path::new("/w"), &[Event::FirstEditOrCommit], 0.0, false), Decision::allow());
        let d = decide(
            &log,
            Path::new("/w"),
            &[Event::FileEdit { criterion_id: Some("c1".into()), file_path: "src/elsewhere.py".into() }],
            0.0,
            false,
        );
        match d {
            Decision::Allow { told: Some(t) } => assert!(t.starts_with("familiar-ai: this edit is outside the place the plan named for "), "{t}"),
            other => panic!("expected told, got {other:?}"),
        }
    }

    #[test]
    fn stopping_with_a_criterion_still_testing_is_a_not_yet() {
        let log = fixture();
        match decide(&log, Path::new("/w"), &[Event::Stops { main: true }], 0.0, false) {
            Decision::Refuse { reason } => {
                assert!(reason.starts_with("familiar-ai: not done yet · \"a right password opens a session\" · the test comes first\n  next: "), "{reason}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_full_path_criterion_that_names_no_behavior_is_told_so_and_taught_the_reclaim() {
        use familiar_ai_facts::append_raw;
        use serde_json::{json, Value};
        let mut log: Log = Vec::new();
        for (i, (kind, fields)) in [
            ("intent_submitted", json!({"text": "fix the schema", "sha": "abc"})),
            ("criterion", json!({"criterion_id": "c1", "text": "a field gets a schema", "path": "full"})),
            ("behavior", json!({"behavior_id": "b1", "text": "schema generation"})),
            ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a model", "gives_back": "a schema", "refuses": "nothing"})),
        ]
        .into_iter()
        .enumerate()
        {
            let Value::Object(m) = fields else { panic!() };
            log = append_raw(&log, kind, m, i as f64, "t").expect(kind);
        }
        let Decision::Refuse { reason } = decide(&log, Path::new("/w"), &[Event::FileEdit { criterion_id: None, file_path: "src/x.py".into() }], 0.0, false) else {
            panic!("refused")
        };
        let lines: Vec<&str> = reason.lines().collect();
        assert_eq!(lines[0], "familiar-ai: the plan comes first · next: \"a field gets a schema\" is on the full path but names no behavior and promise: behavior b1 and promise p1 are claimed; re-claim the criterion with --behavior_id b1 --promise_id p1");
        assert_eq!(lines[1], "  claim it: familiar-ai claim criterion --criterion_id c1 --text 'a field gets a schema' --path full --behavior_id b1 --promise_id p1");
        assert_eq!(lines[2], "  the plan's eleven lines, 3 of 11 hold:", "the checklist follows the gap's claims: {reason}");
        assert!(!reason.contains("write the behavior (behavior)"), "the model's words for line 3 are replaced: {reason}");
    }

    #[test]
    fn tests_not_red_gets_the_teaching_line_with_the_command_the_binary_runs() {
        let log = fixture();
        let r = familiar_ai_rules::Refusal {
            hold: Some("tests_not_red".into()),
            state: None,
            criterion_id: Some("c1".into()),
            clears: familiar_ai_rules::Clears::Moves(vec![familiar_ai_rules::holds::tests_only_clear()]),
        };
        assert_eq!(
            tests_not_red_teaching(&log, &r, &Event::Stops { main: true }),
            "  write a test that fails on the code as it is now; familiar-ai runs python -m pytest tests/test_c1.py after each commit\n  \
             a second run that passes counts as a failed try; two in a row and the person decides"
        );
        let unknown = familiar_ai_rules::Refusal { criterion_id: None, ..r };
        assert!(tests_not_red_teaching(&Vec::new(), &unknown, &Event::Stops { main: true }).contains("familiar-ai runs the test command after each commit\n"));
        let replaced = RulesRefusal { clear: Some("new".into()), after: vec!["  claim it: x".into()], ..RulesRefusal::from_terminal("a\n  to clear it: old") };
        assert_eq!(replaced.words(), "a\n  to clear it: new\n  claim it: x");
        let before = |text: &str| RulesRefusal { before_clear: vec!["  new".into()], ..RulesRefusal::from_terminal(text) }.words();
        assert_eq!(before("a\n  b\n  to clear it: old"), "a\n  b\n  new\n  to clear it: old");
        assert_eq!(before("a\n  b"), "a\n  b\n  new");
    }

    #[test]
    fn then_keeps_a_refusal_and_joins_two_tolds() {
        let refused = Decision::Refuse { reason: "no".into() };
        let told = |t: &str| Decision::Allow { told: Some(t.into()) };
        assert_eq!(refused.clone().then(Some(told("x"))), refused);
        assert_eq!(Decision::allow().then(Some(refused.clone())), refused);
        assert_eq!(told("a").then(Some(told("b"))), told("a\nb"));
        assert_eq!(told("a").then(None), told("a"));
        assert_eq!(Decision::allow().then(Some(Decision::allow())), Decision::allow());
    }

    #[test]
    fn relative_paths_strip_the_root_and_keep_a_foreign_path() {
        assert_eq!(relative_to(Path::new("/w"), "/w/a/b.rs"), "a/b.rs");
        assert_eq!(relative_to(Path::new("/w"), "/elsewhere/b.rs"), "/elsewhere/b.rs");
        assert_eq!(relative_to(Path::new("/w"), "a/b.rs"), "a/b.rs");
    }
}
