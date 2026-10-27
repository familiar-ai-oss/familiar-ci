//! End to end: the real binary, fed payloads captured from Claude Code, walks
//! one criterion from plan to green against a real repository and record, and
//! the plan is built line by line through `familiar-ai claim`.

mod common;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, PLAN_FILE, TENANT};
use familiar_ai_facts::{
    AnchorShaMatched, CheckedAt, Criterion, CriterionAssigned, CriterionHead, FixLocation, FixLocationCategory, IntentSubmitted,
    Path as CriterionPath, Place, PlaceChosen, Record, TestPlanned, TestStackDetected, Will,
};
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";
const ANCHOR_FILE: &str = "src/app.sh";
const TEST_FILE: &str = "tests/test_c1.sh";
const TEST_COMMAND: &str = "sh tests/test_c1.sh";

/// An empty repository, no setup, no commit yet.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().no_setup().no_base().create()
}

impl Project {
    fn plan_text(&self) -> String {
        std::fs::read_to_string(self.path(FAMILIAR_FOLDER).join(PLAN_FILE)).expect("the plan file")
    }

    /// `familiar-ai claim <kind> <args..>` as the agent runs it, with the session id set.
    fn claim_raw(&self, kind: &str, args: &[&str]) -> (i32, String, String) {
        let mut argv = vec!["claim", kind];
        argv.extend_from_slice(args);
        let (code, stdout, stderr) = self.run(&argv, "");
        (code, stdout, stderr)
    }

    /// The captured Edit payload, on `rel`.
    fn edit_of(&self, rel: &str) -> Value {
        let mut v = self.fixture("pre_tool_use_edit", json!({}));
        v["tool_input"]["file_path"] = Value::from(self.path(rel).to_string_lossy().into_owned());
        v
    }

    /// `familiar-ai hook <event>` with the payload on stdin, which must exit 0
    /// with nothing on stderr; the JSON it printed.
    fn told(&self, label: &str, event: &str, payload: &Value) -> Value {
        let (code, stdout, stderr) = self.hook_raw(event, payload);
        assert_eq!(code, 0, "{label}: stderr: {stderr}");
        assert!(stderr.is_empty(), "{label}: stderr: {stderr}");
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{label}: not JSON ({e}): {stdout}"))
    }
}

/// The plan as facts: one light-path criterion anchored on `src/app.sh` lines
/// 1–3 at the base commit, with one planned test and its command, assigned.
fn write_plan(project: &Project, base: &str) {
    std::fs::create_dir_all(project.path(FAMILIAR_FOLDER)).expect("folder");
    let record = Record::open_waiting(&project.record_path(), std::time::Duration::from_secs(5)).expect("record");
    let lines_hash = familiar_ai_git::lines_hash(&project.repo(), base, ANCHOR_FILE, 1, 3).expect("lines hash");
    let at = 1.0;
    record
        .append(&IntentSubmitted { text: "greet with hello".into(), sha: base.into(), for_t: None }, at, TENANT)
        .expect("intent");
    // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    record.append(&familiar_ai_facts::Behavior { behavior_id: "b1".into(), text: "the greeting".into() }, at, TENANT).expect("behavior");
    record.append(&familiar_ai_facts::Promise { promise_id: "p1".into(), behavior_id: "b1".into(), accepts: "a name".into(), gives_back: "a greeting".into(), refuses: "nothing".into() }, at, TENANT).expect("promise");
    record
        .append(
            &Criterion {
                criterion_id: "c1".into(),
                text: "greet says hello".into(),
                path: CriterionPath::Full,
                behavior_id: Some("b1".into()),
                promise_id: Some("p1".into()),
                for_t: None,
            },
            at,
            TENANT,
        )
        .expect("criterion");
    record
        .append(
            &PlaceChosen {
                criterion_id: "c1".into(),
                places: vec![Place { file_path: ANCHOR_FILE.into(), symbol: "greet".into(), line_start: 1, line_end: 3 }],
                hold: None,
                choice: None,
            },
            at,
            TENANT,
        )
        .expect("place");
    record
        .append(
            &AnchorShaMatched {
                criterion_id: "c1".into(),
                file_path: ANCHOR_FILE.into(),
                symbol: "greet".into(),
                line_start: 1,
                line_end: 3,
                sha: base.into(),
                lines_hash,
                matched: true,
                checked_at: CheckedAt::Planning,
                tip_matched: None,
            },
            at,
            TENANT,
        )
        .expect("check");
    record
        .append(&TestPlanned { criterion_id: "c1".into(), file_path: TEST_FILE.into(), will: Will::Create, test_command: None }, at, TENANT)
        .expect("test planned");
    record
        .append(
            &TestStackDetected {
                codebase_id: "cb1".into(),
                project_root: "src/".into(),
                test_stack: "sh".into(),
                test_command: TEST_COMMAND.into(),
            },
            at,
            TENANT,
        )
        .expect("stack");
    record.append(&CriterionHead { criterion_id: "c1".into() }, at, TENANT).expect("head");
    record
        .append(&CriterionAssigned { criterion_id: "c1".into(), agent_conversation_id: SESSION.into() }, at, TENANT)
        .expect("assigned");
    record
        .append(
            &FixLocationCategory { criterion_id: "c1".into(), category: FixLocation::ThisRepository, where_: String::new(), needs: None },
            at,
            TENANT,
        )
        .expect("claim");
}

fn kinds(p: &Project) -> Vec<(String, String)> {
    p.facts()
        .iter()
        .map(|f| {
            let id = f.fields.get("commit").and_then(Value::as_str).unwrap_or("").to_string();
            (f.kind.to_string(), id)
        })
        .collect()
}

/// The anchor checks of c1 as `(checked_at, matched, lines_hash)`.
fn checks(p: &Project) -> Vec<(String, bool, String)> {
    p.facts()
        .iter()
        .filter(|f| f.kind == "anchor_sha_matched")
        .map(|f| {
            let text = |k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            (text("checked_at"), f.fields.get("matched") == Some(&Value::Bool(true)), text("lines_hash"))
        })
        .collect()
}

#[test]
fn the_hook_enforces_end_to_end() {
    let project = project("enforces");
    project.write(ANCHOR_FILE, "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
    // the setup skill ignores the record, which would otherwise be tracked
    project.write(".gitignore", ".familiar/record\n");
    let base = project.land("base");

    // (a) no .familiar folder: nothing created, allow
    let out = project.told("(a) no .familiar", "PreToolUse", &project.edit_of("src/app.sh"));
    assert_eq!(out, json!({}));
    assert!(!project.path(FAMILIAR_FOLDER).exists(), "(a): nothing is created");

    write_plan(&project, &base);

    // (d) the agent stops while c1 is `testing`: the not-yet words, and the start check is recorded
    let out = project.told("(d) Stop while testing", "Stop", &project.fixture("stop", json!({})));
    assert_eq!(out["decision"], "block");
    let reason = out["reason"].as_str().expect("reason");
    assert!(reason.starts_with("familiar-ai: not done yet · \"greet says hello\" · the test comes first\n  next: "), "{reason}");
    let cs = checks(&project);
    assert_eq!(cs.len(), 2, "{cs:?}");
    assert_eq!(cs[0].0, "planning");
    assert_eq!((cs[1].0.as_str(), cs[1].1), ("start", true));
    assert_eq!(cs[1].2, cs[0].2, "the lines did not move: the same hash");
    // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path: behavior and promise come first (9 became 11, 10 became 12)
    assert_eq!(kinds(&project)[11].0, "anchor_sha_matched");
    // the plan file mirrors the record's claims, the checks included
    assert_eq!(project.plan_text().lines().count(), 12);
    assert!(project.plan_text().lines().all(|l| l.starts_with("{\"kind\":\"")), "{}", project.plan_text());

    // (c) an edit of an unrelated file: allowed, told it is outside the anchor
    let out = project.told("(c) Edit of an unrelated file", "PreToolUse", &project.edit_of("src/other.sh"));
    assert!(!out.to_string().contains("permissionDecision"), "(c): no decision at all: {out}");
    assert!(out.get("decision").is_none());
    // the agent reads it before the edit: the place the plan named, and the way out
    let told = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(told.starts_with("familiar-ai: this edit is outside the place the plan named for \"greet says hello\", src/app.sh greet lines 1–3; it goes ahead\n  if the fix belongs elsewhere: ask the person with AskUserQuestion"), "{out}");

    // the tests-first commit lands: the test fails against today's code
    project.write(TEST_FILE, "grep -q hello src/app.sh\n");
    let t1 = project.land("c1: tests first");
    let mut post = project.fixture("post_tool_use_bash", json!({}));
    post["tool_input"]["command"] = Value::from("git add -A && git commit -m 'c1: tests first'");
    let out = project.told("tests-first commit landed", "PostToolUse", &post);
    assert!(out.get("decision").is_none(), "{out}");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("state line");
    assert_eq!(context, "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
    assert_eq!(
        kinds(&project)[12..], // definition change (the owner, 2026-10-11): 10 became 12 (behavior, promise)
        [
            ("commit_seen".to_string(), t1.clone()),
            // c1's fingerprint observed beside the commit
            ("tests_observed".to_string(), t1.clone()),
            ("test_written".to_string(), String::new()),
            // the run is reserved, started and ended by the detached child
            ("run_reserved".to_string(), t1.clone()),
            ("run_started".to_string(), String::new()),
            ("run_ended".to_string(), String::new()),
            // every run is translated beside its run_ended (plan §1.1; the owner, 2026-10-15: c4's sealed walk updated with the definition, announced)
            ("test_log_translation".to_string(), t1.clone()),
            ("pass_please_pass".to_string(), t1.clone()),
            ("fingerprint_stamped".to_string(), t1.clone()),
            // the inline teller tells the run once (plan §1.2, D11; the owner, 2026-10-16: c4's sealed walk updated with the definition, announced)
            ("told".to_string(), String::new()),
        ]
    );

    // (b) an edit of the sealed test: denied with the proof_tampered words
    let out = project.told("(b) Edit of the sealed test", "PreToolUse", &project.edit_of(TEST_FILE));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(
        reason.starts_with("familiar-ai: \"greet says hello\" · the sealed test was changed; fix the code without editing the test, or commit the changed test alone and see it fail · next: announce the test change first\n"),
        "{reason}"
    );
    // the claim-it line is the form's example with the criterion filled in: no placeholder
    assert!(reason.ends_with("\n  claim it: familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion named the wrong file'"), "{reason}");
    assert!(!reason.contains('<'), "{reason}");

    // a commit attempt here is decided as stop 1 only
    let mut pre = project.fixture("pre_tool_use_bash", json!({}));
    pre["tool_input"]["command"] = Value::from("git commit -am wip");
    let out = project.told("Bash git commit (courtesy)", "PreToolUse", &pre);
    assert_eq!(out, json!({}));

    // the code commit lands on the anchor: the test passes and the stamp matches
    project.write(ANCHOR_FILE, "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    let k1 = project.land("c1: code");
    let out = project.told("code commit landed", "PostToolUse", &post);
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("state line");
    assert_eq!(context, "familiar-ai: ran the tests of \"greet says hello\": they pass · stamped");

    // (e) both commits recorded as commit_seen, oldest first
    let all = kinds(&project);
    let commits: Vec<&str> = all.iter().filter(|(k, _)| k == "commit_seen").map(|(_, id)| id.as_str()).collect();
    assert_eq!(commits, [t1.as_str(), k1.as_str()]);
    assert_eq!(
        all[22..], // definition changes: 18 became 20 (behavior, promise; the owner, 2026-10-11); 20 became 21 (the tests-first run's translation; the owner, 2026-10-15); 21 became 22 (the tests-first run's told; the owner, 2026-10-16)
        [
            ("commit_seen".to_string(), k1.clone()),
            ("tests_observed".to_string(), k1.clone()),
            ("run_reserved".to_string(), k1.clone()),
            ("run_started".to_string(), String::new()),
            ("run_ended".to_string(), String::new()),
            ("test_log_translation".to_string(), k1.clone()),
            ("pass_please_pass".to_string(), k1.clone()),
            ("fingerprint_stamped".to_string(), k1.clone()),
            ("told".to_string(), String::new()),
        ]
    );

    // every criterion green: the agent may stop and is told to ask the person
    let out = project.told("Stop when green", "Stop", &project.fixture("stop", json!({})));
    assert!(out.get("decision").is_none(), "{out}");
    assert_eq!(
        out["hookSpecificOutput"]["additionalContext"],
        "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts."
    );

    // (f) the record held by another step for the whole wait: the busy answer
    let holder = Record::open_waiting(&project.record_path(), std::time::Duration::from_secs(5)).expect("hold the record");
    let out = project.told("(f) busy record", "PreToolUse", &project.edit_of("src/app.sh"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap_or("")
        .contains("busy with another step"));
    assert!(!out.to_string().contains("hold"), "busy names no hold");
    drop(holder);

    // (g) the plan file is the binary's to write: an edit is refused, naming the door
    let out = project.told("(g) Edit of .familiar/plan", "PreToolUse", &project.edit_of(".familiar/plan"));
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason");
    assert!(reason.contains("familiar-ai claim"), "{reason}");
}

/// The plan built line by line through `familiar-ai claim`, each answer naming
/// the next missing line; the binary's own kinds are refused at the door.
#[test]
fn the_plan_door_builds_the_plan_line_by_line() {
    let project = project("door");
    project.write(ANCHOR_FILE, "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
    project.write(".gitignore", ".familiar/record\n");
    let base = project.land("base");
    std::fs::create_dir_all(project.path(FAMILIAR_FOLDER)).expect("folder");

    // the binary's kinds are refused whoever asks
    for kind in ["commit_seen", "pass_please_pass", "fingerprint_stamped", "anchor_sha_matched", "test_written", "pipeline_run_seen"] {
        let (code, _, stderr) = project.claim_raw(kind, &["--criterion_id", "c1"]);
        assert_eq!(code, 1, "{kind}");
        assert!(stderr.contains("written by familiar-ai itself"), "{kind}: {stderr}");
    }
    // an unknown kind and an unregistered key are the facts door's refusals
    let (code, _, stderr) = project.claim_raw("wish", &["--text", "x"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("unknown kind 'wish'"), "{stderr}");
    let (code, _, stderr) = project.claim_raw("criterion", &["--criterion_id", "c1", "--text", "t", "--path", "light", "--state", "x"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("does not register the key 'state'"), "{stderr}");
    assert_eq!(project.facts().len(), 0, "nothing refused was written");

    // line 1: the intent; the sha is read from the head when left out
    let (code, stdout, _) = project.claim_raw("intent_submitted", &["--for", "0", "--text", "greet with hello"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("familiar-ai: claimed {\"t\":0,"), "{stdout}");
    assert!(stdout.contains(&format!("\"sha\":\"{base}\"")), "{stdout}");
    assert!(stdout.contains("claim it: familiar-ai claim criterion --criterion_id"), "line 2 is next: {stdout}");

    // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path, line 3 first
    assert_eq!(project.claim_raw("behavior", &["--behavior_id", "b1", "--text", "the greeting"]).0, 0);
    assert_eq!(project.claim_raw("promise", &["--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]).0, 0);
    // the case-finder's none and the person's keep_planned stand already, read from the intent on
    // (plan cases-card, 2026-10-25) [draft names]
    project.cases_kept("c1");
    let (code, stdout, _) = project.claim_raw("criterion", &["--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("claim it: familiar-ai claim place_chosen"), "line 4 is next (line 3 claimed): {stdout}");
    assert!(stdout.contains("or:       familiar-ai claim new_work_spot_chosen"), "{stdout}");

    // line 4: the anchor; the binary checks the lines at the sha at once (line 5)
    let places = format!("[{{\"file_path\":\"{ANCHOR_FILE}\",\"symbol\":\"greet\",\"line_start\":1,\"line_end\":3}}]");
    let (code, stdout, _) = project.claim_raw("place_chosen", &["--criterion_id", "c1", "--places", &places]);
    assert_eq!(code, 0);
    let cs = checks(&project);
    assert_eq!(cs.len(), 1);
    assert_eq!((cs[0].0.as_str(), cs[0].1), ("planning", true));
    assert_eq!(cs[0].2, familiar_ai_git::lines_hash(&project.repo(), &base, ANCHOR_FILE, 1, 3).expect("hash"));
    assert!(stdout.contains("claim it: familiar-ai claim test_planned"), "line 6 is next: {stdout}");

    let (code, stdout, _) = project.claim_raw("test_planned", &["--criterion_id", "c1", "--file_path", TEST_FILE, "--will", "create"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("claim it: familiar-ai claim test_stack_detected"), "line 7 is next: {stdout}");

    let (code, stdout, _) = project.claim_raw(
        "test_stack_detected",
        &["--codebase_id", "e2e", "--project_root", "src/", "--test_stack", "sh", "--test_command", TEST_COMMAND],
    );
    assert_eq!(code, 0);
    assert!(stdout.contains("claim it: familiar-ai claim criterion_head"), "line 9 is next (line 8 allows none): {stdout}");

    let (code, stdout, _) = project.claim_raw("criterion_head", &["--criterion_id", "c1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("claim it: familiar-ai claim criterion_assigned"), "line 10 is next: {stdout}");

    // line 10: the conversation comes from the environment
    let (code, stdout, _) = project.claim_raw("criterion_assigned", &["--criterion_id", "c1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains(&format!("\"agent_conversation_id\":\"{SESSION}\"")), "{stdout}");
    assert!(stdout.contains("claim it: familiar-ai claim fix_location_category --criterion_id c1"), "line 11 is next: {stdout}");

    // line 11: the holder's claim of where the fix lives
    let (code, stdout, _) = project.claim_raw("fix_location_category", &["--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert_eq!(code, 0);
    assert!(stdout.contains("familiar-ai: the plan is confirmed\nfamiliar-ai: \"greet says hello\" · the test comes first · unscoped · scope the command to prove in parallel\n"), "{stdout}");
    assert!(!stdout.contains("claim it:"), "nothing is missing: {stdout}");
    // the start check was recorded as c1 entered testing
    let cs = checks(&project);
    assert_eq!(cs.len(), 2);
    assert_eq!((cs[1].0.as_str(), cs[1].1), ("start", true));

    // the plan file carries every claim, kind first, in order
    let plan = project.plan_text();
    let plan_kinds: Vec<String> = plan
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("a JSON line")["kind"].as_str().expect("kind").to_string())
        .collect();
    assert_eq!(
        plan_kinds,
        [
            "intent_submitted",
            // definition change (the owner, 2026-10-11): the full path's line 3
            "behavior",
            "promise",
            // definition change (the owner's ruling of 2026-10-25, "Mirror them") [draft names]: the
            // case-finder's finding and the person's cases card answer are plan lines; (the owner's
            // plan of 2026-10-26, the lens panel) each lens's answer before the synthesis's too
            "lens_found",
            "lens_found",
            "lens_found",
            "lens_found",
            "lens_found",
            "cases_found",
            "hold_answered",
            "criterion",
            "place_chosen",
            "anchor_sha_matched",
            "test_planned",
            "test_stack_detected",
            "criterion_head",
            "criterion_assigned",
            "fix_location_category",
            "anchor_sha_matched",
        ]
    );
    assert!(!plan.contains("\"t\":"), "a claim carries no t, at or tenant");

    // the first edit is now allowed; status shows the picture
    let out = project.told("first edit after the plan", "PreToolUse", &project.edit_of(ANCHOR_FILE));
    assert_eq!(out, json!({}));
    let (code, text, _) = project.run_in(&project.path("src"), &["status"], "");
    assert_eq!(code, 0);
    assert!(text.starts_with("familiar-ai: the plan is confirmed\nfamiliar-ai: \"greet says hello\" · the test comes first"), "{text}");
}

/// The cases card passed in a test [draft names] (the owner's plan cases-card, 2026-10-25, lane C):
/// a full-path part planned through the claim door waits on the case-finder (`cases_owed`), so
/// its plan is not confirmed; `Project::cases_kept` records the finder's "none found" and the
/// person's `keep_planned` on the card, after which the plan is confirmed and the test comes first.
#[test]
fn cases_kept_records_the_finders_none_and_the_persons_keep_planned() {
    let p = Project::new("cases-kept");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST_FILE, "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", TEST_COMMAND]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    let before = p.status();
    assert!(!before.starts_with("familiar-ai: the plan is confirmed"), "the lens panel is owed first: {before}");
    let holds = p.criterion_json("c1")["holds"].to_string();
    assert!(holds.contains("cases_owed"), "{holds}");

    p.cases_kept("c1");
    let found = p.facts_of("cases_found");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0]["criterion_id"].as_str(), found[0]["outcome"].as_str(), found[0]["cases"].as_array().map(Vec::len)), (Some("c1"), Some("none"), Some(0)));
    assert_eq!(p.facts_of("hold_answered"), [json!({ "criterion_id": "c1", "hold": "cases_unconfirmed", "choice": "keep_planned", "typed": "" })]);
    let holds = p.criterion_json("c1")["holds"].to_string();
    assert!(!holds.contains("cases_"), "{holds}");
    let after = p.status();
    assert!(after.starts_with("familiar-ai: the plan is confirmed\nfamiliar-ai: \"greet says hello\" · the test comes first"), "{after}");
}
