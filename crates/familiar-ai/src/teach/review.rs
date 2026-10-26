//! The reviewer's words [draft] (the owner's plan of 2026-10-20, step 3): its prompt, the fix as
//! a numbered diff, the code around each change, and the lines the person and the agent read.
//! This module only formats; `door::review` asks the reviewer, checks its quotes and writes the
//! facts.

use familiar_ai_facts::Fact;
use familiar_ai_git::{line_hunks, split_lines, Hunk};
use familiar_ai_rules::View;
use serde_json::Value;

use crate::teach::judge::{cut, cut_test, MOST_INPUTS};
use crate::teach::words::LEAD;

/// The cap per changed file, for its diff and for the code around it, each.
pub const MOST_PER_FILE: usize = 8_000;
/// The changed files shown; the reviewer can read the rest.
pub const MOST_FILES: usize = 12;
/// Lines of context around each hunk of the diff.
const CONTEXT: u64 = 3;
/// The lines of a file's head shown, where its docstring and imports are.
const HEAD_LINES: usize = 15;
/// The lines of an enclosing definition shown whole.
const MOST_BLOCK_LINES: usize = 80;
/// The lines of the outline of a definition's siblings.
const MOST_SIBLING_LINES: usize = 40;

/// One file the fix changed, as it was at the tests-first commit and is at the green commit.
pub struct Changed {
    pub path: String,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

/// What the reviewer is shown, read by `door::review` from the record and git.
pub struct Inputs {
    pub criterion_id: String,
    /// The person's words, one entry per prompt.
    pub intent: Vec<String>,
    /// The agent's claim of the intent, when it differs from the person's words.
    pub reading: Option<String>,
    pub criterion: String,
    /// Each planned test file as the judge saw it at the seal.
    pub tests: Vec<(String, String)>,
    /// The files changed from the tests-first commit to the green commit.
    pub changed: Vec<Changed>,
}

/// The question, generic: it names no case.
pub const QUESTION: &str = "Against the person's words and the code around the change: does the fix do what was asked, everywhere it was asked? \
Does it change anything else the code around it relies on (documented precedence, a sibling path, a repr or other derived output, \
a hot path that bypasses the change)? For any gap, quote the exact repo lines that show it, with file:line.";

/// The reviewer's prompt, each part cut to its cap.
pub fn prompt(i: &Inputs) -> String {
    let most = crate::teach::person::MOST_WORDS;
    let mut out = String::from(
        "A coding agent was given the intent below. It wrote the criterion below and sealed the test below, which failed on the code \
         before the fix. It then changed the code, and the test passes at the green commit. You review the fix itself: the test \
         was read before you.\n\n",
    );
    out.push_str("THE INTENT (what the person asked for, in their own words)\n");
    let said: Vec<String> = i.intent.iter().map(|w| cut(w, most)).collect();
    out.push_str(&said.join("\n\nLater, the person also said:\n"));
    let mut reading = String::from("THE AGENT'S READING (what the agent wrote down; not the person's words)\n");
    if let Some(r) = &i.reading {
        reading.push_str(&format!("the intent, as the agent put it: {r}\n"));
    }
    reading.push_str(&format!("criterion {}: {}", i.criterion_id, i.criterion));
    out.push_str("\n\n");
    out.push_str(&cut(&reading, MOST_INPUTS));
    for (path, shown) in &i.tests {
        out.push_str(&format!("\n\nTHE SEALED TEST {path} {}", cut_test(shown)));
    }
    let shown: Vec<&Changed> = i.changed.iter().take(MOST_FILES).collect();
    out.push_str("\n\nTHE FIX (from the tests-first commit to the green commit)");
    if shown.is_empty() {
        out.push_str("\nno file changed");
    }
    for c in &shown {
        out.push_str(&format!("\n{}", cut(&diff(c), MOST_PER_FILE)));
    }
    if i.changed.len() > shown.len() {
        let rest: Vec<&str> = i.changed[shown.len()..].iter().map(|c| c.path.as_str()).collect();
        out.push_str(&format!("\nand {} more changed files, not shown: {}", rest.len(), rest.join(", ")));
    }
    for c in &shown {
        if let Some(around) = around(c) {
            out.push_str(&format!("\n\nTHE CODE AROUND THE CHANGE {} (at the green commit; each line numbered)\n{}", c.path, cut(&around, MOST_PER_FILE)));
        }
    }
    out.push_str(
        "\n\nYOUR TOOLS\nYou are in a read-only copy of the repository at the green commit: Read and Grep any file in it. \
         Its line numbers are the ones above.",
    );
    out.push_str(&format!("\n\nTHE QUESTION\n{QUESTION}\n\n"));
    out.push_str(
        "A quote is one or more whole lines exactly as the file holds them at the green commit, without the line numbers, and \
         line is the number of its first line; file is the path from the repository's root. A gap counts only when every quote \
         is found at its file and line. A quote of the test alone shows no gap in the fix.\n\n\
         The answer is exactly one JSON object and nothing else (no code fence, no other text):\n\
         {\"covers\": true or false, \"gap\": \"<what the fix does not do, or what it breaks, or empty>\", \
         \"quotes\": [{\"file\": \"<path>\", \"line\": <its first line>, \"text\": \"<the exact lines>\"}], \
         \"reason\": \"<what was asked, where the fix does it, and what it leaves out>\"}\n\
         When the fix does everything asked and breaks nothing around it: covers is true, gap is empty and quotes is [].\n",
    );
    out
}

/// Line `n` (1-based) of `lines`, without its line ending.
fn line(lines: &[&[u8]], n: u64) -> Option<String> {
    let l = lines.get(usize::try_from(n).ok()?.checked_sub(1)?)?;
    Some(String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string())
}

pub(crate) fn numbered(sign: char, n: u64, text: &str) -> String {
    format!("{sign}{n:>5}| {text}")
}

pub(crate) fn binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8_000).any(|b| *b == 0)
}

/// The file's diff, each line numbered: a removed line by its number at the tests-first commit,
/// the rest by theirs at the green commit.
pub fn diff(c: &Changed) -> String {
    let (old, new) = (c.before.as_deref().unwrap_or(b""), c.after.as_deref().unwrap_or(b""));
    let head = format!(
        "--- {} {}\n+++ {} {}",
        c.path,
        if c.before.is_some() { "(at the tests-first commit)" } else { "(new in the fix)" },
        c.path,
        if c.after.is_some() { "(at the green commit)" } else { "(deleted by the fix)" }
    );
    if binary(old) || binary(new) {
        return format!("{head}\n(a binary file, not shown)");
    }
    let (ol, nl) = (split_lines(old), split_lines(new));
    let mut out = vec![head];
    for h in line_hunks(old, new) {
        out.push(format!("@@ lines {}–{} @@", h.new_start, h.new_start + h.new_count.saturating_sub(1)));
        for n in h.new_start.saturating_sub(CONTEXT).max(1)..h.new_start {
            out.extend(line(&nl, n).map(|t| numbered(' ', n, &t)));
        }
        for n in h.old_start..h.old_start + h.old_count {
            out.extend(line(&ol, n).map(|t| numbered('-', n, &t)));
        }
        for n in h.new_start..h.new_start + h.new_count {
            out.extend(line(&nl, n).map(|t| numbered('+', n, &t)));
        }
        let after = h.new_start + h.new_count;
        for n in after..after + CONTEXT {
            out.extend(line(&nl, n).map(|t| numbered(' ', n, &t)));
        }
    }
    out.join("\n")
}

/// A line's indent in columns, a tab as four; `None` for a blank line.
pub(crate) fn indent(l: &str) -> Option<usize> {
    if l.trim().is_empty() {
        return None;
    }
    Some(l.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum())
}

/// The nearest line above `at` (0-based) indented less than `at`'s own: the definition `at` is in.
pub(crate) fn enclosing(lines: &[String], at: usize) -> Option<usize> {
    let own = (at..lines.len()).chain((0..at).rev()).find_map(|i| indent(&lines[i]))?;
    (0..at).rev().find(|&i| indent(&lines[i]).is_some_and(|x| x < own))
}

/// The last line of the block that `header` (0-based) opens: up to the next line indented no
/// deeper than it, that line too when it closes the block.
pub(crate) fn block_end(lines: &[String], header: usize) -> usize {
    let own = indent(&lines[header]).unwrap_or(0);
    match (header + 1..lines.len()).find(|&i| indent(&lines[i]).is_some_and(|x| x <= own)) {
        Some(i) if lines[i].trim_start().starts_with(['}', ')', ']']) || lines[i].trim_start().starts_with("end") => i,
        Some(i) => i.saturating_sub(1),
        None => lines.len().saturating_sub(1),
    }
}

/// The code around each change of the file at the green commit, read by indentation only: the
/// file's head, then for each hunk the definitions it is inside, the innermost whole, and the
/// lines beside it at its own indent in its parent (its siblings' first lines and the parent's
/// docstring); `None` for a file the fix deleted or a binary one.
pub fn around(c: &Changed) -> Option<String> {
    let after = c.after.as_deref()?;
    if binary(after) {
        return None;
    }
    let lines: Vec<String> = split_lines(after).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    if lines.is_empty() {
        return None;
    }
    let show = |i: usize| numbered(' ', i as u64 + 1, &lines[i]);
    let mut out = vec!["the head of the file:".to_string()];
    out.extend((0..lines.len().min(HEAD_LINES)).map(show));
    let hunks: Vec<Hunk> = line_hunks(c.before.as_deref().unwrap_or(b""), after);
    let mut seen: Vec<usize> = Vec::new();
    for h in hunks {
        let at = usize::try_from(h.new_start.saturating_sub(1)).unwrap_or(0).min(lines.len() - 1);
        let Some(def) = enclosing(&lines, at) else {
            continue;
        };
        if seen.contains(&def) {
            continue;
        }
        seen.push(def);
        let mut chain = Vec::new();
        let mut up = enclosing(&lines, def);
        while let Some(p) = up {
            chain.push(p);
            up = enclosing(&lines, p);
        }
        out.push(format!("around lines {}–{}:", h.new_start, h.new_start + h.new_count.saturating_sub(1)));
        if !chain.is_empty() {
            out.push("inside:".to_string());
            out.extend(chain.iter().rev().map(|&p| show(p)));
        }
        out.push("the definition the change is in, whole:".to_string());
        let end = block_end(&lines, def).min(def + MOST_BLOCK_LINES - 1);
        out.extend((def..=end).map(show));
        let own = indent(&lines[def]).unwrap_or(0);
        let (from, to) = match chain.first() {
            Some(&p) => (p + 1, block_end(&lines, p)),
            None => (0, lines.len() - 1),
        };
        let beside: Vec<String> = (from..=to).filter(|&i| i != def && indent(&lines[i]) == Some(own)).take(MOST_SIBLING_LINES).map(show).collect();
        if !beside.is_empty() {
            out.push("beside it, at its indent (its siblings' first lines):".to_string());
            out.extend(beside);
        }
    }
    Some(out.join("\n"))
}

fn text<'a>(f: &'a Fact, k: &str) -> Option<&'a str> {
    f.fields.get(k).and_then(Value::as_str)
}

fn trimmed(s: &str) -> &str {
    s.trim().trim_end_matches('.')
}

/// The verdict as the person reads it.
pub fn verdict(model: &str, covers: bool, gap: &str) -> String {
    let model = crate::teach::words::model(model);
    if covers {
        format!("{LEAD}the reviewer ({model}) read the fix against your request: it does what was asked")
    } else {
        format!("{LEAD}the reviewer ({model}) read the fix against your request: it does not yet do {}", trimmed(gap))
    }
}

/// Where a green part stands while the reviewer's verified "no" stands against its fix.
pub fn found(no: &Fact) -> String {
    let model = crate::teach::words::model(text(no, "model").unwrap_or(""));
    format!("the reviewer ({model}) found the fix does not yet do {}; the code may be changed", trimmed(text(no, "gap").unwrap_or("")))
}

/// The line when the reviewer's "no" did not check out.
pub fn not_judged(detail: &str) -> String {
    format!("{LEAD}the reviewer's no is not judged: {detail}; it never blocks; its call is logged in .familiar/judge.log")
}

/// The line when the reviewer gave no readable answer.
pub fn unanswered(how: &str) -> String {
    format!("{LEAD}the reviewer did not answer ({how}); the fix stands, not judged; its call is logged in .familiar/judge.log")
}

/// A verified quote as the agent and the person read it: `file:line "text"`, on one line.
pub fn quote_words(q: &Value) -> String {
    let file = q.get("file_path").and_then(Value::as_str).unwrap_or("?");
    let line = q.get("line").and_then(Value::as_u64).unwrap_or(0);
    let text = crate::teach::person_words::one_line(q.get("text").and_then(Value::as_str).unwrap_or(""));
    format!("{file}:{line} \"{text}\"")
}

/// The stop refusal while a verified "no" stands against `c`.
pub fn not_done(v: &View, c: &str, no: &Fact) -> String {
    let model = crate::teach::words::model(text(no, "model").unwrap_or(""));
    let proof: Vec<String> = no.fields.get("quotes").and_then(Value::as_array).map(|qs| qs.iter().map(quote_words).collect()).unwrap_or_default();
    format!(
        "{LEAD}not done yet · {} · the reviewer ({model}) found the fix does not yet do {}\n  the reviewer's reason: {}\n  its proof: {}\n  next: change the code so it does, and commit it; the new commit is reviewed again at your next stop",
        crate::teach::words::quoted(v, c),
        trimmed(text(no, "gap").unwrap_or("")),
        trimmed(text(no, "reason").unwrap_or("")),
        proof.join(" · ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(before: &str, after: &str) -> Changed {
        Changed { path: "src/config.py".into(), before: Some(before.as_bytes().to_vec()), after: Some(after.as_bytes().to_vec()) }
    }

    const BEFORE: &str = "\"\"\"Settings.\"\"\"\n\n\nclass Config:\n    \"\"\"Values are read from the file first, then the environment.\"\"\"\n\n    def get(self, k):\n        return self.env.get(k)\n\n    def __repr__(self):\n        return f\"Config({self.env})\"\n\n\ndef load():\n    return Config()\n";

    #[test]
    fn the_diff_numbers_each_line_by_its_own_side() {
        let after = BEFORE.replace("return self.env.get(k)", "return self.file.get(k, self.env.get(k))");
        let d = diff(&changed(BEFORE, &after));
        assert!(d.starts_with("--- src/config.py (at the tests-first commit)\n+++ src/config.py (at the green commit)\n@@ lines 8–8 @@\n"), "{d}");
        assert!(d.contains("     7|     def get(self, k):\n-    8|         return self.env.get(k)\n+    8|         return self.file.get(k, self.env.get(k))\n     9| \n"), "{d}");
        let new = diff(&Changed { path: "src/new.py".into(), before: None, after: Some(b"x = 1\n".to_vec()) });
        assert_eq!(new, "--- src/new.py (new in the fix)\n+++ src/new.py (at the green commit)\n@@ lines 1–1 @@\n+    1| x = 1");
    }

    #[test]
    fn the_code_around_a_change_is_its_definition_its_parent_and_its_siblings() {
        let after = BEFORE.replace("return self.env.get(k)", "return self.file.get(k, self.env.get(k))");
        let a = around(&changed(BEFORE, &after)).unwrap_or_default();
        assert!(a.starts_with("the head of the file:\n     1| \"\"\"Settings.\"\"\"\n"), "{a}");
        assert!(a.contains("around lines 8–8:\ninside:\n     4| class Config:\nthe definition the change is in, whole:\n     7|     def get(self, k):\n     8|         return self.file.get(k, self.env.get(k))\n"), "{a}");
        assert!(a.contains("beside it, at its indent (its siblings' first lines):\n     5|     \"\"\"Values are read from the file first, then the environment.\"\"\"\n    10|     def __repr__(self):"), "{a}");
        let beside = &a[a.find("beside it").unwrap_or(0)..];
        assert!(!beside.contains("    11|"), "a sibling's body is not shown: {a}");
    }

    #[test]
    fn a_brace_block_ends_at_its_closing_line() {
        let before = "fn a() {\n    one();\n}\n\nfn b() {\n    two();\n}\n";
        let after = before.replace("two()", "three()");
        let a = around(&Changed { path: "src/lib.rs".into(), before: Some(before.as_bytes().to_vec()), after: Some(after.into_bytes()) }).unwrap_or_default();
        assert!(a.contains("the definition the change is in, whole:\n     5| fn b() {\n     6|     three();\n     7| }"), "{a}");
        assert!(a.contains("beside it, at its indent (its siblings' first lines):\n     1| fn a() {\n     3| }"), "{a}");
    }

    #[test]
    fn the_prompt_asks_the_generic_question_and_the_one_answer_form() {
        let i = Inputs {
            criterion_id: "c1".into(),
            intent: vec!["Read the file before the environment.".into()],
            reading: None,
            criterion: "the file wins".into(),
            tests: vec![("tests/test_c1.py".into(), "(new in the seal commit)\ndef test_file_wins(): ...".into())],
            changed: vec![changed(BEFORE, &BEFORE.replace("self.env.get(k)", "self.file.get(k)"))],
        };
        let p = prompt(&i);
        assert!(p.contains(&format!("THE QUESTION\n{QUESTION}\n")), "{p}");
        assert!(p.contains("{\"covers\": true or false, \"gap\": \"<what the fix does not do, or what it breaks, or empty>\", \"quotes\": [{\"file\": \"<path>\", \"line\": <its first line>, \"text\": \"<the exact lines>\"}], \"reason\": \"<what was asked, where the fix does it, and what it leaves out>\"}"), "{p}");
        for part in ["THE INTENT (what the person asked for, in their own words)\nRead the file before the environment.", "criterion c1: the file wins", "THE SEALED TEST tests/test_c1.py (new in the seal commit)", "THE FIX (from the tests-first commit to the green commit)\n--- src/config.py", "THE CODE AROUND THE CHANGE src/config.py (at the green commit; each line numbered)\nthe head of the file:"] {
            assert!(p.contains(part), "{part:?}: {p}");
        }
        for word in ["13665", "13418", "13390", "13456", "pydantic"] {
            assert!(!QUESTION.contains(word) && !p.contains(word), "the question names no case: {word}");
        }
    }
}
