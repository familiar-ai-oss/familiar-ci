//! Launch the investigator as `claude -p` with our plugin and take its answer.
//! The investigator by layer: decide/ the spawn and the wait · door/ the facts, the run, the brief · teach/ the words.
//! See docs/decisions.md#two-failures-send-an-investigator

use std::path::Path;

use familiar_ai_facts::{Log, Record};
use familiar_ai_rules::{ladder, lifecycle, plan, View};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::door::child::{self, Ended, Ran};
use crate::door::investigator::{hand_over, model_seen, transfer_back};
use crate::door::{FAMILIAR_FOLDER, INVESTIGATOR_LOG};
use crate::decide::hook_output::Rendered;
use crate::input::hook_input::{HookEvent, HookInput};
use crate::input::{Investigating, Surroundings};

/// The values `claude --permission-mode` accepts; the holder's mode is passed on
/// when it is one of these, so the investigator has the person's grants.
pub const PERMISSION_MODES: [&str; 6] = ["acceptEdits", "auto", "bypassPermissions", "manual", "dontAsk", "plan"];

const STDERR_SHOWN: usize = 300;

/// No user, project or local settings and no MCP servers; unlike the judge, the
/// investigator keeps its tools and loads our plugin.
pub const LEAN: [&str; 3] = ["--setting-sources", "", "--strict-mcp-config"];

/// An investigator about to be launched: the facts are written, the child not yet run.
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    pub criterion_id: String,
    /// `<holder's session>/<session_id>`: the conversation the record names.
    pub conversation: String,
    /// The UUID the child is launched with (`--session-id`).
    pub session_id: String,
    pub rung: &'static str,
    pub prompt: String,
    pub permission_mode: Option<String>,
}

/// What came back from the child, for the holder's stop.
#[derive(Debug, Clone, PartialEq)]
pub struct Returned {
    pub criterion_id: String,
    pub conversation: String,
    /// The model it reported using, else the rung's name.
    pub model: String,
    /// How it ended, such as `exit 0` or `stopped after 1600 s`.
    pub how: String,
    pub wall_secs: u64,
    pub cost_usd: Option<f64>,
    /// The start of its stderr when it did not exit 0.
    pub stderr: String,
}

fn sent_for(v: &View, c: &str) -> Option<&'static str> {
    let hs = lifecycle::holds(v, c);
    ladder::SENT_FOR.iter().copied().find(|w| hs.iter().any(|h| h.why == *w))
}

/// What is due: criterion, current holder, rung, and the hold.
pub type Due = (String, String, &'static str, &'static str);

/// The criterion `me` (or an investigator `me` started) should hand to an
/// investigator, if any. At a PostToolUse `since` is the log before this call,
/// so only a new hold counts and one failed try launches once; at the stop it is `None`.
pub fn due(log: &Log, me: &str, since: Option<&Log>) -> Option<Due> {
    let v = View::new(log);
    let was = since.map(|b| View::new(b));
    plan::criteria(&v).into_iter().find_map(|c| {
        let h = plan::assigned_to(&v, &c)?.to_string();
        let started_here = ladder::climb_of(&v, &c).first().and_then(|f| f.fields.get("from_conversation")).and_then(Value::as_str) == Some(me);
        if h != me && !started_here {
            return None;
        }
        let why = sent_for(&v, &c)?;
        // A fix that lives outside this repository is the person's card, not another rung's try.
        if lifecycle::holds(&v, &c).iter().any(|h| h.why == "fix_elsewhere") {
            return None;
        }
        if was.as_ref().is_some_and(|w| sent_for(w, &c) == Some(why)) {
            return None;
        }
        let r = crate::door::investigator::rung(&v, &c)?;
        Some((c, h, r, why))
    })
}

/// A version-4-shaped UUID derived from known values, so nothing reads a random source.
pub fn minted(holder: &str, c: &str, now: f64, n: usize) -> String {
    let mut b: Vec<u8> = Sha256::digest(format!("{holder}|{c}|{now}|{n}").as_bytes()).iter().take(16).copied().collect();
    if let Some(x) = b.get_mut(6) {
        *x = (*x & 0x0f) | 0x40;
    }
    if let Some(x) = b.get_mut(8) {
        *x = (*x & 0x3f) | 0x80;
    }
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let part = |a: usize, z: usize| hex.get(a..z).unwrap_or("");
    format!("{}-{}-{}-{}-{}", part(0, 8), part(8, 12), part(12, 16), part(16, 20), part(20, 32))
}

/// A `claude -p` the binary launches, after the program's name: the model,
/// our plugin, the output form, lean, the budget, then `session` (the session
/// id it is given, or the one it resumes).
pub fn claude_args(model: Option<&str>, plugin_dir: &Path, output: &[&str], budget: Option<&str>, session: [&str; 2]) -> Vec<String> {
    let mut a = vec!["-p".to_string()];
    if let Some(m) = model {
        a.extend(["--model".to_string(), m.to_string()]);
    }
    a.extend(["--plugin-dir".to_string(), plugin_dir.to_string_lossy().into_owned(), "--output-format".to_string()]);
    a.extend(output.iter().chain(LEAN.iter()).map(|s| s.to_string()));
    if let Some(b) = budget {
        a.extend(["--max-budget-usd".to_string(), b.to_string()]);
    }
    a.extend(session.iter().map(|s| s.to_string()));
    a
}

/// The child's argv after the program's name.
pub fn args(go: &Launch, i: &Investigating) -> Vec<String> {
    let mut a = claude_args(Some(go.rung), &i.plugin_dir, &["json"], Some(&i.budget), ["--session-id", &go.session_id]);
    if let Some(m) = &go.permission_mode {
        a.extend(["--permission-mode".to_string(), m.clone()]);
    }
    a
}

/// If an investigator is due, write its `model_seen` and the transfer and return
/// what to launch. Only the main conversation launches.
pub fn take(record: &Record, log: &mut Log, input: &HookInput, root: &Path, around: &Surroundings, before: &Log) -> Result<Option<Launch>, String> {
    let since = match input.event {
        HookEvent::Stop => None,
        HookEvent::PostToolUse if input.agent_id.is_none() => Some(before),
        _ => return Ok(None),
    };
    if around.investigator.is_none() {
        return Ok(None); // FAMILIAR_AI_INVESTIGATOR=off, or no plugin folder
    }
    let Some(me) = input.agent_conversation_id() else {
        return Ok(None);
    };
    // D5: only two_failures climbs; two_tries_not_red gets the read-only diagnosis (door::diagnosis).
    let Some((c, holder, rung, _why)) = due(log, &me, since) else {
        return Ok(None);
    };
    let session_id = minted(&holder, &c, around.now, log.len());
    let conversation = format!("{}/{session_id}", ladder::session_of(Some(&holder)).unwrap_or_default());
    if !hand_over(record, log, &c, &holder, &conversation, rung, around.now)? {
        return Ok(None);
    }
    let b = crate::door::investigator_brief::inputs(log, root, &c);
    let prompt = crate::teach::investigator::launch_brief(&b);
    let permission_mode = input.permission_mode.clone().filter(|m| PERMISSION_MODES.contains(&m.as_str()));
    Ok(Some(Launch { criterion_id: c, conversation, session_id, rung, prompt, permission_mode }))
}

/// Run the child in the project and wait; its own hooks still judge its seal.
pub fn launch(go: &Launch, root: &Path, i: &Investigating) -> Ran {
    let set = [
        ("FAMILIAR_AI_INVESTIGATOR", "off"),
        ("FAMILIAR_AI_CONVERSATION_ID", go.conversation.as_str()),
    ];
    child::run(&i.program.to_string_lossy(), &args(go, i), root, &set, &go.prompt, i.wait)
}

/// Keep the transcript and append the run to `.familiar/investigator.log`. No
/// secret is logged because the prompt goes on stdin.
pub fn log_run(root: &Path, now: f64, go: &Launch, i: &Investigating, ran: &Ran) {
    let transcript = crate::door::kept::keep_transcript(root, i.claude_dir.as_deref(), &go.session_id);
    let argv: Vec<String> = std::iter::once(i.program.to_string_lossy().into_owned()).chain(args(go, i)).collect();
    let result = result_of(&ran.stdout);
    let cost = result.as_ref().and_then(|r| r.get("total_cost_usd")).and_then(Value::as_f64).map_or_else(|| "not reported".to_string(), |x| format!("${x:.4}"));
    let session = result.as_ref().and_then(|r| r.get("session_id")).and_then(Value::as_str).unwrap_or("not reported");
    let path = root.join(FAMILIAR_FOLDER).join(INVESTIGATOR_LOG);
    let head = format!("at {now:.0} · criterion {} · conversation {} · {}", go.criterion_id, go.conversation, child::shown(&argv));
    child::log(&path, &head, ran, &format!(" · cost {cost} · session {session} · {transcript}"));
}

fn result_of(stdout: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(stdout.trim()).ok()? {
        Value::Array(xs) => xs.into_iter().rev().find(|x| x.get("type").and_then(Value::as_str) == Some("result")),
        one => Some(one),
    }
}

/// After the child: record the model it used, then hand the criterion back
/// if it did its job, else write `investigator_unfinished`.
pub fn back(record: &Record, log: &mut Log, go: &Launch, ran: &Ran, now: f64) -> Result<Returned, String> {
    let result = result_of(&ran.stdout);
    let used: Vec<String> = result
        .as_ref()
        .and_then(|r| r.get("modelUsage"))
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let model = used.into_iter().find(|k| ladder::family_of(Some(k)) == Some(go.rung)).unwrap_or_else(|| go.rung.to_string());
    model_seen(record, log, &go.conversation, &model, now)?;
    transfer_back(record, log, &go.conversation, now)?;
    unfinished(record, log, go, ran, result.as_ref(), now)?;
    let cost_usd = result.as_ref().and_then(|r| r.get("total_cost_usd")).and_then(Value::as_f64);
    let stderr = match ran.answer() {
        Some(_) => String::new(),
        None => child::head(ran.stderr.trim(), STDERR_SHOWN).to_string(),
    };
    Ok(Returned {
        criterion_id: go.criterion_id.clone(),
        conversation: go.conversation.clone(),
        model,
        how: ran.how(),
        wall_secs: ran.wall.as_secs(),
        cost_usd,
        stderr,
    })
}

/// If the child never handed back, write `investigator_unfinished` with why.
fn unfinished(record: &Record, log: &mut Log, go: &Launch, ran: &Ran, result: Option<&Value>, now: f64) -> Result<(), String> {
    let sent = {
        let v = View::new(log);
        ladder::investigator_of(&v, &go.criterion_id)
            .filter(|f| f.fields.get("to_conversation").and_then(Value::as_str) == Some(go.conversation.as_str()))
            .map(|f| ladder::investigated_for(&v, f))
    };
    let Some(sent) = sent else {
        return Ok(());
    };
    let subtype = result.and_then(|r| r.get("subtype")).and_then(Value::as_str).unwrap_or("");
    let is_error = result.and_then(|r| r.get("is_error")).and_then(Value::as_bool).unwrap_or(false);
    let why = match &ran.ended {
        Ended::TimedOut => "timeout",
        _ if subtype.contains("budget") => "budget",
        Ended::Exited(Some(0)) if !is_error => match sent {
            Some("two_tries_not_red") => "no_seal",
            _ => "no_green",
        },
        _ => "error",
    };
    let fields = serde_json::json!({ "criterion_id": go.criterion_id, "conversation": go.conversation, "why": why });
    let fact = record
        .append_raw("investigator_unfinished", fields.as_object().cloned().unwrap_or_default(), now, crate::door::TENANT)
        .map_err(|e| crate::door::cannot("write its record", &e))?;
    log.push(fact);
    Ok(())
}

/// The holder's PostToolUse answer after a fix-phase investigator returns: it ends
/// the turn with the words in `stopReason`, repeated in `additionalContext`, which
/// Claude Code has not delivered alongside `continue: false`.
pub fn ended(log: &Log, r: &Returned, for_person: &[String]) -> Rendered {
    let line = crate::teach::investigator::escalated(log, r);
    let mut out = serde_json::Map::new();
    out.insert("continue".into(), Value::Bool(false));
    out.insert("stopReason".into(), Value::String(line.clone()));
    if !for_person.is_empty() {
        out.insert("systemMessage".into(), Value::String(for_person.join("\n")));
    }
    out.insert("hookSpecificOutput".into(), serde_json::json!({ "hookEventName": "PostToolUse", "additionalContext": line }));
    let mut stdout_text = Value::Object(out).to_string();
    stdout_text.push('\n');
    Rendered { exit_code: 0, stdout_text, stderr_text: String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn the_session_id_is_a_version_4_uuid_made_from_what_is_known() {
        let u = minted("s1", "c1", 12.5, 40);
        assert_eq!(u, minted("s1", "c1", 12.5, 40), "the same facts, the same id");
        assert_ne!(u, minted("s1", "c1", 12.5, 41));
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12], "{u}");
        assert!(parts[2].starts_with('4') && "89ab".contains(&parts[3][..1]), "{u}");
        assert!(u.chars().all(|ch| ch == '-' || ch.is_ascii_hexdigit()), "{u}");
    }

    #[test]
    fn the_argv_is_lean_with_the_rung_the_plugin_the_budget_the_session_and_the_holders_mode() {
        let i = Investigating {
            program: PathBuf::from("claude"),
            plugin_dir: PathBuf::from("/opt/plugin"),
            budget: crate::Surroundings::INVESTIGATOR_BUDGET.into(),
            wait: Duration::ZERO,
            claude_dir: None,
        };
        let mut go = Launch {
            criterion_id: "c1".into(),
            conversation: "s1/u1".into(),
            session_id: "u1".into(),
            rung: "sonnet",
            prompt: String::new(),
            permission_mode: None,
        };
        assert_eq!(
            child::shown(&args(&go, &i)),
            "-p --model sonnet --plugin-dir /opt/plugin --output-format json --setting-sources '' --strict-mcp-config --max-budget-usd 3.00 --session-id u1"
        );
        go.permission_mode = Some("bypassPermissions".into());
        assert!(args(&go, &i).join(" ").ends_with("--session-id u1 --permission-mode bypassPermissions"));
    }

    #[test]
    fn the_result_is_the_last_result_object() {
        let one = r#"{"type":"result","session_id":"u1","total_cost_usd":0.2}"#;
        assert_eq!(result_of(one).and_then(|r| r.get("total_cost_usd").and_then(Value::as_f64)), Some(0.2));
        let listed = r#"[{"type":"system"},{"type":"result","session_id":"u1"}]"#;
        assert_eq!(result_of(listed).and_then(|r| r.get("session_id").cloned()), Some(Value::from("u1")));
        assert_eq!(result_of("not json"), None);
    }
}
