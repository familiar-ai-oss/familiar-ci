//! The next step once every criterion is green or proven here: push, open the pull
//! request, then report what CI's checks say. Without `gh`, or with a detached head
//! or no commits, nothing past the push is said.

use std::path::Path;
use std::time::Duration;

use familiar_ai_facts::Log;
use familiar_ai_rules::{lifecycle, plan, View};
use serde_json::Value;

use crate::door::child::{self, Ended};
use crate::teach::words::LEAD;

/// GitHub's command line, asked about the pull request.
pub const GH: &str = "gh";
/// The check whose success alone earns the ✓: the job name `gh pr checks` reports for `familiar-ai verify`.
/// A step's name makes no check, so the job itself must carry this name.
pub const FAMILIAR_VERIFY: &str = "familiar-verify";
/// How long one `gh` call may take before it counts as no answer.
pub const GH_WAIT: Duration = Duration::from_secs(3);

/// Where the work stands once it is proven here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Not pushed (ahead of its upstream, or no upstream).
    Push,
    /// Pushed, and no pull request for the branch.
    OpenPr,
    /// A pull request is open and CI has not finished (or says nothing).
    Proving,
    /// The `familiar-verify` check succeeded on the head here.
    Proven,
    /// A check failed: `familiar-verify`'s description (else its name), or
    /// another check's name.
    Found(String),
}

/// True when every unreleased criterion is green or proven here and no judge's no stands against its seal.
pub fn proven_here(log: &Log) -> bool {
    let v = View::new(log);
    let live: Vec<String> = plan::criteria(&v).into_iter().filter(|c| !lifecycle::released(&v, c)).collect();
    !live.is_empty() && live.iter().all(|c| matches!(lifecycle::state(&v, c), "green" | "proven") && !crate::teach::judge::not_covered(&v, c))
}

/// Where the work stands once proven here. `gh: None` reads git alone.
pub fn step(log: &Log, root: &Path, gh: Option<&str>) -> Option<Step> {
    if !proven_here(log) {
        return None;
    }
    let up = familiar_ai_git::upstream(root).ok().flatten()?;
    if up.ahead != Some(0) {
        return Some(Step::Push);
    }
    let gh = gh?;
    let ask = |args: &[&str]| child::run(gh, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), root, &[], "", GH_WAIT);
    let view = ask(&["pr", "view", "--json", "number,headRefOid"]);
    match view.ended {
        Ended::NotStarted(_) | Ended::TimedOut => return None,
        Ended::Exited(Some(0)) => {}
        Ended::Exited(_) => return Some(Step::OpenPr),
    }
    let checks = ask(&["pr", "checks", "--json", "name,bucket,state,description"]);
    let here = familiar_ai_git::head_commit(root).ok().flatten();
    Some(summarise(&checks.stdout, pr_head(&view.stdout).as_deref() == here.as_deref() && here.is_some()))
}

/// The pull request's head commit, from `gh pr view --json headRefOid`.
fn pr_head(out: &str) -> Option<String> {
    serde_json::from_str::<Value>(out.trim()).ok()?.get("headRefOid")?.as_str().map(str::to_string)
}

/// Summarise `gh pr checks`: a failed check is `Found`, and only `familiar-verify` passing on the
/// current head is `Proven`; missing, pending, skipped, cancelled or timed out still reads as proving.
pub fn summarise(out: &str, current: bool) -> Step {
    let Ok(Value::Array(checks)) = serde_json::from_str::<Value>(out.trim()) else {
        return Step::Proving;
    };
    let field = |c: &Value, k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let ours = checks.iter().find(|c| field(c, "name") == FAMILIAR_VERIFY);
    // gh's `fail` bucket also holds a timed-out run, which is not a failure of the proof.
    if let Some(c) = ours.filter(|c| field(c, "bucket") == "fail" && field(c, "state") != "TIMED_OUT") {
        // The description is the failure only when it is verify's own line, not GitHub's exit-code message.
        let said = field(c, "description");
        return Step::Found(said.strip_prefix(LEAD).map_or_else(|| field(c, "name"), str::to_string));
    }
    let others = checks.iter().filter(|c| field(c, "name") != FAMILIAR_VERIFY);
    if let Some(failed) = others.into_iter().find(|c| matches!(field(c, "bucket").as_str(), "fail" | "cancel")) {
        return Step::Found(field(failed, "name"));
    }
    let succeeded = ours.is_some_and(|c| field(c, "bucket") == "pass" && matches!(field(c, "state").as_str(), "SUCCESS" | ""));
    if current && succeeded {
        return Step::Proven;
    }
    Step::Proving
}

/// The step as the one line `status` and the session's start show.
pub fn words(s: &Step) -> String {
    match s {
        Step::Push => format!("{LEAD}locally green · next: push and open the pull request, and CI proves it"),
        Step::OpenPr => format!("{LEAD}locally green · next: open the pull request"),
        Step::Proving => format!("{LEAD}CI is proving it"),
        Step::Proven => format!("{LEAD}proven ✓"),
        Step::Found(name) => format!("{LEAD}CI found: {name} (see the PR)"),
    }
}

/// What the agent is told about the step, stated as facts because injected instructions are unreliable.
pub fn facts(s: &Step) -> String {
    const PROVEN: &str = "every criterion is locally green. CI proves it on the pull request.";
    match s {
        // The push begins with the tidy; see docs/decisions.md#tidy
        Step::Push => format!(
            "{LEAD}{PROVEN} The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts."
        ),
        Step::OpenPr => format!(
            "{LEAD}{PROVEN} The branch is pushed and has no pull request. The person has not been asked whether to open the pull request."
        ),
        other => words(other),
    }
}

/// True when the step is the person's to decide: the push or the PR.
pub fn to_ask(s: &Step) -> bool {
    matches!(s, Step::Push | Step::OpenPr)
}

/// The step as the status line's short words.
pub fn short(s: &Step) -> String {
    match s {
        Step::Push => "next: push and open the PR".into(),
        Step::OpenPr => "next: open the PR".into(),
        Step::Proving => "CI is proving it".into(),
        Step::Proven => "proven ✓".into(),
        Step::Found(name) => format!("CI found: {name}"),
    }
}

/// The step's `status` line, when there is one.
pub fn line(log: &Log, root: &Path, gh: Option<&str>) -> Option<String> {
    step(log, root, gh).map(|s| words(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ours(bucket: &str, state: &str, description: &str) -> String {
        format!(r#"[{{"name":"build","bucket":"pass","state":"SUCCESS"}},{{"name":"familiar-verify","bucket":"{bucket}","state":"{state}","description":"{description}"}}]"#)
    }

    #[test]
    fn the_check_is_proven_only_when_familiar_verify_succeeded_on_the_current_head() {
        assert_eq!(summarise(&ours("pass", "SUCCESS", ""), true), Step::Proven);
        assert_eq!(summarise(&ours("pass", "SUCCESS", ""), false), Step::Proving, "an older head's success proves nothing here");
        // every other check passed or was skipped, and no familiar-verify: not proven
        assert_eq!(summarise(r#"[{"name":"build","bucket":"pass"},{"name":"docs","bucket":"skipping"}]"#, true), Step::Proving);
        assert_eq!(summarise("[]", true), Step::Proving);
        assert_eq!(summarise("no checks reported", true), Step::Proving);
    }

    #[test]
    fn familiar_verify_pending_skipped_cancelled_or_timed_out_is_still_proving() {
        assert_eq!(summarise(&ours("pending", "IN_PROGRESS", ""), true), Step::Proving, "pending");
        assert_eq!(summarise(&ours("skipping", "SKIPPED", ""), true), Step::Proving, "skipped");
        assert_eq!(summarise(&ours("cancel", "CANCELLED", ""), true), Step::Proving, "cancelled");
        assert_eq!(summarise(&ours("fail", "TIMED_OUT", ""), true), Step::Proving, "timed out");
    }

    #[test]
    fn a_failed_familiar_verify_is_its_failure_and_another_failed_check_its_name() {
        let said = "familiar-ai: \\\"greet says hello\\\" · not proven: its tests fail at the head";
        assert_eq!(summarise(&ours("fail", "FAILURE", said), true), Step::Found("\"greet says hello\" · not proven: its tests fail at the head".into()));
        assert_eq!(summarise(&ours("fail", "FAILURE", ""), true), Step::Found("familiar-verify".into()), "no description: the check's name");
        assert_eq!(summarise(&ours("fail", "FAILURE", "Process completed with exit code 1."), true), Step::Found("familiar-verify".into()), "not verify's line: the check's name");
        assert_eq!(summarise(r#"[{"name":"build","bucket":"pass"},{"name":"lint","bucket":"fail"},{"name":"x","bucket":"fail"}]"#, true), Step::Found("lint".into()));
        assert_eq!(summarise(r#"[{"name":"lint","bucket":"fail"},{"name":"familiar-verify","bucket":"pass","state":"SUCCESS"}]"#, true), Step::Found("lint".into()));
    }

    #[test]
    fn the_lines_are_the_pinned_words() {
        assert_eq!(words(&Step::Push), "familiar-ai: locally green · next: push and open the pull request, and CI proves it");
        assert_eq!(
            facts(&Step::Push),
            "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts."
        );
        assert_eq!(
            facts(&Step::OpenPr),
            "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is pushed and has no pull request. The person has not been asked whether to open the pull request."
        );
        assert_eq!(facts(&Step::Proving), "familiar-ai: CI is proving it");
        assert!(to_ask(&Step::Push) && to_ask(&Step::OpenPr) && !to_ask(&Step::Proving));
        assert!(!facts(&Step::Push).contains("/familiar-ai:prove"), "no shortcut for the person");
        assert_eq!(words(&Step::OpenPr), "familiar-ai: locally green · next: open the pull request");
        assert_eq!(words(&Step::Proving), "familiar-ai: CI is proving it");
        assert_eq!(words(&Step::Proven), "familiar-ai: proven ✓");
        assert_eq!(words(&Step::Found("test (ubuntu)".into())), "familiar-ai: CI found: test (ubuntu) (see the PR)");
        assert_eq!(short(&Step::Push), "next: push and open the PR");
    }

    #[test]
    fn nothing_is_said_until_every_criterion_is_proven_here() {
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        assert!(!proven_here(&log));
        assert!(!proven_here(&Vec::new()), "an empty plan proves nothing");
        assert_eq!(step(&log, &std::env::temp_dir(), Some(GH)), None);
    }
}
