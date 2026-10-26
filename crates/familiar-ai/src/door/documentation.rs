//! The binary's check that a light criterion's anchor is documentation (the
//! owner, 2026-10-11): "a flag for light that is only allowed if the anchor
//! line is clearly documentation, a markdown, or has the standard lint
//! attributes of a docstring." One `anchor_is_documentation` fact per spot of
//! the anchor, read at the record's tip sha, as line 5's check is.
//!
//! ```text
//!   markdown      the file's extension is .md
//!   Python        the lines are one docstring (PEP 257): the first non-blank
//!                 line opens with """ or ''' (r or u prefixed or not), the
//!                 range closes it, and the nearest non-blank line above is a
//!                 def or class line ending with ':' (or the range starts at
//!                 the file's top, below only comments)
//!   Rust          every non-blank line starts with /// or //!
//!   JS / TS       the range is one /** … */ block
//!   new work      documentation only when its file is markdown
//! ```
//!
//! Anything else, a missing file, or lines outside it: not documentation.
//! [reading] the exact tests of each language are this module's.

use std::path::Path;

use familiar_ai_facts::{AnchorIsDocumentation, Fact, Log, Record, RecordError};
use familiar_ai_rules::plan::{self, DocSpot};
use familiar_ai_rules::View;

use crate::door::commits::last_commit_seen;

/// One documentation check owed: a light criterion's spot with no check
/// after its anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct DocCheck {
    pub criterion_id: String,
    pub spot: DocSpot,
}

/// The next documentation check owed at this log; `None` when none.
pub fn next_doc_check(log: &Log) -> Option<DocCheck> {
    let v = View::new(log);
    for c in plan::criteria(&v) {
        if plan::path_of(&v, &c) != Some("light") {
            continue;
        }
        let Some(a) = plan::anchor_of(&v, &c) else {
            continue;
        };
        for spot in plan::doc_spots_of(a).unwrap_or_default() {
            if plan::doc_of(&v, &c, &spot).is_none() {
                return Some(DocCheck { criterion_id: c, spot });
            }
        }
    }
    None
}

fn extension(file_path: &str) -> String {
    Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Whether `file_path` is markdown: its extension is `.md`.
pub fn is_markdown(file_path: &str) -> bool {
    extension(file_path) == "md"
}

/// Whether lines `line_start..=line_end` (1-based) of `bytes` are one
/// docstring of the file's language.
pub fn is_docstring(file_path: &str, bytes: &[u8], line_start: u64, line_end: u64) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let all: Vec<&str> = text.lines().collect();
    let (Ok(s), Ok(e)) = (usize::try_from(line_start), usize::try_from(line_end)) else {
        return false;
    };
    if s == 0 || e < s || e > all.len() {
        return false;
    }
    let range = &all[s - 1..e];
    if range.iter().all(|l| l.trim().is_empty()) {
        return false;
    }
    match extension(file_path).as_str() {
        "py" | "pyi" => python_docstring(&all[..s - 1], range),
        "rs" => range.iter().filter(|l| !l.trim().is_empty()).all(|l| {
            let t = l.trim_start();
            t.starts_with("///") || t.starts_with("//!")
        }),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => jsdoc_block(range),
        _ => false,
    }
}

/// One `/** … */` block: it opens the range and its one close ends it.
fn jsdoc_block(range: &[&str]) -> bool {
    let joined = range.join("\n");
    let t = joined.trim();
    t.len() >= 5 && t.starts_with("/**") && t.ends_with("*/") && t[3..].find("*/") == Some(t.len() - 5)
}

/// PEP 257: a string literal that is the first statement of a module, class
/// or function.
fn python_docstring(above: &[&str], range: &[&str]) -> bool {
    let joined = range.join("\n");
    let t = joined.trim();
    let body = t.strip_prefix(['r', 'R', 'u', 'U']).unwrap_or(t);
    let Some(q) = ["\"\"\"", "'''"].into_iter().find(|q| body.starts_with(q)) else {
        return false;
    };
    // the close: the first unescaped q after the open ends the range
    let rest = &body[3..];
    let mut i = 0;
    let bytes = rest.as_bytes();
    let close = loop {
        if i >= bytes.len() {
            return false;
        }
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if rest[i..].starts_with(q) {
            break i;
        }
        i += 1;
    };
    if !rest[close + 3..].trim().is_empty() {
        return false;
    }
    let mut prior = above.iter().rev().filter(|l| !l.trim().is_empty());
    match prior.next() {
        // the module's: only comments above
        None => true,
        Some(_) if above.iter().all(|l| l.trim().is_empty() || l.trim_start().starts_with('#')) => true,
        Some(l) => {
            let l = l.trim_end();
            l.ends_with(':') && opens_a_def_or_class(above, l)
        }
    }
}

/// The `:` line ends a `def`/`class` header: the header's first line is the
/// nearest line above (or the line itself) that starts with `def`, `async
/// def` or `class`, and its brackets balance by the `:`.
fn opens_a_def_or_class(above: &[&str], colon_line: &str) -> bool {
    let header = |l: &str| {
        let t = l.trim_start();
        t.starts_with("def ") || t.starts_with("async def ") || t.starts_with("class ")
    };
    let end = above.iter().rposition(|l| l.trim_end() == colon_line).unwrap_or(0);
    let Some(start) = above[..=end].iter().rposition(|l| header(l)) else {
        return false;
    };
    let text = above[start..=end].join("\n");
    let open = text.matches(['(', '[']).count();
    let shut = text.matches([')', ']']).count();
    open == shut
}

/// Whether the spot is documentation, read at `sha`: markdown by its
/// extension; a place's lines one docstring; new work only markdown.
pub fn is_documentation(root: &Path, sha: &str, spot: &DocSpot) -> bool {
    if is_markdown(&spot.file_path) {
        return true;
    }
    let (Some(a), Some(b)) = (spot.line_start, spot.line_end) else {
        return false;
    };
    let (Ok(a), Ok(b)) = (u64::try_from(a), u64::try_from(b)) else {
        return false;
    };
    match familiar_ai_git::file_at(root, sha, &spot.file_path) {
        Ok(bytes) => is_docstring(&spot.file_path, &bytes, a, b),
        Err(_) => false,
    }
}

/// The check's fact.
pub fn look(root: &Path, sha: &str, check: &DocCheck) -> AnchorIsDocumentation {
    let s = &check.spot;
    AnchorIsDocumentation {
        criterion_id: check.criterion_id.clone(),
        file_path: s.file_path.clone(),
        symbol: s.symbol.clone(),
        line_start: s.line_start.and_then(|n| u64::try_from(n).ok()),
        line_end: s.line_end.and_then(|n| u64::try_from(n).ok()),
        sha: sha.to_string(),
        matched: is_documentation(root, sha, s),
    }
}

/// Write the next owed documentation check, when one is owed and there is a
/// sha to read at.
pub fn write_one(record: &Record, root: &Path, log: &mut Log, at: f64, tenant: &str) -> Result<Option<Fact>, RecordError> {
    let Some(check) = next_doc_check(log) else {
        return Ok(None);
    };
    let Some(sha) = last_commit_seen(log) else {
        return Ok(None);
    };
    let fact = record.append(&look(root, &sha, &check), at, tenant)?;
    log.push(fact.clone());
    Ok(Some(fact))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(file: &str, body: &str, a: u64, b: u64) -> bool {
        is_docstring(file, body.as_bytes(), a, b)
    }

    #[test]
    fn markdown_is_by_the_extension() {
        assert!(is_markdown("README.md") && is_markdown("docs/a.MD"));
        assert!(!is_markdown("src/app.py") && !is_markdown("notes.markdown.txt"));
    }

    #[test]
    fn a_python_docstring_is_the_first_statement_of_a_def_class_or_module() {
        let f = "def greet(name):\n    \"\"\"Return the greeting.\n\n    Twice.\n    \"\"\"\n    return name\n";
        assert!(doc("a.py", f, 2, 5), "the function's docstring");
        assert!(!doc("a.py", f, 1, 6), "the whole function");
        assert!(!doc("a.py", f, 6, 6), "code");
        assert!(!doc("a.py", f, 2, 3), "the range does not close it");
        let m = "#!/usr/bin/env python\n'''The module.'''\nimport os\n";
        assert!(doc("m.py", m, 2, 2), "the module's, below a comment");
        let c = "class A(\n    B,\n):\n    r\"\"\"A thing.\"\"\"\n";
        assert!(doc("c.py", c, 4, 4), "a class over three lines, r-prefixed");
        let x = "x = 1\n\"\"\"not a docstring\"\"\"\n";
        assert!(!doc("x.py", x, 2, 2), "a string after code");
        let e = "def f():\n    \"\"\"a \\\"\"\" b\"\"\"\n";
        assert!(doc("e.py", e, 2, 2), "an escaped quote inside");
    }

    #[test]
    fn rust_doc_lines_and_one_jsdoc_block() {
        let r = "/// One.\n\n//! Two.\nfn f() {}\n";
        assert!(doc("a.rs", r, 1, 3));
        assert!(!doc("a.rs", r, 1, 4));
        assert!(!doc("a.rs", "// plain\n", 1, 1));
        let j = "/**\n * One.\n */\nfunction f() {}\n/** Two. */\n";
        assert!(doc("a.js", j, 1, 3) && doc("a.ts", j, 5, 5));
        assert!(!doc("a.js", j, 1, 4), "code after the block");
        assert!(!doc("a.js", "/** a */ x(); /** b */\n", 1, 1), "two blocks");
        assert!(!doc("a.js", "/* plain */\n", 1, 1));
        assert!(!doc("a.go", "// Doc.\n", 1, 1), "another language");
        assert!(!doc("a.rs", "/// x\n", 2, 2), "outside the file");
    }
}
