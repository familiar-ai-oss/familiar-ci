//! The "words" ranker of `familiar-ai try`: rank the functions a failing test
//! ran by word match with the person's request.
//!
//! Ports the experiment's Python (anchor-check `fn_rank.py`: `tokenize` and
//! `bm25_ranks`, k1 = 1.2, b = 0.75; `rank_c.py`: `extract_fn`'s 40-line and
//! 2000-character caps) and finds each function's lines textually, the way
//! `ast`'s ranges do: from the function's first line (in Python 3.8+ a
//! decorated function's first line is its first decorator) to the last
//! non-blank line of its indented body.

use std::collections::HashMap;

/// fn_rank.py's `K1` and `B`.
const K1: f64 = 1.2;
const B: f64 = 0.75;

/// fn_rank.py's 60 stopwords, in its order (a set in Python; the order does
/// not change the tokenizing).
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "if", "then", "else", "when",
    "while", "for", "to", "of", "in", "on", "at", "by", "with", "from",
    "into", "onto", "about", "as", "is", "are", "was", "were", "be", "been",
    "being", "do", "does", "did", "done", "have", "has", "had", "having", "all",
    "it", "its", "this", "that", "these", "those", "you", "they", "them",
    "we", "us", "not", "no", "so", "up", "down", "out", "off", "over",
    "under",
];

/// `extract_fn`'s caps, in its order: at most 40 lines, then at most 2000
/// characters.
const MAX_LINES: usize = 40;
const MAX_CHARS: usize = 2000;

/// One function the failing test ran (the kit's calls).
#[derive(Debug, Clone, PartialEq)]
pub struct Called {
    /// The function's file, relative to the project root, forward slashes.
    pub file: String,
    /// The function's bare name.
    pub symbol: String,
    /// The line the trace recorded (1-based): a decorated function's first
    /// line is its first decorator.
    pub def_line: u32,
    /// When the test last called it; the recency order, higher = later.
    pub last_call: u64,
}

/// A called function's place in the rank.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    /// The 1-based position: the score-positive functions first (score
    /// descending, ties by `file:symbol:def_line`), the rest after (last_call
    /// descending, ties by the same key).
    pub rank: u32,
    pub file: String,
    pub symbol: String,
    /// The function's first line (1-based; a decorator's when it has one).
    pub line_start: u32,
    /// The last non-blank line of the function's indented body (1-based);
    /// `def_line` when the span was not found.
    pub line_end: u32,
}

/// fn_rank.py's `tokenize`: the runs of `[A-Za-z0-9_]+`, lowercased; the whole
/// identifier kept as a token, then the identifier's `_`-parts each split at
/// camelCase boundaries with `[A-Z]+(?![a-z])|[A-Z][a-z]*|[a-z]+|[0-9]+`
/// (hand-written in [`camel_pieces`]; the split runs on the already lowercased
/// identifier, as in Python, so only the lowercase and digit alternatives can
/// match); tokens shorter than 2 characters and the stopwords dropped, in the
/// same order and set; duplicates kept (they count as term frequency).
pub fn tokenize(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        // one run of [A-Za-z0-9_]+ (WORD_RE.finditer); a byte no run takes is skipped
        let start = i;
        while i < b.len() && is_word_byte(b[i]) {
            i += 1;
        }
        if i == start {
            i += 1;
            continue;
        }
        let ident: String = text[start..i].to_ascii_lowercase();
        if ident.len() >= 2 && !stopword(&ident) {
            out.push(ident.clone());
        }
        for part in ident.split('_') {
            for piece in camel_pieces(part) {
                // the pieces are already lowercase (the identifier was lowercased
                // before the split), as in Python's `piece.lower()`
                if piece.len() >= 2 && !stopword(&piece) {
                    out.push(piece);
                }
            }
        }
    }
    out
}

/// A byte WORD_RE takes: one of `[A-Za-z0-9_]`.
fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Whether `w` is one of the [`STOPWORDS`].
fn stopword(w: &str) -> bool {
    STOPWORDS.contains(&w)
}

/// fn_rank.py's `CAMEL_RE.findall` — `[A-Z]+(?![a-z])|[A-Z][a-z]*|[a-z]+|[0-9]+` —
/// hand-written: at each position the alternatives are tried in that order.
/// An `[A-Z]+` run followed by a lowercase letter keeps all but its last
/// letter (the shorter run's lookahead, the next letter, is not a lowercase);
/// a lone uppercase the first alternative rejects is taken by `[A-Z][a-z]*`
/// with the lowercase run after it.
fn camel_pieces(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_uppercase() {
            let mut j = i;
            while j < b.len() && b[j].is_ascii_uppercase() {
                j += 1;
            }
            if j == b.len() || !b[j].is_ascii_lowercase() {
                // [A-Z]+(?![a-z]) matches the whole run
                out.push(s[i..j].to_string());
                i = j;
            } else if j - i >= 2 {
                // [A-Z]+(?![a-z]) backtracks to the run minus its last letter
                out.push(s[i..j - 1].to_string());
                i = j - 1;
            } else {
                // [A-Z][a-z]*: the uppercase and the lowercase run after
                let mut k = j;
                while k < b.len() && b[k].is_ascii_lowercase() {
                    k += 1;
                }
                out.push(s[i..k].to_string());
                i = k;
            }
        } else if c.is_ascii_lowercase() {
            let mut j = i;
            while j < b.len() && b[j].is_ascii_lowercase() {
                j += 1;
            }
            out.push(s[i..j].to_string());
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            out.push(s[i..j].to_string());
            i = j;
        } else {
            // a byte no alternative matches; findall leaves it
            i += 1;
        }
    }
    out
}

/// A string the header scan is inside (the quotes can span lines).
#[derive(Clone, Copy, PartialEq)]
enum StrState {
    None,
    Single,
    Double,
    TripleSingle,
    TripleDouble,
}

/// One line of a (possibly multi-line) def header, continuing the bracket
/// `depth` and the `state` from the lines before it. Returns (depth, state,
/// the header ended on this line): the header's terminating `:` is the one at
/// bracket depth 0 (the brackets of a multi-line signature keep the depth
/// above 0), a `#` (outside a string) ends the code, and a string's own
/// brackets and `:`s are not counted.
fn scan_header(line: &str, mut depth: i32, mut state: StrState) -> (i32, StrState, bool) {
    let b = line.as_bytes();
    let mut i = 0;
    let mut ended = false;
    while i < b.len() {
        let c = b[i];
        match state {
            StrState::None => {
                if c == b'\\' {
                    // the continuation keeps the line open; not significant
                    i += 2;
                    continue;
                }
                if c == b'#' {
                    break; // the rest of the line is a comment
                }
                if c == b'\'' && b[i..].starts_with(b"'''") {
                    state = StrState::TripleSingle;
                    i += 3;
                    continue;
                }
                if c == b'"' && b[i..].starts_with(b"\"\"\"") {
                    state = StrState::TripleDouble;
                    i += 3;
                    continue;
                }
                if c == b'\'' {
                    state = StrState::Single;
                    i += 1;
                    continue;
                }
                if c == b'"' {
                    state = StrState::Double;
                    i += 1;
                    continue;
                }
                if c == b':' && depth == 0 {
                    ended = true; // the header's terminating ':'
                }
                if matches!(c, b'(' | b'[' | b'{') {
                    depth += 1;
                } else if matches!(c, b')' | b']' | b'}') {
                    depth = depth.saturating_sub(1);
                }
                i += 1;
            }
            StrState::Single => {
                if c == b'\\' {
                    i += 2; // an escaped quote or line continuation
                    continue;
                }
                if c == b'\'' {
                    state = StrState::None;
                }
                i += 1;
            }
            StrState::Double => {
                if c == b'\\' {
                    i += 2; // an escaped quote or line continuation
                    continue;
                }
                if c == b'"' {
                    state = StrState::None;
                }
                i += 1;
            }
            StrState::TripleSingle => {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if b[i..].starts_with(b"'''") {
                    state = StrState::None;
                    i += 3;
                    continue;
                }
                i += 1;
            }
            StrState::TripleDouble => {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if b[i..].starts_with(b"\"\"\"") {
                    state = StrState::None;
                    i += 3;
                    continue;
                }
                i += 1;
            }
        }
    }
    (depth, state, ended)
}

/// Whether the line's code (after its leading whitespace) starts a `def` or
/// `class` header.
fn is_def_line(t: &str) -> bool {
    t.starts_with("def ") || t.starts_with("async def ") || t.starts_with("class ")
}

/// The function or class that starts at `def_line` (1-based), as (first line,
/// last line), both 1-based. In Python 3.8+ a decorated function's first line
/// is its first decorator, so the decorator lines between `def_line` and the
/// `def` / `async def` / `class` line are skipped. The span ends at the last
/// non-blank line of the indented body: a multi-line signature ends at the
/// line where the brackets close and the line ends with `:`, and comment-only
/// and blank lines do not end the body. `None` when no `def` / `async def` /
/// `class` line sits at or just below `def_line`.
pub fn span(src: &str, def_line: u32) -> Option<(u32, u32)> {
    if def_line < 1 {
        return None; // not 1-based
    }
    let lines: Vec<&str> = src.split('\n').collect();
    let mut i = def_line as usize - 1; // 1-based to 0-based
    if i >= lines.len() {
        return None;
    }
    // skip the decorator lines down to the def line
    loop {
        let t = lines[i].trim_start();
        if is_def_line(t) {
            break;
        }
        if !t.starts_with('@') {
            return None;
        }
        i += 1;
        if i >= lines.len() {
            return None;
        }
    }
    let def_idx = i;
    // the signature: the def line, or the lines until the header's ':' (the
    // one at bracket depth 0; a multi-line signature's open brackets keep
    // the depth above 0, so only the closing line's ':' is at depth 0)
    let mut depth = 0i32;
    let mut state = StrState::None;
    let mut i = def_idx;
    loop {
        let (d, s, ended) = scan_header(lines[i], depth, state);
        depth = d;
        state = s;
        if ended {
            break;
        }
        i += 1;
        if i >= lines.len() {
            return None; // the header never ended
        }
    }
    // the body: the lines indented past the def line, until a non-blank,
    // non-comment line at the def's indent or above; a comment-only line
    // never ends it, and a blank line never does either
    let def_ws = lines[def_idx].len() - lines[def_idx].trim_start().len();
    let mut end = i; // no body line: the last line of the signature
    let mut i = i + 1;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if !t.is_empty() {
            let ws = lines[i].len() - t.len();
            let comment = t.starts_with('#');
            if !comment && ws <= def_ws {
                break;
            }
            if ws > def_ws {
                end = i;
            }
        }
        i += 1;
    }
    Some((def_line, end as u32 + 1))
}

/// `extract_fn`'s text: the span's lines, at most 40 lines and, then, at most
/// 2000 characters (the caps in their order).
pub fn text_of(src: &str, span: (u32, u32)) -> String {
    let (start, end) = span;
    let lines: Vec<&str> = src.split('\n').collect();
    let s = start as usize;
    if s < 1 || s > lines.len() {
        return String::new();
    }
    let e = (end as usize).min(lines.len());
    if e < s {
        return String::new();
    }
    let last = (e - 1).min(s - 1 + MAX_LINES - 1);
    let seg: Vec<&str> = lines[s - 1..=last].to_vec();
    let mut text = seg.join("\n");
    if text.chars().count() > MAX_CHARS {
        let at = text.char_indices().nth(MAX_CHARS).map(|(i, _)| i).unwrap_or(text.len());
        text.truncate(at);
    }
    text
}

/// Rank the called functions against the request: BM25 of the query's tokens
/// over the functions' token bags (fn_rank.py's `bm25_ranks`: the repeated
/// query tokens accumulate, idf = ln((n - df + 0.5) / (df + 0.5)), a negative
/// idf allowed). The score-positive functions come first, score descending;
/// the rest come after, last_call descending; ties by `file:symbol:def_line`
/// (ascending). Every called function appears exactly once, at its 1-based
/// rank. A function whose file cannot be read, or whose span is not found,
/// keeps line_start = line_end = def_line and an empty text (an empty token
/// bag).
pub fn rank_words(query: &str, called: &[Called], read: impl Fn(&str) -> Option<String>) -> Vec<Ranked> {
    let n = called.len();
    if n == 0 {
        return Vec::new();
    }
    // each function's token bag; each file read (and every failure
    // remembered) once
    let mut files: HashMap<String, Option<String>> = HashMap::new();
    let mut tfs: Vec<HashMap<String, usize>> = Vec::with_capacity(n);
    let mut lens: Vec<u32> = Vec::with_capacity(n);
    let mut spans: Vec<(u32, u32)> = Vec::with_capacity(n);
    let mut has_text: Vec<bool> = Vec::with_capacity(n);
    for c in called {
        let src = files.entry(c.file.clone()).or_insert_with(|| read(&c.file));
        // a function whose body is not found has no text, and, as in the measured ranker (its
        // extract_fn gave None), is left out of the word statistics too
        let found = src.as_deref().and_then(|s| span(s, c.def_line).map(|sp| (s, sp)));
        spans.push(found.map_or((c.def_line, c.def_line), |(_, sp)| sp));
        has_text.push(found.is_some());
        let tokens = match found {
            Some((s, sp)) => tokenize(&text_of(s, sp)),
            None => Vec::new(),
        };
        let mut tf: HashMap<String, usize> = HashMap::new();
        for t in &tokens {
            *tf.entry(t.clone()).or_insert(0) += 1;
        }
        lens.push(tokens.len() as u32);
        tfs.push(tf);
    }
    // fn_rank.py's bm25_ranks over the token bags
    let mut df: HashMap<String, usize> = HashMap::new();
    for (i, tf) in tfs.iter().enumerate() {
        if has_text[i] {
            for t in tf.keys() {
                *df.entry(t.clone()).or_insert(0) += 1;
            }
        }
    }
    let docs = has_text.iter().filter(|h| **h).count();
    let nf = docs as f64;
    let avgdl = if docs == 0 { 0.0 } else { (0..n).filter(|&i| has_text[i]).map(|i| lens[i]).sum::<u32>() as f64 / nf };
    let idf: HashMap<String, f64> = df
        .iter()
        .map(|(t, c)| (t.clone(), ((nf - *c as f64 + 0.5) / (*c as f64 + 0.5)).ln()))
        .collect();
    let mut scores = vec![0.0f64; n];
    for q in tokenize(query) {
        if let Some(w) = idf.get(&q) {
            for i in 0..n {
                // fn_rank.py's `if f:`: only a present term is scored
                if let Some(f) = tfs[i].get(&q).copied().filter(|f| *f > 0) {
                    let f = f as f64;
                    let len = lens[i] as f64;
                    scores[i] += w * f * (K1 + 1.0) / (f + K1 * (1.0 - B + B * len / avgdl));
                }
            }
        }
    }
    // the score-positive functions first (score descending), the rest after
    // (last_call descending), ties by `file:symbol:def_line`
    let key = |i: usize| format!("{}:{}:{}", called[i].file, called[i].symbol, called[i].def_line);
    let mut pos: Vec<usize> = (0..n).filter(|&i| scores[i] > 0.0).collect();
    pos.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then_with(|| key(a).cmp(&key(b))));
    let mut rest: Vec<usize> = (0..n).filter(|&i| scores[i] <= 0.0).collect();
    rest.sort_by(|&a, &b| called[b].last_call.cmp(&called[a].last_call).then_with(|| key(a).cmp(&key(b))));
    let mut order = pos;
    order.extend(rest);
    order
        .into_iter()
        .enumerate()
        .map(|(i, idx)| Ranked {
            rank: i as u32 + 1,
            file: called[idx].file.clone(),
            symbol: called[idx].symbol.clone(),
            line_start: spans[idx].0,
            line_end: spans[idx].1,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The expected vectors are pasted verbatim from fn_rank.py's `tokenize`
    /// (standard library only), run on 12 varied strings (camelCase,
    /// snake_case, digits, punctuation, stopwords, a code line) by:
    ///
    ///     python3 -c "import sys; sys.path.insert(0, '/var/tmp/familiar-lanes/anchor-check');
    ///         import fn_rank; [print(repr(t), '->', fn_rank.tokenize(t)) for t in [...]]"
    #[test]
    fn tokenize_matches_the_python() {
        let cases: [(&str, &[&str]); 12] = [
            ("camelCase", &["camelcase", "camelcase"]),
            ("snake_case_example", &["snake_case_example", "snake", "case", "example"]),
            ("HTTPServer", &["httpserver", "httpserver"]),
            ("getHTTPResponse2", &["gethttpresponse2", "gethttpresponse"]),
            ("to be or not to be", &[]),
            ("foo-bar (baz) 42??", &["foo", "foo", "bar", "bar", "baz", "baz", "42", "42"]),
            ("item42abc", &["item42abc", "item", "42", "abc"]),
            ("the quick-brown Fox_2024", &["quick", "quick", "brown", "brown", "fox_2024", "fox", "2024"]),
            ("def fetch_cache(key):\n    value = cache.get(key)", &["def", "def", "fetch_cache", "fetch", "cache", "key", "key", "value", "value", "cache", "cache", "get", "get", "key", "key"]),
            ("a1B2c", &["a1b2c"]),
            ("", &[]),
            ("x", &[]),
        ];
        for (text, want) in cases {
            let tokens = tokenize(text);
            let got: Vec<&str> = tokens.iter().map(|t| t.as_str()).collect();
            assert_eq!(got, want);
        }
    }

    /// The spans the text heuristic gives (1-based); the well-formed fixtures
    /// also match the `ast` ranges (checked with python3.14's `ast.parse`:
    /// the ends below are the `end_lineno`s, and a decorated function's
    /// first line is its first decorator).
    #[test]
    fn span_finds_the_function() {
        // a plain def
        let plain = "def plain(x):\n    y = x + 1\n    return y\n";
        assert_eq!(span(plain, 1), Some((1, 3)));

        // a decorated function's first line is its first decorator
        let decorated = "@a\n@b\ndef multi(x):\n    return x\n";
        assert_eq!(span(decorated, 1), Some((1, 4)));
        assert_eq!(span(decorated, 3), Some((3, 4)));

        // an async def
        let async_def = "async def aw(x):\n    await x\n";
        assert_eq!(span(async_def, 1), Some((1, 2)));

        // a method in a class (indented), and the class itself
        let method = "class Outer:\n    def method(self, x):\n        return self.value + x\n    def other(self):\n        return 0\n";
        assert_eq!(span(method, 2), Some((2, 3)));
        assert_eq!(span(method, 1), Some((1, 5)));

        // a multi-line signature: it ends where the brackets close and the
        // line ends with ':'
        let multiline = "def split(\n    a,\n    b,\n    c: int = 0,\n) -> int:\n    return a + b + c\n";
        assert_eq!(span(multiline, 1), Some((1, 6)));

        // a body with blank lines and comments; the comment-only and blank
        // lines do not end it
        let blanks = "def body(x):\n    # the first note\n    y = x\n\n    # a mid note\n    return y\n";
        assert_eq!(span(blanks, 1), Some((1, 6)));

        // a comment at the def's indent does not end the body
        let outside = "def tail(x):\n    y = 1\n# an outside note\ndef after(x):\n    return 0\n";
        assert_eq!(span(outside, 1), Some((1, 2)));

        // a one-line body, a class, and a def at the end of the file
        let one_line = "def one(x): return x\ndef two(x):\n    return x\n";
        assert_eq!(span(one_line, 1), Some((1, 1)));
        let class = "class K:\n    x = 1\n    def m(self):\n        return self.x\n";
        assert_eq!(span(class, 1), Some((1, 4)));
        let eof = "def last(x):\n    return x";
        assert_eq!(span(eof, 1), Some((1, 2)));

        // a comment after the header's ':' does not stop the header, and a
        // bracket inside the header's string is not counted
        let commented = "def f():  # a note\n    return 1\n";
        assert_eq!(span(commented, 1), Some((1, 2)));
        let quoted = "def quoted(\n    a='x) y',\n):\n    return a\n";
        assert_eq!(span(quoted, 1), Some((1, 4)));
    }

    #[test]
    fn span_none_when_there_is_no_def() {
        assert_eq!(span("x = 1\ny = 2\n", 1), None);
        assert_eq!(span("@deco\nx = 1\n", 1), None); // a decorator with no def after
        assert_eq!(span("def f(x):\n    return x\n", 99), None); // past the end
        assert_eq!(span("def f(x):\n    return x\n", 0), None); // not 1-based
        assert_eq!(span("def o(\n    a,\n", 1), None); // the header never closes
    }

    /// `extract_fn`'s caps, in their order: at most 40 lines, then at most
    /// 2000 characters.
    #[test]
    fn text_of_caps() {
        // the whole span when it is under both caps
        let small = "def f(x):\n    return x\n";
        assert_eq!(text_of(small, (1, 2)), "def f(x):\n    return x");

        // a 51-line function is capped at 40 lines: the def line and 39
        // body lines
        let mut big = String::from("def big(x):\n");
        for i in 0..50 {
            big.push_str(&format!("    l{:<02} = x\n", i));
        }
        let got = text_of(&big, (1, 51));
        let lines: Vec<&str> = got.split('\n').collect();
        assert_eq!(lines.len(), 40);
        assert_eq!(lines[0], "def big(x):");
        assert_eq!(lines[39], "    l38 = x");

        // 40 lines past 2000 characters are capped at 2000
        let mut long = String::from("def long(x):\n");
        for i in 0..40 {
            long.push_str(&format!("    v{i} = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'\n"));
        }
        assert!(long.chars().count() > MAX_CHARS);
        let got = text_of(&long, (1, 41));
        assert_eq!(got.chars().count(), MAX_CHARS);
        assert!(got.starts_with("def long(x):\n"));
        // the cut is inside a body line: the first body line is kept whole
        assert!(got.contains("    v0 = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'"));
    }

    /// A 4-function case with a known BM25 order. The expected order and the
    /// scores are pasted verbatim from fn_rank.py's `bm25_ranks` (and its
    /// score arithmetic) run on the same token bags — the 4 functions' texts
    /// as `ast` extracts them (the fixture's spans match the `ast` ranges, so
    /// the bags match), by:
    ///
    ///     python3 -c "import sys, ast, collections; sys.path.insert(0,
    ///         '/var/tmp/familiar-lanes/anchor-check'); import fn_rank; ...
    ///         (the 4 functions' token bags, bm25_ranks over them)"
    ///
    /// the query "cache invalidation entry value stale" tokenizes to
    /// [cache, cache, invalidation, invalidation, entry, entry, value, value,
    /// stale, stale]; over the bags the scores are, to 1e-9:
    ///     fetch_cache 1.3573713314175597, invalidate_entry 3.250400246452365,
    ///     save_row 0.0, note_value -3.196353580883917
    /// and bm25_ranks ranks {invalidate_entry: 1, fetch_cache: 2} (save_row
    /// and note_value left unranked).
    #[test]
    fn rank_words_ranks_the_called_functions() {
        let src = "def fetch_cache(key):\n    value = cache.get(key)\n    if value is None:  # a stale miss\n        value = compute(key)\n    return value\n\n\ndef invalidate_entry(entry):\n    # drop the invalidation of the entry\n    del entries[entry]\n    purge(entry.value)\n\n\ndef save_row(row):\n    db.write(row)\n    return row\n\n\ndef note_value(value):\n    log.append(value)\n";
        let called = [
            Called { file: "a.py".into(), symbol: "fetch_cache".into(), def_line: 1, last_call: 100 },
            Called { file: "a.py".into(), symbol: "invalidate_entry".into(), def_line: 8, last_call: 40 },
            Called { file: "a.py".into(), symbol: "save_row".into(), def_line: 14, last_call: 7 },
            Called { file: "a.py".into(), symbol: "note_value".into(), def_line: 19, last_call: 9 },
        ];
        let reads = std::cell::Cell::new(0);
        let read = |f: &str| {
            reads.set(reads.get() + 1);
            (f == "a.py").then(|| src.to_string())
        };
        let got = rank_words("cache invalidation entry value stale", &called, read);
        // every called function once; the score-positive first (score
        // descending), the rest after (last_call descending: 9, 7)
        assert_eq!(got.len(), 4);
        let places: Vec<_> = got.iter().map(|r| (r.rank, r.symbol.as_str(), r.line_start, r.line_end)).collect();
        assert_eq!(places, [(1, "invalidate_entry", 8, 11), (2, "fetch_cache", 1, 5), (3, "note_value", 19, 20), (4, "save_row", 14, 16)]);
        // each file is read once (the 4 functions share it)
        assert_eq!(reads.get(), 1);
        // a function off the file keeps its def_line lines and an empty bag:
        // it scores 0 and comes by last_call
        let missing = Called { file: "b.py".into(), symbol: "ghost".into(), def_line: 3, last_call: 50 };
        let got2 = rank_words("cache invalidation entry value stale", &[missing], |f| (f == "a.py").then(|| src.to_string()));
        assert_eq!(got2.len(), 1);
        assert_eq!(got2[0].rank, 1);
        assert_eq!(got2[0].line_start, 3);
        assert_eq!(got2[0].line_end, 3);
    }

    /// Functions with no span (or no file) keep an empty token bag: with no
    /// score, the order is last_call descending.
    #[test]
    fn rank_words_without_spans_or_query_tokens() {
        let called = [
            Called { file: "c.py".into(), symbol: "ghost".into(), def_line: 1, last_call: 5 },
            Called { file: "d.py".into(), symbol: "other".into(), def_line: 2, last_call: 7 },
        ];
        let read = |f: &str| match f {
            "c.py" => Some("x = 1\n".to_string()), // no def at line 1
            "d.py" => Some("y = 2\n".to_string()), // no def at line 2
            _ => None,
        };
        let got = rank_words("anything at all", &called, read);
        assert_eq!(got.len(), 2);
        let places: Vec<_> = got.iter().map(|r| (r.rank, r.symbol.as_str(), r.line_start, r.line_end)).collect();
        assert_eq!(places, [(1, "other", 2, 2), (2, "ghost", 1, 1)]);
    }
}
