//! Familiar's shell trace (H-2, lane H5; the A0 spike's findings §10): a test that is a shell
//! script is read through the shell's own trace in Familiar's own run, never by rewriting the
//! person's tests. The trace goes to a side file, read back as the run's report stream:
//!
//! - bash read as bash: `BASH_ENV` loads Familiar's kit ([`KIT`]: an ERR trap and a one-line
//!   DEBUG ring), which writes `kind pid status file line command` rows;
//! - bash read as sh (Fedora's `/bin/sh`, a `#!/bin/sh` script there), which ignores
//!   `BASH_ENV`: `SHELLOPTS=xtrace` with `BASH_XTRACEFD` on a descriptor the spawner opens to
//!   the side file and a `PS4` holding the file and the line ([`PS4`]);
//! - an `sh` that is not bash (Debian's dash, busybox): no `LINENO` on Debian and no route by
//!   the environment, so `-x` is added to Familiar's own run of the script ([`with_x`]) with
//!   [`PS4_SH`]; its trace is on stderr, moved out of it after the run and given the script's
//!   file and line, matched from the command's text where the shell gives none ([`DashTrace`]).
//!
//! The planned command, the person's terminal and the person's stderr are never changed.

use std::path::{Path, PathBuf};

/// The bash kit, written once per version to a folder outside the run's copy ([`kit_path`]).
pub const KIT: &str = include_str!("../kit/bash_env.sh");

/// The descriptor the trace is written to in the child (`BASH_XTRACEFD`); high, so it does not
/// meet the script's own.
pub const TRACE_FD: i32 = 213;

/// The xtrace prefix for bash: the marker, the file, the line.
pub const PS4: &str = "+familiar-ai\t${BASH_SOURCE}\t${LINENO}\t";

/// The xtrace prefix for an `sh` that is not bash: its own marker, the shell's `$0` (the script
/// a line is in, or `sh` for the shell Familiar starts: dash names no file) and the line (empty
/// where the shell has no `LINENO`, as on Debian).
pub const PS4_SH: &str = "+familiar-ai-sh\t${0}\t${LINENO}\t";

/// The marker of an `sh`'s own trace lines, read by [`split_dash`] (before `$0`, recordings
/// have [`MARK`] and the line only).
pub const MARK_SH: &str = "familiar-ai-sh\t";

/// The marker every trace line starts with (after bash's repeated `+` for a nested level).
pub const MARK: &str = "familiar-ai\t";

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

/// The kit's file, written once per version (and kit text) under the temp folder.
pub fn kit_path() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("familiar-ai-kit-{}-{:016x}", env!("CARGO_PKG_VERSION"), fnv(KIT)));
    let file = dir.join("bash_env.sh");
    if std::fs::read_to_string(&file).ok().as_deref() == Some(KIT) {
        return Some(file);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let part = dir.join(format!("bash_env.sh.{}", std::process::id()));
    std::fs::write(&part, KIT).ok()?;
    std::fs::rename(&part, &file).ok()?;
    Some(file)
}

/// Whether the `sh` the run starts is bash (Fedora, Arch, macOS's bash 3 aside); else it is
/// dash, busybox or another `sh` with no environment route.
pub fn sh_is_bash() -> bool {
    static IS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS.get_or_init(|| {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .map(|d| d.join("sh"))
            .find(|p| p.is_file())
            .and_then(|p| std::fs::canonicalize(p).ok())
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().contains("bash")))
            .unwrap_or(false)
    })
}

/// The shell words a segment starts with before its command word.
const LEAD: &[&str] = &["do", "then", "else", "{", "(", "!", "time", "exec"];
const SHELLS: &[&str] = &["sh", "dash", "ash"];

fn basename(w: &str) -> &str {
    w.rsplit('/').next().unwrap_or(w)
}

/// One `sh` script a command runs at its top level, and where `-x` goes in for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShScript {
    /// The script's path as the command names it.
    pub path: String,
    /// The byte offset in the command where ` -x` (after an `sh` word) or `sh -x ` (before a
    /// `#!/bin/sh` script) is inserted.
    pub at: usize,
    /// Whether the script is named on its own and needs `sh -x ` before it.
    pub bare: bool,
}

/// The `sh` scripts `command` runs at its top level (`sh x.sh`, or `./x.sh` whose first line
/// is `#!/bin/sh`), read in `dir`.
pub fn sh_scripts(command: &str, dir: &Path) -> Vec<ShScript> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = command.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    let mut segments = Vec::new();
    while i <= bytes.len() {
        let c = bytes.get(i).copied();
        match (quote, c) {
            (Some(q), Some(b)) if b == q => quote = None,
            (Some(_), Some(_)) => {}
            (None, Some(b'\'' | b'"')) => quote = c,
            (None, None | Some(b'\n' | b';' | b'&' | b'|')) => {
                segments.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    for (a, b) in segments {
        let seg = &command[a..b];
        let mut words = Vec::new();
        let mut pos = 0;
        for w in seg.split(|c: char| c.is_whitespace()) {
            if !w.is_empty() {
                words.push((a + pos, w));
            }
            pos += w.len() + 1;
        }
        let mut k = words.iter().take_while(|(_, w)| LEAD.contains(w)).count();
        k += words[k..].iter().take_while(|(_, w)| w.contains('=') && !w.starts_with('=') && !w.starts_with('-')).count();
        let Some((at, word)) = words.get(k) else { continue };
        if SHELLS.contains(&basename(word)) {
            // a script named through a variable (`sh "$f"`) is traced too: its lines name it
            // through `$0` ([`PS4_SH`])
            let Some((_, script)) = words.get(k + 1).filter(|(_, w)| !w.starts_with('-')) else { continue };
            out.push(ShScript { path: (*script).to_string(), at: at + word.len(), bare: false });
        } else if word.contains('/') && !word.contains(['$', '`', '\'', '"']) {
            let first = std::fs::read_to_string(dir.join(word)).ok().and_then(|t| t.lines().next().map(str::to_string)).unwrap_or_default();
            let sheb = first.trim_end();
            if matches!(sheb, "#!/bin/sh" | "#! /bin/sh" | "#!/usr/bin/env sh" | "#!/bin/dash" | "#!/usr/bin/env dash") {
                out.push(ShScript { path: (*word).to_string(), at: *at, bare: true });
            }
        }
    }
    out
}

/// `command` with `-x` added for each of `scripts` (Familiar's own run only, A-2).
pub fn with_x(command: &str, scripts: &[ShScript]) -> String {
    let mut out = command.to_string();
    for s in scripts.iter().rev() {
        out.insert_str(s.at, if s.bare { "sh -x " } else { " -x" });
    }
    out
}

/// The trace an `sh` that is not bash wrote to stderr under `-x`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DashTrace {
    /// stderr without the trace lines: the person's own.
    pub stderr: String,
    /// The trace, one `+familiar-ai\t<file>\t<line>\t<command>` line each (the line empty where
    /// it could not be told).
    pub trace: String,
}

/// Split `stderr` of a run whose `scripts` ran under `-x` with [`PS4_SH`]: each trace line is
/// given its file, the shell's `$0` (none for the shell Familiar starts, the command's own
/// level), and its line: the shell's own `LINENO`, else the one line of that script whose text
/// the expanded command matches (a check repeated on two lines matches neither: the line stays
/// empty, can't tell). A line with the line only (recorded before `$0`) is given the script's
/// file only when one script ran.
///
/// dash writes a trace line in pieces (the PS4, each word, the newline), so commands traced at
/// once (a pipeline's) can tear each other's lines, and dash's own message (`<file>: N: <word>:
/// not found`) can land among them. A torn stretch is read back as the untorn lines it holds
/// when one reading of it is possible ([`Split::untear`]); else it is kept as it was.
pub fn split_dash(stderr: &str, scripts: &[ShScript], dir: &Path) -> DashTrace {
    let mut s = Split::new(scripts, dir);
    let lines: Vec<&str> = stderr.split_inclusive('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        if !s.torn(lines[i]) {
            s.line(lines[i]);
            i += 1;
            continue;
        }
        let end = s.stretch_end(&lines, i);
        match s.untear(&lines[i..end].concat()) {
            Some((read, persons)) => {
                for (ps4, cmd) in read {
                    s.sh_line(&ps4.zero, &ps4.line, &cmd);
                }
                s.out.stderr.push_str(&persons);
                i = end;
            }
            // its first line as it is; a stretch may start again after it (its `. file` read)
            None => {
                s.line(lines[i]);
                i += 1;
            }
        }
    }
    s.out
}

/// The start of a [`PS4_SH`] piece as dash writes it, wherever it lands in a line.
const PS4_SH_START: &str = "+familiar-ai-sh\t";

/// The most lines a torn stretch is read over.
const STRETCH_LINES: usize = 64;

/// How many steps [`Split::untear`] takes before it says it cannot tell.
const UNTEAR_STEPS: usize = 10_000;

/// The script a trace line's `$0` names: none for a shell (the command's own level).
fn file_of(zero: &str) -> &str {
    if SHELLS.contains(&basename(zero)) || basename(zero) == "bash" {
        ""
    } else {
        zero.strip_prefix("./").unwrap_or(zero)
    }
}

/// Whether `l` is torn in its shape: a PS4 in mid-line, or a traced command with no words.
fn torn_shape(l: &str) -> bool {
    let body = l.trim_start_matches('+');
    match body.strip_prefix(MARK_SH).filter(|_| body.len() < l.len()) {
        Some(rest) => {
            let (_, _, cmd) = fields(rest);
            cmd.contains(PS4_SH_START) || cmd.trim().is_empty()
        }
        None => l.contains(PS4_SH_START),
    }
}

/// A [`PS4_SH`] line's fields after its marker: `$0`, the line, the command.
fn fields(rest: &str) -> (&str, &str, &str) {
    let mut f = rest.trim_end_matches('\n').splitn(3, '\t');
    (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""))
}

/// [`split_dash`]'s reading so far: the scripts' texts and commands, what each sourced, the split.
struct Split<'a> {
    dir: &'a Path,
    one: Option<&'a ShScript>,
    source: Option<String>,
    texts: std::collections::HashMap<String, Option<String>>,
    commands: std::collections::HashMap<String, Vec<Command>>,
    /// The project scripts each script sourced (`. file`): dash gives their lines its `$0`.
    sourced: std::collections::HashMap<String, Vec<String>>,
    out: DashTrace,
}

impl<'a> Split<'a> {
    fn new(scripts: &'a [ShScript], dir: &'a Path) -> Split<'a> {
        let one = match scripts {
            [s] => Some(s),
            _ => None,
        };
        let source = one.and_then(|s| std::fs::read_to_string(dir.join(&s.path)).ok());
        Split { dir, one, source, texts: Default::default(), commands: Default::default(), sourced: Default::default(), out: DashTrace::default() }
    }

    fn text(&mut self, f: &str) -> Option<String> {
        let dir = self.dir;
        self.texts.entry(f.to_string()).or_insert_with(|| std::fs::read_to_string(dir.join(f)).ok()).clone()
    }

    /// The lines of `file` that `cmd` can come from, and of each script it sourced.
    fn hits(&mut self, file: &str, cmd: &str) -> (Vec<usize>, Vec<(String, Vec<usize>)>) {
        let own = self.text(file).map(|t| match_hits(&t, cmd)).unwrap_or_default();
        let theirs = self.sourced.get(file).cloned().unwrap_or_default();
        let theirs = theirs.into_iter().map(|f| {
            let h = self.text(&f).map(|t| match_hits(&t, cmd)).unwrap_or_default();
            (f, h)
        });
        (own, theirs.collect())
    }

    /// One traced command, `$0` `zero`, written as a trace line with its file and line.
    fn sh_line(&mut self, zero: &str, line: &str, cmd: &str) {
        let file = file_of(zero);
        let (at, line) = if !line.trim().is_empty() || file.is_empty() {
            (file.to_string(), line.trim().to_string())
        } else {
            let (own, theirs) = self.hits(file, cmd);
            match own.as_slice() {
                [n] => (file.to_string(), n.to_string()),
                // none of its own: the one script it sourced with one line for it
                [] => {
                    let found: Vec<(String, usize)> = theirs.iter().filter_map(|(f, h)| if let [n] = h.as_slice() { Some((f.clone(), *n)) } else { None }).collect();
                    match found.as_slice() {
                        [(f, n)] => (f.clone(), n.to_string()),
                        _ => (file.to_string(), String::new()),
                    }
                }
                _ => (file.to_string(), String::new()),
            }
        };
        if let [".", p, ..] = cmd.split_whitespace().collect::<Vec<_>>().as_slice() {
            let p = p.strip_prefix("./").unwrap_or(p);
            if !file.is_empty() && self.dir.join(p).is_file() {
                self.sourced.entry(file.to_string()).or_default().push(p.to_string());
            }
        }
        self.out.trace.push_str(&format!("+{MARK}{at}\t{line}\t{cmd}\n"));
    }

    /// One line of stderr as it is: a trace line, or the person's.
    fn line(&mut self, l: &str) {
        let body = l.trim_start_matches('+');
        if let Some(rest) = body.strip_prefix(MARK_SH).filter(|_| body.len() < l.len()) {
            let (zero, line, cmd) = fields(rest);
            self.sh_line(zero, line, cmd);
            return;
        }
        let Some(rest) = body.strip_prefix(MARK).filter(|_| body.len() < l.len()) else {
            self.out.stderr.push_str(l);
            return;
        };
        let rest = rest.trim_end_matches('\n');
        let (line, cmd) = rest.split_once('\t').unwrap_or(("", rest));
        let line = if line.trim().is_empty() { self.source.as_deref().and_then(|s| match_line(s, cmd)).map(|n| n.to_string()).unwrap_or_default() } else { line.trim().to_string() };
        let file = self.one.map_or("", |s| s.path.as_str());
        self.out.trace.push_str(&format!("+{MARK}{file}\t{line}\t{cmd}\n"));
    }

    /// Whether `l` may be torn: torn in its shape ([`torn_shape`]), or a traced command that no
    /// line of its script, nor of a script it sourced, can come from.
    fn torn(&mut self, l: &str) -> bool {
        if torn_shape(l) {
            return true;
        }
        let body = l.trim_start_matches('+');
        let Some(rest) = body.strip_prefix(MARK_SH).filter(|_| body.len() < l.len()) else {
            return false;
        };
        let (zero, line, cmd) = fields(rest);
        let file = file_of(zero);
        if file.is_empty() || !line.trim().is_empty() {
            return false;
        }
        let (own, theirs) = self.hits(file, cmd);
        own.is_empty() && theirs.iter().all(|(_, h)| h.is_empty())
    }

    /// Where the torn stretch starting at `lines[i]` ends: past every line while its PS4s
    /// outnumber its newlines (each traced command writes one of each), then past the lines with
    /// no PS4 after it (the person's, or a torn command's last words).
    fn stretch_end(&self, lines: &[&str], i: usize) -> usize {
        let marks = |l: &str| l.matches(PS4_SH_START).count();
        let (mut m, mut n) = (marks(lines[i]), 1);
        let mut j = i + 1;
        while j < lines.len() && j - i < STRETCH_LINES {
            let l = lines[j];
            if m > n || !l.contains(PS4_SH_START) {
                m += marks(l);
                n += 1;
                j += 1;
            } else {
                break;
            }
        }
        j
    }

    /// The commands `file` can trace ([`commands`]).
    fn commands_in(&mut self, file: &str) -> Vec<Command> {
        if let Some(c) = self.commands.get(file) {
            return c.clone();
        }
        let c = self.text(file).map(|t| commands(&t)).unwrap_or_default();
        self.commands.insert(file.to_string(), c.clone());
        c
    }

    /// A torn stretch `r` read back: the traced commands it holds, each with its PS4's fields,
    /// in the order their first words were written, and the person's text (dash's own message
    /// or a line of the person's torn into it, and the lines after it). Each piece of `r` must be
    /// a piece of one of the commands its PS4's script (or a script that one sourced) has, of
    /// dash's own message, or a line of the person's written at once, and there must be one such
    /// reading: two (two lines of the script could explain it), none (a word no command has, the
    /// command's own level), or a search past [`UNTEAR_STEPS`] is `None`, can't tell.
    fn untear(&mut self, r: &str) -> Option<Reading> {
        let mut commands: std::collections::HashMap<String, Vec<Command>> = std::collections::HashMap::new();
        for piece in r.split(PS4_SH_START).skip(1) {
            let zero = piece.split('\t').next().unwrap_or("");
            if commands.contains_key(zero) {
                continue;
            }
            let file = file_of(zero);
            if file.is_empty() {
                return None;
            }
            let theirs = self.sourced.get(file).cloned().unwrap_or_default();
            let mut all: Vec<Command> = Vec::new();
            for f in std::iter::once(file.to_string()).chain(theirs) {
                for c in self.commands_in(&f) {
                    if !all.contains(&c) {
                        all.push(c);
                    }
                }
            }
            commands.insert(zero.to_string(), all);
        }
        let last_ps4 = r.rfind(PS4_SH_START).unwrap_or(0);
        let mut u = Untear { r, last_ps4, commands: &commands, ps4s: Vec::new(), dir: self.dir, steps: 0, ends: 0, found: Vec::new(), dead: Default::default() };
        u.go(0, Stretch::default());
        match u.found.as_slice() {
            [one] if u.steps <= UNTEAR_STEPS => Some(one.clone()),
            _ => None,
        }
    }
}

/// One piece pattern: literal text, and `Any` for an expansion's text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Lit(String),
    Any,
}

/// A command as dash's xtrace writes it, one write each: its first word, then each later word
/// after a space (an unquoted expansion alone may vanish: `true`), then the newline.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Command {
    pieces: Vec<(Vec<Seg>, bool)>,
}

/// The commands a script's `text` can trace: each line's parts ([`split_parts`]), their words as
/// the trace prints them (quotes and redirections dropped, each expansion or glob any text). A
/// part that cannot be read so (a quote left open where the line was cut) is none.
fn commands(text: &str) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    for l in text.lines() {
        for p in split_parts(l) {
            if let Some(c) = command(&p).filter(|c| !out.contains(c)) {
                out.push(c);
            }
        }
    }
    out
}

fn command(part: &str) -> Option<Command> {
    // a subshell's parentheses are not traced: `(cd x && make)` traces `cd x` and `make`
    let mut part = part.trim_start_matches(|c: char| c == '(' || c.is_whitespace()).trim_end();
    while part.ends_with(')') && part.matches('(').count() < part.matches(')').count() {
        part = part[..part.len() - 1].trim_end();
    }
    let mut words: Vec<(String, Vec<Seg>, bool)> = Vec::new();
    let mut skip = false;
    for w in shell_words(part)? {
        // a redirection is not traced; `> file` names its target in the next word
        match redirection(&w.0) {
            _ if skip => skip = false,
            Some(alone) => skip = alone,
            None => words.push(w),
        }
    }
    // nor a keyword before the command, a case pattern (`a)`) or a function's name (`greet()`)
    while words.first().is_some_and(|(raw, ..)| matches!(raw.as_str(), "if" | "elif" | "then" | "else" | "while" | "until" | "do" | "!" | "{") || (raw.ends_with(')') && !raw.contains("$("))) {
        words.remove(0);
    }
    if words.is_empty() {
        return None;
    }
    let pieces = words
        .into_iter()
        .enumerate()
        .map(|(k, (_, segs, optional))| {
            let mut piece: Vec<Seg> = Vec::new();
            for s in (k > 0).then(|| Seg::Lit(" ".to_string())).into_iter().chain(segs) {
                match (piece.last_mut(), s) {
                    (Some(Seg::Lit(a)), Seg::Lit(b)) => a.push_str(&b),
                    (_, s) => piece.push(s),
                }
            }
            (piece, optional && k > 0)
        })
        .collect();
    Some(Command { pieces })
}

/// Whether the word `raw` is a redirection, and if so whether its target is the next word.
fn redirection(raw: &str) -> Option<bool> {
    let rest = raw.trim_start_matches(|c: char| c.is_ascii_digit());
    ["<<-", "<<", ">>", ">&", "<&", "<>", ">|", "&>", ">", "<"].iter().find_map(|op| rest.strip_prefix(*op)).map(str::is_empty)
}

/// The words of `part`: each one's text as written, its pieces (quotes dropped, an expansion or
/// a glob any text), and whether it is an unquoted expansion alone. `None` for a quote, `${`,
/// `$(` or a backquote left open.
fn shell_words(part: &str) -> Option<Vec<(String, Vec<Seg>, bool)>> {
    fn flush(lit: &mut String, segs: &mut Vec<Seg>) {
        if !lit.is_empty() {
            segs.push(Seg::Lit(std::mem::take(lit)));
        }
    }
    let cs: Vec<char> = part.chars().collect();
    let mut words = Vec::new();
    let mut i = 0;
    loop {
        while cs.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        if i >= cs.len() || cs[i] == '#' {
            return Some(words);
        }
        let start = i;
        let (mut segs, mut lit, mut quoted) = (Vec::new(), String::new(), false);
        while let Some(&c) = cs.get(i).filter(|c| !c.is_whitespace()) {
            match c {
                '\'' => {
                    quoted = true;
                    let j = (i + 1..cs.len()).find(|&j| cs[j] == '\'')?;
                    lit.extend(&cs[i + 1..j]);
                    i = j + 1;
                }
                '"' => {
                    quoted = true;
                    i += 1;
                    loop {
                        match *cs.get(i)? {
                            '"' => {
                                i += 1;
                                break;
                            }
                            '\\' if matches!(cs.get(i + 1), Some('$' | '`' | '"' | '\\')) => {
                                lit.push(cs[i + 1]);
                                i += 2;
                            }
                            '$' | '`' => match expansion_end(&cs, i)? {
                                Some(j) => {
                                    flush(&mut lit, &mut segs);
                                    segs.push(Seg::Any);
                                    i = j;
                                }
                                None => {
                                    lit.push('$');
                                    i += 1;
                                }
                            },
                            d => {
                                lit.push(d);
                                i += 1;
                            }
                        }
                    }
                }
                '\\' => {
                    lit.extend(cs.get(i + 1));
                    i += 2;
                }
                '$' | '`' => match expansion_end(&cs, i)? {
                    Some(j) => {
                        flush(&mut lit, &mut segs);
                        segs.push(Seg::Any);
                        i = j;
                    }
                    None => {
                        lit.push('$');
                        i += 1;
                    }
                },
                '*' | '?' => {
                    flush(&mut lit, &mut segs);
                    segs.push(Seg::Any);
                    i += 1;
                }
                // a bracket glob; `[` and `[[` alone are the test command
                '[' if i > start || cs.get(i + 1).is_some_and(|d| !d.is_whitespace() && *d != '[') => {
                    match (i + 1..cs.len()).take_while(|&j| !cs[j].is_whitespace()).find(|&j| cs[j] == ']') {
                        Some(j) => {
                            flush(&mut lit, &mut segs);
                            segs.push(Seg::Any);
                            i = j + 1;
                        }
                        None => {
                            lit.push('[');
                            i += 1;
                        }
                    }
                }
                '~' if i == start => {
                    segs.push(Seg::Any);
                    while cs.get(i).is_some_and(|d| !d.is_whitespace() && *d != '/') {
                        i += 1;
                    }
                }
                _ => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        flush(&mut lit, &mut segs);
        let raw: String = cs[start..i.min(cs.len())].iter().collect();
        let alone = !quoted && segs == [Seg::Any];
        words.push((raw, segs, alone));
    }
}

/// Where the expansion at `cs[i]` (`$` or a backquote) ends: `Some(None)` for a `$` that starts
/// none (it is itself), `None` for one left open.
fn expansion_end(cs: &[char], i: usize) -> Option<Option<usize>> {
    let close = |from: usize, c: char| (from..cs.len()).find(|&j| cs[j] == c).map(|j| Some(j + 1));
    match (cs[i], cs.get(i + 1).copied()) {
        ('`', _) => close(i + 1, '`'),
        (_, Some('{')) => close(i + 2, '}'),
        (_, Some('(')) => {
            let mut depth = 0;
            for (j, c) in cs.iter().enumerate().skip(i + 1) {
                match c {
                    '(' => depth += 1,
                    ')' if depth == 1 => return Some(Some(j + 1)),
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            None
        }
        (_, Some(c)) if c.is_ascii_alphabetic() || c == '_' => Some(Some((i + 1..cs.len()).find(|&j| !(cs[j].is_ascii_alphanumeric() || cs[j] == '_')).unwrap_or(cs.len()))),
        (_, Some(c)) if c.is_ascii_digit() || "?#@*$!-".contains(c) => Some(Some(i + 2)),
        _ => Some(None),
    }
}

/// The ends `j` such that `r[i..j]` matches `segs`, where `Any` is any text with no newline or
/// tab (a PS4 has both).
fn ends(r: &str, i: usize, segs: &[Seg]) -> Vec<usize> {
    fn go(r: &str, i: usize, segs: &[Seg], out: &mut Vec<usize>) {
        match segs.split_first() {
            None => out.push(i),
            Some((Seg::Lit(t), rest)) => {
                if r[i..].starts_with(t.as_str()) {
                    go(r, i + t.len(), rest, out);
                }
            }
            Some((Seg::Any, rest)) => {
                let mut j = i;
                loop {
                    go(r, j, rest, out);
                    match r[j..].chars().next() {
                        Some(c) if c != '\n' && c != '\t' => j += c.len_utf8(),
                        _ => break,
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    go(r, i, segs, &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

/// The fields of one [`PS4_SH`] piece: `$0` and the line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ps4 {
    zero: String,
    line: String,
}

/// A torn stretch read back: its traced commands in the order their first words were written,
/// each with its PS4's fields, and the person's text.
type Reading = (Vec<(Ps4, String)>, String);

/// A traced command being read: its PS4 (in [`Untear::ps4s`]), then (once its first word is
/// read) which command it is and its next piece, where its first word was written, its text so
/// far.
#[derive(Debug, Clone)]
struct Open {
    ps4: usize,
    read: Option<(usize, usize)>,
    at: usize,
    text: String,
}

/// A reading of a stretch so far: the commands open, dash's messages open (where, the text so
/// far, whether its text is read), the commands read and the messages read.
#[derive(Debug, Clone, Default)]
struct Stretch {
    open: Vec<Open>,
    said: Vec<(usize, String, bool)>,
    done: Vec<(usize, usize, String)>,
    told: Vec<(usize, String)>,
}

/// Where a reading is and what is open there: the commands open (their PS4, which command and
/// its next piece) and dash's messages open (whether the text is read).
type Place = (usize, Vec<(usize, Option<(usize, usize)>)>, Vec<bool>);

/// The search [`Split::untear`] runs over a stretch `r`: every way to read each piece as one
/// write of a traced command, of dash's own message (`<file>: N: ` then its text then the
/// newline) or of the person's own line, until two readings differ.
struct Untear<'s> {
    r: &'s str,
    /// Where the last PS4 of `r` starts.
    last_ps4: usize,
    commands: &'s std::collections::HashMap<String, Vec<Command>>,
    /// The PS4s read so far, each with the commands its script can trace.
    ps4s: Vec<(Ps4, &'s [Command])>,
    dir: &'s Path,
    steps: usize,
    /// How many times a reading reached its end, the same one again included.
    ends: usize,
    found: Vec<Reading>,
    /// The places no reading ends from (what is open there, not how it was read).
    dead: std::collections::HashSet<Place>,
}

impl Untear<'_> {
    /// Whether the search is over: two readings, or too many steps.
    fn stopped(&self) -> bool {
        self.steps > UNTEAR_STEPS || self.found.len() > 1
    }

    fn go(&mut self, i: usize, st: Stretch) {
        if self.stopped() {
            return;
        }
        let mut open: Vec<(usize, Option<(usize, usize)>)> = st.open.iter().map(|o| (o.ps4, o.read)).collect();
        open.sort_unstable();
        let mut said: Vec<bool> = st.said.iter().map(|s| s.2).collect();
        said.sort_unstable();
        let place: Place = (i, open, said);
        if self.dead.contains(&place) {
            return;
        }
        let ends = self.ends;
        self.step(i, st);
        if self.ends == ends && !self.stopped() {
            self.dead.insert(place);
        }
    }

    fn step(&mut self, i: usize, st: Stretch) {
        self.steps += 1;
        let r = self.r;
        let rest = &r[i..];
        // a reading ends at a line's end with nothing open and no PS4 to come; the lines after it
        // are the person's
        if st.open.is_empty() && st.said.is_empty() && !st.done.is_empty() && (rest.is_empty() || r[..i].ends_with('\n')) && i > self.last_ps4 {
            let (mut done, mut told) = (st.done.clone(), st.told.clone());
            done.sort_by_key(|d| d.0);
            told.sort_by_key(|t| t.0);
            let lines = done.into_iter().map(|(_, p, t)| (self.ps4s[p].0.clone(), t)).collect();
            let reading: Reading = (lines, told.into_iter().map(|(_, t)| t).chain([rest.to_string()]).collect());
            self.ends += 1;
            if !self.found.contains(&reading) {
                self.found.push(reading);
            }
        }
        if rest.is_empty() {
            return;
        }
        // a PS4: a traced command's line begins, its words to come
        if let Some(after) = rest.strip_prefix(PS4_SH_START) {
            let mut f = after.splitn(3, '\t');
            if let (Some(zero), Some(line), Some(_)) = (f.next(), f.next(), f.next()) {
                if !zero.contains('\n') && !line.contains('\n') {
                    let at = i + PS4_SH_START.len() + zero.len() + line.len() + 2;
                    let ps4 = Ps4 { zero: zero.to_string(), line: line.to_string() };
                    let k = match self.ps4s.iter().position(|(p, _)| *p == ps4) {
                        Some(k) => k,
                        None => {
                            let cands = self.commands.get(zero).map_or(&[][..], Vec::as_slice);
                            self.ps4s.push((ps4, cands));
                            self.ps4s.len() - 1
                        }
                    };
                    let mut next = st;
                    next.open.push(Open { ps4: k, read: None, at, text: String::new() });
                    self.go(at, next);
                }
            }
            return;
        }
        for k in 0..st.open.len() {
            let o = &st.open[k];
            let cands = self.ps4s[o.ps4].1;
            match o.read {
                // its first word: one of its script's commands; open lines of one PS4 are alike
                None if st.open[..k].iter().any(|p| p.read.is_none() && p.ps4 == o.ps4) => {}
                None => {
                    for (c, cmd) in cands.iter().enumerate() {
                        for j in ends(r, i, &cmd.pieces[0].0).into_iter().filter(|j| *j > i) {
                            let mut next = st.clone();
                            next.open[k] = Open { read: Some((c, 1)), at: i, text: r[i..j].to_string(), ..next.open[k].clone() };
                            self.go(j, next);
                            if self.stopped() {
                                return;
                            }
                        }
                    }
                }
                Some((c, p)) => match cands[c].pieces.get(p) {
                    None => {
                        if rest.starts_with('\n') {
                            let mut next = st.clone();
                            let o = next.open.remove(k);
                            next.done.push((o.at, o.ps4, o.text));
                            self.go(i + 1, next);
                        }
                    }
                    Some((segs, optional)) => {
                        for j in ends(r, i, segs).into_iter().filter(|j| *j > i) {
                            let mut next = st.clone();
                            next.open[k].read = Some((c, p + 1));
                            next.open[k].text.push_str(&r[i..j]);
                            self.go(j, next);
                            if self.stopped() {
                                return;
                            }
                        }
                        if *optional {
                            let mut next = st.clone();
                            next.open[k].read = Some((c, p + 1));
                            self.go(i, next);
                        }
                    }
                },
            }
            if self.stopped() {
                return;
            }
        }
        // dash's own message: its text, one write with no newline (it starts with a word, never
        // a space or a colon), then the newline
        for k in 0..st.said.len() {
            if !st.said[k].2 {
                if rest.starts_with([' ', ':']) {
                    continue;
                }
                let stop = rest.find(['\n', '\t']).unwrap_or(rest.len());
                for b in (1..=stop).filter(|b| rest.is_char_boundary(*b)) {
                    let mut next = st.clone();
                    next.said[k].1.push_str(&rest[..b]);
                    next.said[k].2 = true;
                    self.go(i + b, next);
                    if self.stopped() {
                        return;
                    }
                }
            } else if rest.starts_with('\n') {
                let mut next = st.clone();
                let (at, mut text, _) = next.said.remove(k);
                text.push('\n');
                next.told.push((at, text));
                self.go(i + 1, next);
            }
        }
        if let Some(n) = self.message_start(rest) {
            let mut next = st.clone();
            next.said.push((i, rest[..n].to_string(), false));
            self.go(i + n, next);
        }
        // the person's own line in one write (an `echo` to stderr)
        if let Some(e) = rest.find('\n').filter(|e| !rest[..*e].contains('\t')) {
            let mut next = st;
            next.told.push((i, rest[..=e].to_string()));
            self.go(i + e + 1, next);
        }
    }

    /// The length of the start of dash's own message at the head of `rest`: `<file>: N: `, the
    /// file one this stretch traces or one of the project.
    fn message_start(&self, rest: &str) -> Option<usize> {
        let k = rest.find(|c: char| c.is_whitespace() || c == ':').filter(|k| *k > 0 && rest[*k..].starts_with(": "))?;
        let zero = &rest[..k];
        let digits = rest[k + 2..].bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !rest[k + 2 + digits..].starts_with(": ") {
            return None;
        }
        let known = self.commands.contains_key(zero) || (!zero.starts_with('/') && !zero.split('/').any(|s| s == "..") && self.dir.join(zero).is_file());
        known.then_some(k + digits + 4)
    }
}

/// The one line of `source` an expanded command `cmd` can come from: each line is cut at
/// `||`, `&&`, `|`, `;`, `{`, `}`, `then`, `if` and `!`, quotes dropped, and each `$name`, `${…}`
/// or `$(…)` matches any text. `None` when no line or several lines match.
pub fn match_line(source: &str, cmd: &str) -> Option<usize> {
    match match_hits(source, cmd).as_slice() {
        [n] => Some(*n),
        _ => None,
    }
}

/// The lines of `source` an expanded command `cmd` can come from, as [`match_line`] reads them.
fn match_hits(source: &str, cmd: &str) -> Vec<usize> {
    let norm = |s: &str| s.replace(['"', '\''], "").trim().to_string();
    let c = norm(cmd);
    if c.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<usize> = Vec::new();
    for (i, l) in source.lines().enumerate() {
        let parts = split_parts(l);
        if parts.iter().any(|p| {
            let p = norm(p);
            !p.is_empty() && glob_match(&p, &c)
        }) {
            hits.push(i + 1);
        }
    }
    hits.dedup();
    hits
}

fn split_parts(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut cur = String::new();
    // inside quotes, or after a backslash, a separator is the command's own text
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            cur.push(c);
            if c == '\\' && q == '"' && i + 1 < chars.len() {
                cur.push(chars[i + 1]);
                i += 1;
            } else if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            cur.push(c);
            i += 1;
            continue;
        }
        if c == '\\' && i + 1 < chars.len() {
            cur.push(c);
            cur.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        if two == "||" || two == "&&" {
            parts.push(std::mem::take(&mut cur));
            i += 2;
            continue;
        }
        // a pipeline's commands are traced one by one
        if matches!(chars[i], ';' | '{' | '}' | '!' | '|') {
            parts.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(chars[i]);
        i += 1;
    }
    parts.push(cur);
    parts
        .into_iter()
        .flat_map(|p| {
            let t = p.trim().to_string();
            let t = ["then ", "if ", "elif ", "while ", "until ", "do "].iter().fold(t, |t, k| t.strip_prefix(k).map_or(t.clone(), |r| r.trim().to_string()));
            if matches!(t.as_str(), "then" | "fi" | "else" | "do" | "done") {
                None
            } else {
                Some(t)
            }
        })
        .collect()
}

/// Whether `text` matches `pattern`, where each shell expansion in the pattern (`$name`, `${…}`,
/// `$(…)`) matches any run of characters and everything else is literal.
fn glob_match(pattern: &str, text: &str) -> bool {
    // the pattern as literal pieces between expansions
    let p: Vec<char> = pattern.chars().collect();
    let mut pieces: Vec<String> = vec![String::new()];
    let mut i = 0;
    while i < p.len() {
        if p[i] == '$' && i + 1 < p.len() {
            let j = match p[i + 1] {
                '{' => p[i..].iter().position(|c| *c == '}').map_or(p.len(), |k| i + k + 1),
                '(' => p[i..].iter().position(|c| *c == ')').map_or(p.len(), |k| i + k + 1),
                c if c.is_ascii_alphanumeric() || matches!(c, '_' | '?' | '@' | '*' | '#' | '$' | '!' | '-') => {
                    let mut k = i + 1;
                    if c.is_ascii_alphabetic() || c == '_' {
                        while k < p.len() && (p[k].is_ascii_alphanumeric() || p[k] == '_') {
                            k += 1;
                        }
                    } else {
                        k += 1;
                    }
                    k
                }
                _ => {
                    if let Some(s) = pieces.last_mut() {
                        s.push('$');
                    }
                    i += 1;
                    continue;
                }
            };
            pieces.push(String::new());
            i = j;
            continue;
        }
        if let Some(s) = pieces.last_mut() {
            s.push(p[i]);
        }
        i += 1;
    }
    let n = pieces.len();
    if n == 1 {
        return pieces[0] == text;
    }
    let (first, last) = (&pieces[0], &pieces[n - 1]);
    if !text.starts_with(first.as_str()) || !text[first.len()..].ends_with(last.as_str()) || text.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for piece in &pieces[1..n - 1] {
        match rest.find(piece.as_str()) {
            Some(k) => rest = &rest[k + piece.len()..],
            None => return false,
        }
    }
    true
}

/// The first `sh` on PATH, as PATH names it (busybox is `sh` by the name it is run as).
fn first_sh() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).map(|d| d.join("sh")).find(|p| p.is_file())
}

/// The `sh` Familiar's run of an `sh` that is not bash puts first on PATH: it starts `real` with
/// `-x` when its stdout and stderr are the run's own (`FAMILIAR_AI_SH_OUT`, `FAMILIAR_AI_SH_ERR`:
/// those of the shell Familiar starts), so a script a test script runs is traced as bash's
/// `SHELLOPTS` would trace it. A shell whose output the script captures, pipes or sends
/// elsewhere runs as it is: its trace would land in what the script reads, and dash writes a
/// trace line in pieces, so two shells tracing at once in a pipeline can tear each other's
/// lines. Where `/proc` cannot say, every shell runs as it is.
pub fn sh_wrapper(real: &Path) -> String {
    let r = real.display();
    format!("#!{r}\nif [ -n \"${{FAMILIAR_AI_SH_ERR:-}}\" ] && [ /proc/self/fd/2 -ef \"$FAMILIAR_AI_SH_ERR\" ] && [ /proc/self/fd/1 -ef \"${{FAMILIAR_AI_SH_OUT:-}}\" ]; then exec '{r}' -x \"$@\"; fi\nexec '{r}' \"$@\"\n")
}

/// One run's trace: the side folder, and how the run is changed for it.
#[derive(Debug)]
pub struct Trace {
    side: PathBuf,
    kit: Option<PathBuf>,
    bash: bool,
    scripts: Vec<ShScript>,
    /// The folder holding [`sh_wrapper`], first on the run's PATH (an `sh` that is not bash).
    wrap: Option<PathBuf>,
}

impl Trace {
    /// A trace for a run of `command` in `dir`, its side folder made under the temp folder.
    pub fn new(command: &str, dir: &Path) -> Option<Trace> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let side = std::env::temp_dir().join(format!("familiar-ai-trace-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&side);
        std::fs::create_dir_all(&side).ok()?;
        let bash = sh_is_bash();
        let scripts = if bash { Vec::new() } else { sh_scripts(command, dir) };
        let wrap = (!scripts.is_empty()).then(first_sh).flatten().and_then(|real| {
            use std::os::unix::fs::PermissionsExt;
            let bin = side.join("bin");
            std::fs::create_dir_all(&bin).ok()?;
            std::fs::write(bin.join("sh"), sh_wrapper(&real)).ok()?;
            std::fs::set_permissions(bin.join("sh"), std::fs::Permissions::from_mode(0o755)).ok()?;
            Some(bin)
        });
        Some(Trace { side, kit: kit_path(), bash, scripts, wrap })
    }

    /// The side file xtrace is written to (through [`TRACE_FD`]).
    pub fn xtrace_file(&self) -> PathBuf {
        self.side.join("xtrace")
    }

    /// Whether the run opens [`TRACE_FD`] on [`Trace::xtrace_file`].
    pub fn opens_fd(&self) -> bool {
        self.bash
    }

    /// The env the run gets.
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if let Some(k) = &self.kit {
            // the person's own BASH_ENV is loaded by the kit
            if let Some(own) = std::env::var_os("BASH_ENV").filter(|v| !v.is_empty()) {
                env.push(("FAMILIAR_AI_BASH_ENV".to_string(), own.to_string_lossy().into_owned()));
            }
            env.push(("BASH_ENV".to_string(), k.to_string_lossy().into_owned()));
            env.push(("FAMILIAR_AI_SH_TRACE".to_string(), self.side.join("trace.tsv").to_string_lossy().into_owned()));
        }
        if self.bash {
            env.push(("SHELLOPTS".to_string(), "xtrace".to_string()));
            env.push(("BASH_XTRACEFD".to_string(), TRACE_FD.to_string()));
            env.push(("PS4".to_string(), PS4.to_string()));
        } else if !self.scripts.is_empty() {
            env.push(("PS4".to_string(), PS4_SH.to_string()));
        }
        if let Some(bin) = &self.wrap {
            let path = std::env::var_os("PATH").unwrap_or_default();
            if let Ok(p) = std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&path))) {
                env.push(("PATH".to_string(), p.to_string_lossy().into_owned()));
            }
        }
        env
    }

    /// The command Familiar's run starts: for an `sh` that is not bash, `-x` added for each of
    /// its scripts and its own level traced (`set -x`), so the trace says which script decided,
    /// as bash's does; its stderr named for [`sh_wrapper`].
    pub fn command(&self, command: &str) -> String {
        if self.bash || self.scripts.is_empty() {
            command.to_string()
        } else {
            format!("FAMILIAR_AI_SH_OUT=/proc/$$/fd/1 FAMILIAR_AI_SH_ERR=/proc/$$/fd/2; export FAMILIAR_AI_SH_OUT FAMILIAR_AI_SH_ERR\nset -x\n{}", with_x(command, &self.scripts))
        }
    }

    /// After the run: the trace as the report stream, and `stderr` without a non-bash shell's
    /// trace lines; the side folder is removed.
    pub fn finish(self, stderr: &mut String, dir: &Path) -> String {
        let read = |name: &str| std::fs::read(self.side.join(name)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let mut report = read("trace.tsv");
        report.push_str(&read("xtrace"));
        if !self.scripts.is_empty() {
            let d = split_dash(stderr, &self.scripts, dir);
            *stderr = d.stderr;
            report.push_str(&d.trace);
        }
        let _ = std::fs::remove_dir_all(&self.side);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_sh_script_gets_x_in_familiars_run_only() {
        let d = std::env::temp_dir();
        let s = sh_scripts("sh tests/a.sh && FOO=1 sh tests/b.sh x; bash c.sh; sh -e d.sh", &d);
        assert_eq!(s.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["tests/a.sh", "tests/b.sh"]);
        assert_eq!(with_x("sh tests/a.sh && FOO=1 sh tests/b.sh x", &s), "sh -x tests/a.sh && FOO=1 sh -x tests/b.sh x");
    }

    #[test]
    fn an_expanded_command_finds_its_one_line() {
        let src = "#!/bin/sh\nf=/tmp/a.txt\necho alpha > \"$f\"\ngrep -q gamma \"$f\"\n";
        assert_eq!(match_line(src, "grep -q gamma /tmp/a.txt"), Some(4));
        let twice = "grep -q a x\ngrep -q a x\n";
        assert_eq!(match_line(twice, "grep -q a x"), None, "a check repeated on two lines: can't tell");
        assert_eq!(match_line("[ \"$n\" = 3 ] || exit 1\n", "[ 2 = 3 ]"), Some(1));
        assert_eq!(match_line("if ! grep -q x f; then exit 1; fi\n", "grep -q x f"), Some(1));
    }

    /// An `sh` traces each command of a pipeline on its own line, so a pipeline's commands are
    /// matched one by one: dash on Ubuntu, where `LINENO` is empty (hosted CI, 2026-10-26).
    #[test]
    fn a_command_in_a_pipeline_finds_its_line() {
        let src = ". src/app.sh; greet | grep -qx hello\n";
        assert_eq!(match_line(src, "grep -qx hello"), Some(1));
        assert_eq!(match_line(src, "greet"), Some(1));
        assert_eq!(match_line("a || b | c\n", "c"), Some(1));
        assert_eq!(match_line("a || b\n", "a || b"), None, "an || is still two commands");
    }

    /// A `;` or `|` inside quotes is the command's own text, not a cut between commands: a test
    /// that hands code to python3 in quotes, then pipes it to a check (hosted CI, 2026-10-26).
    #[test]
    fn a_separator_inside_quotes_is_part_of_the_command() {
        let src = "python3 -c 'import sys; print(m.greet())' | grep -qx hello\n";
        assert_eq!(split_parts(src.trim_end()), vec!["python3 -c 'import sys; print(m.greet())'", "grep -qx hello"]);
        assert_eq!(match_line(src, "python3 -c import sys; print(m.greet())"), Some(1));
        assert_eq!(match_line(src, "grep -qx hello"), Some(1));
        assert_eq!(split_parts(r#"echo "a | b; c" && grep -q x f"#), vec![r#"echo "a | b; c""#, "grep -q x f"]);
        assert_eq!(split_parts(r"echo a\;b; true"), vec![r"echo a\;b", "true"], "an escaped ; is no cut either");
    }

    /// A command that runs its scripts through a variable (`for f in tests/test_*.sh; do sh "$f"
    /// || exit 1; done`, the sh stack's usual command) gets `-x` there too: on Ubuntu, where sh is
    /// dash, a script with no `-x` leaves no trace and its run reads as can't tell (hosted CI,
    /// 2026-10-26).
    #[test]
    fn a_script_named_by_an_expansion_gets_x_too() {
        let d = std::env::temp_dir();
        let cmd = "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done";
        assert_eq!(with_x(cmd, &sh_scripts(cmd, &d)), "for f in tests/test_*.sh; do sh -x \"$f\" || exit 1; done");
        let cmd = "for f in $(git diff --name-only a HEAD -- tests); do sh $f || exit 1; done";
        assert_eq!(with_x(cmd, &sh_scripts(cmd, &d)), "for f in $(git diff --name-only a HEAD -- tests); do sh -x $f || exit 1; done");
    }

    fn two_scripts(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("familiar-ai-dash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("tests")).unwrap();
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("src/app.sh"), "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").unwrap();
        std::fs::write(d.join("tests/test_a.sh"), "grep -q hi src/app.sh\n").unwrap();
        std::fs::write(d.join("tests/test_b.sh"), "echo checking\ngrep -q hello src/app.sh\n").unwrap();
        d
    }

    /// What `split_dash` keeps of the for-loop over `two_scripts`: the command's own level (the
    /// shell Familiar starts) has no file; each script's lines name it, matched from its text.
    const TWO_KEPT: &str = "+familiar-ai\t\t\tsh -x tests/test_a.sh\n+familiar-ai\ttests/test_a.sh\t1\tgrep -q hi src/app.sh\n+familiar-ai\t\t\tsh -x tests/test_b.sh\n+familiar-ai\ttests/test_b.sh\t1\techo checking\n+familiar-ai\ttests/test_b.sh\t2\tgrep -q hello src/app.sh\n+familiar-ai\t\t\texit 1\n";

    /// dash's trace names no file and, on Debian, no line: [`PS4_SH`] carries the shell's `$0`,
    /// so each line names the script it is in when a command runs several (or one through a
    /// variable), and the shell Familiar starts (`$0` is `sh`) is the command's own level.
    #[test]
    fn each_line_of_an_sh_trace_names_its_own_script() {
        let d = two_scripts("zero");
        let cmd = "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done";
        let stderr = "+familiar-ai-sh\tsh\t\tsh -x tests/test_a.sh\n+familiar-ai-sh\ttests/test_a.sh\t\tgrep -q hi src/app.sh\n+familiar-ai-sh\tsh\t\tsh -x tests/test_b.sh\n+familiar-ai-sh\ttests/test_b.sh\t\techo checking\nboom\n+familiar-ai-sh\ttests/test_b.sh\t\tgrep -q hello src/app.sh\n+familiar-ai-sh\tsh\t\texit 1\n";
        let got = split_dash(stderr, &sh_scripts(cmd, &d), &d);
        assert_eq!(got.stderr, "boom\n");
        assert_eq!(got.trace, TWO_KEPT);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A `dash` on this machine: on PATH, else where Debian puts it; none on Fedora's CI.
    fn a_dash() -> Option<PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path).map(|p| p.join("dash")).chain([PathBuf::from("/usr/bin/dash"), PathBuf::from("/bin/dash")]).find(|p| p.is_file())
    }

    /// The trace a real dash writes under [`PS4_SH`] for Familiar's run of the for-loop (its own
    /// level traced, `-x` for each script) is what `split_dash` reads. Runs only where a dash is
    /// installed (Ubuntu's /bin/sh); the test above reads its shape everywhere.
    #[test]
    fn a_real_dash_trace_is_read_line_by_line() {
        let Some(dash) = a_dash() else {
            eprintln!("no dash on this machine: the real dash run is skipped");
            return;
        };
        let d = two_scripts("real");
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink(&dash, bin.join("sh")).unwrap();
        let cmd = "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done";
        let scripts = sh_scripts(cmd, &d);
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("set -x\n{}", with_x(cmd, &scripts)))
            .current_dir(&d)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("PS4", PS4_SH)
            .output()
            .unwrap();
        let got = split_dash(&String::from_utf8_lossy(&out.stderr), &scripts, &d);
        assert_eq!((out.status.code(), got.stderr.as_str(), got.trace.as_str()), (Some(1), "", TWO_KEPT));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// dash gives a sourced script's lines the `$0` of the script that sourced it: a line that
    /// script has none of is the sourced script's, when that one has one line for it (bash names
    /// the sourced file, `BASH_SOURCE`). A pipeline's commands come in either order.
    #[test]
    fn a_line_of_a_sourced_script_is_given_that_script() {
        let d = std::env::temp_dir().join(format!("familiar-ai-dash-sourced-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("tests")).unwrap();
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("src/app.sh"), "#!/bin/sh\ngreet() {\n  exit 3\n}\nbye() { echo bye; }\n").unwrap();
        std::fs::write(d.join("tests/t.sh"), ". src/app.sh\ngreet\n").unwrap();
        let stderr = "+familiar-ai-sh\tsh\t\tsh -x tests/t.sh\n+familiar-ai-sh\ttests/t.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/t.sh\t\tgreet\n+familiar-ai-sh\ttests/t.sh\t\texit 3\n";
        let got = split_dash(stderr, &sh_scripts("sh tests/t.sh", &d), &d);
        assert_eq!(got.trace, "+familiar-ai\t\t\tsh -x tests/t.sh\n+familiar-ai\ttests/t.sh\t1\t. src/app.sh\n+familiar-ai\ttests/t.sh\t2\tgreet\n+familiar-ai\tsrc/app.sh\t3\texit 3\n");
        std::fs::write(d.join("tests/t.sh"), ". src/app.sh; bye | grep -qx hello\n").unwrap();
        let stderr = "+familiar-ai-sh\ttests/t.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/t.sh\t\tbye\n+familiar-ai-sh\ttests/t.sh\t\tgrep -qx hello\n+familiar-ai-sh\ttests/t.sh\t\techo bye\n";
        let got = split_dash(stderr, &sh_scripts("sh tests/t.sh", &d), &d);
        assert_eq!(got.trace, "+familiar-ai\ttests/t.sh\t1\t. src/app.sh\n+familiar-ai\ttests/t.sh\t1\tbye\n+familiar-ai\ttests/t.sh\t1\tgrep -qx hello\n+familiar-ai\tsrc/app.sh\t5\techo bye\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";

    /// A stand-in: `src/app.sh` ([`APP`]) and the test `tests/test_c1.sh` (`body`).
    fn stand_in(name: &str, body: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("familiar-ai-dash-torn-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::create_dir_all(d.join("tests")).unwrap();
        std::fs::write(d.join("src/app.sh"), APP).unwrap();
        std::fs::write(d.join("tests/test_c1.sh"), body).unwrap();
        d
    }

    /// The outer level's line of Familiar's run of `sh tests/test_c1.sh` under Ubuntu's dash.
    const OUTER: &str = "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n";

    /// Ubuntu's dash writes a trace line in pieces (PS4, each word, the newline), so the commands
    /// of a pipeline, traced at once, tear each other's lines (hosted CI, 2026-10-26). A torn
    /// stretch reads as its commands' untorn lines: each is one of the script's commands (or of a
    /// script it sourced) whose words the stretch holds, written in the order its first word was;
    /// dash's own message torn into it, and a line after it, are the person's stderr. Each stderr
    /// is Ubuntu's dash's in Familiar's run of the stand-in, verbatim.
    #[test]
    fn a_torn_stretch_reads_as_its_commands_untorn_lines() {
        let d = stand_in("pipe", "sh src/app.sh | grep -qx hello\n");
        let s = sh_scripts("sh tests/test_c1.sh", &d);
        let untorn = "+familiar-ai\t\t\tsh -x tests/test_c1.sh\n+familiar-ai\ttests/test_c1.sh\t1\tsh src/app.sh\n+familiar-ai\ttests/test_c1.sh\t1\tgrep -qx hello\n";
        let grep_first = "+familiar-ai\t\t\tsh -x tests/test_c1.sh\n+familiar-ai\ttests/test_c1.sh\t1\tgrep -qx hello\n+familiar-ai\ttests/test_c1.sh\t1\tsh src/app.sh\n";
        for (torn, trace, stderr) in [
            ("+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\ngrep -qx hello\n", untorn, ""),
            ("+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh\n -qx hello\n", untorn, ""),
            ("+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh -qx src/app.sh hello\n\n", grep_first, ""),
            ("+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx+familiar-ai-sh\ttests/test_c1.sh\t\t hello\nsh src/app.sh\n", grep_first, ""),
            ("+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.shgrep\n -qx hello\n", untorn, ""),
            ("+familiar-ai-sh\ttests/test_c1.sh\t\tsh+familiar-ai-sh\ttests/test_c1.sh\t\t src/app.sh\ngrep -qx hello\n", untorn, ""),
            // src/app.sh writes `oops` to stderr after both lines
            ("+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh -qx\n hello\noops\n", untorn, "oops\n"),
        ] {
            let got = split_dash(&format!("{OUTER}{torn}"), &s, &d);
            assert_eq!((got.trace.as_str(), got.stderr.as_str()), (trace, stderr), "{torn:?}");
        }
        let _ = std::fs::remove_dir_all(&d);

        // a function the test sourced traces its body in the pipeline's own shell: three lines
        let d = stand_in("sourced", ". src/app.sh >/dev/null; greet | grep -qx hello\n");
        let s = sh_scripts("sh tests/test_c1.sh", &d);
        let before = "+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n";
        let kept = "+familiar-ai\t\t\tsh -x tests/test_c1.sh\n+familiar-ai\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai\ttests/test_c1.sh\t1\tgreet\n+familiar-ai\tsrc/app.sh\t2\techo hi\n";
        let torn = "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepgreet\n -qx+familiar-ai-sh\ttests/test_c1.sh\t\t helloecho\n hi\n";
        let got = split_dash(&format!("{OUTER}{before}{torn}"), &s, &d);
        let want = format!("{kept}+familiar-ai\ttests/test_c1.sh\t1\tgrep -qx hello\n+familiar-ai\ttests/test_c1.sh\t1\tgreet\n+familiar-ai\tsrc/app.sh\t2\techo hi\n");
        assert_eq!((got.trace.as_str(), got.stderr.as_str()), (want.as_str(), ""));
        let _ = std::fs::remove_dir_all(&d);

        // dash's own `not found` torn into the trace is the person's line again
        let d = stand_in("missing", ". src/app.sh >/dev/null; farewell | grep -qx goodbye\n");
        let s = sh_scripts("sh tests/test_c1.sh", &d);
        let before = format!("{before}+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n");
        // its test has no `greet`: the one the sourcing ran is src/app.sh's line 3
        let kept = kept.replace("tests/test_c1.sh\t1\tgreet", "src/app.sh\t3\tgreet");
        let want = format!("{kept}+familiar-ai\ttests/test_c1.sh\t1\tfarewell\n+familiar-ai\ttests/test_c1.sh\t1\tgrep -qx goodbye\n");
        for torn in [
            "+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: farewell: not found\ngrep -qx goodbye\n",
            "tests/test_c1.sh: 1: +familiar-ai-sh\ttests/test_c1.sh\t\tfarewell: not found\ngrep -qx goodbye\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\tgreptests/test_c1.sh: 1:  -qxfarewell: not found goodbye\n\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: grepfarewell: not found\n -qx goodbye\n",
            // the message's text glued to a word: its text starts with a word, never `: `
            "+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: farewell: not foundgrep -qx goodbye\n\n",
        ] {
            let got = split_dash(&format!("{OUTER}{before}{torn}"), &s, &d);
            assert_eq!((got.trace.as_str(), got.stderr.as_str()), (want.as_str(), "tests/test_c1.sh: 1: farewell: not found\n"), "{torn:?}");
        }
        // farewell's own line torn: a stretch from `. src/app.sh` on, before its sourcing is read,
        // cannot be read; from the torn line on it can (Familiar's own run under load)
        let before = before.replace("+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n", "");
        let torn = "+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngreptests/test_c1.sh: 1:  -qxfarewell: not found goodbye\n\n";
        let got = split_dash(&format!("{OUTER}{before}{torn}"), &s, &d);
        assert_eq!((got.trace.as_str(), got.stderr.as_str()), (want.as_str(), "tests/test_c1.sh: 1: farewell: not found\n"));
        let _ = std::fs::remove_dir_all(&d);

        // a line of the person's written at once lands inside the stretch: it stays theirs
        let d = stand_in("oops", "sh src/app.sh | grep -qx hello\n");
        std::fs::write(d.join("src/app.sh"), "#!/bin/sh\necho oops >&2\necho hi\n").unwrap();
        let s = sh_scripts("sh tests/test_c1.sh", &d);
        for torn in [
            "+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrepoops\n -qx hello\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\tsh+familiar-ai-sh\ttests/test_c1.sh\t\t src/app.sh\noops\ngrep -qx hello\n",
        ] {
            let got = split_dash(&format!("{OUTER}{torn}"), &s, &d);
            assert_eq!((got.trace.as_str(), got.stderr.as_str()), (untorn, "oops\n"), "{torn:?}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A torn stretch is read only when one reading of it is possible: one that two lines of the
    /// script could explain, or that holds words no command of the script has, stays as it was
    /// (can't tell).
    #[test]
    fn a_torn_stretch_two_readings_could_explain_stays_as_it_was() {
        let torn = "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh -qx hello\n\n";
        let d = stand_in("two", "sh src/app.sh | grep -qx hello\nsh -qx | grep src/app.sh hello\n");
        let got = split_dash(&format!("{OUTER}{torn}"), &sh_scripts("sh tests/test_c1.sh", &d), &d);
        let kept = "+familiar-ai\t\t\tsh -x tests/test_c1.sh\n+familiar-ai\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh -qx hello\n";
        assert_eq!((got.trace.as_str(), got.stderr.as_str()), (kept, "\n"));
        let _ = std::fs::remove_dir_all(&d);
        let d = stand_in("foreign", "sh src/app.sh | grep -qx hi\n");
        let got = split_dash(&format!("{OUTER}{torn}"), &sh_scripts("sh tests/test_c1.sh", &d), &d);
        assert_eq!((got.trace.as_str(), got.stderr.as_str()), (kept, "\n"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dash_trace_lines_leave_stderr() {
        let d = std::env::temp_dir().join(format!("familiar-ai-dash-{}", std::process::id()));
        std::fs::create_dir_all(d.join("t")).unwrap();
        std::fs::write(d.join("t/x.sh"), "#!/bin/sh\necho hi\ngrep -q z t/x.sh\n").unwrap();
        let s = sh_scripts("sh t/x.sh", &d);
        let got = split_dash("+familiar-ai\t\techo hi\nboom\n+familiar-ai\t\tgrep -q z t/x.sh\n", &s, &d);
        assert_eq!(got.stderr, "boom\n");
        assert_eq!(got.trace, "+familiar-ai\tt/x.sh\t2\techo hi\n+familiar-ai\tt/x.sh\t3\tgrep -q z t/x.sh\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Dash tore a pipeline whose first command carries quoted code with `;` in it: each tear reads as
    /// the untorn lines (verbatim captures of Ubuntu's dash, 2026-10-26; hosted CI's
    /// the_binary_runs_the_existing_tests_that_import_the_fix).
    #[test]
    fn a_torn_pipeline_with_quoted_code_reads_as_its_untorn_lines() {
        let d = stand_in("quoted", "python3 -c 'import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())' | grep -qx hello\n");
        let s = sh_scripts("sh tests/test_c1.sh", &d);
        let python = "+familiar-ai\ttests/test_c1.sh\t1\tpython3 -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\n";
        let grep = "+familiar-ai\ttests/test_c1.sh\t1\tgrep -qx hello\n";
        for torn in [
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tpython3 -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())grep\n -qx hello\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tpython3 -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\ngrep -qx hello\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tpython3 -cgrep import sys; sys.path.insert(0, \"src\"); import m; print(m.greet()) -qx\n hello\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\npython3 -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreppython3 -qx -c hello\n import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hellopython3\n -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\n",
            "+familiar-ai-sh\ttests/test_c1.sh\t\tpython3+familiar-ai-sh\ttests/test_c1.sh\t\t -c import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())\ngrep -qx hello\n",
        ] {
            let got = split_dash(&format!("{OUTER}{torn}"), &s, &d);
            assert!(got.trace.contains(python) && got.trace.contains(grep), "{torn:?} read as {:?}", got.trace);
            assert_eq!(got.stderr, "", "{torn:?}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

}
