//! Every sentence the person reads on a card, in one table, and the asks and parts
//! that name a criterion to the person. Placeholders in angle brackets are filled
//! from the record; no sentence names a fact kind, a field, an id or a sha.

use familiar_ai_facts::Fact;
use familiar_ai_rules::terminal::Card;
use familiar_ai_rules::{lifecycle, View};
use serde_json::Value;

/// The labels, in the order the person reads them.
pub const YOU_ASKED: &str = "you asked";
pub const SPLIT: &str = "the agent split that into <n> parts:";
pub const SPLIT_ONE: &str = "the agent split that into one part:";
pub const WHAT_HAPPENED: &str = "what happened:";
pub const AGENTS_WORDS: &str = "In the agent's words:";
pub const WHAT_NOW: &str = "what now?";

/// The picker's option past the card's choices, and its line.
pub const SOMETHING_ELSE: &str = "something else";
pub const SOMETHING_ELSE_WORDS: &str = "type what to do in your own words";
/// The one option of `no_test_stack`, whose answer is typed.
pub const TYPE_THE_COMMAND: &str = "type the command";
pub const TYPE_THE_COMMAND_WORDS: &str = "type the command that runs the tests in the box below; it is recorded as the test command";
/// The terminal's last line of a card: the typed answer.
pub const TYPED: &str = "or type your own answer";
pub const TYPED_NO_TEST_STACK: &str = "type the command that runs the tests";

/// What happened, by the card; `anchor_moved` is used when a recorded commit of another
/// part of the same ask touched the lines, `anchor_moved_outside` when none did.
pub const HAPPENED: [(&str, &str); 24] = [
    ("anchor_moved", "doing part <k>, the agent changed the code it had picked for part <j>."),
    ("anchor_moved_outside", "someone changed the code the agent had picked for part <j>, outside this work."),
    ("two_failures", "the agent tried part <j> twice and its tests still fail."),
    ("two_tries_not_red", "the tests the agent wrote for part <j> passed before any fix, twice, so they do not show the problem yet."),
    ("fix_elsewhere", "the fix for part <j> is not in this repository: it lives in <where>."),
    ("fix_elsewhere_unnamed", "the fix for part <j> is not in this repository."),
    ("fix_elsewhere_needs", "It needs <needs>."),
    ("no_test_stack", "the agent found no command that runs the tests for part <j>."),
    ("anchor_untouched", "the tests for part <j> pass, but none of the changes touched the code the agent had picked for it."),
    ("two_test_broken", "the agent tried part <j> twice and both times its test itself broke, before it could check the change."),
    ("two_crashed", "the agent tried part <j> twice and both times its test crashed."),
    ("two_timed_out", "the agent tried part <j> twice and both times its test ran out of time."),
    ("environment_error", "the tests for part <j> could not run here: something they need is missing from this machine."),
    // H-4 by stage (2026-10-20) [draft words]: before the fix the person is asked whether it is
    // the failure, after it whether they passed
    ("run_unclassified", "the tests for part <j> ran before any fix, and Familiar can't confirm they failed because of this problem."),
    ("run_unclassified_after", "the tests for part <j> ran after the fix, and Familiar can't confirm they passed."),
    ("retry_spent", "the test run for part <j> was killed or its test tool crashed, twice in a row."),
    ("crashed", "the test the agent wrote for part <j> crashed before any fix."),
    ("command_broke", "the way the tests for part <j> are run worked before and fails now."),
    ("miscoped", "the tests for part <j> passed once the part it depends on was fixed: part <j> was that part."),
    // the adapter (plan A-4, A-5) [draft words, lane a-surf]
    ("reading_gap", "Familiar ran small probe tests of its own through the test runner here, and some of their outcomes could not be read."),
    ("setup_changed", "the way the tests run changed after the tests for part <j> were shown failing, and run that way they no longer fail for the reason they test."),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
    ("review_disputed", "the tests for part <j> pass, but a stronger model read the fix against your words twice and both times found something you asked for that it does not do yet."),
    // a clarifying question on a signal (the owner's plan of 2026-10-21, step 2) [draft words]
    ("reading_unclear", "your ask can be read more than one way, and the agent will plan for the one you pick."),
    // the one question (the owner's plan of 2026-10-23, change D) [draft words]
    ("repro_unknown", "before the agent writes a test for part <j>, it needs to make the problem happen, and your words don't show how yet."),
];

/// The one question's options, described [draft words].
pub const TYPE_IT_LINE: &str = "pick Other and type what you run and what you see; the agent's test starts from it";
pub const GO_AHEAD_LINE: &str = "the agent writes its test from your words as they are";

/// A reading's line on the ask's card [draft words]: the reading itself is the choice, verbatim.
pub const READING_LINE: &str = "you meant this; the agent plans the work for it";

/// A choice's short name on the picker (2026-10-20) [draft words], where it has one; the line is
/// then its description: when to pick it, and what happens.
pub const LABELS: [(&str, &str, &str); 8] = [
    ("run_unclassified", "its_the_red", "It's the bug"),
    ("run_unclassified", "its_the_pass", "It passed"),
    ("run_unclassified", "try_again", "Run it again"),
    ("run_unclassified", "re_plan", "Fix it elsewhere"),
    ("run_unclassified", "release", "Drop this part"),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
    ("review_disputed", "keep_going", "Keep the fix"),
    ("review_disputed", "try_again", "Fix it again"),
    ("review_disputed", "release", "Drop this part"),
];

/// The short name of `choice` on `hold`'s card, when it has one.
pub fn label_of(hold: &str, choice: &str) -> Option<&'static str> {
    LABELS.iter().find(|(h, c, _)| *h == hold && *c == choice).map(|r| r.2)
}

/// The choices, by card and answer, in reading order; the second column is the `hold_answered.choice` written.
pub const CHOICES: [(&str, &str, &str); 55] = [
    ("anchor_moved", "keep_going", "carry on with part <j> on the code as it is now"),
    ("anchor_moved", "re_plan", "have the agent pick the code for part <j> again"),
    ("anchor_moved", "release", DROP),
    ("two_failures", "try_again", "let the agent try part <j> once more"),
    ("two_failures", "take_over", "you take over part <j> yourself"),
    // Decided B (the owner, 2026-10-15) [draft words]
    ("two_failures", "a_dependency", DEPENDS),
    ("two_failures", "release", DROP),
    ("two_tries_not_red", "try_again", "let the agent write the tests for part <j> once more"),
    ("two_tries_not_red", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("two_tries_not_red", "release", DROP),
    ("fix_elsewhere", "release", DROP),
    ("fix_elsewhere", "re_plan", "have the agent look again at where the fix lives"),
    ("fix_elsewhere", "take_over", "you make the fix where it lives; the agent carries on here"),
    ("anchor_untouched", "keep_going", "keep the change for part <j> where it landed"),
    ("anchor_untouched", "re_plan", "have the agent pick the code for part <j> again"),
    ("anchor_untouched", "release", DROP),
    ("two_test_broken", "try_again", "let the agent try part <j> twice more"),
    ("two_test_broken", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("two_test_broken", "a_dependency", DEPENDS),
    ("two_test_broken", "release", DROP),
    ("two_crashed", "try_again", "let the agent try part <j> twice more"),
    ("two_crashed", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("two_crashed", "a_dependency", DEPENDS),
    ("two_crashed", "release", DROP),
    ("two_timed_out", "try_again", "let the agent try part <j> twice more"),
    ("two_timed_out", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("two_timed_out", "a_dependency", DEPENDS),
    ("two_timed_out", "release", DROP),
    ("environment_error", "try_again", "you have fixed this machine; run the tests for part <j> again"),
    ("environment_error", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("environment_error", "release", DROP),
    ("run_unclassified", "try_again", "it looks like a fluke (a timeout, the network, a flaky test) → the same tests run again"),
    // H-4 (the owner, 2026-10-19) [draft words]: the person confirms what the tools could not read
    ("run_unclassified", "its_the_red", "the failure is the problem part <j> fixes → its test is kept as proof; the agent fixes the code"),
    ("run_unclassified", "its_the_pass", "the tests for part <j> did pass on this run → it counts as their pass"),
    ("run_unclassified", "re_plan", "the failure is real, but the fix for part <j> belongs in other code → the agent picks again where to fix"),
    ("run_unclassified", "release", DROP),
    ("retry_spent", "try_again", "run the tests for part <j> once more as they are"),
    ("retry_spent", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("retry_spent", "release", DROP),
    ("crashed", "its_the_bug", "the crash is the problem part <j> fixes; go on to the fix"),
    ("crashed", "a_dependency", "the crash is in something part <j> depends on; plan that first"),
    ("crashed", "release", DROP),
    ("command_broke", "fix_what_broke_it", "have the agent fix what broke the way the tests run, without changing how they are run"),
    ("command_broke", "a_dependency", "what broke is something part <j> depends on; plan that first"),
    ("command_broke", "release", DROP),
    ("miscoped", "covered_by_dependency", "close part <j>: the part it depends on covers it"),
    ("miscoped", "re_plan", "have the agent plan what part <j> still has to do"),
    ("miscoped", "release", DROP),
    // the adapter (plan A-4, A-5; lane a5) [draft words, lane A7's to settle]
    ("reading_gap", "keep_going", "go on; each run Familiar cannot read will ask you"),
    ("reading_gap", "try_again", "you have changed how the tests run; check how their runs are read again"),
    ("setup_changed", "re_plan", "have the agent plan part <j> again, from the code it picks"),
    ("setup_changed", "release", DROP),
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft words]
    ("review_disputed", "keep_going", "the fix for part <j> does what you asked → it stays as it is, and the agent carries on"),
    ("review_disputed", "try_again", "the model is right → the agent changes the code for part <j> once more, and the model reads it again"),
    ("review_disputed", "release", DROP),
];
/// A release, when other parts go on; `DROP_IT` when it is the only part.
pub const DROP: &str = "drop part <j>; the rest goes on";
/// `a_dependency` on a card of two tries (Decided B) [draft words].
pub const DEPENDS: &str = "what fails is in something part <j> depends on; plan that first";
pub const DROP_IT: &str = "drop it";

/// What the agent's one sentence must say, by card; the agent reads this, the person reads its sentence.
pub const SENTENCE: [(&str, &str); 20] = [
    ("anchor_moved", "what the code it had picked for part <j> now does differently"),
    ("two_failures", "what still fails in part <j>"),
    ("two_tries_not_red", "why the tests for part <j> pass on the code as it is"),
    ("fix_elsewhere", "what has to change where the fix lives"),
    ("no_test_stack", "where you looked for the command that runs the tests"),
    ("anchor_untouched", "where the change for part <j> landed instead"),
    ("two_test_broken", "what breaks in the test of part <j>"),
    ("two_crashed", "where the test of part <j> crashes"),
    ("two_timed_out", "what makes the test of part <j> slow"),
    ("environment_error", "what is missing from this machine"),
    ("run_unclassified", "what the test run printed at its end"),
    ("retry_spent", "what killed the test run, if you can tell"),
    ("crashed", "where the crash happens and whether it is in the code part <j> changes"),
    ("command_broke", "what changed since the command last worked"),
    ("miscoped", "what part <j> asked for that the other part already does"),
    ("reading_gap", "which setting or plugin of the test runner could change what its runs print, if you can tell"),
    ("setup_changed", "what changed in how the tests for part <j> are run"),
    ("review_disputed", "what the fix for part <j> does, and what the model found it leaves out"),
    // a clarifying question on a signal [draft words]
    ("reading_unclear", "what differs between the readings, and which one you would plan for"),
    // the one question [draft words]
    ("repro_unknown", "what you would run to make the problem happen, if you can tell from the code"),
];

/// What a stronger model found, on a card whose finding was read again [draft words] (S7b).
pub const FOUND: &str = "a stronger model read the work and found:";
/// The proof after it, each quote in double quotes.
pub const PROOF: &str = "its proof:";
/// The finding in the person's words, by category.
pub const FOUND_ROWS: [(&str, &str); 3] = [
    ("wrong_test", "the test for part <j> checks something other than what you asked."),
    ("behavior_exists", "the code already does what you asked for part <j>, so no test of it can fail first."),
    ("requirement_missed", "part <j> left out something you asked for."),
];
/// What the reviewer found, on its card [draft words] (the owner's plan of 2026-10-20, step 3).
pub const REVIEWED: &str = "the fix for part <j> does not yet do this: <gap>.";
/// The choice a finding lists first.
pub const FINDING_CHOICE: [(&str, &str); 3] = [("wrong_test", "try_again"), ("behavior_exists", "release"), ("requirement_missed", "re_plan")];
/// Each quote of the proof is cut at this many characters on a card.
const PROOF_CUT: usize = 160;

/// A card's finding: the line and each quote of its proof, on one line each, cut at `PROOF_CUT`.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub line: String,
    pub proof: Vec<String>,
}

/// A criterion's name, as a person reads it.
pub const PART: &str = "part <n>";
/// The reserved part `setup` (plan A-4), as a person reads it [draft words].
pub const THE_TEST_SETUP: &str = "the test setup";
pub const PART_OF: &str = "part <n> of your <h:mm> ask";
/// An ask, as a person reads it; `YOUR_ASK_UNTIMED` when the record has no time for it.
pub const YOUR_ASK: &str = "your <h:mm> ask";
pub const YOUR_ASK_UNTIMED: &str = "your ask";

/// The person's words are cut at this many characters on a card.
const SAID_CUT: usize = 300;

/// A card's pieces, read from the record.
#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    /// The person's words, on one line, cut at `SAID_CUT`.
    pub said: String,
    /// When they were said; `None` when the words are the intent's own.
    pub at: Option<f64>,
    /// Each part's text, in order.
    pub parts: Vec<String>,
    /// The card's part, 1-based.
    pub part: usize,
    pub happened: String,
    /// The can't-tell card's run in short sentences (H-4), each on its own line after what
    /// happened; empty on other cards.
    pub run: Vec<String>,
    /// What the agent's sentence says.
    pub sentence: String,
    /// `(hold_answered.choice, the line)`, in the table's order, the finding's choice first.
    pub choices: Vec<(String, String)>,
    /// The finding read again, on `two_tries_not_red` (S7b); the question quotes it.
    pub found: Option<Found>,
    /// The cases card as the person reads it [draft]: its lines take the place of the pieces above.
    pub cases: Option<crate::teach::cases_card::Shown>,
}

fn text<'a>(f: &'a Fact, k: &str) -> Option<&'a str> {
    f.fields.get(k).and_then(Value::as_str)
}

/// `s` on one line, with each whitespace run as one space.
pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn row<'a>(table: &[(&str, &'a str)], key: &str) -> &'a str {
    table.iter().find(|(k, _)| *k == key).map_or("", |(_, w)| w)
}

fn fill(line: &str, j: usize) -> String {
    line.replace("<j>", &j.to_string())
}

/// Each criterion once, with the `t` of its first claim, in that order.
fn claimed<'a>(v: &View<'a>) -> Vec<(u64, &'a str)> {
    let mut out: Vec<(u64, &str)> = Vec::new();
    for f in v.kind("criterion") {
        if let Some(c) = f.criterion_id() {
            if !out.iter().any(|(_, x)| *x == c) {
                out.push((f.t, c));
            }
        }
    }
    out
}

/// The intent `c` was first claimed under, which defines its ask.
fn intent_of<'a>(v: &View<'a>, claims: &[(u64, &str)], c: &str) -> Option<&'a Fact> {
    let t = claims.iter().find(|(_, x)| *x == c)?.0;
    v.kind("intent_submitted").filter(|i| i.t < t).last()
}

/// The prompt `intent` names with `for`, else the person's last prompt before it.
fn prompt<'a>(v: &View<'a>, intent: Option<&'a Fact>) -> Option<&'a Fact> {
    let prompts = || v.kind("person_said");
    let named = intent.and_then(|i| i.fields.get("for")).and_then(Value::as_u64).and_then(|t| prompts().find(|p| p.t == t));
    named.or_else(|| intent.and_then(|i| prompts().filter(|p| p.t < i.t).last()))
}

/// The person's words for `intent`, whole, and when they were said; an older record gives the intent's own words and no time.
fn whole<'a>(v: &View<'a>, intent: Option<&'a Fact>) -> (&'a str, Option<f64>) {
    match prompt(v, intent) {
        Some(p) => (text(p, "text").unwrap_or(""), Some(p.at)),
        None => (intent.and_then(|i| text(i, "text")).unwrap_or(""), None),
    }
}

/// The person's words for `intent` on one line, cut at `SAID_CUT`, and when they were said.
fn said<'a>(v: &View<'a>, intent: Option<&'a Fact>) -> (String, Option<f64>) {
    let (words, at) = whole(v, intent);
    let words = one_line(words);
    if words.chars().count() <= SAID_CUT {
        return (words, at);
    }
    (words.chars().take(SAID_CUT).collect(), at)
}

/// The criteria first claimed under the intent at `t`, in claim order; part numbers are 1-based in this order.
fn parts_under<'a>(v: &View<'a>, claims: &[(u64, &'a str)], t: Option<u64>) -> Vec<&'a str> {
    claims.iter().map(|(_, x)| *x).filter(|x| intent_of(v, claims, x).map(|i| i.t) == t).collect()
}

/// An ask: the person's prompt an intent was claimed for, and its parts.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// The prompt's turn; `None` when the words are the intent's own.
    pub turn: Option<u64>,
    /// When the person said it.
    pub at: Option<f64>,
    /// Their words, whole.
    pub text: String,
    /// The criteria, part 1 first.
    pub parts: Vec<String>,
}

impl Ask {
    /// The ask's heading, such as `your 12:36 ask`.
    pub fn heading(&self) -> String {
        self.at.map_or_else(|| YOUR_ASK_UNTIMED.to_string(), |at| YOUR_ASK.replace("<h:mm>", &crate::door::person::clock(at)))
    }
}

fn ask_of<'a>(v: &View<'a>, intent: Option<&'a Fact>, parts: Vec<&str>) -> Ask {
    let (words, at) = whole(v, intent);
    Ask { turn: prompt(v, intent).map(|p| p.t), at, text: words.trim().to_string(), parts: parts.into_iter().map(str::to_string).collect() }
}

/// The asks that have parts, in the order their first part was claimed.
pub fn asks(v: &View) -> Vec<Ask> {
    let claims = claimed(v);
    let mut seen: Vec<Option<u64>> = Vec::new();
    let mut out = Vec::new();
    for (_, c) in &claims {
        let intent = intent_of(v, &claims, c);
        let t = intent.map(|i| i.t);
        if !seen.contains(&t) {
            seen.push(t);
            out.push(ask_of(v, intent, parts_under(v, &claims, t)));
        }
    }
    out
}

/// The ask `intent` was claimed for, before any part is claimed under it.
pub fn ask_for(v: &View, intent: &Fact) -> Ask {
    ask_of(v, Some(intent), Vec::new())
}

/// Part `n` of `ask` by name: `PART` when the run has one ask, else `PART_OF`.
/// Every surface a person reads names a criterion this way; the ids stay the agent's.
pub fn name(ask: &Ask, n: usize, many: bool) -> String {
    if !many {
        return PART.replace("<n>", &n.to_string());
    }
    let of = PART_OF.replace("<n>", &n.to_string());
    match ask.at {
        Some(at) => of.replace("<h:mm>", &crate::door::person::clock(at)),
        None => of.replace(" <h:mm>", ""),
    }
}

/// `c`'s ask and its part number in it, among `asks`.
pub fn part_of<'a>(asks: &'a [Ask], c: &str) -> Option<(&'a Ask, usize)> {
    asks.iter().find_map(|a| a.parts.iter().position(|x| x == c).map(|i| (a, i + 1)))
}

/// `c` by name, among `asks`; `None` for a criterion never claimed. The reserved part `setup`
/// is [`THE_TEST_SETUP`].
pub fn name_of(asks: &[Ask], c: &str) -> Option<String> {
    if c == familiar_ai_rules::reading::SETUP_PART {
        return Some(THE_TEST_SETUP.to_string());
    }
    part_of(asks, c).map(|(a, n)| name(a, n, asks.len() > 1))
}

/// The pull request body `status --pr` prints, in reading order.
pub const PR_LOGO: &str = r#"<p align="center"><img src="https://raw.githubusercontent.com/familiar-ai-oss/familiar-ci/main/docs/logo-on-dark.png" width="180" alt="Familiar"></p>"#;
pub const PR_PARTS: &str = "<n> parts, each tested first and fixed on its anchor.";
pub const PR_ONE_PART: &str = "1 part, each tested first and fixed on its anchor.";
pub const PR_ASKED_FOR: &str = "asked for:";
pub const PR_TESTED_BY: &str = "tested by:";
pub const PR_JUDGE: &str = "judge:";
/// The judge line when the judge said nothing of the part's tests.
pub const PR_NOT_ASKED: &str = "not asked";
pub const PR_CI: &str = "CI (familiar-verify) proves each part: red at its tests-first commit, green at the head.";
pub const PR_CLOSING: &str = "Proven by [Familiar](https://github.com/familiar-ai-oss/familiar-ci#the-proof), tests that failed first, then passed, verified by CI in a fresh clone.";

/// What a prompt of the person's became, as `status --json` names it.
pub const BECAME_ASK: &str = "ask";
pub const BECAME_ANSWER: &str = "answer";
pub const BECAME_NOTE: &str = "note";

/// One prompt the person typed, and what it became.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    /// Its turn: the `t` of its `person_said`.
    pub turn: u64,
    pub at: f64,
    /// Their words, whole.
    pub text: String,
    /// `BECAME_ASK`, `BECAME_ANSWER` or `BECAME_NOTE`.
    pub became: &'static str,
    /// The ask's 1-based number among `asks`, when it has one.
    pub ask: Option<usize>,
}

impl Prompt {
    /// The label a person reads: `ask 1`, `answer`, `note`.
    pub fn label(&self) -> String {
        self.ask.map_or_else(|| self.became.to_string(), |n| format!("{} {n}", self.became))
    }
}

/// Every prompt the person typed, oldest first, each marked by what it became.
/// A prompt is an ask when it is an ask's prompt or an intent or criterion names it, an answer when a `person_said_answered` names it, else a note.
pub fn said_and_became(v: &View) -> Vec<Prompt> {
    let asks = asks(v);
    let names = |kind: &str, t: u64| v.kind(kind).any(|f| f.fields.get("for").and_then(Value::as_u64) == Some(t));
    v.kind("person_said")
        .map(|p| {
            let by_criterion = || {
                v.kind("criterion")
                    .filter(|f| f.fields.get("for").and_then(Value::as_u64) == Some(p.t))
                    .find_map(|f| f.criterion_id().and_then(|c| part_of(&asks, c)).and_then(|(a, _)| asks.iter().position(|x| x == a)))
            };
            let ask = asks.iter().position(|a| a.turn == Some(p.t)).or_else(by_criterion).map(|i| i + 1);
            let became = if ask.is_some() || names("intent_submitted", p.t) || names("criterion", p.t) {
                BECAME_ASK
            } else if names("person_said_answered", p.t) {
                BECAME_ANSWER
            } else {
                BECAME_NOTE
            };
            Prompt { turn: p.t, at: p.at, text: text(p, "text").unwrap_or("").to_string(), became, ask }
        })
        .collect()
}

/// For each recorded commit since `c`'s planning check that touched its anchor's lines,
/// the criteria the record's stamps at that commit say it was for, in order.
pub fn commits_on_anchor(v: &View, c: &str) -> Vec<Vec<String>> {
    let since = lifecycle::planning_check(v, c).map_or(0, |f| f.t);
    let touched = v.kind("commit_seen").filter(|f| {
        f.t > since && f.fields.get("anchor_overlap").and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x.as_str() == Some(c)))
    });
    touched
        .filter_map(|f| text(f, "commit"))
        .map(|sha| {
            let stamps = v.facts.iter().filter(|s| ["tests_observed", "pass_please_pass", "fingerprint_stamped"].contains(&s.kind) && text(s, "commit") == Some(sha));
            let mut for_: Vec<String> = stamps.filter_map(|s| text(s, "criterion_id")).map(str::to_string).collect();
            for_.dedup();
            for_
        })
        .collect()
}

/// What happened, for `card` on part `j` of `ids`.
fn happened(v: &View, card: &Card, ids: &[&str], j: usize) -> String {
    let c = card.criterion_id.as_str();
    match card.hold.as_str() {
        "anchor_moved" => {
            let by = commits_on_anchor(v, c).into_iter().flatten().find_map(|o| ids.iter().position(|x| *x == o && *x != c));
            match by {
                Some(k) => fill(row(&HAPPENED, "anchor_moved"), j).replace("<k>", &(k + 1).to_string()),
                None => fill(row(&HAPPENED, "anchor_moved_outside"), j),
            }
        }
        "fix_elsewhere" => {
            let fl = lifecycle::fix_location(v, c);
            let field = |k: &str| fl.and_then(|f| text(f, k)).map(str::trim).filter(|s| !s.is_empty());
            let mut out = match field("where") {
                Some(w) => fill(row(&HAPPENED, "fix_elsewhere"), j).replace("<where>", w),
                None => fill(row(&HAPPENED, "fix_elsewhere_unnamed"), j),
            };
            if let Some(n) = field("needs") {
                out = format!("{out} {}", row(&HAPPENED, "fix_elsewhere_needs").replace("<needs>", n));
            }
            out
        }
        "run_unclassified" if familiar_ai_rules::criterion::seal_raw(v, c, familiar_ai_rules::criterion::Place::Laptop).is_some() => fill(row(&HAPPENED, "run_unclassified_after"), j),
        hold => fill(row(&HAPPENED, hold), j),
    }
}

/// Reads the card's pieces from the record. The setup part's card (plan A-4) quotes no ask and
/// no part: it is the project's own.
pub fn asked(v: &View, card: &Card) -> Asked {
    let claims = claimed(v);
    let c = card.criterion_id.as_str();
    if c == familiar_ai_rules::reading::SETUP_PART {
        let offered = |ch: &str| card.choices.iter().any(|(x, _)| x == ch);
        return Asked {
            said: String::new(),
            at: None,
            parts: Vec::new(),
            part: 0,
            happened: row(&HAPPENED, &card.hold).to_string(),
            run: card.summary.clone(),
            sentence: row(&SENTENCE, &card.hold).to_string(),
            choices: CHOICES.iter().filter(|(h, ch, _)| *h == card.hold && offered(ch)).map(|(_, ch, line)| ((*ch).to_string(), (*line).to_string())).collect(),
            found: None,
            cases: None,
        };
    }
    let intent = intent_of(v, &claims, c);
    let ids = parts_under(v, &claims, intent.map(|i| i.t));
    let part = ids.iter().position(|x| *x == c).map_or(1, |i| i + 1);
    let parts: Vec<String> = ids.iter().map(|x| one_line(crate::teach::words::text_of(v, x).unwrap_or(x))).collect();
    let (said, at) = said(v, intent);
    // the cases card [draft]: its own lines, its options' labels as its choices' lines
    if card.hold == familiar_ai_rules::cases::CARD {
        let shown = crate::teach::cases_card::shown(v, card);
        let choices = shown.options.iter().map(|(ch, label, _)| (ch.clone(), label.clone())).collect();
        return Asked { said, at, parts, part, happened: String::new(), run: Vec::new(), sentence: String::new(), choices, found: None, cases: Some(shown) };
    }
    let offered = |ch: &str| card.choices.iter().any(|(x, _)| x == ch);
    let mut choices: Vec<(String, String)> = CHOICES
        .iter()
        .filter(|(h, ch, _)| *h == card.hold && offered(ch))
        .map(|(_, ch, line)| {
            let line = if *ch == "release" && parts.len() == 1 { DROP_IT } else { line };
            ((*ch).to_string(), fill(line, part))
        })
        .collect();
    // the ask's card: the agent's readings are its choices, verbatim [draft]; so the one question's
    if card.hold == familiar_ai_rules::ask::HOLD {
        choices = card.choices.iter().map(|(ch, _)| (ch.clone(), ch.clone())).collect();
    }
    if card.hold == familiar_ai_rules::trace::REPRO {
        choices = card.choices.clone();
    }
    let finding = (card.hold == familiar_ai_rules::finding::HOLD).then(|| familiar_ai_rules::finding::finding(v, c)).flatten();
    let category = finding.and_then(familiar_ai_rules::finding::category).unwrap_or("");
    // the finding's choice first; the choices themselves stay the rules'
    let first = FINDING_CHOICE.iter().find(|(k, _)| *k == category).map(|(_, ch)| *ch);
    if let Some(i) = choices.iter().position(|(ch, _)| Some(ch.as_str()) == first) {
        let it = choices.remove(i);
        choices.insert(0, it);
    }
    let reviewed = (card.hold == familiar_ai_rules::review::HOLD).then(|| reviewed(v, c, part)).flatten();
    let found = reviewed.or_else(|| finding.map(|f| Found {
        line: fill(row(&FOUND_ROWS, category), part),
        proof: f
            .fields
            .get("quotes")
            .and_then(Value::as_array)
            .map(|qs| {
                qs.iter()
                    .filter_map(|q| q.get("text").and_then(Value::as_str))
                    .map(|t| one_line(t).chars().take(PROOF_CUT).collect::<String>())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    }));
    Asked {
        found,
        happened: happened(v, card, &ids, part),
        run: card.summary.clone(),
        sentence: fill(row(&SENTENCE, &card.hold), part),
        said,
        at,
        parts,
        part,
        choices,
        cases: None,
    }
}

/// What the reviewer found on part `j`: the gap of `c`'s latest verified "no" and its quotes.
fn reviewed(v: &View, c: &str, j: usize) -> Option<Found> {
    let no = familiar_ai_rules::review::verdicts(v, c).into_iter().rev().find(|f| text(f, "covers") == Some("no"))?;
    let gap = text(no, "gap").unwrap_or("").trim().trim_end_matches('.');
    let proof = no.fields.get("quotes").and_then(Value::as_array).map(|qs| qs.iter().map(|q| crate::teach::review::quote_words(q).chars().take(PROOF_CUT).collect()).collect()).unwrap_or_default();
    Some(Found { line: fill(REVIEWED, j).replace("<gap>", gap), proof })
}

/// The card's lines up to what happened, with each part's line indented two spaces; a card with
/// no ask and no part (the setup part's) starts at what happened.
pub fn lines(a: &Asked) -> Vec<String> {
    if let Some(s) = &a.cases {
        return s.lines.clone();
    }
    let mut out = Vec::new();
    if !(a.said.is_empty() && a.parts.is_empty()) {
        let when = a.at.map(|at| format!(" ({})", crate::door::person::clock(at))).unwrap_or_default();
        out.push(format!("{YOU_ASKED}{when}: \"{}\"", a.said));
        out.push(if a.parts.len() == 1 { SPLIT_ONE.to_string() } else { SPLIT.replace("<n>", &number(a.parts.len())) });
        out.extend(a.parts.iter().enumerate().map(|(i, t)| format!("  {}  {t}", i + 1)));
    }
    out.push(format!("{WHAT_HAPPENED} {}", a.happened));
    out.extend(a.run.iter().map(|l| format!("  {l}")));
    if let Some(f) = &a.found {
        out.push(found_line(f));
    }
    out
}

/// The finding and its proof, on one line.
pub fn found_line(f: &Found) -> String {
    let proof: Vec<String> = f.proof.iter().map(|p| format!("\"{p}\"")).collect();
    format!("{FOUND} {} {PROOF} {}", f.line, proof.join(" · "))
}

/// A count in words, to ten.
fn number(n: usize) -> String {
    const WORDS: [&str; 11] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    WORDS.get(n).map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// The pieces `question` lacks, among the person's words, each part's text and a sentence
/// after `AGENTS_WORDS`, named as the refusal names them; whitespace runs compare as one space.
pub fn missing(a: &Asked, question: &str) -> Vec<String> {
    if let Some(s) = &a.cases {
        return crate::teach::cases_card::missing(s, question);
    }
    let q = one_line(question);
    let mut out = Vec::new();
    if !a.said.is_empty() && !q.contains(&a.said) {
        out.push(format!("the person's words \"{}\"", a.said));
    }
    for (i, t) in a.parts.iter().enumerate() {
        if !q.contains(t.as_str()) {
            out.push(format!("part {} \"{t}\"", i + 1));
        }
    }
    if let Some(f) = &a.found {
        // the question is refused until it quotes the proof (S7b)
        if !q.contains(&one_line(&f.line)) {
            out.push(format!("what a stronger model found \"{}\"", f.line));
        }
        for p in &f.proof {
            if !q.contains(p.as_str()) {
                out.push(format!("its proof \"{p}\""));
            }
        }
    }
    // what happened and the run's lines are the binary's, word for word: the agent may not reword
    // or leave out a fact of the run (2026-10-20)
    if !a.happened.is_empty() && !q.contains(&one_line(&format!("{WHAT_HAPPENED} {}", a.happened))) {
        out.push(format!("what happened \"{} {}\"", WHAT_HAPPENED, a.happened));
    }
    for l in &a.run {
        if !q.contains(&one_line(l)) {
            out.push(format!("the run's line \"{}\"", one_line(l)));
        }
    }
    let sentence = q.find(AGENTS_WORDS).map(|i| &q[i + AGENTS_WORDS.len()..]).map(|rest| rest.split(WHAT_NOW).next().unwrap_or(""));
    if !sentence.is_some_and(|s| s.chars().any(char::is_alphanumeric)) {
        out.push(format!("one sentence of yours after \"{AGENTS_WORDS}\""));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_of_the_rules_has_a_line_and_the_lines_of_a_card_differ() {
        // the cases card's lines are its finding's (`terminal::cases_options`), not this table's
        for (hold, choices) in familiar_ai_rules::criterion::ANSWERS.iter().filter(|(h, _)| *h != familiar_ai_rules::cases::CARD) {
            let lines: Vec<&str> = choices.iter().map(|ch| CHOICES.iter().find(|(h, c, _)| h == hold && c == ch).map_or("", |r| r.2)).collect();
            assert!(lines.iter().all(|l| !l.is_empty()), "{hold}: {lines:?}");
            let mut distinct = lines.clone();
            distinct.dedup();
            assert_eq!(distinct.len(), lines.len(), "{hold}");
        }
    }

    #[test]
    fn the_person_reads_no_word_of_the_binarys() {
        let mut all: Vec<&str> = HAPPENED.iter().map(|r| r.1).chain(CHOICES.iter().map(|r| r.2)).chain(FOUND_ROWS.iter().map(|r| r.1)).collect();
        // the cases card [draft names]: its lines and options, as the owner approved them (2026-10-25)
        use familiar_ai_rules::terminal as t;
        all.extend([t::CASES_ALSO, t::CASES_NONE, t::CASES_FAILED, t::CASES_PLANNED, crate::teach::cases_card::OTHER_FOUND, crate::teach::cases_card::OTHER_NONE]);
        all.extend(["Before I write the test for", "Add all three (Recommended)", "the test checks all five", "Just the first two", "the test checks the two above", "Looks right", "I'll write the test", "Look again", "one more search", "Test these", "Add my own", "pick Other and describe a case to add"]);
        all.extend([READING_LINE, FOUND, PROOF, REVIEWED, YOU_ASKED, SPLIT, SPLIT_ONE, WHAT_HAPPENED, AGENTS_WORDS, WHAT_NOW, SOMETHING_ELSE, SOMETHING_ELSE_WORDS, TYPE_THE_COMMAND, TYPED, TYPED_NO_TEST_STACK, DROP, DROP_IT]);
        for line in all {
            for word in ["criterion", "anchor", "hold", "planning", "string", "sha", "commit", "_"] {
                assert!(!line.contains(word), "{word:?} in {line:?}");
            }
        }
    }

    #[test]
    fn a_missing_piece_is_named() {
        let a = Asked {
            said: "fix the greeting".into(),
            at: None,
            parts: vec!["greet says hello".into(), "bye says goodbye".into()],
            part: 1,
            happened: String::new(),
            run: Vec::new(),
            sentence: String::new(),
            choices: Vec::new(),
            found: None,
            cases: None,
        };
        let all = "you asked: \"fix the   greeting\"\n 1 greet says hello 2 bye says goodbye\nIn the agent's words: \"it prints hey\"\nwhat now?";
        assert_eq!(missing(&a, all), Vec::<String>::new());
        assert_eq!(missing(&a, &all.replace("fix the", "mend the")), ["the person's words \"fix the greeting\""]);
        assert_eq!(missing(&a, &all.replace("\"it prints hey\"", "")), ["one sentence of yours after \"In the agent's words:\""]);
        assert_eq!(missing(&a, ""), ["the person's words \"fix the greeting\"", "part 1 \"greet says hello\"", "part 2 \"bye says goodbye\"", "one sentence of yours after \"In the agent's words:\""]);
    }
}
