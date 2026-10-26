//! Precedents for the seal's judge (ports 2026-10-24, piece 4) [draft names]. At a pytest part's
//! seal the judge's prompt prints, under PRECEDENTS and its own cap, the lines outside the tests
//! that write a compound name the sealed test passes, or writes in its own f-string, inside the
//! `{...}` of a format string: click-3328's `show_default`, pydantic-13418's `version_short` past
//! the anchor's cut. Nothing found, nothing printed: the prompt is as it was.

use std::path::{Path, PathBuf};

use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

/// The section's cap, the probe's (MOST_PRECEDENTS [draft name]).
const CAP: usize = 1_000;
const HEAD: &str = "PRECEDENTS (lines outside the tests, at the seal commit, that format a name the sealed test passes or formats itself: how the code already writes that value out)";
const CHECK: &str = "For each line under PRECEDENTS: does the sealed test expect that name's value written the way that line writes it? If the test writes it differently and the person's words do not ask for the difference, answers_intent is false and gap names the line as file:line.";

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repository with `code` committed, then `tests` committed alone: its folder, the first
/// commit (the intent's) and the second (the seal).
fn repo(name: &str, code: &[(&str, String)], tests: &[(&str, String)]) -> (PathBuf, String, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-precedents-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let put = |(p, t): &(&str, String)| {
        let f = dir.join(p);
        std::fs::create_dir_all(f.parent().expect("parent")).expect("dir");
        std::fs::write(f, t).expect("write");
    };
    std::fs::create_dir_all(&dir).expect("dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    code.iter().for_each(put);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "the code"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    tests.iter().for_each(put);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "c1: tests"]);
    let seal = git(&dir, &["rev-parse", "HEAD"]);
    (dir, base, seal)
}

/// Part c1, its tests run under `stack`, its place `places` chosen at `base`, its test `test`
/// planned and stamped at `seal`: the log and the seal's place in it.
fn sealed(stack: &str, base: &str, seal: &str, places: Value, test: &str) -> (Log, usize) {
    let add = |log: &Log, kind: &str, f: Value| append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
    let mut log = add(&Vec::new(), "intent_submitted", json!({ "text": "the prompt shows the default the option names", "sha": base }));
    log = add(&log, "behavior", json!({ "behavior_id": "b1", "text": "the prompt's default" }));
    log = add(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "an option", "gives_back": "a prompt", "refuses": "nothing" }));
    log = add(&log, "criterion", json!({ "criterion_id": "c1", "text": "the prompt shows the default", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = add(&log, "place_chosen", json!({ "criterion_id": "c1", "places": places }));
    log = add(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": test, "will": "create" }));
    log = add(&log, "test_stack_detected", json!({ "codebase_id": "repo", "project_root": "", "test_stack": stack, "test_command": format!("python -m pytest -q {test}") }));
    log = add(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": seal, "proof_not_tampered": "h", "ran_in": "laptop" }));
    let at = log.len() - 1;
    (log, at)
}

/// The judge's prompt for c1's seal, as `judge-prompt` prints it.
fn prompt(dir: &Path, log: &Log, seal: usize) -> String {
    let i = crate::door::judge::inputs(log, dir, "c1", seal).expect("the judge's inputs");
    crate::teach::judge::prompt(&i)
}

/// The section, from its header to the blank line after it.
fn section(p: &str) -> Option<&str> {
    let at = p.find(&format!("\n\n{HEAD}\n"))? + 2;
    let rest = &p[at..];
    Some(&rest[..rest.find("\n\n").map_or(rest.len(), |e| e + 1)])
}

/// The 1-based line of `text` that holds `marker`.
fn line_of(text: &str, marker: &str) -> usize {
    text.lines().position(|l| l.contains(marker)).map(|i| i + 1).unwrap_or_else(|| panic!("{marker} in {text}"))
}

const TERMUI: &str = "def _build_prompt(text, suffix, show_default=False, default=None):\n    prompt = text\n    if default is not None and show_default:\n        prompt = f\"{prompt} [{_format_default(default)}]\"\n    return f\"{prompt}{suffix}\"\n\n\ndef _format_default(default):\n    return default\n";

const CORE: &str = "class Option:\n    def __init__(self, show_default=None, default=None):\n        self.show_default = show_default\n        self.default = default\n\n    def get_help_extra(self, ctx):\n        extra = {}\n        show_default_is_str = isinstance(self.show_default, str)\n\n        if show_default_is_str:\n            default_string = f\"({self.show_default})\"\n            extra[\"default\"] = default_string\n        elif self.default is not None:\n            extra[\"default\"] = f\"{self.default}\"\n        return extra\n";

/// click-3328's sealed test: it passes `show_default`, and expects it in brackets.
const CLICK_TEST: &str = "import click\n\n\ndef test_string_show_default_is_displayed_in_prompt(runner):\n    @click.command()\n    @click.option(\"--arg1\", show_default=\"custom\", prompt=True, default=\"my-default-value\")\n    def cmd(arg1):\n        pass\n\n    result = runner.invoke(cmd, input=\"\\n\", standalone_mode=False)\n    assert \"[custom]\" in result.output\n";

fn click(name: &str, stack: &str, test: &str) -> String {
    let code = [("src/click/termui.py", TERMUI.to_string()), ("src/click/core.py", CORE.to_string())];
    let (dir, base, seal) = repo(name, &code, &[("tests/test_termui.py", test.to_string())]);
    let places = json!([{ "file_path": "src/click/termui.py", "symbol": "_build_prompt", "line_start": 1, "line_end": 5 }]);
    let (log, at) = sealed(stack, &base, &seal, places, "tests/test_termui.py");
    let p = prompt(&dir, &log, at);
    let _ = std::fs::remove_dir_all(&dir);
    p
}

#[test]
fn a_format_string_that_reads_the_keyword_the_sealed_test_passes_is_shown_to_the_judge() {
    let p = click("click-3328", "pytest", CLICK_TEST);
    let (n, before) = (line_of(CORE, "default_string = f"), line_of(CORE, "if show_default_is_str:"));
    let want = format!("{HEAD}\nsrc/click/core.py:{n}, in Option.get_help_extra (names show_default)\n  {before}: if show_default_is_str:\n  {n}: default_string = f\"({{self.show_default}})\"\n");
    let shown = section(&p).unwrap_or_else(|| panic!("no PRECEDENTS section: {p}"));
    assert_eq!(shown, want, "{p}");
    assert!(shown.chars().count() <= CAP, "{} characters", shown.chars().count());
    // after the sealed test, before the question, which asks of each line
    let (test, head, question) = (p.find("THE SEALED TEST tests/test_termui.py"), p.find(HEAD), p.find("\n\nCriterion c1 is the part"));
    assert!(test < head && head < question && test.is_some(), "{p}");
    assert!(p.contains("Three checks more:\n") && p.contains(&format!("\n3. {CHECK}\n")), "{p}");
    assert!(p.find(CHECK) > question, "{p}");
}

/// The anchor's function, long enough that the judge's cut ends inside it: an early line names
/// `version_short` inside the cut, a late one past it.
fn migration() -> String {
    let mut lines = vec![
        "from typing import Any, Callable".to_string(),
        String::new(),
        "from .version import version_short".to_string(),
        String::new(),
        String::new(),
        "def getattr_migration(module: str) -> Callable[[str], Any]:".to_string(),
        "    \"\"\"Implement PEP 562 for objects that were either moved or removed on the migration to V2.\"\"\"".to_string(),
        String::new(),
        "    def wrapper(name: str) -> object:".to_string(),
        "        if name == '__path__':".to_string(),
        "            raise AttributeError(f'module {module!r} has no attribute {name!r}, see https://pydantic.dev/docs/validation/{version_short()}/errors/')".to_string(),
    ];
    lines.extend((1..=30).map(|k| format!("        moved_{k:02} = 'pydantic.v1.moved_{k:02}: a name that moved in V2 and is looked up here'")));
    lines.extend(
        [
            "        if name == 'BaseSettings':",
            "            raise PydanticImportError(",
            "                '`BaseSettings` has been moved to the `pydantic-settings` package. '",
            "                f'See https://pydantic.dev/docs/validation/{version_short()}/migration/#basesettings-has-moved-to-pydantic-settings '",
            "            )",
            "        return getattr(module, name)",
            "",
            "    return wrapper",
            "",
            "",
            "def removed_link() -> str:",
            "    return f'https://pydantic.dev/docs/validation/{version_short()}/migration/#removed-objects-and-their-replacements-in-the-library-and-what-to-use-instead-of-each-one'",
        ]
        .map(str::to_string),
    );
    lines.join("\n") + "\n"
}

const GENERATE: &str = "import warnings\n\nfrom ..version import version_short\n\n\n\
def _add_custom_serialization_from_json_encoders(json_encoders, tp, schema):\n    \
warnings.warn(  # the encoders are looked up by type, the first that matches wins, and a subclass of a type with an encoder gets the same one unless it has its own\n        \
f'`json_encoders` is deprecated. See https://pydantic.dev/docs/validation/{version_short()}/concepts/serialization/#custom-serializers for alternatives',\n        stacklevel=2,\n    )\n";

const WARNINGS: &str = "from .version import version_short\n\n\nclass PydanticDeprecationWarning(DeprecationWarning):\n    def __str__(self) -> str:\n        message = self.message\n        return f'{message}. Deprecated in Pydantic V{self.since[0]}.{self.since[1]}. See https://pydantic.dev/docs/validation/{version_short()}/migration/'\n";

/// pydantic-13418's sealed test: its own f-string writes `version_short` another way.
const PYDANTIC_TEST: &str = "import pytest\n\nfrom pydantic.version import version_short\n\n\ndef test_redirect_to_v1_warning_link_is_versioned():\n    with pytest.raises(Exception) as caught:\n        from pydantic import BaseSettings  # noqa: F401\n    assert f'/{version_short()}/get-started/migration/' in str(caught.value)\n";

#[test]
fn a_name_in_the_sealed_tests_own_f_string_finds_the_anchors_line_past_the_cut_and_the_section_keeps_its_cap() {
    let m = migration();
    let code = [
        ("pydantic/_migration.py", m.clone()),
        ("pydantic/_internal/_generate_schema.py", GENERATE.to_string()),
        ("pydantic/warnings.py", WARNINGS.to_string()),
        ("pydantic/version.py", "def version_short() -> str:\n    return '2.12'\n".to_string()),
    ];
    let (dir, base, seal) = repo("pydantic-13418", &code, &[("tests/test_versioned_docs_links.py", PYDANTIC_TEST.to_string())]);
    let (first, last) = (line_of(&m, "def getattr_migration"), line_of(&m, "    return wrapper"));
    let places = json!([{ "file_path": "pydantic/_migration.py", "symbol": "getattr_migration", "line_start": first, "line_end": last }]);
    let (log, at) = sealed("pytest", &base, &seal, places, "tests/test_versioned_docs_links.py");
    let p = prompt(&dir, &log, at);
    let _ = std::fs::remove_dir_all(&dir);
    let shown = section(&p).unwrap_or_else(|| panic!("no PRECEDENTS section: {p}"));
    let (late, early, link) = (line_of(&m, "migration/#basesettings"), line_of(&m, "/errors/"), line_of(&m, "#removed-objects"));
    let cut = |l: &str| l.trim().chars().take(160).collect::<String>();
    let lines: Vec<&str> = m.lines().collect();
    let want = format!(
        "{HEAD}\n\
pydantic/_migration.py:{late}, in getattr_migration.wrapper (names version_short)\n  {}: {}\n  {late}: {}\n\
pydantic/_migration.py:{link}, in removed_link (names version_short)\n  {}: def removed_link() -> str:\n  {link}: {}\n",
        late - 1,
        cut(lines[late - 2]),
        cut(lines[late - 1]),
        link - 1,
        cut(lines[link - 1]),
    );
    // first the anchor's line past the cut, then a line in the anchor's file; a third entry would
    // pass the cap, so it is left out whole
    assert_eq!(shown, want, "{p}");
    assert!(shown.chars().count() <= CAP, "{} characters", shown.chars().count());
    assert!(!shown.contains("_generate_schema.py") && !shown.contains("warnings.py"), "{shown}");
    // the early line is inside the cut: the judge reads it already, in the anchor
    assert!(p.contains("/errors/") && !shown.contains(&format!("_migration.py:{early}")), "{shown}");
    assert!(p.contains(&format!("\n3. {CHECK}\n")), "{p}");
}

#[test]
fn nothing_found_prints_no_section_and_no_check() {
    // single words only: `default` and `prompt` are written in braces, `default` outside the anchor
    // too (core.py), but are not compound names
    let single = CLICK_TEST.replace("show_default=\"custom\", ", "").replace(", standalone_mode=False", "");
    let p = click("nothing", "pytest", &single);
    assert!(!p.contains("PRECEDENTS") && !p.contains(CHECK), "{p}");
    assert!(p.ends_with(&crate::teach::judge::question("c1")), "the question as it was: {p}");
    // a part whose tests do not run under pytest: the scan reads Python text, for pytest parts only
    let p = click("not-pytest", "sh", CLICK_TEST);
    assert!(!p.contains("PRECEDENTS") && p.ends_with(&crate::teach::judge::question("c1")), "{p}");
}

/// The lister replayed on kept bench runs, no model asked [draft names]:
/// `FAMILIAR_AI_PRECEDENTS_REPLAY` names a file of `<case>\t<git dir>\t<record>` lines. Each record
/// is read from a copy in the temporary folder (opening a record writes to it); at each seal of
/// each part, on the record as it stood when that seal first stood, the judge's inputs are made as
/// `judge-prompt` makes them, and the precedents found and the section printed are written to
/// `FAMILIAR_AI_PRECEDENTS_OUT`, one JSON line per seal (a run with none gets one line, seal null).
#[test]
#[ignore = "reads kept bench runs: FAMILIAR_AI_PRECEDENTS_REPLAY=<list> FAMILIAR_AI_PRECEDENTS_OUT=<file> cargo test -p familiar-ai --lib door::precedents_tests -- --ignored"]
fn the_kept_runs_replayed_through_the_lister() {
    use familiar_ai_rules::criterion::{sealed_at, Place};
    use familiar_ai_rules::{plan, View};
    let list = std::env::var("FAMILIAR_AI_PRECEDENTS_REPLAY").expect("FAMILIAR_AI_PRECEDENTS_REPLAY names the list of runs");
    let out = std::env::var("FAMILIAR_AI_PRECEDENTS_OUT").expect("FAMILIAR_AI_PRECEDENTS_OUT names the file written");
    let scratch = std::env::temp_dir().join(format!("familiar-ai-precedents-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("the scratch folder");
    let mut lines = String::new();
    for row in std::fs::read_to_string(&list).expect("the list").lines().filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = row.split('\t').collect();
        let [case, git, record] = cols[..] else { panic!("not <case>\\t<git dir>\\t<record>: {row}") };
        let copy = scratch.join(case);
        std::fs::copy(record, &copy).unwrap_or_else(|e| panic!("{record}: {e}"));
        let log = familiar_ai_facts::Record::open(&copy).and_then(|r| r.read_all()).unwrap_or_else(|e| panic!("{case}: {e:?}"));
        std::fs::remove_file(&copy).expect("the copy removed");
        let mut standing: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        let mut seals = 0;
        for j in 0..log.len() {
            let v = View::new(&log[..=j]);
            for c in plan::criteria(&v) {
                let Some(s) = sealed_at(&v, &c, Place::Laptop) else { continue };
                if standing.insert(c.clone(), s.t) == Some(s.t) {
                    continue;
                }
                seals += 1;
                let commit = s.fields.get("commit").and_then(Value::as_str).unwrap_or("").to_string();
                let Some(i) = crate::door::judge::inputs(&log[..=j].to_vec(), Path::new(git), &c, s.t as usize) else { continue };
                let section = crate::teach::precedents::section(&i.precedents);
                let found: Vec<String> = i.precedents.iter().map(|p| format!("{}:{}", p.file, p.line)).collect();
                let line = json!({ "case": case, "criterion_id": c, "seal": s.t, "at": j, "commit": commit, "found": found, "chars": section.chars().count(), "section": section });
                lines.push_str(&format!("{line}\n"));
            }
        }
        if seals == 0 {
            lines.push_str(&format!("{}\n", json!({ "case": case, "seal": null })));
        }
        println!("{case}: {seals} seals, {} facts", log.len());
    }
    std::fs::write(&out, lines).expect("the replay written");
    let _ = std::fs::remove_dir(&scratch);
}
