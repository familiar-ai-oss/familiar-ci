//! Familiar's own run of a claimed command (plan A, lane A-wire): the segment that names the
//! runner, a reader's flags inserted there, and the dry run's re-aim at a probe. The claimed
//! command is never stored changed (`run_reserved.command` stays it); the person's terminal never
//! sees these.
//!
//! ```text
//!   cd sub && FOO=1 npx jest --ci tests/a.test.js | tee log
//!   └ kept ┘  └──── the runner's segment ─────────┘ └ kept ┘
//!                    ▲ flags go after the runner word (and its subcommand: go test, cargo test,
//!                      cargo nextest run, dotnet test, vitest run)
//!   re-aim: positional targets and the runner's selection flags dropped, the probe's appended
//! ```
//!
//! The segment is the last one naming a runner, as `familiar_ai_run::details_for` picks it; words
//! are split quote-aware, and kept words keep their own quoting.

use familiar_ai_run::RUNNER_DETAILS;

/// One word of a segment: its byte span there and its text with the quotes removed.
#[derive(Debug, Clone)]
struct Tok {
    start: usize,
    end: usize,
    text: String,
}

/// The words of `s` (one segment: no operators), quote-aware.
fn tokens(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let start = i;
        let mut text = String::new();
        let mut end = i;
        while let Some(&(j, c)) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            chars.next();
            end = j + c.len_utf8();
            match c {
                '\'' | '"' => {
                    for (k, d) in chars.by_ref() {
                        end = k + d.len_utf8();
                        if d == c {
                            break;
                        }
                        text.push(d);
                    }
                }
                '\\' => {
                    if let Some((k, d)) = chars.next() {
                        end = k + d.len_utf8();
                        text.push(d);
                    }
                }
                c => text.push(c),
            }
        }
        out.push(Tok { start, end, text });
    }
    out
}

/// The command's segments as byte spans, split at the shell's unquoted `&&`, `||`, `;`, `|`, `&`
/// and newlines; with each the separator that follows it (`""` for the last).
fn segments(command: &str) -> Vec<(usize, usize, &str)> {
    let b = command.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == b'\\' && q == b'"' {
                i += 1;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => quote = Some(c),
            b'\\' => i += 1,
            b'&' | b'|' if b.get(i + 1) == Some(&c) => {
                out.push((start, i, &command[i..i + 2]));
                i += 2;
                start = i;
                continue;
            }
            b';' | b'|' | b'\n' => {
                out.push((start, i, &command[i..i + 1]));
                start = i + 1;
            }
            // a lone `&` that is not a redirect's (`2>&1`, `&>`)
            b'&' if i > 0 && b[i - 1] != b'>' && b.get(i + 1) != Some(&b'>') => {
                out.push((start, i, &command[i..i + 1]));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push((start, command.len(), ""));
    out
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

const SHELL_WORDS: &[&str] = &["for", "do", "done", "then", "time", "exec", "nice"];
const WRAPPERS: &[&[&str]] = &[&["npx"], &["bunx"], &["yarn"], &["pnpm", "exec"], &["pnpm"], &["uv", "run"], &["poetry", "run"], &["pipenv", "run"]];

fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// The subcommands a runner's flags and targets follow.
fn subcommands(runner: &str) -> &'static [&'static str] {
    match runner {
        "go-test" => &["test"],
        "cargo-test" => &["test", "t"],
        "cargo-nextest" => &["run", "r"],
        "dotnet" => &["test"],
        "vitest" => &["run"],
        _ => &[],
    }
}

/// Where a segment names `runner`: the index of the runner's word, and the index its own
/// arguments start at (after its subcommand). Mirrors the run crate's reading of a segment.
fn runner_in(toks: &[Tok], runner: &str) -> Option<(usize, usize)> {
    let words: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    let mut i = words.iter().take_while(|w| SHELL_WORDS.contains(w)).count();
    // `nice -n 10 cargo test`
    if i > 0 && words.get(i - 1) == Some(&"nice") && words.get(i).is_some_and(|w| w.starts_with('-')) {
        i += if words[i] == "-n" { 2 } else { 1 };
    }
    i += words[i.min(words.len())..].iter().take_while(|w| assignment(w)).count();
    while let Some(w) = WRAPPERS.iter().find(|w| words.len() >= i + w.len() && words[i..].iter().zip(w.iter()).all(|(a, b)| basename(a) == *b)) {
        i += w.len();
        i += words[i.min(words.len())..].iter().take_while(|w| w.starts_with('-')).count();
    }
    let first = basename(words.get(i)?);
    let (at, name) = if first == "python" || first.starts_with("python3") {
        let m = words[i + 1..].iter().position(|w| *w == "-m")?;
        let at = i + 1 + m + 1;
        (at, *words.get(at)?)
    } else if first == "cargo" && words.get(i + 1) == Some(&"nextest") {
        (i + 1, "cargo-nextest")
    } else {
        (i, first)
    };
    let row = RUNNER_DETAILS.iter().find(|r| r.words.contains(&name) || r.runner == name)?;
    if row.runner != runner {
        return None;
    }
    let mut after = at + 1;
    if words.get(after).is_some_and(|w| subcommands(runner).contains(w)) {
        after += 1;
    }
    Some((at, after))
}

/// The last segment of `command` naming `runner`: its span and its words.
fn runner_segment(command: &str, runner: &str) -> Option<(usize, usize, Vec<Tok>, usize, usize)> {
    segments(command).into_iter().rev().find_map(|(s, e, _)| {
        let toks = tokens(&command[s..e]);
        runner_in(&toks, runner).map(|(at, after)| (s, e, toks, at, after))
    })
}

/// `word` as the shell reads it back: as is when it holds nothing the shell would read, else
/// single-quoted.
pub fn quote(word: &str) -> String {
    let plain = !word.is_empty() && !word.starts_with('~') && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%{}~".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// Whether `command`'s segment naming `runner` has `flag` (as a word, or `flag=value`).
pub fn has_flag(command: &str, runner: &str, flag: &str) -> bool {
    let Some((_, _, toks, _, after)) = runner_segment(command, runner) else {
        return words_have(command, flag);
    };
    toks[after.min(toks.len())..].iter().any(|t| t.text == flag || t.text.starts_with(&format!("{flag}=")))
}

/// Whether any word of the whole command is `flag` or `flag=…`.
fn words_have(command: &str, flag: &str) -> bool {
    segments(command).iter().any(|(s, e, _)| tokens(&command[*s..*e]).iter().any(|t| t.text == flag || t.text.starts_with(&format!("{flag}="))))
}

/// Runners whose options come before the files they run (node's own options end at its script):
/// a reader's flags go right after the runner and its subcommand. The others' go at the end of
/// the runner's segment (before a `--`): jest's `--reporters` and mocha's `--require` are lists,
/// and a list option swallows the files after it.
fn flags_first(runner: &str) -> bool {
    matches!(runner, "node-test" | "go-test")
}

/// `command` with `flags` (each quoted as needed) inserted in the last segment naming `runner`:
/// after its word and subcommand for [`flags_first`] runners, else at the segment's end (before a
/// `--`); `None` when no segment names it.
pub fn insert_flags(command: &str, runner: &str, flags: &[String]) -> Option<String> {
    if flags.is_empty() {
        return Some(command.to_string());
    }
    let (s, _, toks, _, after) = runner_segment(command, runner)?;
    let last = if flags_first(runner) {
        after - 1
    } else {
        toks.iter().position(|t| t.text == "--" || t.text == "-args").map_or(toks.len() - 1, |i| i - 1).max(after - 1)
    };
    let at = s + toks[last].end;
    let words: Vec<String> = flags.iter().map(|f| quote(f)).collect();
    Some(format!("{} {}{}", &command[..at], words.join(" "), &command[at..]))
}

/// Whether `command`'s only go call is one `go test` with nothing piped after it, so `GOFLAGS`
/// reaches no other go command (the A0 spike: `go vet` exits 0 on findings under `-json`).
pub fn only_go_call_is_go_test(command: &str) -> bool {
    let segs = segments(command);
    let mut go = segs.iter().filter(|(s, e, _)| tokens(&command[*s..*e]).iter().any(|t| basename(&t.text) == "go"));
    let (Some((s, e, sep)), None) = (go.next(), go.next()) else { return false };
    *sep != "|" && runner_in(&tokens(&command[*s..*e]), "go-test").is_some_and(|(at, after)| after == at + 2)
}

/// How a runner's command is re-aimed: the selection flags dropped (flag, takes a value), the
/// kept flags that take a value (their value is not a target), whether positional words are kept
/// (Maven's goals, Gradle's tasks, dotnet's project), and the words after which everything is
/// the program's own (`--`, go's `-args`).
#[derive(Debug, Clone, Copy)]
pub struct Aim {
    pub selection: &'static [(&'static str, bool)],
    pub with_value: &'static [&'static str],
    pub keep_positionals: bool,
    pub dropped_subcommands: &'static [&'static str],
}

/// `command` re-aimed at `targets` for `runner` (plan A's `re_aim`): wrappers, the runner, its
/// subcommand and its flags kept; positional targets and the selection flags dropped; the targets
/// appended (before a `--`). Other segments are kept as written. `None` when no segment names
/// the runner (no dry run).
pub fn re_aim(command: &str, runner: &str, aim: &Aim, targets: &[String]) -> Option<String> {
    let (s, e, toks, _, after) = runner_segment(command, runner)?;
    let seg = &command[s..e];
    let raw = |t: &Tok| seg[t.start..t.end].to_string();
    let mut out: Vec<String> = toks[..after].iter().map(raw).collect();
    let mut tail: Vec<String> = Vec::new();
    let mut i = after;
    let short_attached = |w: &str| {
        aim.selection.iter().any(|(f, v)| *v && f.len() == 2 && f.starts_with('-') && !f.starts_with("--") && w.starts_with(f) && w.len() > 2 && !w.contains('='))
    };
    while i < toks.len() {
        let w = toks[i].text.as_str();
        if w == "--" || w == "-args" || w == "--args" {
            tail = toks[i..].iter().map(raw).collect();
            break;
        }
        if let Some((_, takes)) = aim.selection.iter().find(|(f, _)| *f == w) {
            i += if *takes { 2 } else { 1 };
            continue;
        }
        if aim.selection.iter().any(|(f, v)| *v && w.starts_with(&format!("{f}="))) || (w.starts_with("-D") && aim.selection.iter().any(|(f, _)| f.starts_with("-D") && w.starts_with(f))) || short_attached(w) {
            i += 1;
            continue;
        }
        if aim.dropped_subcommands.contains(&w) {
            i += 1;
            continue;
        }
        if aim.with_value.contains(&w) {
            out.push(raw(&toks[i]));
            if let Some(v) = toks.get(i + 1) {
                out.push(raw(v));
            }
            i += 2;
            continue;
        }
        if !w.starts_with('-') && !aim.keep_positionals {
            i += 1; // a positional target
            continue;
        }
        out.push(raw(&toks[i]));
        i += 1;
    }
    // after `--`, libtest's own selection goes with the targets: its filters and skips
    if !tail.is_empty() && runner.starts_with("cargo") {
        tail = keep_libtest_options(&tail);
    }
    out.extend(targets.iter().map(|t| quote(t)));
    out.extend(tail);
    let mut text = String::new();
    text.push_str(&command[..s]);
    if command[..s].ends_with(|c: char| !c.is_whitespace()) {
        text.push(' ');
    }
    text.push_str(&out.join(" "));
    let rest = &command[e..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        text.push(' ');
    }
    text.push_str(rest);
    Some(text.trim_start().to_string())
}

/// After cargo's `--`: libtest's options kept, its filters, `--skip`, `--exact` and the ignored
/// switches dropped.
fn keep_libtest_options(tail: &[String]) -> Vec<String> {
    let with_value = ["--test-threads", "--format", "--logfile", "--color", "-Z", "--report-time"];
    let mut out = vec![tail[0].clone()];
    let mut i = 1;
    while i < tail.len() {
        let w = tail[i].as_str();
        if w == "--skip" {
            i += 2;
            continue;
        }
        if matches!(w, "--exact" | "--ignored" | "--include-ignored") || !w.starts_with('-') {
            i += 1;
            continue;
        }
        out.push(tail[i].clone());
        if with_value.contains(&w) {
            if let Some(v) = tail.get(i + 1) {
                out.push(v.clone());
            }
            i += 1;
        }
        i += 1;
    }
    if out.len() == 1 {
        return Vec::new();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_split_at_the_shells_operators_outside_quotes() {
        let c = "cd a && npx jest 'x && y' | tee l; echo 2>&1 done";
        let parts: Vec<&str> = segments(c).iter().map(|(s, e, _)| c[*s..*e].trim()).collect();
        assert_eq!(parts, ["cd a", "npx jest 'x && y'", "tee l", "echo 2>&1 done"]);
    }

    #[test]
    fn flags_go_after_the_runner_and_its_subcommand() {
        let f = |c: &str, r: &str| insert_flags(c, r, &["-json".to_string()]).unwrap();
        assert_eq!(f("go test ./...", "go-test"), "go test -json ./...");
        assert_eq!(f("cd x && go test -run A ./... | tee l", "go-test"), "cd x && go test -json -run A ./... | tee l");
        assert_eq!(insert_flags("npx jest --ci src/a.test.js", "jest", &["--reporters=default".into(), "a;b".into()]).unwrap(), "npx jest --ci src/a.test.js --reporters=default 'a;b'");
        assert_eq!(insert_flags("npx jest --ci -- x | tee l", "jest", &["--r".into()]).unwrap(), "npx jest --ci --r -- x | tee l");
        assert_eq!(insert_flags("make test", "jest", &["-x".into()]), None);
    }
}
