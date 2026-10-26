//! A person sees each criterion as a part of their ask, named by the ask's time
//! and words; ids never reach any surface, and one-line surfaces carry no emoji
//! and no text cut with three dots.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "b3a1f2c4-6d5e-4f70-8a19-2c3d4e5f6a7b";
const ASKED: &str = "fix the greeting so it says hello, and make bye say goodbye";

impl Project {
    fn parts(name: &str) -> Project {
        Project::builder(name)
            .session(SESSION)
            .file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n")
            .env("PATH", "/usr/bin:/bin") // no `gh`: the push question is not read here
            .env("TZ", "UTC")
            .create()
    }

    /// The status line's start: the codebase, Familiar and ON.
    fn bar_start(&self) -> String {
        format!("{} | Familiar | ON | ", self.repo().file_name().and_then(|n| n.to_str()).unwrap_or_default())
    }

    fn statusline(&self) -> String {
        let (code, out, err) = self.run_as(None, &["statusline"], "");
        assert_eq!((code, err.as_str()), (0, ""));
        out.trim_end_matches('\n').to_string()
    }

    /// The person asks `asked`; the agent claims its intent for that prompt.
    fn asks(&self, asked: &str, intent: &str) -> u64 {
        self.hook_with("UserPromptSubmit", &self.fixture("user_prompt_submit", json!({ "prompt": asked })));
        let said = self.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded");
        self.claim(&["intent_submitted", "--for", &said.to_string(), "--text", intent]);
        // definition change (the owner's plan of 2026-10-21, step 2) [draft]: a short ask with no example
        // fires a trigger; the agent says it reads one way before the tests
        self.claim(&["one_reading", "--why", "the ask names what must change and how"]);
        said
    }

    /// The ask split into c1 (src/app.sh 1-3) and c2 (src/bye.sh, strung on c1);
    /// c1's lines are edited before its last line, so its card is open.
    fn moved(name: &str, asked: &str) -> (Project, u64) {
        let p = Project::parts(name);
        let said = p.asks(asked, "greet says hello; bye says goodbye");
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["criterion", "--criterion_id", "c2", "--text", "bye says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/bye.sh","symbol":"bye","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c2"]);
        p.claim(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
        p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        (p, said)
    }
}

fn criterion<'a>(j: &'a Value, c: &str) -> &'a Value {
    j["criteria"].as_array().and_then(|a| a.iter().find(|x| x["id"] == c)).unwrap_or_else(|| panic!("{c} in {j}"))
}

/// `h:mm`: the hour without a leading zero, two digits of minutes.
fn is_clock(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else { return false };
    !h.is_empty() && h.len() <= 2 && !(h.len() == 2 && h.starts_with('0')) && h.chars().all(|c| c.is_ascii_digit()) && m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())
}

/// What a person reads on a one-line surface: no id, no emoji, no cut text.
fn one_line_for_a_person(l: &str) {
    for bad in ["c1", "c2", "c3", "…", "\"", "⛔", "◆", "▸", "✓", "+"] {
        assert!(!l.contains(bad), "{bad:?} in {l:?}");
    }
}

/// (a) One ask: each criterion is `part <n>` in claim order, and the card and
/// status line use the same name.
#[test]
fn one_ask_its_criteria_are_its_parts_by_number() {
    let (p, said) = Project::moved("one", ASKED);
    let j = p.status_json();
    let (c1, c2) = (criterion(&j, "c1"), criterion(&j, "c2"));
    assert_eq!((c1["name"].as_str(), c1["part"].as_u64()), (Some("part 1"), Some(1)), "{j}");
    assert_eq!((c2["name"].as_str(), c2["part"].as_u64()), (Some("part 2"), Some(2)), "{j}");
    let at = c1["ask"]["at"].as_str().unwrap_or_else(|| panic!("the ask's time: {j}")).to_string();
    assert!(is_clock(&at), "{at}");
    assert_eq!(c1["ask"], json!({ "at": at, "text": ASKED, "for": said }), "{j}");
    assert_eq!(c2["ask"], c1["ask"], "{j}");
    let card = &j["cards"][0];
    assert_eq!(card["criterion_id"], "c1", "the id stays the agent's: {j}");
    assert_eq!(card["name"], "part 1", "{j}");
    let runs = j["runs"].as_array().cloned().unwrap_or_default();
    assert!(runs.iter().all(|r| r["name"].is_string()), "each run names its part: {j}");

    let l = p.statusline();
    assert!(l.starts_with(&format!("{}your {at} ask: {ASKED} · 1 needs you · 2 ", p.bar_start())), "{l}");
    one_line_for_a_person(&l);
    assert!(!l.contains("CI") && !l.contains("waiting on you"), "no counts and no CI on the status line: {l}");
}

/// (b) Two asks in a run: a part's name carries its ask's time, and the status
/// line maps each ask.
#[test]
fn two_asks_each_part_names_its_ask() {
    let (p, _) = Project::moved("two", ASKED);
    let second = "and say good night at the end";
    let said2 = p.asks(second, "good night at the end of the day");
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c3", "--text", "good night is said", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c3"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    let j = p.status_json();
    let (c1, c2, c3) = (criterion(&j, "c1"), criterion(&j, "c2"), criterion(&j, "c3"));
    let at1 = c1["ask"]["at"].as_str().unwrap_or_default().to_string();
    let at2 = c3["ask"]["at"].as_str().unwrap_or_default().to_string();
    assert!(is_clock(&at1) && is_clock(&at2), "{j}");
    assert_eq!(c1["name"], format!("part 1 of your {at1} ask"), "{j}");
    assert_eq!(c2["name"], format!("part 2 of your {at1} ask"), "{j}");
    assert_eq!(c3["name"], format!("part 1 of your {at2} ask"), "{j}");
    assert_eq!(c3["part"], 1, "{j}");
    assert_eq!(c3["ask"], json!({ "at": at2, "text": second, "for": said2 }), "{j}");
    assert_eq!(j["cards"][0]["name"], format!("part 1 of your {at1} ask"), "{j}");

    let l = p.statusline();
    assert!(l.starts_with(&format!("{}your {at1} ask: {ASKED} · 1 needs you · 2 ", p.bar_start())), "{l}");
    assert!(l.contains(&format!(" · your {at2} ask: {second} · 1 ")), "{l}");
    one_line_for_a_person(&l);
}

/// (c) An ask too long for the bar shows its first sentence; `status --json` carries it whole.
#[test]
fn a_long_ask_is_its_first_sentence_on_the_status_line_and_whole_in_the_status() {
    let long = "Fix the greeting so it says hello to every person who logs in. Then also make sure that the goodbye message says goodbye, and that nothing else anywhere in the app changes its words at all, not even the help text.";
    let (p, _) = Project::moved("long", long);
    assert_eq!(criterion(&p.status_json(), "c1")["ask"]["text"], long);
    let l = p.statusline();
    assert!(l.contains(" ask: Fix the greeting so it says hello to every person who logs in. · 1 needs you"), "{l}");
    assert!(!l.contains("Then also"), "{l}");
    one_line_for_a_person(&l);
}
