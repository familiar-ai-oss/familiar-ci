//! Decide a PreToolUse Bash call's file writes as Edits: each path the command
//! text writes inside the project is decided exactly as an Edit of that file.
//! This reads only the text, so it turns back the obvious forms and nothing
//! more; a file written past it is still outside any stamp, and CI decides.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use familiar_ai_facts::Log;
use familiar_ai_rules::{lifecycle, Event, View};

use crate::decide::decision::{self, Decision};
use crate::door::folder::{is_separator, strip_redirect};
use crate::input::hook_input::{HookEvent, HookInput};
use crate::door::shell_forms::{args_after, files_of, is_python, program_name, python_writes, shell_words, split_heredocs};
use crate::teach::words::{LANE_BY_HAND, LEAD};

/// The one line after the Edit's own words.
pub const COUNTS_AS_EDIT: &str = "(a shell write counts as an edit)";

/// What a `patch` or `git apply` edits: a file, path unknown.
pub const A_FILE: &str = "a file";

/// A file the command's text writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    /// The path as the command spells it.
    Path(String),
    /// A file whose path the text does not carry (`patch`, `git apply`).
    Unknown,
}

/// The first refusal among the written paths, with `COUNTS_AS_EDIT` added,
/// else an allow carrying every told line. `None` when the call is not a
/// PreToolUse Bash call or writes nothing inside the project.
pub fn decision(input: &HookInput, log: &Log, root: &Path, now: f64, judge_on: bool) -> Option<Decision> {
    if input.event != HookEvent::PreToolUse || input.file_path().is_some() {
        return None;
    }
    let command = input.command()?;
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    let active = lifecycle::edit_lane(&View::new(log), input.agent_conversation_id().as_deref());
    let mut told: Vec<String> = Vec::new();
    let mut any = false;
    for w in written_paths(command) {
        let file_path = match w {
            Written::Unknown => A_FILE.to_string(),
            Written::Path(p) => match inside_project(root, cwd, &p) {
                Some(rel) => rel,
                None => continue,
            },
        };
        any = true;
        let event = Event::FileEdit { criterion_id: active.clone(), file_path };
        match decision::decide(log, root, &[event], now, judge_on) {
            Decision::Refuse { reason } => {
                return Some(Decision::Refuse { reason: format!("{reason}\n{COUNTS_AS_EDIT}") });
            }
            Decision::Allow { told: Some(t) } => told.push(t),
            Decision::Allow { told: None } => {}
        }
    }
    any.then(|| Decision::Allow { told: if told.is_empty() { None } else { Some(told.join("\n")) } })
}

/// The refusal of an agent's command that starts a lane by hand (`claude -p`)
/// or does what only the person does (`familiar-ai on` or `off`, `share on` or
/// `off`, a card's answer), read from the command's text alone, so unsetting the session's
/// variable changes nothing. The person's own terminal passes no hook. While
/// switched off (`governed` false) only the switch is refused.
pub fn agent_command_refusal(input: &HookInput, governed: bool) -> Option<String> {
    if input.event != HookEvent::PreToolUse || input.file_path().is_some() {
        return None;
    }
    let (shell, bodies) = split_heredocs(input.command()?);
    let names_approve = shell.contains("approve");
    for (program, args) in simple_commands(&shell) {
        let first = args.first().map(String::as_str);
        match program_name(&program) {
            "familiar-ai" if matches!(first, Some("on" | "off")) => return first.map(crate::door::switch::refusal),
            // The person's one-time yes to sharing gap recordings (plan A-6), refused while off too.
            "familiar-ai" if first == Some("share") && matches!(args.get(1).map(String::as_str), Some("on" | "off")) => return args.get(1).map(|to| crate::door::share::refusal(to)),
            // The person's signature on a card; refused even while switched off,
            // and fail closed when the program or its subcommand is an expansion
            // the text does not spell (`$f approve`, `familiar-ai $a`).
            "familiar-ai" if first == Some("approve") || (first.is_some_and(expands) && names_approve) => return Some(crate::door::approval::REFUSAL.to_string()),
            _ if expands(&program) && names_approve => return Some(crate::door::approval::REFUSAL.to_string()),
            _ if !governed => {}
            "claude" if args.iter().any(|a| a == "-p" || a == "--print" || a.starts_with("--print=")) => return Some(LANE_BY_HAND.to_string()),
            "familiar-ai" if (first == Some("claim") && args.get(1).map(String::as_str) == Some("hold_answered")) || (first == Some("plan") && bodies.iter().any(|b| b.contains("hold_answered"))) => {
                let command = crate::door::child::shown(&std::iter::once("familiar-ai".to_string()).chain(args).collect::<Vec<_>>());
                return Some(format!("{LEAD}not claimed yet · {}", crate::door::claim::ClaimError::PersonsCard { command, card: String::new() }));
            }
            _ => {}
        }
    }
    None
}

/// B-4 (plan §6.7): a subagent's own `familiar-ai claim criterion_assigned`, alone in its
/// command and without `--agent_conversation_id`, as the tool input to run instead: the same
/// input with `--agent_conversation_id session/agent_id` appended, so the part's holder is the
/// subagent and not its session. An explicit flag, a compound command or the main
/// conversation's call is left as typed (`None`).
pub fn assigned_rewrite(input: &HookInput) -> Option<serde_json::Value> {
    if input.event != HookEvent::PreToolUse || !input.in_subagent() || input.tool_name.as_deref() != Some("Bash") {
        return None;
    }
    let command = input.command()?;
    let (shell, bodies) = split_heredocs(command);
    let commands = simple_commands(&shell);
    let [(program, args)] = commands.as_slice() else {
        return None;
    };
    let mine = program_name(program) == "familiar-ai"
        && args.first().map(String::as_str) == Some("claim")
        && args.get(1).map(String::as_str) == Some("criterion_assigned")
        && !args.iter().any(|a| a.starts_with("--agent_conversation_id"));
    // A trailing comment or `&` would swallow or background the appended flag: left as typed.
    if !mine || !bodies.is_empty() || command.contains('\n') || command.contains('#') || command.trim_end().ends_with('&') {
        return None;
    }
    let id = input.agent_conversation_id()?;
    let mut updated = input.tool_input.clone()?;
    updated["command"] = serde_json::Value::from(format!("{} --agent_conversation_id {id}", command.trim_end()));
    Some(updated)
}

/// Each simple command in the shell text with its arguments, past assignments
/// and wrappers, and the commands inside a `sh -c` too.
fn simple_commands(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let words: Vec<String> = shell_words(line).into_iter().map(|w| w.text).collect();
        for part in words.split(|w| is_separator(w)) {
            let Some((program, args)) = past_wrappers(part) else {
                continue;
            };
            if program_name(program) == "eval" {
                out.extend(simple_commands(&args.join(" ")));
            }
            if matches!(program_name(program), "sh" | "bash" | "zsh" | "dash") {
                let c = args.iter().position(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('c'));
                if let Some(inner) = c.and_then(|i| args.get(i + 1)) {
                    out.extend(simple_commands(inner));
                }
            }
            out.push((program.to_string(), args.to_vec()));
        }
    }
    out
}

/// A word the shell expands before it runs: a variable or a command's output.
fn expands(w: &str) -> bool {
    w.contains('$') || w.contains('`')
}

/// `NAME=value`, as a command's prefix or `env`'s argument.
fn is_assignment(w: &str) -> bool {
    w.split_once('=').is_some_and(|(name, _)| name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// The program a simple command runs and its arguments, past `NAME=value`
/// prefixes and the wrappers that run another program.
fn past_wrappers(words: &[String]) -> Option<(&str, &[String])> {
    // The options of each wrapper that take a value.
    let valued: &[&str] = &["-u", "--unset", "-C", "--chdir", "-S", "-g", "-n", "-s", "-k", "--signal", "--kill-after", "-c", "-p"];
    let mut i = 0;
    loop {
        let w = words.get(i)?;
        if is_assignment(w) {
            i += 1;
            continue;
        }
        match program_name(w) {
            wrapper @ ("env" | "nohup" | "exec" | "command" | "setsid" | "time" | "sudo" | "nice" | "ionice" | "stdbuf" | "timeout") => {
                i += 1;
                while let Some(a) = words.get(i) {
                    match a.as_str() {
                        a if valued.contains(&a) => i += 2,
                        a if a.starts_with('-') || (wrapper == "env" && is_assignment(a)) => i += 1,
                        _ => break,
                    }
                }
                // `timeout` takes its duration before the program.
                if wrapper == "timeout" {
                    i += 1;
                }
            }
            _ => return Some((w.as_str(), &words[i + 1..])),
        }
    }
}

/// The refusal of an agent's shell write into `.familiar/approvals/` (the
/// witness's files), `.familiar/adapter` (the dry run's choice) or `.github/` (CI's wiring), whatever the plan says and
/// whether or not a run is open (plan §6.9, B-5). It fails closed: when the
/// text names either folder and writes a file it does not spell (a variable,
/// `~`, `patch`) or runs an interpreter, the write is refused too.
pub fn out_of_reach_refusal(input: &HookInput, root: &Path) -> Option<String> {
    if input.event != HookEvent::PreToolUse || input.file_path().is_some() {
        return None;
    }
    let cwd = input.cwd.as_deref().map(Path::new).unwrap_or(root);
    let command = input.command()?;
    let written = written_paths(command);
    if let Some(r) = written.iter().find_map(|w| match w {
        Written::Path(p) => approvals_or_github(root, cwd, p),
        Written::Unknown => None,
    }) {
        return Some(format!("{r}\n{COUNTS_AS_EDIT}"));
    }
    let (shell, bodies) = split_heredocs(command);
    let text = std::iter::once(shell.as_str()).chain(bodies.iter().map(String::as_str)).collect::<Vec<_>>().join("\n");
    // `workflows/` too: `$X/workflows/a.yml` may be `.github/workflows/a.yml`
    // once the shell expands `$X` (lane l5r, S9d-w's fence wins).
    // `.familiar/adapter` (plan A-1): the folder and the file both named, so a test of an adapter
    // elsewhere is not refused
    let adapter = text.contains(crate::FAMILIAR_FOLDER) && text.contains(crate::door::ADAPTER_FILE);
    if !(text.contains(".github") || text.contains("workflows/") || text.contains(crate::door::approval::FOLDER) || adapter) {
        return None;
    }
    let hidden = written.iter().any(|w| match w {
        Written::Unknown => true,
        Written::Path(p) => resolved(cwd, p).is_none(),
    });
    let interpreter = simple_commands(&shell).iter().any(|(program, _)| {
        let name = program_name(program);
        is_python(name) || matches!(name, "perl" | "ruby" | "node" | "php" | "deno" | "bun")
    });
    (hidden || interpreter).then(|| format!("{UNSPELLED}\n{COUNTS_AS_EDIT}"))
}

/// The refusal of a write into either folder whose path the text does not spell.
pub const UNSPELLED: &str = "familiar-ai: this command names .github, .familiar/approvals or .familiar/adapter and writes a file whose path its text does not spell, so it is refused: .github is the person's and the approvals and the adapter are familiar-ai's own; the agent never writes them";

/// The refusal of a write into `.familiar/approvals/`, `.familiar/adapter` or `.github/`; `.github`
/// is the repository's, read from its top, whatever folder the project is.
fn approvals_or_github(root: &Path, cwd: &Path, path: &str) -> Option<String> {
    let full = resolved(cwd, path)?;
    if let Some(rel) = decision::repository_relative(root, &full) {
        if let Some(r) = decision::out_of_reach(&rel) {
            return Some(r);
        }
    }
    let rel = project_relative(root, cwd, path)?;
    let approvals = format!("{}/{}", crate::FAMILIAR_FOLDER, crate::door::approval::FOLDER);
    if rel == approvals || rel.starts_with(&format!("{approvals}/")) {
        return Some(format!("familiar-ai: {rel} is familiar-ai's own and stays as it is; it is never edited by hand"));
    }
    if rel == decision::adapter_rel() {
        return Some(decision::ADAPTER_OWN.to_string());
    }
    decision::out_of_reach(&rel)
}

/// The path relative to the project when the write lands inside it. `None`
/// for a path the shell expands and for `.familiar/`, which `folder.rs` decides.
pub fn inside_project(root: &Path, cwd: &Path, path: &str) -> Option<String> {
    project_relative(root, cwd, path).filter(|rel| !crate::door::commits::is_familiars_own(rel))
}

/// The path relative to the project when the write lands inside it, `.familiar/` included.
fn project_relative(root: &Path, cwd: &Path, path: &str) -> Option<String> {
    let rel = resolved(cwd, path)?.strip_prefix(root).ok()?.to_string_lossy().into_owned();
    (!rel.is_empty()).then_some(rel)
}

/// The absolute path a write names, `.` and `..` resolved; `None` for a path
/// the shell expands (`$`, `~`, a backtick) or for none at all.
fn resolved(cwd: &Path, path: &str) -> Option<PathBuf> {
    if path.is_empty() || path == "-" || path.contains(['$', '`']) || path.starts_with('~') {
        return None;
    }
    let full = if Path::new(path).is_absolute() { PathBuf::from(path) } else { cwd.join(path) };
    Some(decision::lexically_clean(&full))
}

/// The path as the text before word `at` makes it: a `$VAR` is what `VAR=<literal>`
/// set earlier, and a path that cannot be resolved is a file the text does
/// not carry. A `~` or `$HOME` is left to `inside_project`, which puts it outside.
fn expanded(words: &[String], at: usize, path: &str) -> Written {
    if !path.contains('$') {
        return Written::Path(path.to_string());
    }
    let mut vars: HashMap<String, String> = HashMap::new();
    let mut start = true;
    for w in &words[..at] {
        if is_separator(w) {
            start = true;
        } else if start && is_assignment(w) {
            let (name, value) = w.split_once('=').unwrap_or((w, ""));
            match expand(value, &vars) {
                Some(v) => vars.insert(name.to_string(), v),
                None => vars.remove(name),
            };
        } else if !(start && (matches!(w.as_str(), "export" | "local" | "readonly" | "declare" | "typeset") || w.starts_with('-'))) {
            start = false;
        }
    }
    // `$HOME` is outside the project, as `~` is, unless the text set it; a
    // `$PWD` the text did not set is left as written, as before lane l5 (the
    // write is passive, and one naming `.github` or the approvals is refused
    // by `out_of_reach_refusal`, S9d-w).
    for var in ["HOME", "PWD"] {
        let rest = path.strip_prefix(&format!("${var}")).or_else(|| path.strip_prefix(&format!("${{{var}}}")));
        if !vars.contains_key(var) && rest.is_some_and(|r| r.is_empty() || r.starts_with('/')) {
            return Written::Path(path.to_string());
        }
    }
    expand(path, &vars).map_or(Written::Unknown, Written::Path)
}

/// `$NAME` and `${NAME}` replaced by their values; `None` for one without a
/// value and for a command substitution.
fn expand(text: &str, vars: &HashMap<String, String>) -> Option<String> {
    if text.contains('`') {
        return None;
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let (name, tail) = match after.strip_prefix('{') {
            Some(braced) => braced.split_once('}')?,
            None => after.split_at(after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len())),
        };
        out.push_str(vars.get(name).filter(|_| !name.is_empty())?);
        rest = tail;
    }
    out.push_str(rest);
    Some(out)
}

/// Every file the command's text writes, in the order the text names them.
pub fn written_paths(command: &str) -> Vec<Written> {
    let (shell, bodies) = split_heredocs(command);
    let shell_words = shell_words(&shell);
    let words: Vec<String> = shell_words.iter().map(|w| w.text.clone()).collect();
    let mut out: Vec<Written> = Vec::new();
    let mut push = |w: Written| {
        if !out.contains(&w) {
            out.push(w);
        }
    };
    for (i, w) in shell_words.iter().enumerate() {
        if w.quoted {
            continue; // A quoted `>` is text.
        }
        if let Some(after) = strip_redirect(&w.text) {
            let target = if after.is_empty() { words.get(i + 1).map(String::as_str) } else { Some(after) };
            if let Some(t) = target {
                if !t.starts_with('&') && !is_separator(t) {
                    push(expanded(&words, i, t));
                }
            }
        }
    }
    let mut i = 0;
    while i < words.len() {
        let args = args_after(&words, i);
        for w in files_of(program_name(&words[i]), args) {
            push(match w {
                Written::Path(p) => expanded(&words, i, &p),
                unknown => unknown,
            });
        }
        i += 1 + args.len();
        while i < words.len() && is_separator(&words[i]) {
            i += 1;
        }
    }
    if words.iter().any(|w| is_python(program_name(w))) {
        let mut text = shell.clone();
        for b in &bodies {
            text.push('\n');
            text.push_str(b);
        }
        for w in python_writes(&text) {
            push(w);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(cmd: &str) -> Vec<String> {
        written_paths(cmd)
            .into_iter()
            .map(|w| match w {
                Written::Path(p) => p,
                Written::Unknown => A_FILE.to_string(),
            })
            .collect()
    }

    #[test]
    fn the_obvious_writing_forms_name_their_paths() {
        let table: [(&str, &[&str]); 22] = [
            ("cat > src/x.py", &["src/x.py"]),
            ("cat >> tests/test_x.py << 'EOF'\nimport x\nassert 1 > 0\nEOF", &["tests/test_x.py"]),
            ("echo hi >src/x.py 2>&1", &["src/x.py"]),
            ("cmd 2> err.log", &["err.log"]),
            ("cmd &> both.log", &["both.log"]),
            ("echo x | tee -a notes.txt other.txt", &["notes.txt", "other.txt"]),
            ("sed -i 's/a/b/' src/x.py src/y.py", &["src/x.py", "src/y.py"]),
            ("sed -i.bak -e 's/a/b/' src/x.py", &["src/x.py"]),
            ("sed -n 's/a/b/p' src/x.py", &[]),
            ("perl -pi -e 's/a/b/' src/x.py", &["src/x.py"]),
            ("perl -i.bak -pe 's/a/b/' src/x.py", &["src/x.py"]),
            ("perl -ne 'print' src/x.py", &[]),
            ("cp -r a.py b.py", &["b.py"]),
            ("mv src/old.py src/new.py", &["src/new.py"]),
            ("cp a.py b.py -t dest/", &["dest/"]),
            ("install -m 644 a.conf /etc/a.conf", &["/etc/a.conf"]),
            ("truncate -s 0 log.txt", &["log.txt"]),
            ("dd if=/dev/zero of=blob.bin bs=1k count=1", &["blob.bin"]),
            ("patch -p1 < fix.diff", &[A_FILE]),
            ("git apply fix.diff", &[A_FILE]),
            ("python3 - << 'EOF'\nwith open('pydantic/json_schema.py', 'w') as f:\n    f.write('x')\nEOF", &["pydantic/json_schema.py"]),
            ("python3 -c \"from pathlib import Path; Path('out.txt').write_text('x')\"", &["out.txt"]),
        ];
        for (cmd, want) in table {
            assert_eq!(paths(cmd), want, "{cmd}");
        }
        assert_eq!(paths("ls; cat > a.py && cp b.py c.py | tee d.py"), ["a.py", "c.py", "d.py"], "every write in a chain");
    }

    #[test]
    fn reads_and_other_commands_write_nothing() {
        for cmd in [
            "cat src/x.py",
            "ls -la",
            "grep -rn foo src/",
            "git status",
            "git diff",
            "git commit -am x",
            "pytest -q tests/test_x.py",
            "python3 -m pytest",
            "sed 's/a/b/' src/x.py",
            "cat << EOF\nhello > world\nEOF",
            "cmd 2>&1",
            "cmd >&2",
            "familiar-ai claim intent_submitted --text 'x > y'",
            "rm -rf build",
            "cp -r src",
        ] {
            assert_eq!(paths(cmd), Vec::<String>::new(), "{cmd}");
        }
    }

    #[test]
    fn a_path_is_inside_the_project_or_not() {
        let root = Path::new("/w/repo");
        let at = |cwd: &str, p: &str| inside_project(root, Path::new(cwd), p);
        assert_eq!(at("/w/repo", "src/x.py").as_deref(), Some("src/x.py"));
        assert_eq!(at("/w/repo", "./src/x.py").as_deref(), Some("src/x.py"));
        assert_eq!(at("/w/repo", "/w/repo/src/x.py").as_deref(), Some("src/x.py"));
        assert_eq!(at("/w/repo/src", "x.py").as_deref(), Some("src/x.py"), "relative to the shell's folder");
        assert_eq!(at("/w/repo/src", "../tests/t.py").as_deref(), Some("tests/t.py"));
        assert_eq!(at("/w/repo", "/tmp/y"), None);
        assert_eq!(at("/w/repo", "/dev/null"), None);
        assert_eq!(at("/w/repo", "../elsewhere/x"), None);
        assert_eq!(at("/w/repo", "$HOME/x"), None);
        assert_eq!(at("/w/repo", "~/x"), None);
        assert_eq!(at("/w/repo", "-"), None);
        assert_eq!(at("/w/repo", ".familiar/plan"), None, "folder.rs's");
        assert_eq!(at("/w/repo", "/w/repo"), None);
    }

    #[test]
    fn the_decision_is_the_edits_with_the_line_after_it_and_nothing_for_a_read() {
        let root = std::env::temp_dir();
        let input = |cmd: &str| {
            let json = format!(
                "{{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"a1\",\"cwd\":\"{}\",\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"{}\"}}}}",
                root.display(),
                cmd.replace('"', "\\\"")
            );
            HookInput::parse(&json, HookEvent::PreToolUse).expect("parse")
        };
        let empty: Log = Vec::new();
        // Before the plan, the Edit's refusal comes first, then the line.
        let d = decision(&input("cat > src/x.py"), &empty, &root, 0.0, false).expect("a write");
        let edit = decision::decide(&empty, &root, &[Event::FileEdit { criterion_id: None, file_path: "src/x.py".into() }], 0.0, false);
        let Decision::Refuse { reason: edits } = edit else { panic!("{edit:?}") };
        assert_eq!(d, Decision::Refuse { reason: format!("{edits}\n{COUNTS_AS_EDIT}") });
        assert!(decision(&input("cat src/x.py"), &empty, &root, 0.0, false).is_none(), "a read");
        assert!(decision(&input("echo x > /nonexistent-elsewhere/zz"), &empty, &root, 0.0, false).is_none(), "outside the project");
        assert!(decision(&input("echo x > .familiar/plan"), &empty, &root, 0.0, false).is_none(), "folder.rs's");
        // Once the plan is confirmed, the anchor may be written and another file is told.
        let log = familiar_ai_rules::sample::criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
        assert_eq!(decision(&input("cat > src/login.py"), &log, &root, 0.0, false), Some(Decision::allow()));
        match decision(&input("sed -i 's/a/b/' src/other.py"), &log, &root, 0.0, false) {
            Some(Decision::Allow { told: Some(t) }) => assert!(t.contains("outside the place the plan named for \"a right password opens a session\""), "{t}"),
            other => panic!("{other:?}"),
        }
        // A call after the tool runs decides nothing.
        let post = HookInput::parse(
            &format!("{{\"hook_event_name\":\"PostToolUse\",\"cwd\":\"{}\",\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"cat > src/x.py\"}}}}", root.display()),
            HookEvent::PostToolUse,
        )
        .expect("parse");
        assert!(decision(&post, &empty, &root, 0.0, false).is_none());
    }
}
