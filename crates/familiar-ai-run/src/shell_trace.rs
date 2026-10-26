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

/// The xtrace prefix for an `sh` that is not bash: the marker and the line (empty where the
/// shell has no `LINENO`).
pub const PS4_SH: &str = "+familiar-ai\t${LINENO}\t";

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
            let Some((_, script)) = words.get(k + 1).filter(|(_, w)| !w.starts_with('-') && !w.contains(['$', '`', '\'', '"'])) else { continue };
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
/// given the script's file (only when one script ran; else none) and its line: the shell's own
/// `LINENO`, else the one line of the script whose text the expanded command matches (a check
/// repeated on two lines matches neither: the line stays empty, can't tell).
pub fn split_dash(stderr: &str, scripts: &[ShScript], dir: &Path) -> DashTrace {
    let one = match scripts {
        [s] => Some(s),
        _ => None,
    };
    let source = one.and_then(|s| std::fs::read_to_string(dir.join(&s.path)).ok());
    let mut out = DashTrace::default();
    for l in stderr.split_inclusive('\n') {
        let body = l.trim_start_matches('+');
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
/// `||`, `&&`, `;`, `{`, `}`, `then`, `if` and `!`, quotes dropped, and each `$name`, `${…}`
/// or `$(…)` matches any text. `None` when no line or several lines match.
pub fn match_line(source: &str, cmd: &str) -> Option<usize> {
    let norm = |s: &str| s.replace(['"', '\''], "").trim().to_string();
    let c = norm(cmd);
    if c.is_empty() {
        return None;
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
    match hits.as_slice() {
        [n] => Some(*n),
        _ => None,
    }
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
        if matches!(chars[i], ';' | '{' | '}' | '!') {
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

/// One run's trace: the side folder, and how the run is changed for it.
#[derive(Debug)]
pub struct Trace {
    side: PathBuf,
    kit: Option<PathBuf>,
    bash: bool,
    scripts: Vec<ShScript>,
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
        Some(Trace { side, kit: kit_path(), bash, scripts })
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
        env
    }

    /// The command Familiar's run starts: `-x` added for an `sh` that is not bash.
    pub fn command(&self, command: &str) -> String {
        if self.bash {
            command.to_string()
        } else {
            with_x(command, &self.scripts)
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
