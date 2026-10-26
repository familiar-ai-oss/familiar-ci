//! An existing test's lines change only when announced (the owner, 2026-10-20; the bench's
//! astroid-3152) [draft names]. At every commit the binary reads, against the plan's base (the
//! intent's sha), each test file that stood there and is not one of the plan's own tests: the
//! base's lines the commit's tree no longer holds as they were (a `-` line of the diff; a line
//! only added is no change), counted only inside a test definition. For Python that is a
//! `def test*` function or a test class, from its decorators to its last line, as the judge's
//! reader cuts them; module-level lines (imports, helpers) never count. For any other language a
//! line counts unless it is blank or an import (`import`, `from`, `use`, `require`, `include`,
//! `using`, `package`). The standing change goes on `commit_seen.existing_tests_changed`.

use std::path::Path;

use familiar_ai_facts::ExistingTestChange;

/// Whether `path` is a test file by the runners' naming: a `test`/`tests`/`__tests__`/`spec`
/// folder holding source, or a file named as a test (`test_*.py`, `*_test.py`, `*_test.go`,
/// `*.test.*`, `*.spec.*`, `*Test.java`, `*Tests.cs`, `*_spec.rb`, `*Test.php`, ...).
pub fn is_test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    const CODE: [&str; 20] = ["py", "rs", "go", "js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx", "java", "kt", "cs", "rb", "php", "sh", "c", "cc", "cpp"];
    if !CODE.contains(&ext) {
        return false;
    }
    let named = stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with(".test")
        || stem.ends_with(".spec")
        || stem.ends_with("_spec")
        || (stem.len() > 4 && (stem.ends_with("Test") || stem.ends_with("Tests")))
        || stem == "test"
        || stem == "tests";
    let folder = path.split('/').rev().skip(1).any(|d| matches!(d, "test" | "tests" | "__tests__" | "spec" | "testing"));
    named || folder
}

/// Whether a line of a non-Python test file is an import, which never counts.
fn import_like(line: &str) -> bool {
    let t = line.trim_start();
    ["import ", "from ", "use ", "require ", "require(", "include ", "#include", "using ", "package ", "extern crate "].iter().any(|p| t.starts_with(p))
        || (t.contains("require(") && (t.starts_with("const ") || t.starts_with("let ") || t.starts_with("var ")))
}

/// The base's lines (1-based) that `hunks` changed or removed.
fn removed(hunks: &[familiar_ai_git::Hunk]) -> Vec<u64> {
    hunks.iter().flat_map(|h| h.old_start..h.old_start + h.old_count).collect()
}

/// One file's change: the base's removed lines that count, the tests they are in.
fn change_of(path: &str, before: &[u8], lines: &[u64]) -> Option<ExistingTestChange> {
    let text: Vec<String> = familiar_ai_git::split_lines(before).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    let at = |n: u64| text.get(usize::try_from(n).ok()?.checked_sub(1)?).map(String::as_str);
    let mut tests: Vec<String> = Vec::new();
    let mut kept: Vec<u64> = Vec::new();
    if path.ends_with(".py") {
        let blocks = crate::teach::sealed_test::test_blocks(before);
        for &n in lines {
            let i = usize::try_from(n).unwrap_or(usize::MAX).saturating_sub(1);
            let Some((name, _, _)) = blocks.iter().find(|(_, a, z)| *a <= i && i <= *z) else { continue };
            if at(n).is_some_and(|l| l.trim().is_empty()) {
                continue;
            }
            kept.push(n);
            if !tests.contains(name) {
                tests.push(name.clone());
            }
        }
    } else {
        for &n in lines {
            let Some(l) = at(n) else { continue };
            if l.trim().is_empty() || import_like(l) {
                continue;
            }
            kept.push(n);
            if let Some((_, f)) = familiar_ai_git::funcname_above(before, n) {
                let f = f.trim().to_string();
                if !tests.contains(&f) {
                    tests.push(f);
                }
            }
        }
    }
    if kept.is_empty() {
        return None;
    }
    let mut ranges: Vec<[u64; 2]> = Vec::new();
    for n in kept {
        match ranges.last_mut() {
            Some(r) if r[1] + 1 == n => r[1] = n,
            _ => ranges.push([n, n]),
        }
    }
    Some(ExistingTestChange { file_path: path.to_string(), tests, lines: ranges })
}

/// The standing change at `commit` against `base` to the test files that stood at the base,
/// the plan's own tests aside. A base git cannot read is no change.
pub fn changes(root: &Path, base: &str, commit: &str, planned: &[String]) -> Vec<ExistingTestChange> {
    let Ok(files) = familiar_ai_git::changed_between(root, base, commit) else {
        return Vec::new();
    };
    files
        .into_iter()
        .filter(|f| is_test_file(&f.path) && !planned.contains(&f.path) && !crate::door::commits::is_familiars_own(&f.path))
        .filter_map(|f| {
            let before = familiar_ai_git::file_at(root, base, &f.path).ok()?;
            change_of(&f.path, &before, &removed(&f.hunks))
        })
        .collect()
}

/// The changed tests of `e` in `bytes`, by name (Python), else the file's lines around `ranges`
/// numbered (`whole` shows the whole file instead).
fn shown(path: &str, bytes: &[u8], tests: &[String], ranges: &[[u64; 2]], whole: bool) -> String {
    let lines: Vec<String> = familiar_ai_git::split_lines(bytes).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    if path.ends_with(".py") {
        let blocks = crate::teach::sealed_test::test_blocks(bytes);
        let parts: Vec<String> = tests
            .iter()
            .map(|t| match blocks.iter().find(|(n, _, _)| n == t) {
                Some((_, a, z)) => lines.get(*a..=*z).unwrap_or(&[]).join("\n"),
                None => format!("({t} is not in the file now)"),
            })
            .collect();
        return parts.join("\n\n");
    }
    if whole {
        return lines.join("\n");
    }
    let mut out: Vec<String> = Vec::new();
    for r in ranges {
        let from = r[0].saturating_sub(3).max(1);
        for n in from..=r[1] + 3 {
            if let Some(l) = usize::try_from(n).ok().and_then(|n| lines.get(n.checked_sub(1)?)) {
                out.push(format!("{n:>5}| {l}"));
            }
        }
        out.push("  ...".to_string());
    }
    out.join("\n")
}

/// What the judge is shown for the standing change `e` announced with `a`.
pub fn inputs(log: &familiar_ai_facts::Log, root: &Path, e: &serde_json::Value, a: &familiar_ai_facts::Fact) -> Option<crate::teach::existing::Inputs> {
    let change: ExistingTestChange = serde_json::from_value(e.clone()).ok()?;
    let v = familiar_ai_rules::View::new(log);
    let base = v.last("intent_submitted").and_then(|f| familiar_ai_rules::reader::text(f, "sha"))?;
    let head = v.last("commit_seen").and_then(|f| familiar_ai_rules::reader::text(f, "commit"))?;
    let before = familiar_ai_git::file_at(root, base, &change.file_path).ok()?;
    let after = familiar_ai_git::file_at(root, head, &change.file_path).unwrap_or_default();
    let criteria = familiar_ai_rules::plan::criteria(&v)
        .into_iter()
        .map(|c| format!("criterion {c}: {}", v.of("criterion", &c).last().and_then(|f| familiar_ai_rules::reader::text(f, "text")).unwrap_or("")))
        .collect();
    Some(crate::teach::existing::Inputs {
        intent: crate::teach::person::words_and_picks(log),
        reading: crate::teach::person::agents_reading(log).map(str::to_string),
        criteria,
        file_path: change.file_path.clone(),
        changed: familiar_ai_rules::existing::words(e).trim_start_matches(change.file_path.as_str()).trim().trim_start_matches('(').trim_end_matches(')').to_string(),
        before: shown(&change.file_path, &before, &change.tests, &change.lines, false),
        after: shown(&change.file_path, &after, &change.tests, &change.lines, true),
        reason: familiar_ai_rules::reader::text(a, "reason").unwrap_or("").to_string(),
    })
}

/// The judge's model for the change: one rung above the holder of the first part it holds, never
/// below opus.
fn alias(log: &familiar_ai_facts::Log) -> &'static str {
    let v = familiar_ai_rules::View::new(log);
    familiar_ai_rules::plan::criteria(&v).first().map(|c| crate::door::judge::rung(log, c)).unwrap_or(crate::door::judge::FLOOR)
}

/// On a hook call with no seal to judge, ask the judge about the oldest announced change to an
/// existing test that it owes a word, and write `existing_test_verdict`. `Ok(None)` when none is owed.
pub fn backstop(
    record: &familiar_ai_facts::Record,
    root: &Path,
    log: &mut familiar_ai_facts::Log,
    around: &crate::input::Surroundings,
    dir: &Path,
    program: &str,
) -> Result<Option<crate::door::judge::Told>, String> {
    use familiar_ai_facts::{AnswersIntent, ExistingTestVerdict, FixCovers, FixNotJudgedWhy, JudgeUnansweredWhy};
    let owed = {
        let v = familiar_ai_rules::View::new(log);
        familiar_ai_rules::existing::owed(&v, crate::teach::judge::TWICE).first().map(|(e, a)| ((*e).clone(), (*a).clone()))
    };
    let Some((e, a)) = owed else { return Ok(None) };
    let Some(inputs) = inputs(log, root, &e, &a) else { return Ok(None) };
    let file = inputs.file_path.clone();
    let head = familiar_ai_rules::View::new(log).last("commit_seen").and_then(|f| familiar_ai_rules::reader::text(f, "commit")).unwrap_or("").to_string();
    let alias = alias(log);
    let ran = crate::door::judge::ask(program, alias, &crate::teach::existing::prompt(&inputs), dir, around.judge_wait);
    crate::door::judge::log_call(root, around.now, &format!("existing test {file}"), alias, &ran);
    let base = |model: String, covers: FixCovers, reason: String| ExistingTestVerdict { file_path: file.clone(), commit: head.clone(), model, covers, reason, gap: None, why: None };
    let (verdict, told) = match crate::door::judge_answer::read(&ran, alias) {
        Err((why, how)) => {
            let why = match why {
                JudgeUnansweredWhy::Error => FixNotJudgedWhy::Error,
                JudgeUnansweredWhy::Timeout => FixNotJudgedWhy::Timeout,
                _ => FixNotJudgedWhy::Unparseable,
            };
            (ExistingTestVerdict { why: Some(why), ..base(alias.to_string(), FixCovers::NotJudged, String::new()) }, crate::door::judge::Told { context: None, message: Some(crate::teach::existing::unanswered(&file, &how)) })
        }
        Ok(ans) => match ans.answers_intent {
            None => (
                ExistingTestVerdict { why: Some(FixNotJudgedWhy::Unparseable), ..base(ans.model.clone(), FixCovers::NotJudged, ans.reason.clone()) },
                crate::door::judge::Told { context: None, message: Some(crate::teach::existing::unanswered(&file, "it could not see the change")) },
            ),
            Some(x) => {
                let yes = x == AnswersIntent::Yes;
                let covers = if yes { FixCovers::Yes } else { FixCovers::No };
                let gap = Some(ans.gap.clone()).filter(|g| !g.is_empty());
                let told = crate::door::judge::Told {
                    message: Some(crate::teach::existing::verdict(&ans.model, &file, yes, &ans.gap)),
                    context: Some(crate::teach::existing::told(&ans.model, &file, yes, &ans.gap, &ans.reason)),
                };
                (ExistingTestVerdict { gap, ..base(ans.model.clone(), covers, ans.reason.clone()) }, told)
            }
        },
    };
    log.push(record.append(&verdict, around.now, crate::TENANT).map_err(|e| crate::door::cannot("write its record", &e))?);
    Ok(Some(told))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_are_named_or_foldered_as_the_runners_name_them() {
        for p in ["tests/brain/test_dataclasses.py", "pkg/m_test.go", "src/a.test.ts", "spec/a_spec.rb", "src/FooTest.java", "tests/cli.rs", "lib/test_x.py"] {
            assert!(is_test_file(p), "{p}");
        }
        for p in ["src/app.py", "tests/fixtures/data.json", "README.md", "src/testing_utils.txt", "src/contest.py"] {
            assert!(!is_test_file(p), "{p}");
        }
    }

    #[test]
    fn other_languages_count_every_removed_line_but_blanks_and_imports() {
        let before = b"use super::*;\n\n#[test]\nfn a() {\n    assert_eq!(1, 1);\n}\n";
        let c = change_of("tests/a.rs", before, &[1, 2, 5]).expect("a change");
        assert_eq!(c.lines, vec![[5, 5]]);
        assert!(change_of("tests/a.rs", before, &[1, 2]).is_none());
    }
}
