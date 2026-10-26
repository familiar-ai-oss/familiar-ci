//! Precedents for the seal's judge (ports 2026-10-24, piece 4) [draft names]. At a pytest part's
//! seal: the compound names the sealed test passes as keywords, or writes in the braces of its
//! own f-strings, and the lines of Python outside the tests, at the seal commit, read through gix,
//! that write one of them inside the `{...}` of a format string; best first, and an anchor line
//! the judge already reads inside `MOST_INPUTS` left out. The read is lexical, never a parser:
//! quotes, escapes, triple quotes carried across lines and `#` comments. `teach::precedents` has
//! the words; the probe this ports is the bench's (piece 4, 2026-10-25).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use familiar_ai_rules::criterion::test_files;
use familiar_ai_rules::{plan, View};
use regex::Regex;

use crate::teach::precedents::Precedent;

/// A name written in braces on more lines than this points at nothing in particular [draft name].
pub const MOST_LINES_PER_NAME: usize = 8;

/// A place of the anchor: its file, symbol and lines, when it has lines.
struct Spot {
    file: String,
    symbol: String,
    lines: Option<(u64, u64)>,
}

/// The precedents of `c`'s seal at `commit`, best first; `reading` is the agent's reading and the
/// anchor as the judge's prompt holds them before its cut. None for a part not run under pytest,
/// nor when the sealed test gives no compound name.
pub(crate) fn listed(v: &View, root: &Path, c: &str, commit: &str, reading: &str) -> Vec<Precedent> {
    if !familiar_ai_rules::trace::traced(v, c) {
        return Vec::new();
    }
    let parent = format!("{commit}^");
    let read = |at: &str, path: &str| familiar_ai_git::file_at(root, at, path).ok();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for fp in test_files(v, c).into_iter().filter(|f| f.ends_with(".py")) {
        if let Some(after) = read(commit, &fp) {
            names.extend(test_names(&sealed_text(read(&parent, &fp).as_deref(), &after)));
        }
    }
    names.retain(|n| compound(n));
    if names.is_empty() {
        return Vec::new();
    }
    let Ok(blobs) = familiar_ai_git::blobs_at(root, commit) else {
        return Vec::new();
    };
    // each name's lines, and the text of each file that has one
    let mut hits: BTreeMap<&str, BTreeSet<(String, u64)>> = BTreeMap::new();
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    for (path, bytes) in blobs.iter().filter(|(p, _)| p.ends_with(".py") && !test_path(p)) {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let mut found = false;
        for (k, fields) in format_fields(&text, true).iter().enumerate() {
            let ids: BTreeSet<&str> = fields.iter().flat_map(|f| identifiers(f)).collect();
            for n in names.iter().filter(|n| ids.contains(n.as_str())) {
                hits.entry(n.as_str()).or_default().insert((path.clone(), k as u64 + 1));
                found = true;
            }
        }
        if found {
            texts.insert(path.clone(), text);
        }
    }
    let mut on: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    for (n, lines) in hits.iter().filter(|(_, l)| l.len() <= MOST_LINES_PER_NAME) {
        for at in lines {
            on.entry(at.clone()).or_default().push(n.to_string());
        }
    }
    let spots = spots(v, c);
    let already = read_already(reading, &spots);
    let rarest = |ns: &[String]| ns.iter().filter_map(|n| hits.get(n.as_str()).map(BTreeSet::len)).min().unwrap_or(0);
    let group = |f: &str, n: u64| {
        if spots.iter().any(|s| s.file == f && s.lines.is_some_and(|(a, z)| a <= n && n <= z)) {
            0 // an anchor line past the cut
        } else if spots.iter().any(|s| s.file == f) {
            1
        } else {
            2
        }
    };
    // best first: the line's rarest name on the fewest lines, more names, an anchor line past the
    // cut, a line in an anchor place's file; then path and line, the order `on` keeps
    let mut ranked: Vec<((String, u64), Vec<String>)> = on.into_iter().filter(|(at, _)| !already.contains(at)).collect();
    ranked.sort_by_key(|((f, n), ns)| (rarest(ns), std::cmp::Reverse(ns.len()), group(f, *n)));
    ranked
        .into_iter()
        .filter_map(|((file, line), names)| {
            let lines: Vec<&str> = texts.get(&file)?.split('\n').collect();
            let at = usize::try_from(line).ok()?.checked_sub(1)?;
            let before = (0..at).rev().find(|&k| !lines[k].trim().is_empty()).map(|k| (k as u64 + 1, lines[k].to_string()));
            Some(Precedent { def: def_path(&lines, at), code: lines.get(at)?.to_string(), before, names, file, line })
        })
        .collect()
}

/// The anchor's places: a chosen place's lines, or the new work's file.
fn spots(v: &View, c: &str) -> Vec<Spot> {
    let Some(a) = plan::anchor_of(v, c) else {
        return Vec::new();
    };
    let text = |k: &str| a.fields.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
    match a.kind {
        "place_chosen" => plan::places_of(a)
            .iter()
            .map(|p| Spot {
                file: p.file_path.to_string(),
                symbol: p.symbol.to_string(),
                lines: p.line_start.zip(p.line_end).and_then(|(s, e)| u64::try_from(s).ok().zip(u64::try_from(e).ok())),
            })
            .collect(),
        "new_work_spot_chosen" => vec![Spot { file: text("file_path"), symbol: text("symbol"), lines: None }],
        _ => Vec::new(),
    }
}

/// The anchor lines the judge reads already: each place's lines, as the anchor shows them under
/// its `file · symbol · lines s–e` line, that end inside the first `MOST_INPUTS` characters of
/// `reading`.
fn read_already(reading: &str, spots: &[Spot]) -> BTreeSet<(String, u64)> {
    let most = crate::teach::judge::MOST_INPUTS;
    let mut out = BTreeSet::new();
    for s in spots {
        let Some((first, last)) = s.lines else { continue };
        let head = format!("{} · {} · lines {first}–{last}", s.file, s.symbol);
        let Some(at) = reading.find(&head).and_then(|h| reading[h..].find('\n').map(|n| h + n + 1)) else {
            continue;
        };
        let (mut pos, mut chars) = (at, reading[..at].chars().count());
        for n in first..=last {
            let rest = &reading[pos..];
            let len = rest.find('\n').unwrap_or(rest.len());
            let end = chars + rest[..len].chars().count();
            if end <= most {
                out.insert((s.file.clone(), n));
            }
            if pos + len >= reading.len() {
                break;
            }
            (pos, chars) = (pos + len + 1, end + 1);
        }
    }
    out
}

/// Whether `path` is outside the tests: no folder named `tests`, `test` or `testing`, and not
/// named `test_*.py`, `*_test.py` or `conftest.py`.
fn test_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let base = parts.last().copied().unwrap_or("");
    parts[..parts.len() - 1].iter().any(|d| matches!(*d, "tests" | "test" | "testing")) || base.starts_with("test_") || base.ends_with("_test.py") || base == "conftest.py"
}

/// A compound name: an `_` inside it (not only at its ends), or a lower-to-upper camel step.
fn compound(name: &str) -> bool {
    name.trim_matches('_').contains('_') || name.as_bytes().windows(2).any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase())
}

/// The text of the test functions the seal changed, as `sealed_test::shown` puts them above
/// `THE_REST`: a file new in the seal whole, else each test function the seal's lines touch.
fn sealed_text(before: Option<&[u8]>, after: &[u8]) -> String {
    let text = String::from_utf8_lossy(after).into_owned();
    let Some(before) = before else {
        return text;
    };
    let changed = crate::teach::sealed_test::changed_lines(before, after);
    let lines: Vec<&str> = text.split('\n').collect();
    crate::teach::sealed_test::test_blocks(after)
        .into_iter()
        .filter(|(_, first, last)| changed.iter().any(|(a, z)| a <= last && z >= first))
        .filter_map(|(_, first, last)| lines.get(first..=last).map(|l| l.join("\n")))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap_or_else(|e| panic!("{pattern}: {e}")))
}

/// The names the sealed test's text gives: each keyword name of a call (`(` or `,`, a name, `=`
/// and not `==`), and each name in the braces of its own f-strings, a dotted chain by its last
/// part, a bare name only when the text does not bind it itself.
fn test_names(src: &str) -> BTreeSet<String> {
    static KEYWORD: OnceLock<Regex> = OnceLock::new();
    static CHAIN: OnceLock<Regex> = OnceLock::new();
    let code = blanked(src);
    let mut out: BTreeSet<String> = not_before_eq(regex(&KEYWORD, r"[(,]\s*([A-Za-z_]\w*)\s*="), &code).into_iter().collect();
    let bound = bound(&code);
    for field in format_fields(src, false).into_iter().flatten() {
        let field = field.char_indices().find(|&(i, ch)| ch == '!' && matches!(field[i + 1..].chars().next(), Some('r' | 's' | 'a'))).map_or(field.as_str(), |(i, _)| &field[..i]);
        for chain in regex(&CHAIN, r"[A-Za-z_]\w*(?:\s*\.\s*[A-Za-z_]\w*)*").find_iter(field) {
            let parts: Vec<&str> = chain.as_str().split('.').map(str::trim).collect();
            match parts.as_slice() {
                [one] if bound.contains(*one) => {}
                [.., last] => {
                    out.insert(last.to_string());
                }
                [] => {}
            }
        }
    }
    out
}

/// The names the test's text binds itself: `NAME =` and `NAME: T =` at a line's start, `for NAME`,
/// `as NAME`, `def`/`class NAME`, and a `def`'s parameters.
fn bound(code: &str) -> BTreeSet<String> {
    static LINE: OnceLock<Regex> = OnceLock::new();
    static AFTER: OnceLock<Regex> = OnceLock::new();
    static PARAMS: OnceLock<Regex> = OnceLock::new();
    static PARAM: OnceLock<Regex> = OnceLock::new();
    let mut out: BTreeSet<String> = not_before_eq(regex(&LINE, r"(?m)^\s*([A-Za-z_]\w*)\s*(?::[^=]*)?="), code).into_iter().collect();
    out.extend(regex(&AFTER, r"\b(?:for|as|def|class)\s+([A-Za-z_]\w*)").captures_iter(code).filter_map(|m| m.get(1)).map(|m| m.as_str().to_string()));
    for params in regex(&PARAMS, r"\bdef\s+\w+\s*\(([^)]*)\)").captures_iter(code).filter_map(|m| m.get(1)) {
        out.extend(regex(&PARAM, r"([A-Za-z_]\w*)\s*(?:[:=,]|$)").captures_iter(params.as_str()).filter_map(|m| m.get(1)).map(|m| m.as_str().to_string()));
    }
    out
}

/// The first group of each match of `re` in `text` whose `=` is not the first of `==`, matches
/// tried from left to right as Python's `findall` tries `re` followed by `(?!=)`.
fn not_before_eq(re: &Regex, text: &str) -> Vec<String> {
    let (mut out, mut at) = (Vec::new(), 0);
    while let Some(m) = re.captures_at(text, at) {
        let Some(whole) = m.get(0) else { break };
        if text[whole.end()..].starts_with('=') {
            at = whole.start() + text[whole.start()..].chars().next().map_or(1, char::len_utf8);
        } else {
            out.extend(m.get(1).map(|g| g.as_str().to_string()));
            at = whole.end();
        }
    }
    out
}

/// Each identifier in `field`, as `[A-Za-z_][A-Za-z0-9_]*` finds them.
fn identifiers(field: &str) -> Vec<&str> {
    let (mut out, mut start) = (Vec::new(), None);
    for (i, ch) in field.char_indices().chain(std::iter::once((field.len(), ' '))) {
        let word = ch.is_ascii_alphanumeric() || ch == '_';
        match start {
            Some(s) if !word => {
                out.push(&field[s..i]);
                start = None;
            }
            None if ch.is_ascii_alphabetic() || ch == '_' => start = Some(i),
            _ => {}
        }
    }
    out
}

/// The quotes a string opens with, longest first.
const QUOTES: [&str; 4] = ["'''", "\"\"\"", "'", "\""];

/// A string that opens at `ch[i]`: its prefix's length, whether the prefix holds `f` and `r`, and
/// its quote; not after a letter, digit or `_`.
fn opens(ch: &[char], i: usize) -> Option<(usize, bool, bool, &'static str)> {
    if i > 0 && ch.get(i - 1).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
        return None;
    }
    (0..=2).rev().find_map(|k: usize| {
        let prefix = ch.get(i..i + k)?;
        if !prefix.iter().all(|c| "rRbBuUfF".contains(*c)) {
            return None;
        }
        let q = QUOTES.iter().find(|q| starts(ch, i + k, q))?;
        let has = |x: char| prefix.iter().any(|c| c.eq_ignore_ascii_case(&x));
        Some((k, has('f'), has('r'), *q))
    })
}

fn starts(ch: &[char], i: usize, q: &str) -> bool {
    q.chars().enumerate().all(|(k, c)| ch.get(i + k) == Some(&c))
}

/// The text inside each `{...}` of a format string, line by line: of an f-string, or, with
/// `dot_format`, of any string on a line that calls `.format(`. `{{` opens no field; a backslash
/// escapes outside a raw string; a triple-quoted string goes on across lines; a `#` outside a
/// string ends the line.
fn format_fields(text: &str, dot_format: bool) -> Vec<Vec<String>> {
    let mut open: Option<(&'static str, bool, bool)> = None;
    let mut out = Vec::new();
    for line in text.split('\n') {
        let ch: Vec<char> = line.chars().collect();
        let mut strings: Vec<(bool, String)> = Vec::new();
        let mut cur: Option<(&'static str, bool, bool, String)> = open.take().map(|(q, f, r)| (q, f, r, String::new()));
        let mut i = 0;
        while i < ch.len() {
            let Some((q, f, raw, buf)) = cur.as_mut() else {
                if ch[i] == '#' {
                    break;
                }
                match opens(&ch, i) {
                    Some((k, f, r, q)) => {
                        cur = Some((q, f, r, String::new()));
                        i += k + q.len();
                    }
                    None => i += 1,
                }
                continue;
            };
            if !*raw && ch[i] == '\\' && i + 1 < ch.len() {
                buf.extend(&ch[i..i + 2]);
                i += 2;
            } else if starts(&ch, i, q) {
                i += q.len();
                strings.push((*f, std::mem::take(buf)));
                cur = None;
            } else {
                buf.push(ch[i]);
                i += 1;
            }
        }
        if let Some((q, f, r, buf)) = cur {
            strings.push((f, buf));
            open = (q.len() == 3).then_some((q, f, r));
        }
        let dot = dot_format && line.contains(".format(");
        out.push(strings.into_iter().filter(|(f, _)| *f || dot).flat_map(|(_, s)| fields(&s)).collect());
    }
    out
}

/// The text inside each `{...}` of `s`, nested braces kept; `{{` is no field.
fn fields(s: &str) -> Vec<String> {
    let ch: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < ch.len() {
        if ch[i] == '{' && ch.get(i + 1) == Some(&'{') {
            i += 2;
        } else if ch[i] == '{' {
            let (mut depth, mut j) = (1, i + 1);
            while j < ch.len() && depth > 0 {
                match ch[j] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let end = if depth == 0 { j - 1 } else { ch.len() };
            out.push(ch[i + 1..end].iter().collect());
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// `src` with each string's text blanked to `""` and each comment dropped, so an `=` inside text
/// is never read as a keyword's; a newline ends a one-quote string.
fn blanked(src: &str) -> String {
    let ch: Vec<char> = src.chars().collect();
    let (mut out, mut i, mut q): (String, usize, Option<&'static str>) = (String::new(), 0, None);
    while i < ch.len() {
        match q {
            None if ch[i] == '#' => {
                while i < ch.len() && ch[i] != '\n' {
                    i += 1;
                }
            }
            None => match opens(&ch, i) {
                Some((k, _, _, quote)) => {
                    out.push_str("\"\"");
                    i += k + quote.len();
                    q = Some(quote);
                }
                None => {
                    out.push(ch[i]);
                    i += 1;
                }
            },
            Some(_) if ch[i] == '\\' && i + 1 < ch.len() => i += 2,
            Some(quote) if starts(&ch, i, quote) => {
                i += quote.len();
                q = None;
            }
            Some(quote) => {
                if ch[i] == '\n' && quote.len() == 1 {
                    q = None;
                }
                i += 1;
            }
        }
    }
    out
}

/// The dotted `def`/`class` path line `at` (0-based) is in: each `def` or `class` above it at a
/// smaller indent, a line at its indent or less closing it, a decorator or a closing `)` never;
/// empty on a `def` or `class` line itself, and outside any.
fn def_path(lines: &[&str], at: usize) -> String {
    static OPENS: OnceLock<Regex> = OnceLock::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for (k, line) in lines.iter().enumerate().take(at + 1) {
        let s = line.trim_start();
        let indent = line.chars().count() - s.chars().count();
        if !s.is_empty() && !s.starts_with('#') {
            if !s.starts_with(['@', ')']) {
                while stack.last().is_some_and(|(i, _)| *i >= indent) {
                    stack.pop();
                }
            }
            if let Some(name) = regex(&OPENS, r"^(?:async\s+)?(?:def|class)\s+(\w+)").captures(s).and_then(|m| m.get(1)) {
                if k == at {
                    return String::new();
                }
                stack.push((indent, name.as_str().to_string()));
                continue;
            }
        }
        if k == at {
            return stack.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(".");
        }
    }
    String::new()
}
