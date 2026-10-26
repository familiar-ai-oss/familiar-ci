//! The touching tests the binary derives (the owner, 2026-10-20) [draft names]. Declared touching
//! tests are opt-in, and one plan in 44 on the bench declared any. So when new commits land, for
//! each part at work past its tests-first commit, the binary reads the source files the fix
//! changed since that commit and finds the project's existing test files that reference them:
//! Python by `import m` / `from m import` / `from pkg import m`, JavaScript and TypeScript by an
//! import or `require` of the file, Go by a `_test.go` file in the same package folder. The
//! nearest first (the most folders shared with the changed file), at most `MOST` per part, each
//! written once as `touching_test_derived`; the rules run them at the head with the part's own
//! tests and green waits for them to pass.

use std::path::Path;

use familiar_ai_facts::{Log, Record, RecordError, TouchingTestDerived};
use familiar_ai_rules::criterion::{test_files, tests_first_commit, touching_tests, TOUCHING_DERIVED};
use familiar_ai_rules::reader::text;
use familiar_ai_rules::View;

/// The derived touching tests per part, which bounds the run's time.
pub const MOST: usize = 5;
/// The test files read for references per call, at most.
const MOST_READ: usize = 2_000;
/// Folders never searched.
const SKIP: [&str; 8] = ["node_modules", ".git", ".venv", "venv", "target", "dist", "build", ".familiar"];

/// The dotted module names a Python file may be imported as: every tail of its path, with a
/// package's `__init__` naming the package.
fn python_names(src: &str) -> Vec<String> {
    let Some(stem) = src.strip_suffix(".py") else { return Vec::new() };
    let mut parts: Vec<&str> = stem.split('/').collect();
    if parts.last() == Some(&"__init__") {
        parts.pop();
    }
    (0..parts.len()).map(|i| parts[i..].join(".")).filter(|n| !n.is_empty()).collect()
}

/// Whether a Python test's text imports one of `names`.
fn python_imports(content: &str, names: &[String]) -> bool {
    content.lines().map(str::trim).any(|l| {
        if let Some(rest) = l.strip_prefix("import ") {
            return rest.split(',').map(|x| x.split_whitespace().next().unwrap_or("")).any(|m| names.iter().any(|n| n == m));
        }
        let Some(rest) = l.strip_prefix("from ") else { return false };
        let mut w = rest.split_whitespace();
        let (Some(module), Some("import")) = (w.next(), w.next()) else { return false };
        if names.iter().any(|n| n == module) {
            return true;
        }
        let imported: Vec<String> = w.collect::<Vec<_>>().join(" ").replace(['(', ')'], "").split(',').map(|x| x.split_whitespace().next().unwrap_or("").to_string()).collect();
        imported.iter().any(|leaf| names.iter().any(|n| *n == format!("{module}.{leaf}")))
    })
}

/// Whether a JavaScript or TypeScript test's text imports or requires the file `src`.
fn script_imports(content: &str, src: &str) -> bool {
    let file = src.rsplit('/').next().unwrap_or(src);
    let stem = file.split('.').next().unwrap_or(file);
    let folder = src.rsplit('/').nth(1).unwrap_or("");
    content.lines().filter(|l| l.contains("import") || l.contains("require(")).any(|l| {
        l.split(['\'', '"', '`']).skip(1).step_by(2).any(|spec| {
            let last = spec.rsplit('/').next().unwrap_or(spec);
            let last = last.split('.').next().unwrap_or(last);
            spec.starts_with('.') && (last == stem || (stem == "index" && last == folder))
        })
    })
}

/// Whether the test file `test`, with `content`, references the changed source file `src`.
fn references(test: &str, content: &str, src: &str) -> bool {
    let ext = |p: &str| p.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    match (ext(test).as_str(), ext(src).as_str()) {
        ("py", "py") => python_imports(content, &python_names(src)),
        ("js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "jsx" | "tsx", "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "jsx" | "tsx") => script_imports(content, src),
        ("go", "go") => test.ends_with("_test.go") && test.rsplit_once('/').map(|(d, _)| d) == src.rsplit_once('/').map(|(d, _)| d),
        _ => false,
    }
}

/// How many folders `a` and `b` share from the root.
fn shared(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| x == y).count()
}

/// The existing test files referencing a source file the fix changed from `from` to `to`, the
/// nearest first, each with the file it references; `own` and `known` left out.
pub fn found(root: &Path, from: &str, to: &str, own: &[String], known: &[String]) -> Vec<(String, String)> {
    let is_test = crate::door::existing::is_test_file;
    let changed: Vec<String> = familiar_ai_git::changed_between(root, from, to)
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.path)
        .filter(|p| !is_test(p) && !own.contains(p) && !crate::door::commits::is_familiars_own(p))
        .filter(|p| familiar_ai_git::file_at(root, to, p).is_ok())
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    let tests: Vec<String> = familiar_ai_git::all_files(root, to, &SKIP)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| is_test(p) && !own.contains(p) && !known.contains(p))
        .take(MOST_READ)
        .collect();
    let mut hits: Vec<(usize, String, String)> = Vec::new();
    for t in tests {
        let Ok(bytes) = familiar_ai_git::file_at(root, to, &t) else { continue };
        let content = String::from_utf8_lossy(&bytes);
        if let Some(s) = changed.iter().filter(|s| references(&t, &content, s)).max_by_key(|s| shared(&t, s)) {
            hits.push((shared(&t, s), t, s.clone()));
        }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, t, s)| (t, s)).collect()
}

/// For each part at work past its tests-first commit, write the touching tests found at the tip,
/// up to `MOST` per part.
pub fn derive(record: &Record, root: &Path, log: &mut Log, at: f64, tenant: &str) -> Result<(), RecordError> {
    let mut out: Vec<TouchingTestDerived> = Vec::new();
    {
        let v = View::new(log);
        let Some(tip) = v.last("commit_seen").and_then(|f| text(f, "commit")).map(str::to_string) else { return Ok(()) };
        for c in familiar_ai_rules::lifecycle::runnable_criteria(&v) {
            let Some(tf) = tests_first_commit(&v, &c).and_then(|t| t.ids.last().cloned()) else { continue };
            let room = MOST.saturating_sub(v.of(TOUCHING_DERIVED, &c).count());
            if tf == tip || room == 0 {
                continue;
            }
            let own = test_files(&v, &c);
            let known: Vec<String> = touching_tests(&v, &c).into_iter().map(|t| t.file_path).collect();
            for (file_path, imports) in found(root, &tf, &tip, &own, &known).into_iter().take(room) {
                out.push(TouchingTestDerived { criterion_id: c.clone(), file_path, imports });
            }
        }
    }
    for f in out {
        log.push(record.append(&f, at, tenant)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_imports_are_read_by_module_and_by_package() {
        let names = python_names("src/astroid/brain/brain_dataclasses.py");
        assert!(names.contains(&"astroid.brain.brain_dataclasses".to_string()));
        assert!(python_imports("from astroid.brain.brain_dataclasses import x\n", &names));
        assert!(python_imports("from astroid.brain import brain_dataclasses, other\n", &names));
        assert!(python_imports("import brain_dataclasses as b\n", &names));
        assert!(!python_imports("from astroid import nodes\n", &names));
        assert!(python_imports("import m\n", &python_names("src/m.py")));
    }

    #[test]
    fn scripts_and_go_are_read_by_path_and_package() {
        assert!(references("test/a.test.ts", "import { f } from '../src/a';\n", "src/a.ts"));
        assert!(references("test/a.test.js", "const a = require(\"../lib/a.js\");\n", "lib/a.js"));
        assert!(!references("test/a.test.js", "const a = require('lodash');\n", "lib/lodash.js"));
        assert!(references("pkg/m_test.go", "", "pkg/m.go"));
        assert!(!references("other/m_test.go", "", "pkg/m.go"));
    }
}
