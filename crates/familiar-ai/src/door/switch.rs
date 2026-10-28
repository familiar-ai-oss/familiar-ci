//! The person's off switch for a project: while the latest switch says off,
//! every hook allows everything and says nothing. The deciding layer reads the
//! switch, as it does plan mode; the rules do not know it. A linked worktree
//! with no switch of its own follows main's. See docs/decisions.md#the-off-switch.

use std::path::Path;
use std::time::Duration;

use familiar_ai_facts::{FamiliarSwitch, Log, Record, RecordError};

use crate::decide::decision::Decision;
use crate::Outcome;
use crate::door::project::{open_project, open_record, plain};
use crate::door::{cannot, TENANT};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::input::Surroundings;

pub const KIND: &str = "familiar_switch";

/// What the agent is told at a session's start while off, in place of the rulebook.
pub const OFF_LINE: &str = "Familiar is turned off for this project. Every hook allows everything and says nothing. The person turns it on with the button or by running familiar-ai on in a terminal of their own.";
/// The first sentence of `OFF_LINE`: the band's line and `status`'s first line while off.
pub const OFF_HERE: &str = "Familiar is turned off for this project.";
/// A worktree's first sentence while off, as main is.
pub const OFF_AS_MAIN: &str = "Familiar is turned off for this worktree, as main is.";
/// A worktree's first sentence while off and main is on.
pub const OFF_MAIN_ON: &str = "Familiar is turned off for this worktree; main is on.";
/// `status`'s first line in a worktree switched on while main is off.
pub const ON_MAIN_OFF: &str = "Familiar is on here; main is off.";

/// Whether the latest switch in the log says off; no switch means on.
pub fn off_in(log: &Log) -> bool {
    latest(log) == Some(true)
}

/// The latest switch in the log: `Some(true)` off, `Some(false)` on, `None` never switched.
fn latest(log: &Log) -> Option<bool> {
    log.iter().rev().find(|f| f.kind == KIND).map(|f| f.fields.get("to").and_then(serde_json::Value::as_str) == Some("off"))
}

/// A checkout's switch as it decides: `off` is the one every reader obeys,
/// `from_main` when a linked worktree with no switch of its own follows
/// main's, `main_off` main's own (`None` in main itself, outside a worktree,
/// or when main is not set up).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switch {
    pub off: bool,
    pub from_main: bool,
    pub main_off: Option<bool>,
}

impl Switch {
    /// The sentence that says it is off, for the band, `status` and the session's start.
    pub fn off_words(&self) -> &'static str {
        match self.main_off {
            None => OFF_HERE,
            Some(true) => OFF_AS_MAIN,
            Some(false) => OFF_MAIN_ON,
        }
    }

    /// `status`'s first line: the off sentence, or, on in a worktree while main is off, that.
    pub fn status_line(&self) -> Option<&'static str> {
        match (self.off, self.main_off) {
            (true, _) => Some(self.off_words()),
            (false, Some(true)) => Some(ON_MAIN_OFF),
            (false, _) => None,
        }
    }

    /// The status line's switch: `ON`/`OFF`, then in a worktree whose it is.
    pub fn bar(&self) -> String {
        let word = |off: bool| if off { "OFF" } else { "ON" };
        match self.main_off {
            None => word(self.off).to_string(),
            Some(_) if self.from_main => format!("{} · as main", word(self.off)),
            Some(m) if m == self.off => format!("{} · here", word(self.off)),
            Some(m) => format!("{} · here · main {}", word(self.off), word(m)),
        }
    }

    /// The keys `status --json` carries: `off`, `switch_from` and `main_off`.
    pub fn json_into(&self, v: &mut serde_json::Value) {
        v["off"] = self.off.into();
        v["switch_from"] = if self.from_main { "main" } else { "here" }.into();
        v["main_off"] = self.main_off.map_or(serde_json::Value::Null, serde_json::Value::from);
    }
}

/// The switch of the checkout at `root`, whose record holds `own_log`: its
/// own latest switch, else, in a linked worktree, main's latest; none means
/// on. Main's record is opened with `wait`, so the caller holds no record
/// of its own while this runs; busy past the wait is `Err(Busy)`.
pub(crate) fn here(root: &Path, own_log: &Log, wait: Duration) -> Result<Switch, RecordError> {
    let main_off = match crate::door::project::main_project(root) {
        Some(main) => match open_record(&main, wait)? {
            Some(record) => Some(off_in(&record.read_all()?)),
            None => None,
        },
        None => None,
    };
    Ok(match latest(own_log) {
        Some(off) => Switch { off, from_main: false, main_off },
        None => Switch { off: main_off.unwrap_or(false), from_main: main_off.is_some(), main_off },
    })
}

/// [`here`] for a hook holding `record`: it is let go while main's is read.
pub(crate) fn here_holding(record: &Record, root: &Path, wait: Duration) -> Result<Switch, RecordError> {
    let log = record.read_all()?;
    if crate::door::project::main_project(root).is_none() {
        return here(root, &log, wait);
    }
    record.while_released(wait, || here(root, &log, wait))?
}

/// A hook's answer while off: allow everything, telling only the session's
/// start that the switch is off. The person's own prompt is still recorded
/// (the owner's choice, 2026-10-12); nothing else is written. `Err` only when
/// the record cannot be written.
pub(crate) fn decide(record: &Record, input: &HookInput, now: f64, switch: &Switch) -> Result<Decision, String> {
    crate::door::person::heard(record, input, &mut Log::new(), now)?;
    Ok(match input.event {
        HookEvent::SessionStart => Decision::Allow { told: Some(OFF_LINE.replacen(OFF_HERE, switch.off_words(), 1)) },
        _ => Decision::allow(),
    })
}

/// Why an agent may not switch it `to` on or off: from its own shell, or by a
/// command the hook reads before it runs.
pub fn refusal(to: &str) -> String {
    format!("familiar-ai: not switched · familiar-ai {to} is the person's switch, not the agent's: the person turns it {to} with the button or by running familiar-ai {to} in a terminal of their own")
}

/// Why an agent may not clear the parts: from its own shell, or by a command the hook reads before
/// it runs [draft words].
pub const CLEAR_REFUSAL: &str = "familiar-ai: not cleared · familiar-ai clear is the person's, not the agent's: the person clears the parts with the button or by running familiar-ai clear in a terminal of their own";

/// `familiar-ai clear` (the owner, 2026-10-27): every part not yet released or proven is released
/// by the person (`hold_answered`, choice `release`, at the hold it stands at, or its state when it
/// has none), its record kept. Refused from an agent's session, as the switch is.
/// The `from` of a release `familiar-ai clear` writes (the owner, 2026-10-28).
pub const FROM_CLEAR: &str = "clear";

/// Whether the person cleared the part: a release `familiar-ai clear` wrote, the person's
/// by construction (refused from an agent), so it leaves the status and names no answer flag.
pub fn cleared(v: &familiar_ai_rules::View, c: &str) -> bool {
    v.of("hold_answered", c).any(|f| {
        let s = |k: &str| f.fields.get(k).and_then(serde_json::Value::as_str);
        s("choice") == Some("release") && s("from") == Some(FROM_CLEAR)
    })
}

pub fn run_clear(around: &Surroundings) -> Outcome {
    use familiar_ai_rules::{lifecycle, plan, View};
    if around.session_id.is_some() {
        return plain(1, String::new(), format!("{CLEAR_REFUSAL}\n"));
    }
    let (_, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let log = match record.read_all() {
        Ok(log) => log,
        Err(e) => return plain(1, String::new(), format!("familiar-ai: {}\n", cannot("read its record", &e))),
    };
    let v = View::new(&log);
    let open: Vec<(String, String)> = plan::criteria(&v)
        .into_iter()
        .filter(|c| !lifecycle::released(&v, c) && lifecycle::state(&v, c) != "proven")
        .map(|c| {
            let at = lifecycle::holds(&v, &c).first().map_or_else(|| lifecycle::state(&v, &c).to_string(), |h| h.why.to_string());
            (c, at)
        })
        .collect();
    let names: Vec<String> = open.iter().map(|(c, _)| crate::teach::words::quoted(&v, c)).collect();
    for (c, at) in &open {
        let fields = serde_json::json!({ "criterion_id": c, "hold": at, "choice": "release", "typed": "", "from": FROM_CLEAR });
        if let Err(e) = record.append_raw("hold_answered", fields.as_object().cloned().unwrap_or_default(), around.now, TENANT) {
            return plain(1, String::new(), format!("familiar-ai: {}\n", cannot("write its record", &e)));
        }
    }
    // [draft words]
    let words = match names.len() {
        0 => "Familiar has no open part to clear in this project.\n".to_string(),
        1 => format!("Familiar cleared 1 part of this project: {}. It is released, its record kept.\n", names[0]),
        n => format!("Familiar cleared {n} parts of this project: {}. They are released, their record kept.\n", names.join(", ")),
    };
    plain(0, words, String::new())
}

/// `familiar-ai off` or `on`, refused from an agent's session so only the
/// person can switch.
pub fn run_switch(to: &str, around: &Surroundings) -> Outcome {
    if around.session_id.is_some() {
        return plain(1, String::new(), format!("{}\n", refusal(to)));
    }
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    if let Err(e) = record.append(&FamiliarSwitch { to: to.to_string() }, around.now, TENANT) {
        return plain(1, String::new(), format!("familiar-ai: {}\n", cannot("write its record", &e)));
    }
    drop(record);
    // In a worktree the words name it, and main's switch, which stays as it is.
    let main_off = match crate::door::project::main_project(&root).map(|m| open_record(&m, around.record_wait)) {
        Some(Ok(Some(main))) => main.read_all().ok().map(|log| off_in(&log)),
        _ => None,
    };
    let words = match main_off {
        Some(m) => format!("Familiar is turned {to} for this worktree. Main stays {}.\n", if m { "off" } else { "on" }),
        None if to == "off" => "Familiar is turned off for this project. Every hook allows everything and says nothing until familiar-ai on.\n".to_string(),
        None => "Familiar is turned on for this project.\n".to_string(),
    };
    plain(0, words, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use serde_json::json;

    fn switched(log: &Log, to: &str) -> Log {
        append_raw(log, KIND, json!({ "to": to }).as_object().cloned().unwrap_or_default(), 1.0, TENANT).expect("switch")
    }

    #[test]
    fn the_latest_switch_decides_and_none_is_on() {
        let log = Log::new();
        assert!(!off_in(&log));
        let log = switched(&log, "off");
        assert!(off_in(&log));
        let log = switched(&log, "on");
        assert!(!off_in(&log));
        assert!(off_in(&switched(&log, "off")));
    }
}
