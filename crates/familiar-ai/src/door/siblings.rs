//! The sibling sites of a fix [draft] (the owner's plan warm-popping-key, the sibling check,
//! 2026-10-20): the places outside the fix's own lines that name what the fix changed, found by
//! search at a commit, never by a model. Brought in from lane-sibling with its tests, its reviewer
//! prompt left behind; the place lister (step 3 of the plan) searches with it before any fix.
//!
//! The rules, all mechanical:
//! - What the fix names (`tokens`): from each changed file outside `tests/` and Familiar's own,
//!   the name of the definition each hunk is in; the identifiers on the removed and added lines
//!   that read as names of their own (at least `LEAST_CORE` characters past leading and trailing
//!   `_`, with an `_` inside or a lower-to-upper hump, not a `STOP` word); and each path segment
//!   between two `/` of a word on those lines, as `/segment/`. At most `MOST_TOKENS`.
//! - Where they are (`list`): every file of the green commit but those under a `tests` or `test`
//!   folder, Familiar's own, binary ones and ones over `MOST_FILE_BYTES`; a name matched as a
//!   whole word, a segment as written; never on a line of the fix's own hunks or the line defining
//!   a function a hunk is in. A token found on more than `COMMON` lines is too common to point
//!   anywhere and is dropped.
//! - A site is the lines found inside one definition (the nearest enclosing line that defines a
//!   name), or found within two lines of each other outside any; it is shown at its line naming
//!   the most tokens.
//! - Ranked: the fix's own files, then their folders, then their top folders, then the rest; then
//!   the most tokens named, then the rarest token, then path and line. At most `MOST_SITES`,
//!   shown in at most `MOST_CHARS`; at most `MOST_OWN` from the own files while other files have
//!   candidates, the own files filling what is left.

use familiar_ai_facts::FixQuote;

use crate::teach::review::Changed;

/// The most sites listed.
pub const MOST_SITES: usize = 12;
/// The most sites from the place's own files (tier 0) while other files have candidates, so the
/// list reaches other files too (the owner, 2026-10-21) [draft name].
pub const MOST_OWN: usize = 6;
/// The most characters the list of sites takes in the prompt.
pub const MOST_CHARS: usize = 4_000;
/// A token found on more lines than this is too common to point at a sibling.
pub const COMMON: usize = 40;
/// The most tokens searched.
pub const MOST_TOKENS: usize = 60;
/// The fewest characters of a name, past leading and trailing `_`.
pub const LEAST_CORE: usize = 5;
/// The fewest characters of a path segment.
pub const LEAST_SEGMENT: usize = 4;
/// Files larger than this are not searched.
pub const MOST_FILE_BYTES: usize = 2_000_000;
/// Lines a quote may be from a listed line and still be at its site.
pub const NEAR: u64 = 2;

/// What the fix names: an identifier, matched as a whole word, or a path segment `/x/`, matched
/// as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Name(String),
    Fragment(String),
}

impl Token {
    pub fn text(&self) -> &str {
        match self {
            Token::Name(s) | Token::Fragment(s) => s,
        }
    }
}

/// One sibling site at the green commit.
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    /// The path from the repository's root.
    pub file: String,
    /// The line shown (1-based).
    pub line: u64,
    /// That line's text.
    pub text: String,
    /// The definition the site is in: its line (1-based) and text.
    pub def: Option<(u64, String)>,
    /// The definition's last line; the site's own line when it is in none.
    pub def_end: u64,
    /// The tokens found at the site.
    pub names: Vec<String>,
}

/// Words that name nothing of their own: keywords, builtins and the commonest library names.
pub const STOP: &[&str] = &[
    "TYPE_CHECKING", "NotImplemented", "NotImplementedError", "ValueError", "TypeError", "KeyError", "AttributeError", "RuntimeError",
    "IndexError", "AssertionError", "ImportError", "Exception", "BaseException", "DeprecationWarning", "UserWarning", "isinstance",
    "issubclass", "staticmethod", "classmethod", "property", "dataclass", "dataclasses", "functools", "typing_extensions", "collections",
    "to_string", "to_owned", "as_str", "as_ref", "unwrap_or", "unwrap_or_default", "unwrap_or_else", "map_err", "and_then", "is_some",
    "is_none", "into_iter", "iter_mut", "ok_or_else", "from_utf8", "assert_eq", "debug_assert", "println", "eprintln", "console", "module_exports",
    "__init__", "__name__", "__class__", "__dict__", "__module__", "__qualname__", "__all__", "__main__",
];

/// A line (1-based) and the indices of the tokens found on it.
type Found = (u64, Vec<usize>);
/// A site's rank: its tier, the most tokens first, the rarest token, its path and line.
type Rank = (u8, std::cmp::Reverse<usize>, usize, String, u64);

/// The words before a definition's name.
const DEF_WORDS: &[&str] = &["def", "fn", "func", "function", "class", "struct", "enum", "trait", "interface", "impl", "module"];
/// The words a definition's line may start with before them.
const MODIFIERS: &[&str] =
    &["pub", "crate", "super", "async", "export", "default", "static", "public", "private", "protected", "abstract", "final", "unsafe", "extern", "override", "inline", "const"];

fn ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The identifiers of `line` with their byte offsets: runs of letters, digits and `_` that do not
/// start with a digit.
fn identifiers(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut prev_ident = false;
    for (i, c) in line.char_indices().chain(std::iter::once((line.len(), ' '))) {
        let is = ident_char(c);
        match start {
            Some(s) if !is => {
                out.push((s, &line[s..i]));
                start = None;
            }
            None if is && !prev_ident && !c.is_ascii_digit() => start = Some(i),
            _ => {}
        }
        prev_ident = is;
    }
    out
}

/// Whether a word reads as a name of its own: its core (without leading and trailing `_`) holds
/// at least `LEAST_CORE` characters and an `_` inside or a lower-to-upper hump, and it is no stop
/// word.
fn distinctive(word: &str) -> bool {
    let core = word.trim_matches('_');
    let hump = core.as_bytes().windows(2).any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase());
    core.len() >= LEAST_CORE && (core.contains('_') || hump) && !STOP.contains(&word)
}

/// The name a line defines: the word after `def`, `fn`, `class` and the like (modifiers before
/// them allowed), or a shell function's `name() {`; `None` for any other line.
pub fn def_name(line: &str) -> Option<String> {
    let t = line.trim_start();
    let words = identifiers(t);
    for (k, (at, w)) in words.iter().enumerate() {
        if MODIFIERS.contains(w) {
            continue;
        }
        if DEF_WORDS.contains(w) {
            return words.get(k + 1).map(|(_, n)| n.to_string());
        }
        let shell = *at == 0 && t[at + w.len()..].trim_start().starts_with("()");
        return shell.then(|| w.to_string());
    }
    None
}

/// The definition line `at` (0-based) is in: itself when it defines a name, else the nearest
/// enclosing line that does.
fn def_of(lines: &[String], at: usize) -> Option<usize> {
    if def_name(&lines[at]).is_some() {
        return Some(at);
    }
    let mut up = crate::teach::review::enclosing(lines, at);
    while let Some(p) = up {
        if def_name(&lines[p]).is_some() {
            return Some(p);
        }
        // the closing line of a signature written on several lines is its definition's
        if let Some(d) = header_of(lines, p) {
            return Some(d);
        }
        up = crate::teach::review::enclosing(lines, p);
    }
    None
}

/// The definition whose header line `p` (0-based) closes: `p` opens with a closing bracket, and
/// above it, past lines indented deeper (the signature's own) and lines at its indent that open
/// with a closing bracket too (a return annotation written on several lines), is a line at its
/// indent that defines a name. `None` for any other line.
fn header_of(lines: &[String], p: usize) -> Option<usize> {
    if !lines[p].trim_start().starts_with([')', ']']) {
        return None;
    }
    let own = crate::teach::review::indent(&lines[p])?;
    for i in (0..p).rev() {
        match crate::teach::review::indent(&lines[i]) {
            None => continue,
            Some(x) if x > own => continue,
            Some(x) if x == own && def_name(&lines[i]).is_some() => return Some(i),
            Some(x) if x == own && lines[i].trim_start().starts_with([')', ']']) => continue,
            Some(_) => return None,
        }
    }
    None
}

/// The definition line `at` (0-based) is in, as `def_of` finds it.
pub(crate) fn def_line_of(lines: &[String], at: usize) -> Option<usize> {
    def_of(lines, at)
}

/// The path segments between two `/` of each word of `line`, as `/segment/`: at least
/// `LEAST_SEGMENT` characters of letters, digits, `.`, `-` and `_`, one a letter.
fn fragments(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in line.split_whitespace().filter(|w| w.contains('/')) {
        let parts: Vec<&str> = word.split('/').collect();
        for seg in parts.iter().take(parts.len().saturating_sub(1)).skip(1) {
            let ok = seg.len() >= LEAST_SEGMENT && seg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) && seg.chars().any(|c| c.is_ascii_alphabetic());
            if ok {
                out.push(format!("/{seg}/"));
            }
        }
    }
    out
}

/// Whether `path` is searched and read: not under a `tests` or `test` folder, not Familiar's own.
fn searched(path: &str) -> bool {
    let folders: Vec<&str> = path.split('/').collect();
    !crate::door::commits::is_familiars_own(path) && !folders[..folders.len().saturating_sub(1)].iter().any(|f| matches!(*f, "tests" | "test"))
}

fn lines_of(bytes: &[u8]) -> Vec<String> {
    familiar_ai_git::split_lines(bytes).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect()
}

/// The tokens the fix names, in order: definition names, then names, then path segments.
pub fn tokens(changed: &[Changed]) -> Vec<Token> {
    let (mut defs, mut names, mut frags) = (Vec::new(), Vec::new(), Vec::new());
    for c in changed.iter().filter(|c| searched(&c.path)) {
        let (old, new) = (c.before.as_deref().unwrap_or(b""), c.after.as_deref().unwrap_or(b""));
        if crate::teach::review::binary(old) || crate::teach::review::binary(new) {
            continue;
        }
        let (ol, nl) = (lines_of(old), lines_of(new));
        let mut touched: Vec<&str> = Vec::new();
        for h in familiar_ai_git::line_hunks(old, new) {
            let side = |lines: &'_ [String], from: u64, n: u64| -> Vec<usize> { (from..from + n).filter_map(|k| usize::try_from(k).ok()?.checked_sub(1)).filter(|&k| k < lines.len()).collect() };
            touched.extend(side(&ol, h.old_start, h.old_count).into_iter().map(|k| ol[k].as_str()));
            touched.extend(side(&nl, h.new_start, h.new_count).into_iter().map(|k| nl[k].as_str()));
            if !nl.is_empty() {
                let at = usize::try_from(h.new_start.saturating_sub(1)).unwrap_or(0).min(nl.len() - 1);
                if let Some(name) = def_of(&nl, at).and_then(|d| def_name(&nl[d])) {
                    if name.trim_matches('_').len() >= LEAST_CORE && !STOP.contains(&name.as_str()) {
                        defs.push(name);
                    }
                }
            }
        }
        for l in touched {
            names.extend(identifiers(l).into_iter().map(|(_, w)| w).filter(|w| distinctive(w)).map(str::to_string));
            frags.extend(fragments(l));
        }
    }
    let mut out: Vec<Token> = Vec::new();
    for t in defs.into_iter().chain(names).map(Token::Name).chain(frags.into_iter().map(Token::Fragment)) {
        if !out.iter().any(|o| o.text() == t.text()) && out.len() < MOST_TOKENS {
            out.push(t);
        }
    }
    out
}

/// Whether `line` holds `t`: a name as a whole word, a segment as written.
fn holds(line: &str, t: &Token) -> bool {
    match t {
        Token::Fragment(f) => line.contains(f.as_str()),
        Token::Name(n) => line.match_indices(n.as_str()).any(|(i, _)| {
            let before = line[..i].chars().next_back().is_none_or(|c| !ident_char(c));
            let after = line[i + n.len()..].chars().next().is_none_or(|c| !ident_char(c));
            before && after
        }),
    }
}

/// How near `path` is to the fix: 0 a changed file, 1 in a changed file's folder, 2 in its top
/// folder, 3 elsewhere.
fn tier(path: &str, fixed: &[&str]) -> u8 {
    let folder = |p: &str| p.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    let top = |p: &str| p.split_once('/').map(|(t, _)| t.to_string());
    if fixed.contains(&path) {
        0
    } else if fixed.iter().any(|f| folder(f) == folder(path)) {
        1
    } else if top(path).is_some() && fixed.iter().any(|f| top(f) == top(path)) {
        2
    } else {
        3
    }
}

/// The last line (0-based) of the definition `d` opens that is not blank.
fn last_line(lines: &[String], d: usize) -> usize {
    let mut e = crate::teach::review::block_end(lines, d);
    while e > d && lines[e].trim().is_empty() {
        e -= 1;
    }
    e
}

/// One planned place: its file, its symbol and its lines (1-based, inclusive; 0 when unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub file: String,
    pub symbol: String,
    pub start: u64,
    pub end: u64,
}

/// The lines of `p` in `blobs`, 0-based indices, within the file.
fn range_of(p: &Planned, lines: &[String]) -> std::ops::Range<usize> {
    if p.start == 0 || p.end < p.start {
        return 0..0;
    }
    let a = usize::try_from(p.start - 1).unwrap_or(usize::MAX).min(lines.len());
    let z = usize::try_from(p.end).unwrap_or(usize::MAX).min(lines.len());
    a..z.max(a)
}

/// What the planned places name, before any fix (the place lister, the owner's plan of
/// 2026-10-21, step 3) [draft]: each place's symbol, its last identifier taken as it is, exempt
/// from `LEAST_CORE` and `distinctive` (the identifiers before it, a class or a module, pass
/// them like any); then the distinctive identifiers on the place's lines at the planning commit.
/// No path segments. At most `MOST_TOKENS`.
pub fn place_tokens(planned: &[Planned], blobs: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<Token> {
    let (mut symbols, mut names) = (Vec::new(), Vec::new());
    for p in planned {
        let ids: Vec<&str> = identifiers(&p.symbol).into_iter().map(|(_, w)| w).collect();
        if let Some((last, before)) = ids.split_last() {
            symbols.push(last.to_string());
            names.extend(before.iter().filter(|w| distinctive(w)).map(|w| w.to_string()));
        }
        let lines = blobs.get(&p.file).map(|b| lines_of(b)).unwrap_or_default();
        for l in &lines[range_of(p, &lines)] {
            names.extend(identifiers(l).into_iter().map(|(_, w)| w).filter(|w| distinctive(w)).map(str::to_string));
        }
    }
    let mut out: Vec<Token> = Vec::new();
    for t in symbols.into_iter().chain(names).map(Token::Name) {
        if !out.iter().any(|o| o.text() == t.text()) && out.len() < MOST_TOKENS {
            out.push(t);
        }
    }
    out
}

/// The sites that name what the planned places name, at the commit `sha` of `root` (the place
/// lister) [draft]: the search `list_tokens` makes, with `place_tokens`; never on a place's own
/// lines or the line defining what they are in, nor on the lines of `others` (the plan's other
/// parts' places, which their own parts plan); never in a file `skip` names (the tests); the
/// places' files are tier 0. Same ranking and caps. Empty when the commit cannot be read.
pub fn list_place(root: &std::path::Path, sha: &str, planned: &[Planned], others: &[Planned], skip: &dyn Fn(&str) -> bool) -> Vec<Site> {
    let Ok(blobs) = familiar_ai_git::blobs_at(root, sha) else {
        return Vec::new();
    };
    let toks = place_tokens(planned, &blobs);
    let mut own: Vec<(String, u64, u64)> = Vec::new();
    for p in planned {
        if p.start == 0 {
            continue;
        }
        own.push((p.file.clone(), p.start, p.end.max(p.start)));
        let lines = blobs.get(&p.file).map(|b| lines_of(b)).unwrap_or_default();
        let r = range_of(p, &lines);
        if r.start < lines.len() {
            if let Some(d) = def_of(&lines, r.start) {
                own.push((p.file.clone(), d as u64 + 1, d as u64 + 1));
            }
        }
    }
    own.extend(others.iter().filter(|p| p.start > 0).map(|p| (p.file.clone(), p.start, p.end.max(p.start))));
    let near: Vec<&str> = planned.iter().map(|p| p.file.as_str()).collect();
    list_tokens(&blobs, &toks, &own, &near, skip)
}

/// The sibling sites of the fix `changed` at the commit `green` of `root`, ranked and capped;
/// empty when the fix names nothing or the commit cannot be read.
pub fn list(root: &std::path::Path, green: &str, changed: &[Changed]) -> Vec<Site> {
    let toks = tokens(changed);
    if toks.is_empty() {
        return Vec::new();
    }
    let Ok(blobs) = familiar_ai_git::blobs_at(root, green) else {
        return Vec::new();
    };
    // the fix's own lines at the green commit: its hunks, and the line defining each function
    // a hunk is in
    let mut own: Vec<(String, u64, u64)> = Vec::new();
    for c in changed {
        let after = c.after.as_deref().unwrap_or(b"");
        let nl = lines_of(after);
        for h in familiar_ai_git::line_hunks(c.before.as_deref().unwrap_or(b""), after) {
            if h.new_count > 0 {
                own.push((c.path.clone(), h.new_start, h.new_start + h.new_count - 1));
            }
            if !nl.is_empty() {
                let at = usize::try_from(h.new_start.saturating_sub(1)).unwrap_or(0).min(nl.len() - 1);
                if let Some(d) = def_of(&nl, at) {
                    own.push((c.path.clone(), d as u64 + 1, d as u64 + 1));
                }
            }
        }
    }
    let fixed: Vec<&str> = changed.iter().map(|c| c.path.as_str()).filter(|p| searched(p)).collect();
    list_tokens(&blobs, &toks, &own, &fixed, &|_| false)
}

/// The sites naming `toks` in `blobs` (a commit's files), ranked and capped: never on a line of
/// `own` (a file and its lines, 1-based and inclusive), nor in a file `skip` names, nor in one
/// `searched` leaves out; `near` is tier 0, their folders 1, their top folders 2. The entry point
/// that takes tokens directly: the reviewer's search and the place lister share it.
pub fn list_tokens(blobs: &std::collections::BTreeMap<String, Vec<u8>>, toks: &[Token], own: &[(String, u64, u64)], near: &[&str], skip: &dyn Fn(&str) -> bool) -> Vec<Site> {
    if toks.is_empty() {
        return Vec::new();
    }
    // each line holding a token: the file, its lines, and per line the tokens' indices
    let mut found: Vec<(&str, Vec<String>, Vec<Found>)> = Vec::new();
    let mut count = vec![0usize; toks.len()];
    for (path, bytes) in blobs {
        if !searched(path) || skip(path) || bytes.len() > MOST_FILE_BYTES || crate::teach::review::binary(bytes) {
            continue;
        }
        let whole = String::from_utf8_lossy(bytes);
        let present: Vec<usize> = (0..toks.len()).filter(|&k| whole.contains(toks[k].text())).collect();
        if present.is_empty() {
            continue;
        }
        let lines = lines_of(bytes);
        let mut at: Vec<Found> = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let n = i as u64 + 1;
            if own.iter().any(|(p, a, b)| p == path && (*a..=*b).contains(&n)) {
                continue;
            }
            let here: Vec<usize> = present.iter().copied().filter(|&k| holds(l, &toks[k])).collect();
            for &k in &here {
                count[k] += 1;
            }
            if !here.is_empty() {
                at.push((n, here));
            }
        }
        if !at.is_empty() {
            found.push((path.as_str(), lines, at));
        }
    }
    let kept = |k: &usize| count[*k] <= COMMON;
    // group the lines into sites: one per definition, or lines within NEAR of each other in none
    let mut ranked: Vec<(Rank, Site)> = Vec::new();
    for (path, lines, at) in &found {
        let mut groups: Vec<(Option<usize>, Vec<Found>)> = Vec::new();
        for (n, ks) in at {
            let ks: Vec<usize> = ks.iter().copied().filter(kept).collect();
            if ks.is_empty() {
                continue;
            }
            let def = def_of(lines, (*n - 1) as usize);
            match groups.last_mut() {
                Some((d, g)) if *d == def && (def.is_some() || g.last().is_some_and(|(m, _)| n - m <= NEAR)) => g.push((*n, ks)),
                _ => groups.push((def, vec![(*n, ks)])),
            }
        }
        for (def, g) in groups {
            let Some((line, _)) = g.iter().fold(None::<&Found>, |best, x| match best {
                Some(b) if b.1.len() >= x.1.len() => Some(b),
                _ => Some(x),
            }) else {
                continue;
            };
            let mut ks: Vec<usize> = g.iter().flat_map(|(_, k)| k.iter().copied()).collect();
            ks.sort_unstable();
            ks.dedup();
            let rarest = ks.iter().map(|&k| count[k]).min().unwrap_or(usize::MAX);
            let site = Site {
                file: path.to_string(),
                line: *line,
                text: lines[(*line - 1) as usize].clone(),
                def: def.map(|d| (d as u64 + 1, lines[d].clone())),
                def_end: def.map(|d| last_line(lines, d) as u64 + 1).unwrap_or(*line),
                names: ks.iter().map(|&k| toks[k].text().to_string()).collect(),
            };
            ranked.push(((tier(path, near), std::cmp::Reverse(ks.len()), rarest, path.to_string(), *line), site));
        }
    }
    ranked.sort_by(|a, b| a.0.cmp(&b.0));
    // the own files' sites past MOST_OWN wait while other files have candidates, then fill what
    // is left of MOST_SITES; the chosen keep their rank
    let mut chosen: Vec<usize> = Vec::new();
    let mut waiting: Vec<usize> = Vec::new();
    for (i, (rank, _)) in ranked.iter().enumerate() {
        if chosen.len() == MOST_SITES {
            break;
        }
        if rank.0 == 0 && chosen.iter().filter(|&&j| ranked[j].0 .0 == 0).count() == MOST_OWN {
            waiting.push(i);
        } else {
            chosen.push(i);
        }
    }
    chosen.extend(waiting.into_iter().take(MOST_SITES.saturating_sub(chosen.len())));
    chosen.sort_unstable();
    let mut keep = vec![false; ranked.len()];
    for i in chosen {
        keep[i] = true;
    }
    let mut out: Vec<Site> = Vec::new();
    let mut chars = 0;
    for (_, site) in ranked.into_iter().zip(keep).filter(|(_, k)| *k).map(|(x, _)| x) {
        let shown = site_lines(out.len() + 1, &site).chars().count() + 1;
        if out.len() == MOST_SITES || chars + shown > MOST_CHARS {
            break;
        }
        chars += shown;
        out.push(site);
    }
    out
}

/// The most characters of a site's line shown.
const MOST_SITE_LINE: usize = 160;

/// Site `n` as it is shown: `n. file:line · names …`, its line numbered, and the line of the
/// definition it is in when that is another.
pub fn site_lines(n: usize, s: &Site) -> String {
    let short = |t: &str| crate::teach::judge::cut(t, MOST_SITE_LINE).replace('\n', " ");
    let numbered = crate::teach::review::numbered;
    let mut out = format!("{n}. {}:{} · names {}\n   {}", s.file, s.line, s.names.join(", "), numbered(' ', s.line, &short(&s.text)));
    if let Some((d, text)) = s.def.as_ref().filter(|(d, _)| *d != s.line) {
        out.push_str(&format!("\n   in{}", numbered(' ', *d, &short(text))));
    }
    out
}

/// Whether a quote found at the green commit lies at a listed site: in its file, and within
/// `NEAR` lines of its line or inside its definition.
pub fn at_listed(sites: &[Site], q: &FixQuote) -> bool {
    let last = q.line + (q.text.lines().count().max(1) as u64) - 1;
    sites.iter().any(|s| {
        s.file == q.file_path
            && ((q.line.saturating_sub(NEAR)..=last + NEAR).contains(&s.line) || s.def.as_ref().is_some_and(|(d, _)| *d <= q.line && q.line <= s.def_end))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository with `files` committed, then `edits` committed; the fix between them.
    fn repo(name: &str, files: &[(&str, &str)], edits: &[(&str, &str)]) -> (PathBuf, String, Vec<Changed>) {
        let dir = std::env::temp_dir().join(format!("familiar-ai-siblings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |(p, t): &(&str, &str)| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().expect("parent")).expect("dir");
            std::fs::write(f, t).expect("write");
        };
        std::fs::create_dir_all(&dir).expect("dir");
        git(&dir, &["init", "-q", "-b", "main"]);
        files.iter().for_each(put);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "tests first"]);
        edits.iter().for_each(put);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "fix"]);
        let green = git(&dir, &["rev-parse", "HEAD"]);
        let changed = edits
            .iter()
            .map(|(p, t)| Changed { path: p.to_string(), before: files.iter().find(|(q, _)| q == p).map(|(_, b)| b.as_bytes().to_vec()), after: Some(t.as_bytes().to_vec()) })
            .collect();
        (dir, green, changed)
    }

    const CONFIG: &str = "\"\"\"Settings.\"\"\"\n\n\nclass Config:\n    \"\"\"Holds the knobs.\"\"\"\n\n    def mode(self):\n        return self.out_mode\n\n    def describe(self):\n        # out_mode wins over legacy_mode when both are set\n        return \"out_mode\"\n\n\ndef helper():\n    return 1\n";

    fn shape(sites: &[Site]) -> Vec<(&str, u64, Option<u64>)> {
        sites.iter().map(|s| (s.file.as_str(), s.line, s.def.as_ref().map(|d| d.0))).collect()
    }

    #[test]
    fn the_fix_names_its_definition_its_own_names_and_its_path_segments_never_common_words() {
        let before = "def link_for(name):\n    if isinstance(name, str):\n        raise ValueError('see https://docs.example.dev/latest/guide/migration/')\n";
        let after = before.replace("'see https://docs.example.dev/latest/", "f'see https://docs.example.dev/{short_version()}/");
        let c = Changed { path: "src/links.py".into(), before: Some(before.as_bytes().to_vec()), after: Some(after.into_bytes()) };
        let names = |t: &[Token]| t.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>();
        assert_eq!(
            names(&tokens(&[c])),
            names(&[
                Token::Name("link_for".into()),
                Token::Name("short_version".into()),
                Token::Fragment("/docs.example.dev/".into()),
                Token::Fragment("/latest/".into()),
                Token::Fragment("/guide/".into()),
                Token::Fragment("/migration/".into()),
            ])
        );
        let test = Changed { path: "tests/test_links.py".into(), before: Some(b"a = 1\n".to_vec()), after: Some(b"a = some_test_value\n".to_vec()) };
        assert!(tokens(&[test]).is_empty(), "a changed test names nothing");
    }

    #[test]
    fn a_sibling_is_found_outside_the_fix_and_the_tests_ranked_same_file_then_folder_then_the_rest() {
        let noise: String = (0..=COMMON).map(|i| format!("busy_name_{} = busy_name\n", i % 3)).collect();
        let fixed = CONFIG.replace("return self.out_mode", "return self.legacy_mode or self.out_mode or busy_name");
        let (dir, green, changed) = repo(
            "list",
            &[
                ("src/pkg/config.py", CONFIG),
                ("src/pkg/other.py", "def legacy(cfg):\n    return cfg.legacy_mode\n"),
                ("src/pkg/noise.py", &noise),
                ("src/other/far.py", "X = \"out_mode\"\n"),
                ("lib/far.py", "Y = \"legacy_mode\"\n"),
                ("tests/test_config.py", "def test_mode():\n    assert Config().out_mode\n"),
                ("src/pkg/tests/test_more.py", "legacy_mode = 1\n"),
                (".familiar/plan", "out_mode legacy_mode\n"),
            ],
            &[("src/pkg/config.py", &fixed)],
        );
        let sites = list(&dir, &green, &changed);
        assert_eq!(
            shape(&sites),
            vec![("src/pkg/config.py", 11, Some(10)), ("src/pkg/other.py", 2, Some(1)), ("src/other/far.py", 1, None), ("lib/far.py", 1, None)],
            "the fix's own line 8 is not a site; tests and Familiar's own are not searched; busy_name is too common: {sites:#?}"
        );
        let first = &sites[0];
        assert_eq!(first.text, "        # out_mode wins over legacy_mode when both are set");
        assert_eq!(first.def, Some((10, "    def describe(self):".to_string())));
        assert_eq!(first.def_end, 12, "line 12 names out_mode too: one site per definition, shown at its line naming the most");
        assert_eq!(first.names, vec!["out_mode".to_string(), "legacy_mode".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_definition_the_fix_changed_is_its_own_never_a_sibling() {
        let before = "def apply_limit(value):\n    return value\n";
        let after = "def apply_limit(value):\n    return min(value, 10)\n";
        let (dir, green, changed) = repo("own-def", &[("src/limits.py", before), ("src/use.py", "from limits import apply_limit\n")], &[("src/limits.py", after)]);
        assert_eq!(tokens(&changed), vec![Token::Name("apply_limit".into())]);
        assert_eq!(shape(&list(&dir, &green, &changed)), vec![("src/use.py", 1, None)], "line 1 of src/limits.py defines what the fix changed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A class whose methods have a signature on several lines, a decorator, and `@overload` stubs.
    const METHODS: &str = "class Holder:
    def plain(
        self,
        knob_value,
    ) -> None:
        first = knob_value
        x = 1
        y = 2
        z = 3
        return knob_value

    @staticmethod
    def decorated(arg):
        return arg.knob_value

    @overload
    def picked(self, key: int) -> int: ...

    @overload
    def picked(self, key: str) -> str: ...

    def picked(
        self, key
    ) -> object:
        return self.knob_value
";

    #[test]
    fn a_line_in_a_method_whose_signature_spans_lines_or_that_is_decorated_or_overloaded_groups_to_that_method() {
        let lines: Vec<String> = METHODS.lines().map(str::to_string).collect();
        // 1-based: plain's body at 6 and 10 and its signature's own line 4; decorated's body at 14;
        // the real picked's body at 25
        for (at, def) in [(4, 2), (6, 2), (10, 2), (14, 13), (25, 22)] {
            assert_eq!(def_of(&lines, at - 1).map(|d| d + 1), Some(def), "line {at}: {}", lines[at - 1]);
        }
        let blobs: std::collections::BTreeMap<String, Vec<u8>> = [("src/holder.py".to_string(), METHODS.as_bytes().to_vec())].into_iter().collect();
        let sites = list_tokens(&blobs, &[Token::Name("knob_value".into())], &[], &["src/holder.py"], &|_| false);
        let mut got: Vec<(u64, Option<u64>)> = sites.iter().map(|s| (s.line, s.def.as_ref().map(|d| d.0))).collect();
        got.sort();
        assert_eq!(got, vec![(4, Some(2)), (14, Some(13)), (25, Some(22))], "one site per method, each in its own method: {sites:#?}");
    }

    #[test]
    fn a_line_in_a_method_whose_return_annotation_spans_lines_groups_to_that_method() {
        let src = "class Holder:
    def infer_it(
        self, context=None
    ) -> Generator[
        Result, None, Info
    ]:
        for operand in self.operand.infer(context):
            yield knob_value
";
        let lines: Vec<String> = src.lines().map(str::to_string).collect();
        assert_eq!(def_of(&lines, 7).map(|d| d + 1), Some(2), "line 8: {}", lines[7]);
    }

    /// `n` one-line functions in `file`, each naming `knob_value`.
    fn knobs(file: &str, n: usize) -> (String, Vec<u8>) {
        (file.to_string(), (0..n).map(|i| format!("def f{i}():\n    return knob_value\n\n\n")).collect::<String>().into_bytes())
    }

    #[test]
    fn at_most_six_sites_come_from_the_places_own_file_while_other_files_have_candidates() {
        let near = ["src/own.py"];
        let toks = [Token::Name("knob_value".into())];
        let count = |sites: &[Site]| sites.iter().filter(|s| s.file == "src/own.py").count();
        // the own file has 15 candidates, the others 8: six of its own, then six of the others
        let blobs: std::collections::BTreeMap<String, Vec<u8>> = [knobs("src/own.py", 15), knobs("src/a.py", 4), knobs("lib/b.py", 4)].into_iter().collect();
        let sites = list_tokens(&blobs, &toks, &[], &near, &|_| false);
        assert_eq!((sites.len(), count(&sites)), (MOST_SITES, MOST_OWN), "{sites:#?}");
        assert!(sites[..MOST_OWN].iter().all(|s| s.file == "src/own.py"), "the ranking keeps the own file first: {sites:#?}");
        // the others have only two: the own file fills the rest, so the list stays full
        let blobs: std::collections::BTreeMap<String, Vec<u8>> = [knobs("src/own.py", 15), knobs("src/a.py", 2)].into_iter().collect();
        let sites = list_tokens(&blobs, &toks, &[], &near, &|_| false);
        assert_eq!((sites.len(), count(&sites)), (MOST_SITES, MOST_SITES - 2), "{sites:#?}");
    }

    #[test]
    fn at_most_twelve_sites_are_listed() {
        let mut files: Vec<(String, String)> = (0..15).map(|i| (format!("src/m{i:02}.py"), "rare_knob = 1\n".to_string())).collect();
        files.push(("src/main.py".into(), "def run():\n    return 1\n".into()));
        let files: Vec<(&str, &str)> = files.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        let (dir, green, changed) = repo("cap", &files, &[("src/main.py", "def run():\n    return rare_knob\n")]);
        let sites = list(&dir, &green, &changed);
        assert_eq!(sites.len(), MOST_SITES, "{sites:#?}");
        assert_eq!(sites[0].file, "src/m00.py", "same folder, then by path");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_quote_is_at_a_listed_site_within_two_lines_of_it_or_inside_its_definition() {
        let site = |file: &str, line, def: Option<u64>, def_end| Site { file: file.into(), line, text: String::new(), def: def.map(|d| (d, String::new())), def_end, names: vec![] };
        let sites = [site("a.py", 10, Some(5), 20), site("b.py", 40, None, 40)];
        let q = |file: &str, line| FixQuote { file_path: file.into(), line, text: "x".into() };
        assert!(at_listed(&sites, &q("a.py", 12)));
        assert!(at_listed(&sites, &q("a.py", 8)));
        assert!(at_listed(&sites, &q("a.py", 18)), "inside its definition");
        assert!(at_listed(&sites, &q("a.py", 5)), "its definition's own line");
        assert!(!at_listed(&sites, &q("a.py", 21)));
        assert!(!at_listed(&sites, &q("a.py", 4)));
        assert!(at_listed(&sites, &q("b.py", 42)));
        assert!(!at_listed(&sites, &q("b.py", 43)), "in no definition: two lines at most");
        assert!(!at_listed(&sites, &q("c.py", 10)), "another file");
        assert!(!at_listed(&[], &q("a.py", 10)), "no site listed, none is at one");
    }
}
