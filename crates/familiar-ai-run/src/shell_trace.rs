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
pub fn split_dash(stderr: &str, scripts: &[ShScript], dir: &Path) -> DashTrace {
    let one = match scripts {
        [s] => Some(s),
        _ => None,
    };
    let source = one.and_then(|s| std::fs::read_to_string(dir.join(&s.path)).ok());
    let mut texts: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    // the project scripts each script sourced (`. file`): dash gives their lines its `$0`
    let mut sourced: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut out = DashTrace::default();
    for l in stderr.split_inclusive('\n') {
        let body = l.trim_start_matches('+');
        if let Some(rest) = body.strip_prefix(MARK_SH).filter(|_| body.len() < l.len()) {
            let mut f = rest.trim_end_matches('\n').splitn(3, '\t');
            let (zero, line, cmd) = (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""));
            let file = if SHELLS.contains(&basename(zero)) || basename(zero) == "bash" { "" } else { zero.strip_prefix("./").unwrap_or(zero) };
            let (at, line) = if !line.trim().is_empty() || file.is_empty() {
                (file.to_string(), line.trim().to_string())
            } else {
                let theirs = sourced.get(file).cloned().unwrap_or_default();
                for f in std::iter::once(file).chain(theirs.iter().map(String::as_str)) {
                    texts.entry(f.to_string()).or_insert_with(|| std::fs::read_to_string(dir.join(f)).ok());
                }
                let hits = |f: &str| texts.get(f).cloned().flatten().map(|t| match_hits(&t, cmd)).unwrap_or_default();
                match hits(file).as_slice() {
                    [n] => (file.to_string(), n.to_string()),
                    // none of its own: the one script it sourced with one line for it
                    [] => {
                        let found: Vec<(String, usize)> = theirs.iter().filter_map(|f| if let [n] = hits(f).as_slice() { Some((f.clone(), *n)) } else { None }).collect();
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
                if !file.is_empty() && dir.join(p).is_file() {
                    sourced.entry(file.to_string()).or_default().push(p.to_string());
                }
            }
            out.trace.push_str(&format!("+{MARK}{at}\t{line}\t{cmd}\n"));
            continue;
        }
        let Some(rest) = body.strip_prefix(MARK).filter(|_| body.len() < l.len()) else {
            out.stderr.push_str(l);
            continue;
        };
        let rest = rest.trim_end_matches('\n');
        let (line, cmd) = rest.split_once('\t').unwrap_or(("", rest));
        let line = if line.trim().is_empty() { source.as_deref().and_then(|s| match_line(s, cmd)).map(|n| n.to_string()).unwrap_or_default() } else { line.trim().to_string() };
        let file = one.map_or("", |s| s.path.as_str());
        out.trace.push_str(&format!("+{MARK}{file}\t{line}\t{cmd}\n"));
    }
    out
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
    while i < chars.len() {
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
}
