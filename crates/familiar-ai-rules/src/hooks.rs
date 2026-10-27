//! The hooks' five stops and the referee as one pure `decide(log, event)`.
//! States and holds are read from the lifecycle and the plan, never re-derived here.

use serde_json::{json, Map, Value};

use crate::criterion::{sealed_at, Clear, Place, PAUSING};
use crate::lifecycle::{announced, first_act, holds, paused, state, touched_here, turn};
use crate::moves::{binary_moves, criterion_admissible};
use crate::plan;
use crate::reader::{text, View};
use crate::sample::plan_admissible;

/// The agent's events, named as plain sentences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Stop 1.
    FirstEditOrCommit,
    /// Stop 2.
    TestsCommit { criterion_id: String, touches_code: bool },
    /// Stop 3.
    FileEdit { criterion_id: Option<String>, file_path: String },
    /// Stop 3.
    AnnounceTestChange { criterion_id: String, file_path: Option<String> },
    /// Stop 4.
    DeclaresFinished { criterion_id: String },
    /// Stop 5. `main`: the stop is in main's checkout, which reads every part; any other
    /// checkout reads the parts touched there (the owner, 2026-10-15).
    Stops { main: bool },
    /// The referee.
    AboutToStart { criterion_id: String },
}

/// The events' names, in `Event`'s order.
pub const EVENT_NAMES: [&str; 7] = [
    "the agent's first edit or commit",
    "a commit of a criterion's tests",
    "an edit of a file",
    "announce_test_change",
    "the agent declares a criterion finished",
    "the agent stops",
    "a criterion about to start",
];

impl Event {
    /// The event's `on` name.
    pub fn name(&self) -> &'static str {
        match self {
            Event::FirstEditOrCommit => EVENT_NAMES[0],
            Event::TestsCommit { .. } => EVENT_NAMES[1],
            Event::FileEdit { .. } => EVENT_NAMES[2],
            Event::AnnounceTestChange { .. } => EVENT_NAMES[3],
            Event::DeclaresFinished { .. } => EVENT_NAMES[4],
            Event::Stops { .. } => EVENT_NAMES[5],
            Event::AboutToStart { .. } => EVENT_NAMES[6],
        }
    }

    pub fn criterion_id(&self) -> Option<&str> {
        match self {
            Event::TestsCommit { criterion_id, .. }
            | Event::AnnounceTestChange { criterion_id, .. }
            | Event::DeclaresFinished { criterion_id }
            | Event::AboutToStart { criterion_id } => Some(criterion_id),
            Event::FileEdit { criterion_id, .. } => criterion_id.as_deref(),
            Event::FirstEditOrCommit | Event::Stops { .. } => None,
        }
    }

    /// The event as the model's object: `on` plus its fields.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("on".into(), Value::from(self.name()));
        match self {
            Event::TestsCommit { criterion_id, touches_code } => {
                m.insert("criterion_id".into(), Value::from(criterion_id.as_str()));
                m.insert("touches_code".into(), Value::Bool(*touches_code));
            }
            Event::FileEdit { criterion_id, file_path } => {
                if let Some(c) = criterion_id {
                    m.insert("criterion_id".into(), Value::from(c.as_str()));
                }
                m.insert("file_path".into(), Value::from(file_path.as_str()));
            }
            Event::AnnounceTestChange { criterion_id, file_path } => {
                m.insert("criterion_id".into(), Value::from(criterion_id.as_str()));
                if let Some(f) = file_path {
                    m.insert("file_path".into(), Value::from(f.as_str()));
                }
            }
            Event::DeclaresFinished { criterion_id } | Event::AboutToStart { criterion_id } => {
                m.insert("criterion_id".into(), Value::from(criterion_id.as_str()));
            }
            Event::Stops { main } => {
                m.insert("main".into(), Value::Bool(*main));
            }
            Event::FirstEditOrCommit => {}
        }
        Value::Object(m)
    }
}

/// What a refusal names as what must come first: nothing, one name (a fact
/// kind or an event), or the hold's own clearing moves, passed on unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum Clears {
    Nothing,
    Name(String),
    Moves(Vec<Clear>),
}

impl Clears {
    pub fn to_json(&self) -> Value {
        match self {
            Clears::Nothing => Value::Null,
            Clears::Name(s) => Value::from(s.as_str()),
            Clears::Moves(ms) => Value::Array(ms.iter().cloned().map(Value::Object).collect()),
        }
    }
}

/// A refusal: one hold, or no hold and only too early, with the state and
/// what must come first.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub hold: Option<String>,
    pub state: Option<&'static str>,
    pub criterion_id: Option<String>,
    pub clears: Clears,
}

/// What `decide` returns; a decision is computed, never stored.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Allowed; `told` carries what the agent is told without being refused.
    Allow { told: Option<String> },
    Refuse(Refusal),
}

impl Decision {
    pub fn allowed(&self) -> bool {
        matches!(self, Decision::Allow { .. })
    }

    /// The model's object: `{allow: true}` | `{allow: true, told}` |
    /// `{allow: false, hold, clears}` | `{allow: false, hold: null, state, criterion_id, clears}`.
    pub fn to_json(&self) -> Value {
        match self {
            Decision::Allow { told: None } => json!({ "allow": true }),
            Decision::Allow { told: Some(t) } => json!({ "allow": true, "told": t }),
            Decision::Refuse(r) => {
                let mut m = Map::new();
                m.insert("allow".into(), Value::Bool(false));
                m.insert("hold".into(), r.hold.as_deref().map(Value::from).unwrap_or(Value::Null));
                if let Some(s) = r.state {
                    m.insert("state".into(), Value::from(s));
                }
                if let Some(c) = &r.criterion_id {
                    m.insert("criterion_id".into(), Value::from(c.as_str()));
                }
                m.insert("clears".into(), r.clears.to_json());
                Value::Object(m)
            }
        }
    }
}

fn allow() -> Decision {
    Decision::Allow { told: None }
}

fn refuse_hold(hold: &str, clears: Clears) -> Decision {
    Decision::Refuse(Refusal { hold: Some(hold.to_string()), state: None, criterion_id: None, clears })
}

/// The criterion's planned and written test files.
pub fn test_files(v: &View, c: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in v.of("test_planned", c).chain(v.of("test_written", c)) {
        if let Some(fp) = text(f, "file_path") {
            if !out.iter().any(|x| x == fp) {
                out.push(fp.to_string());
            }
        }
    }
    out
}

/// The anchor's file, from `place_chosen` or `new_work_spot_chosen`; `None`
/// for `new_work_no_spot` or no anchor.
pub fn anchor_file(v: &View, c: &str) -> Option<String> {
    let a = plan::anchor_of(v, c)?;
    if let Some(p) = a.fields.get("places").and_then(Value::as_array).and_then(|ps| ps.first()) {
        return p.get("file_path").and_then(Value::as_str).map(str::to_string);
    }
    text(a, "file_path").map(str::to_string)
}

fn released(v: &View, c: &str) -> bool {
    holds(v, c).iter().any(|h| h.why == "released")
}

/// Whether the criterion is green, proven or released.
fn done(v: &View, c: &str) -> bool {
    matches!(state(v, c), "green" | "proven") || released(v, c)
}

/// Whether the work is done as main's stop reads it: every part done (stop 5 with `main`), and
/// at least one part not released, so a plan released whole is no done work.
fn all_done(v: &View) -> bool {
    let cs = plan::criteria(v);
    cs.iter().any(|c| !released(v, c)) && cs.iter().all(|c| done(v, c))
}

/// What `done` needs and is cheaper to read than the holds: released, or the conditions of
/// green or proven, which `state` requires beside the rest.
fn may_be_done(v: &View, c: &str) -> bool {
    crate::lifecycle::released(v, c)
        || crate::lifecycle::green_conditions(v, c, Place::Laptop)
        || crate::lifecycle::proven_conditions(v, c)
}

/// The first done commit [draft name] (the owner's CI bar, 2026-10-20): the head, the latest
/// `commit_seen`'s `commit`, at the shortest prefix of the log where every part is done and
/// one is active ([`all_done`]); the same tip the binary's pipeline run is owed at once all is
/// done. History: a later commit or a part added after it never moves it.
pub fn first_done_commit(v: &View) -> Option<String> {
    // nothing is done before the first commit is seen
    let from = v.facts.iter().position(|f| f.kind == "commit_seen")? + 1;
    (from..=v.len()).find_map(|n| {
        let p = v.prefix(n);
        if !plan::criteria(&p).iter().all(|c| may_be_done(&p, c)) || !all_done(&p) {
            return None;
        }
        p.last("commit_seen").and_then(|f| text(f, "commit")).map(str::to_string)
    })
}

fn clear_of(kind: &str) -> Clear {
    let mut m = Map::new();
    m.insert("kind".into(), Value::from(kind));
    m
}

/// What each state waits for: the tests-first commit alone while testing, the
/// code once sealed, in the same shapes the holds name as their clears.
fn comes_next(state: &str) -> Option<Clear> {
    match state {
        "testing" => Some(crate::holds::tests_only_clear()),
        "sealed" => Some(crate::holds::code_clear()),
        _ => None,
    }
}

/// The holder's claim as a clear, of any category.
fn claim_clear(c: &str) -> Clear {
    let mut m = clear_of("fix_location_category");
    m.insert("criterion_id".into(), Value::from(c));
    m
}

/// A hold's clears; before the holder's claim the claim comes first, except
/// for a card a person answers.
fn hold_clears(v: &View, c: &str, h: crate::criterion::Hold) -> Clears {
    if first_act(v, c) && crate::criterion::answers_of(&h.why).is_none() && binary_moves(v).is_empty() {
        return Clears::Moves(vec![claim_clear(c)]);
    }
    Clears::Moves(h.clears)
}

/// The move that advances this criterion in its current state, not merely
/// any move that carries its id.
fn next(v: &View, c: &str) -> Clears {
    // the binary's run comes first: its tests written, run and stamped
    if let Some(b) = binary_moves(v).first() {
        let mut m = clear_of(&b.kind);
        if let Some(cid) = b.fields.get("criterion_id") {
            m.insert("criterion_id".into(), cid.clone());
        }
        return Clears::Moves(vec![m]);
    }
    // the referee's check before the criterion starts
    let start_check = criterion_admissible(v).iter().any(|m| {
        m.kind == "anchor_sha_matched"
            && m.fields.get("criterion_id").and_then(Value::as_str) == Some(c)
            && m.fields.get("checked_at").and_then(Value::as_str) == Some("start")
    });
    if start_check {
        let mut m = clear_of("anchor_sha_matched");
        m.insert("criterion_id".into(), Value::from(c));
        m.insert("checked_at".into(), Value::from("start"));
        return Clears::Moves(vec![m]);
    }
    if first_act(v, c) {
        return Clears::Moves(vec![claim_clear(c)]); // the holder's first act is its claim
    }
    match comes_next(state(v, c)) {
        Some(m) => Clears::Moves(vec![m]),
        None => Clears::Nothing,
    }
}

/// The fact kinds that supply each plan line.
pub const LINE_SUPPLY: [&[&str]; 11] = [
    &["intent_submitted"],
    // the owner (2026-10-11): line 2's documentation check is the binary's, its cure when false the re-claim --path full
    &["criterion", "anchor_is_documentation"],
    &["behavior", "promise"],
    &["place_chosen", "new_work_spot_chosen", "new_work_no_spot"],
    &["anchor_sha_matched", "place_chosen", "new_work_spot_chosen", "new_work_no_spot"],
    &["test_planned"],
    &["test_stack_detected"],
    &["touching_test_declared"],
    &["criterion_head", "criterion_strung_on_head"],
    &["criterion_assigned"],
    // line 11's claim is made by the holder, so `criterion_assigned` comes first
    &["fix_location_category", "criterion_assigned"],
];

/// The kinds that supply the first failing plan line, narrowed to those
/// admissible now when any are.
fn plan_clears(v: &View) -> Clears {
    let Some(i) = lines_in_order(v).into_iter().find(|&i| !plan::line(v, plan::PLAN_LINES[i]).unwrap_or(true)) else {
        return Clears::Nothing;
    };
    // a place off the try's list [draft]: another place, or the agent's reason for this one
    if i == 3 {
        if let Some(c) = plan::criteria(v).into_iter().find(|c| crate::trace::reason_owed(v, c)) {
            let mut reason = clear_of(crate::trace::OFF_LIST);
            reason.insert("criterion_id".into(), Value::from(c.as_str()));
            return Clears::Moves(vec![clear_of("place_chosen"), reason]);
        }
    }
    let adm: Vec<String> = plan_admissible(v)
        .into_iter()
        .chain(criterion_admissible(v))
        .map(|m| m.kind)
        .collect();
    let supply = LINE_SUPPLY[i];
    let ks: Vec<&str> = supply.iter().copied().filter(|k| adm.iter().any(|a| a == k)).collect();
    let ks = if ks.is_empty() { supply.to_vec() } else { ks };
    Clears::Moves(ks.into_iter().map(clear_of).collect())
}

/// The first non-released hold, or the pausing one while paused, so that what
/// the person must answer comes first.
fn first_hold(v: &View, c: &str) -> Option<crate::criterion::Hold> {
    let hs: Vec<crate::criterion::Hold> = holds(v, c).iter().filter(|h| h.why != "released").cloned().collect();
    if paused(v, c) {
        if let Some(h) = hs.iter().find(|h| PAUSING.contains(&h.why.as_str())) {
            return Some(h.clone());
        }
    }
    hs.into_iter().next()
}

/// Refuses a waiting criterion with what clears its nearest predecessor that
/// is not done.
fn wait_refusal(v: &View, c: &str) -> Decision {
    let mut seen: Vec<String> = vec![c.to_string()];
    let mut p = plan::string_before(v, c);
    while let Some(x) = p.clone() {
        if seen.contains(&x) || !done(v, &x) {
            break;
        }
        seen.push(x.clone());
        p = plan::string_before(v, &x);
    }
    let pending = p.filter(|x| !seen.contains(x));
    let clears = match pending.as_deref().and_then(|x| first_hold(v, x).map(|h| (x, h))) {
        Some((x, h)) => hold_clears(v, x, h),
        None => next(v, pending.as_deref().unwrap_or(c)),
    };
    Decision::Refuse(Refusal { hold: None, state: Some("waiting"), criterion_id: Some(c.to_string()), clears })
}

/// Nothing is wrong, only too early: the state and what must come first.
fn not_yet(v: &View, c: &str) -> Decision {
    let st = state(v, c);
    if st == "waiting" {
        return wait_refusal(v, c);
    }
    Decision::Refuse(Refusal { hold: None, state: Some(st), criterion_id: Some(c.to_string()), clears: next(v, c) })
}

/// The first non-released hold, which the refusal names.
fn held_refusal(v: &View, c: &str) -> Decision {
    match first_hold(v, c) {
        Some(h) => {
            let why = h.why.clone();
            refuse_hold(&why, hold_clears(v, c, h))
        }
        None => not_yet(v, c),
    }
}

/// The plan's lines in the order they are owed: as numbered, except while a traced part still owes
/// its red try, when lines 1–3 and 6–10 come before the place (the owner's plan of 2026-10-23) [draft].
fn lines_in_order(v: &View) -> Vec<usize> {
    let try_first = plan::criteria(v).iter().any(|c| crate::trace::needs_try(v, c) && crate::trace::red_try(v, c).is_none());
    if try_first {
        vec![0, 1, 2, 5, 6, 7, 8, 9, 3, 4, 10]
    } else {
        (0..plan::PLAN_LINES.len()).collect()
    }
}

/// A refusal of `hold` on `c` naming what clears it.
fn refuse_on(hold: &str, c: &str, clears: Vec<Clear>) -> Decision {
    Decision::Refuse(Refusal { hold: Some(hold.to_string()), state: None, criterion_id: Some(c.to_string()), clears: Clears::Moves(clears) })
}

/// The try's step, when it is the plan's next (the owner's plan of 2026-10-23) [draft]: once lines
/// 1–3 and 6–10 hold, the first part whose person's card on how to make it happen is open, else the
/// first whose try is owed or passed.
fn try_refusal(v: &View) -> Option<Decision> {
    if lines_in_order(v).into_iter().take(8).any(|i| !plan::line(v, plan::PLAN_LINES[i]).unwrap_or(true)) {
        return None;
    }
    let cs = plan::criteria(v);
    if let Some(c) = cs.iter().find(|c| crate::trace::repro_unknown(v, c)) {
        return Some(refuse_on(crate::trace::REPRO, c, crate::holds::repro_clears(c)));
    }
    cs.iter().find_map(|c| crate::trace::try_hold(v, c).map(|h| refuse_on(h, c, crate::holds::try_clears(c))))
}

/// Stop 1 for an edit of a traced part's planned test file before all eleven lines hold [draft]:
/// allowed once the part's test may be written, unless the person's card on how to make it happen
/// is open; `None` when the file is no such part's test.
fn test_edit(v: &View, file_path: &str) -> Option<Decision> {
    let c = plan::criteria(v).into_iter().find(|c| {
        crate::trace::needs_try(v, c) && v.of("test_planned", c).any(|f| text(f, "file_path") == Some(file_path)) && plan::test_may_be_written(v, c)
    })?;
    // another part's sealed test is edited only after its announcement, as once planning is confirmed
    if let Some(d) = sealed_edit(v, file_path) {
        return Some(d);
    }
    // no card of the person's open on the part: the one question, or any other card
    let card = holds(v, &c).iter().find(|h| h.why == crate::trace::REPRO || crate::criterion::answers_of(&h.why).is_some()).cloned();
    if let Some(h) = card {
        return Some(refuse_on(&h.why, &c, h.clears));
    }
    Some(allow())
}

/// Stop 3's first rule: an edit of a sealed test before its announcement names the part whose test
/// the file is, not the one the edit was made for.
fn sealed_edit(v: &View, file_path: &str) -> Option<Decision> {
    plan::criteria(v).into_iter().find_map(|x| {
        (sealed_at(v, &x, Place::Laptop).is_some() && test_files(v, &x).iter().any(|f| f == file_path) && !announced(v, &x)).then(|| {
            Decision::Refuse(Refusal {
                hold: Some("proof_tampered".to_string()),
                state: None,
                criterion_id: Some(x.to_string()),
                clears: Clears::Name("announce_test_change".to_string()),
            })
        })
    })
}

/// Allows or refuses an event, reading only the log and the event.
/// The cases card's step [draft]: the first part whose case-finder is owed, else the first whose
/// card is open; the test and the rest of the work wait for it. A stop goes while that part's panel
/// waits only on its calls (the owner's answer, 2026-10-27: only waiting is left).
fn cases_refusal(v: &View, event: &Event) -> Option<Decision> {
    let cs = plan::criteria(v);
    if let Some(c) = cs.iter().find(|c| crate::cases::owed(v, c)) {
        if matches!(event, Event::Stops { .. }) && crate::cases::only_waiting(v, c) {
            return Some(allow());
        }
        return Some(refuse_on(crate::cases::OWED, c, crate::holds::cases_owed_clears(c)));
    }
    let c = cs.iter().find(|c| crate::cases::card_open(v, c))?;
    Some(refuse_on(crate::cases::CARD, c, crate::holds::cases_card_clears(v, c)))
}

pub fn decide(v: &View, event: &Event) -> Decision {
    if !plan::planning_confirm_complete(v) {
        // the cases card (the owner's plan of 2026-10-25) [draft]: the finder, then the person's card
        if let Some(d) = cases_refusal(v, event) {
            return d;
        }
        // a traced part's planned test is written before its place, once lines 1–3 and 6–10 hold [draft]
        if let Event::FileEdit { file_path, .. } = event {
            if let Some(d) = test_edit(v, file_path) {
                return d;
            }
        }
        // the try, or the person's card before it, when it is the plan's next step [draft]
        if let Some(d) = try_refusal(v) {
            return d;
        }
        // stop 1 holds any edit or commit until planning is confirmed
        return refuse_hold("planning_incomplete", plan_clears(v));
    }
    match event {
        Event::FirstEditOrCommit => allow(),
        Event::TestsCommit { criterion_id: c, touches_code } => {
            // stop 2: its turn, not paused, tests only
            if !turn(v, c) {
                return wait_refusal(v, c);
            }
            if paused(v, c) {
                return held_refusal(v, c);
            }
            // places before the fix [draft]: every listed site classified first
            if crate::places::holds(v, c) {
                return refuse_hold(crate::places::HOLD, Clears::Moves(crate::holds::place_clears(v, c)));
            }
            // a clarifying question on a signal [draft]: the agent's word on the ask, then the person's pick
            if crate::ask::holds(v, c) {
                return refuse_hold(crate::ask::HOLD, Clears::Moves(crate::holds::reading_clears(v, c)));
            }
            if *touches_code {
                return refuse_hold("tests_and_code_mixed", Clears::Name(EVENT_NAMES[1].to_string()));
            }
            allow()
        }
        Event::FileEdit { criterion_id, file_path } => {
            // stop 3: a sealed test is edited only after an announcement
            if let Some(d) = sealed_edit(v, file_path) {
                return d;
            }
            // S9c review 1 (the owner's rule): an edit of a waiting part's place or test is refused,
            // whichever part the conversation works on, unless a part in its turn owns the file too
            let owns = |x: &str| {
                plan::anchor_of(v, x).is_some_and(|a| plan::file_paths_of(a).iter().any(|f| f == file_path)) || test_files(v, x).iter().any(|f| f == file_path)
            };
            let cs = plan::criteria(v);
            if let Some(w) = cs.iter().find(|x| !turn(v, x) && !crate::lifecycle::released(v, x) && owns(x)) {
                if !cs.iter().any(|y| turn(v, y) && !crate::lifecycle::released(v, y) && owns(y)) {
                    return wait_refusal(v, w);
                }
            }
            if let Some(c) = criterion_id {
                // S9a's two (the owner, 2026-10-16): a waiting part's edits are refused as its commits are
                if !turn(v, c) {
                    return wait_refusal(v, c);
                }
                if let Some(anchor) = anchor_file(v, c) {
                    if &anchor != file_path && !test_files(v, c).iter().any(|f| f == file_path) {
                        return Decision::Allow { told: Some(format!("this edit is outside the anchor of {c}; it is told, not refused")) };
                    }
                }
            }
            allow()
        }
        Event::AnnounceTestChange { .. } => allow(), // stop 3: the proof restarts
        Event::DeclaresFinished { criterion_id: c } => {
            // stop 4: finish only when done
            if done(v, c) {
                return allow();
            }
            if holds(v, c).iter().any(|h| h.why != "released") {
                return held_refusal(v, c);
            }
            not_yet(v, c)
        }
        Event::Stops { main } => {
            // stop 5: every criterion green, proven or released; off main, every one touched
            // here (the owner, 2026-10-15): an inherited part never keeps a lane from stopping
            for x in plan::criteria(v) {
                if !*main && !touched_here(v, &x) {
                    continue;
                }
                if !done(v, &x) {
                    if holds(v, &x).iter().any(|h| h.why != "released") {
                        return held_refusal(v, &x);
                    }
                    return not_yet(v, &x);
                }
            }
            allow()
        }
        Event::AboutToStart { criterion_id } => {
            // the referee checks only this criterion's own anchor
            if let Some(h) = holds(v, criterion_id).iter().find(|h| h.why == "anchor_moved") {
                return refuse_hold("anchor_moved", Clears::Moves(h.clears.clone()));
            }
            allow()
        }
    }
}

/// Every event to try at a log, in a deterministic order.
pub fn events(v: &View) -> Vec<Event> {
    let cs = plan::criteria(v);
    let files_for = |c: &str| -> Vec<String> {
        let mut fs = test_files(v, c);
        if let Some(a) = anchor_file(v, c) {
            fs.push(a);
        }
        fs.extend(v.of("touching_test_declared", c).filter_map(|f| text(f, "file_path")).map(str::to_string));
        fs.sort();
        fs.dedup();
        fs
    };
    let mut out = vec![Event::FirstEditOrCommit];
    for c in &cs {
        for tc in [false, true] {
            out.push(Event::TestsCommit { criterion_id: c.clone(), touches_code: tc });
        }
    }
    for c in &cs {
        for f in files_for(c) {
            out.push(Event::FileEdit { criterion_id: Some(c.clone()), file_path: f });
        }
    }
    for c in &cs {
        for f in files_for(c) {
            out.push(Event::AnnounceTestChange { criterion_id: c.clone(), file_path: Some(f) });
        }
    }
    for c in &cs {
        out.push(Event::DeclaresFinished { criterion_id: c.clone() });
    }
    out.push(Event::Stops { main: true });
    for c in &cs {
        out.push(Event::AboutToStart { criterion_id: c.clone() });
    }
    out
}
