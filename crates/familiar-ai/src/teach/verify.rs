//! The words of `familiar-ai verify`: one line per criterion, in string order, with its
//! result, its evidence and the next clearing move. Every reading is the rules'
//! (`familiar_ai_rules::action`); this module names the first of them that is not true.

use familiar_ai_rules::{action, plan, View};

use crate::door::verify::{Rechecked, Run};
use crate::door::git_program::short;
use crate::teach::words::{quoted, LEAD};

/// The one line for a proof that could not be established at all.
pub fn could_not(why: &str, next: &str) -> String {
    format!("{LEAD}verify could not prove anything: {why} · next: {next}")
}

/// The why of a `could_not` line, for setup's line when the history could
/// not be rechecked.
pub fn why_could_not(line: &str) -> &str {
    let rest = line.strip_prefix(LEAD).unwrap_or(line);
    let rest = rest.strip_prefix("verify could not prove anything: ").unwrap_or(rest);
    rest.split(" · next: ").next().unwrap_or(rest)
}

/// The next move for a part whose card answer CI could not verify.
pub const NOT_VERIFIED_NEXT: &str = "the person answers the card with familiar-ai approve, signing with the key CI's FAMILIAR_AI_ALLOWED_SIGNERS names, and the approval is committed beside the plan";

/// What one criterion came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Proven(String),
    Released,
    NotProven { why: String, next: String },
    CouldNot { why: String, next: String },
}

/// One criterion as the report carries it, with its outcome
/// (`proven`, `not_proven`, `released` or `could_not`) and its printed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub text: String,
    pub outcome: &'static str,
    pub line: String,
}

/// The exit code and the lines, one per criterion in string order.
pub fn lines(r: &Rechecked) -> (i32, Vec<String>) {
    let (exit, rows) = rows(r);
    (exit, rows.into_iter().map(|row| row.line).collect())
}

/// The exit code and one `Row` per criterion in string order.
/// The exit is 0 when every criterion is proven or released, 2 when any could not be proven, else 1.
/// Each criterion is read from its own part's recheck, at its own last commit.
pub fn rows(r: &Rechecked) -> (i32, Vec<Row>) {
    let v = View::new(&r.log);
    let mut exit = 0;
    let mut out = Vec::new();
    for c in in_string_order(&v) {
        let p = r.parts.iter().find(|p| action::crits(&View::new(&p.log)).contains(&c)).unwrap_or(r);
        let pv = View::new(&p.log);
        // An answer CI cannot verify fails closed, whatever the part's own recheck says.
        let verdict = match r.unverified.iter().find(|(x, _)| *x == c) {
            Some((_, why)) => not_proven(format!("approval not verified ({why})"), NOT_VERIFIED_NEXT),
            None => verdict(p, &pv, &c),
        };
        let (code, outcome) = match verdict {
            Verdict::Proven(_) => (0, "proven"),
            Verdict::Released => (0, "released"),
            Verdict::NotProven { .. } => (1, "not_proven"),
            Verdict::CouldNot { .. } => (2, "could_not"),
        };
        exit = exit.max(code);
        if code == 0 && action::check_state(&pv) != "passed" {
            // Unreachable: a part proven or released is the rules' `passed` on its own recheck.
            exit = 1;
        }
        let text = crate::teach::words::text_of(&pv, &c).unwrap_or_default().to_string();
        out.push(Row { line: line(&pv, &c, &verdict), id: c, text, outcome });
    }
    (exit, out)
}

fn line(v: &View, c: &str, verdict: &Verdict) -> String {
    let name = quoted(v, c);
    match verdict {
        Verdict::Proven(evidence) => format!("{LEAD}{name} · proven · {evidence}"),
        Verdict::Released => format!("{LEAD}{name} · released by you · shown, not rechecked"),
        Verdict::NotProven { why, next } => format!("{LEAD}{name} · not proven: {why} · next: {next}"),
        Verdict::CouldNot { why, next } => format!("{LEAD}{name} · could not be proven: {why} · next: {next}"),
    }
}

/// Heads in plan order, each followed by the criteria strung on it, then any unstrung criterion in plan order.
fn in_string_order(v: &View) -> Vec<String> {
    let crits = action::crits(v);
    let place = |c: &String| -> (usize, usize) {
        let (mut head, mut depth) = (c.clone(), 0);
        while let Some(b) = plan::string_before(v, &head) {
            head = b;
            depth += 1;
            if depth > crits.len() {
                break;
            }
        }
        let i = if plan::on_string(v, c) { crits.iter().position(|x| *x == head).unwrap_or(crits.len()) } else { crits.len() };
        (i, depth)
    };
    let mut out = crits.clone();
    out.sort_by_key(place);
    out
}

/// A run of the command, as evidence: `(<command>, exit N)` or why it did not run.
fn exit_of(r: Option<&Run>) -> String {
    match r.map(|r| (&r.ran.exit, &r.ran.not_run)) {
        Some((_, Some(why))) => format!("did not run: {why}"),
        Some((Some(code), None)) => format!("exit {code}"),
        _ => "no run".to_string(),
    }
}

fn run_of<'a>(r: &'a Rechecked, c: &str, commit: &str, teeth: bool) -> Option<&'a Run> {
    r.runs.iter().find(|x| x.criterion_id == c && x.commit == commit && x.teeth == teeth)
}

fn not_proven(why: String, next: &str) -> Verdict {
    Verdict::NotProven { why, next: next.to_string() }
}

fn could(why: String, next: &str) -> Verdict {
    Verdict::CouldNot { why, next: next.to_string() }
}

/// The first of the rules' readings that is not true, in the recheck's order.
pub fn verdict(r: &Rechecked, v: &View, c: &str) -> Verdict {
    if action::released(v, c) {
        return Verdict::Released;
    }
    if let Some(s) = r.stopped.as_ref().filter(|s| s.criterion_id == c) {
        let cmd = action::command(v, c).unwrap_or_default();
        let (where_, not) = if s.teeth { ("with the change taken back out at", "a failing test") } else { ("at the tests-first commit", "a red test") };
        return could(
            format!("{where_} {} the test command ({cmd}) did not run: {}; a run that did not happen is not {not}", short(&s.commit), s.why),
            "fix the command or the environment so the tests run, and run verify again",
        );
    }
    let h = short(&r.head);
    let Some(cmd) = action::command(v, c) else {
        return could(format!("the plan committed at {h} claims no test command for its part of the code"), "claim the stack with --test_command and commit the plan");
    };
    if action::tests(v, c).is_empty() {
        return could(format!("the plan committed at {h} names no test written for it"), "commit the plan as the binary wrote it after the tests-first commit");
    }
    let Some(t) = action::tests_first(v, c) else {
        return no_tests_first(v, c, h);
    };
    if r.stopped.as_ref().is_some_and(|s| s.criterion_id != c) && run_of(r, c, t, false).is_none() {
        return could(format!("not checked at {h}: the recheck stopped at a criterion before it"), "clear that criterion, and run verify again");
    }
    checked(r, v, c, t, &cmd)
}

/// The verdict when no tests-only commit holds the tests as they are at the head:
/// not proven if they changed after the seal, could not if they were never committed alone.
fn no_tests_first(v: &View, c: &str, h: &str) -> Verdict {
    let files = action::test_files(v, c);
    let tests_only = v.kind("commit_seen").find(|f| {
        f.fields.get("touches_tests") == Some(&serde_json::json!(true))
            && f.fields.get("touches_code") == Some(&serde_json::json!(false))
            && f.fields.get("files").and_then(|x| x.as_array()).is_some_and(|a| a.iter().any(|p| p.as_str().is_some_and(|p| files.iter().any(|x| x == p))))
    });
    match tests_only.and_then(|f| f.fields.get("commit")).and_then(|x| x.as_str()) {
        Some(t) => not_proven(
            format!("its sealed tests were changed after the tests-first commit {}: the tests at the head {h} are not the stamped ones", short(t)),
            "put the sealed test back, or commit the changed test alone and see it fail",
        ),
        None => could(
            format!("no tests-first commit: its tests were never committed alone, before the fix, in the history up to {h}"),
            "commit the tests alone before the fix (and check out the whole history: fetch-depth: 0)",
        ),
    }
}

/// A tests-only commit of `c`'s tests that a laptop stamp sealed under a proof other than the one claimed now.
fn stamped_before<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let now = action::proof(v, c)?;
    let files = action::test_files(v, c);
    let tests_only = |id: &str| {
        v.kind("commit_seen").any(|f| {
            f.fields.get("commit").and_then(|x| x.as_str()) == Some(id)
                && f.fields.get("touches_tests") == Some(&serde_json::json!(true))
                && f.fields.get("touches_code") == Some(&serde_json::json!(false))
                && f.fields.get("files").and_then(|x| x.as_array()).is_some_and(|a| a.iter().any(|p| p.as_str().is_some_and(|p| files.iter().any(|x| x == p))))
        })
    };
    v.of("fingerprint_stamped", c)
        .filter(|s| s.fields.get("ran_in").and_then(|x| x.as_str()) == Some("laptop"))
        .filter(|s| s.fields.get("proof_not_tampered").and_then(|x| x.as_str()) != Some(now))
        .filter_map(|s| s.fields.get("commit").and_then(|x| x.as_str()))
        .find(|id| tests_only(id))
}

/// The checks after the tests-first commit is found, in order: red, the same stamp,
/// green, the anchor touched, the touching tests, the teeth.
fn checked(r: &Rechecked, v: &View, c: &str, t: &str, cmd: &str) -> Verdict {
    let h = r.head.as_str();
    let red_run = exit_of(run_of(r, c, t, false));
    if !action::red(v, c) {
        // A second seal after the way back is the seal itself: its tests passing there is not red.
        if let Some(sealed) = stamped_before(v, c).filter(|_| action::second_seal(v, c).is_none()) {
            // An earlier tests-first commit was stamped under another proof and its change was never
            // announced after green, so the change does not re-prove.
            return not_proven(
                format!("its sealed tests were changed after the tests-first commit {}: the tests at the head {} are not the stamped ones", short(sealed), short(h)),
                "put the sealed test back, or announce the change (test_change_announced) before committing it alone",
            );
        }
        return not_proven(format!("its tests passed at the tests-first commit {} ({cmd}, {red_run})", short(t)), "commit a test that fails on the code as it is, alone, before the fix");
    }
    if !action::stamped(v, c) {
        return not_proven(format!("the tests that ran at {} are not the sealed ones", short(t)), "commit the changed test alone and see it fail");
    }
    if h == t {
        return not_proven(format!("the head {} is the tests-first commit: no fix is committed after it", short(h)), "commit the fix");
    }
    let green_run = exit_of(run_of(r, c, h, false));
    if !action::green_at_head(v, c) {
        return not_proven(format!("its tests fail at the head {} ({cmd}, {green_run})", short(h)), "fix the code until its tests pass, without changing them");
    }
    if !action::overlap(v, c) {
        return not_proven(format!("no commit after the tests-first one {} up to the head {} changed the place the plan named", short(t), short(h)), "change the code at the plan's place, or choose its place again");
    }
    if !action::touching_ok(v, c) {
        return not_proven(format!("a test that touches its code fails at the head {} ({cmd}, {green_run})", short(h)), "fix the code so the tests that touch it pass again");
    }
    let teeth_run = exit_of(run_of(r, c, h, true));
    // New work's teeth run puts back its integration point, or its file when a spot was chosen.
    let put_back = match familiar_ai_rules::plan::anchor_of(v, c).map(|a| a.kind) {
        Some("new_work_no_spot") => "the integration point put back",
        Some("new_work_spot_chosen") => "the file put back",
        _ => "the change taken back out",
    };
    if !action::teeth(v, c) {
        return not_proven(format!("its tests still pass with {put_back} at {} ({cmd}, {teeth_run})", short(h)), "make the test check what the change does");
    }
    if !action::proven(v, c) {
        return not_proven(format!("the pipeline's facts at {} do not read proven", short(h)), "run verify again on a clean clone");
    }
    let teeth = format!("fails with {put_back} ({teeth_run})");
    Verdict::Proven(format!("red at {} ({red_run}) · green at {} ({green_run}) · {teeth} · ran: {cmd}", short(t), short(h)))
}
