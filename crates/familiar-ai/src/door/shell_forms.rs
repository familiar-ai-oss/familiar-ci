//! Read the files each known file-writing command form writes, for
//! `shell_write.rs`. This reads text only, so a command built to hide its
//! writes gets past it.

use crate::door::folder::is_separator;
use crate::door::shell_write::Written;

/// Split the command into its shell text and its heredoc bodies, since a body
/// is not shell text.
pub fn split_heredocs(command: &str) -> (String, Vec<String>) {
    let mut shell: Vec<&str> = Vec::new();
    let mut bodies: Vec<String> = Vec::new();
    let mut lines = command.lines();
    while let Some(line) = lines.next() {
        shell.push(line);
        let Some(delim) = heredoc_delimiter(line) else {
            continue;
        };
        let mut body: Vec<&str> = Vec::new();
        for l in lines.by_ref() {
            if l.trim_start_matches('\t') == delim {
                break;
            }
            body.push(l);
        }
        bodies.push(body.join("\n"));
    }
    (shell.join("\n"), bodies)
}

/// The delimiter of a `<<` on the line, when there is one (`<<<` is a
/// here-string, not a heredoc).
pub fn heredoc_delimiter(line: &str) -> Option<String> {
    let mut rest = line;
    loop {
        let at = rest.find("<<")?;
        let after = &rest[at + 2..];
        if after.starts_with('<') || rest[..at].ends_with('<') {
            rest = after;
            continue;
        }
        let after = after.strip_prefix('-').unwrap_or(after).trim_start();
        let delim: String = after
            .trim_start_matches(['\'', '"'])
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, '\'' | '"' | ';' | '|' | '&' | ')' | '<' | '>'))
            .collect();
        return if delim.is_empty() { None } else { Some(delim) };
    }
}

/// One word of the shell text, and whether any of it was quoted (a quoted
/// `>` is text, not a redirect).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub quoted: bool,
}

/// Split the shell text into words, with each operator as its own word and
/// quotes removed but remembered.
pub fn shell_words(text: &str) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let push = |out: &mut Vec<Word>, word: &mut String, quoted: &mut bool| {
        if !word.is_empty() || *quoted {
            out.push(Word { text: std::mem::take(word), quoted: *quoted });
        }
        *quoted = false;
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                quoted = true;
                for d in chars.by_ref() {
                    if d == c {
                        break;
                    }
                    word.push(d);
                }
            }
            '\\' => {
                match chars.next() {
                    Some('\n') => {} // a line continuation joins the lines
                    Some(d) => word.push(d),
                    None => {}
                }
            }
            '\n' => {
                push(&mut out, &mut word, &mut quoted);
                out.push(Word { text: "\n".to_string(), quoted: false });
            }
            c if c.is_whitespace() => push(&mut out, &mut word, &mut quoted),
            '&' | '|' if chars.peek() == Some(&c) => {
                chars.next();
                push(&mut out, &mut word, &mut quoted);
                out.push(Word { text: format!("{c}{c}"), quoted: false });
            }
            ';' | '|' | '(' | ')' => {
                push(&mut out, &mut word, &mut quoted);
                out.push(Word { text: c.to_string(), quoted: false });
            }
            '&' if !word.is_empty() && !word.ends_with('>') && chars.peek() != Some(&'>') => {
                push(&mut out, &mut word, &mut quoted);
                out.push(Word { text: "&".to_string(), quoted: false });
            }
            c => word.push(c),
        }
    }
    push(&mut out, &mut word, &mut quoted);
    out
}

/// `/usr/bin/python3` → `python3`.
pub fn program_name(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

pub fn is_python(name: &str) -> bool {
    name == "python" || name.starts_with("python2") || name.starts_with("python3")
}

/// The words after `words[at]` up to the next separator.
pub fn args_after(words: &[String], at: usize) -> &[String] {
    let rest = &words[at + 1..];
    let end = rest.iter().position(|w| is_separator(w)).unwrap_or(rest.len());
    &rest[..end]
}

/// The files one program's arguments write; empty for a program that is not
/// a writing form.
pub fn files_of(verb: &str, args: &[String]) -> Vec<Written> {
    let paths = |ps: Vec<String>| ps.into_iter().map(Written::Path).collect();
    match verb {
        "tee" => paths(args.iter().filter(|a| !a.starts_with('-')).cloned().collect()),
        "sed" => paths(sed_files(args)),
        "perl" => paths(perl_files(args)),
        "cp" | "mv" | "install" => paths(destination(args).into_iter().collect()),
        "truncate" => paths(truncate_files(args)),
        "dd" => paths(args.iter().filter_map(|a| a.strip_prefix("of=")).map(str::to_string).collect()),
        "patch" => vec![Written::Unknown],
        "git" if args.first().is_some_and(|a| a == "apply") => vec![Written::Unknown],
        _ => Vec::new(),
    }
}

/// The non-flag words after the script, when `sed` edits in place.
fn sed_files(args: &[String]) -> Vec<String> {
    let in_place = args.iter().any(|a| {
        a == "--in-place" || a.starts_with("--in-place=") || (a.starts_with('-') && !a.starts_with("--") && a[1..].contains('i'))
    });
    if !in_place {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut script_seen = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-e" || a == "-f" || a == "--expression" || a == "--file" {
            script_seen = true;
            i += 2;
            continue;
        }
        if a == "-l" {
            i += 2;
            continue;
        }
        if a.starts_with("--expression=") || a.starts_with("--file=") {
            script_seen = true;
            i += 1;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            i += 1;
            continue;
        }
        if script_seen {
            files.push(a.to_string());
        }
        script_seen = true;
        i += 1;
    }
    files
}

/// The non-flag words after the script, when `perl` edits in place.
fn perl_files(args: &[String]) -> Vec<String> {
    let in_place = args.iter().any(|a| a.starts_with('-') && !a.starts_with("--") && a[1..].contains('i'));
    if !in_place {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut script_seen = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            // The script is the next word unless it is attached to the flag.
            if let Some(at) = a.find(['e', 'E']) {
                script_seen = true;
                if at == a.len() - 1 {
                    i += 2;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if script_seen {
            files.push(a.to_string());
        }
        script_seen = true;
        i += 1;
    }
    files
}

/// The destination of `cp`, `mv` or `install`: the target directory, else the
/// last of two or more non-flag words.
fn destination(args: &[String]) -> Option<String> {
    let mut plain: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-t" || a == "--target-directory" {
            return args.get(i + 1).cloned();
        }
        if let Some(d) = a.strip_prefix("--target-directory=") {
            return Some(d.to_string());
        }
        if a == "--" {
            plain.extend(args[i + 1..].iter().map(String::as_str));
            break;
        }
        if a.starts_with('-') && a.len() > 1 {
            i += 1;
            continue;
        }
        plain.push(a);
        i += 1;
    }
    if plain.len() >= 2 {
        plain.last().map(|s| s.to_string())
    } else {
        None
    }
}

/// Every non-flag word of `truncate` that is not a size or a reference file.
fn truncate_files(args: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-s" || a == "-r" || a == "--size" || a == "--reference" {
            i += 2;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            i += 1;
            continue;
        }
        files.push(a.to_string());
        i += 1;
    }
    files
}

/// The writes in python text, from `open` with a writing mode and from
/// `Path(...).write_text` or `.write_bytes`. An `open` whose path is not a
/// literal is a write to an unknown file.
pub fn python_writes(text: &str) -> Vec<Written> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("open(") {
        let inner = &rest[at + 5..];
        let (path, after_path) = quoted(inner);
        // When the path is not a literal, the mode still follows the first comma.
        let after_path = match (path, inner.find([',', ')'])) {
            (None, Some(i)) if inner[i..].starts_with(',') => &inner[i..],
            (None, _) => "",
            (Some(_), _) => after_path,
        };
        let mode = after_path
            .strip_prefix(',')
            .map(|s| s.trim_start())
            .map(|s| match s.strip_prefix("mode") {
                Some(m) => m.trim_start().strip_prefix('=').unwrap_or("").trim_start(),
                None => s,
            })
            .and_then(|s| quoted(s).0);
        if mode.is_some_and(|m| m.contains(['w', 'a', 'x', '+'])) {
            out.push(match path.map(str::to_string).or_else(|| variable_path(inner).and_then(|name| assigned_literal(text, name))) {
                Some(p) => Written::Path(p),
                None => Written::Unknown,
            });
        }
        rest = inner;
    }
    let mut rest = text;
    while let Some(at) = rest.find("Path(") {
        let inner = &rest[at + 5..];
        let (path, after) = quoted(inner);
        let after = after.trim_start().strip_prefix(')').unwrap_or("").trim_start();
        if let Some(p) = path {
            if after.starts_with(".write_text(") || after.starts_with(".write_bytes(") {
                out.push(Written::Path(p.to_string()));
            }
        }
        rest = inner;
    }
    out
}

/// The name a python call's first argument is, when it is only a name.
fn variable_path(args: &str) -> Option<&str> {
    let t = args.trim_start();
    let name = &t[..t.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(t.len())];
    let first = name.chars().next()?;
    let after = t[name.len()..].trim_start();
    ((first.is_ascii_alphabetic() || first == '_') && (after.starts_with(',') || after.starts_with(')'))).then_some(name)
}

/// The string literal the last `name = '…'` of the python text assigns; `None`
/// when there is no assignment, or the last one is not a plain literal (an
/// f-string, a call, a sum).
fn assigned_literal(text: &str, name: &str) -> Option<String> {
    let mut last: Option<&str> = None;
    for (at, _) in text.match_indices(name) {
        let before = text[..at].trim_end_matches([' ', '\t']);
        let boundary = text[..at].chars().next_back().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'));
        let statement = before.is_empty() || before.ends_with(['\n', ';']);
        let after = text[at + name.len()..].trim_start_matches([' ', '\t']);
        if boundary && statement && after.starts_with('=') && !after.starts_with("==") {
            last = Some(after[1..].trim_start());
        }
    }
    let value = last?;
    let prefix: String = value.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if prefix.len() > 2 || prefix.contains(['f', 'F']) {
        return None;
    }
    let (literal, rest) = quoted(value);
    let rest = rest.trim_start_matches([' ', '\t']);
    if rest.is_empty() || rest.starts_with(['\n', ';', '#', '"', '\\', ')']) {
        literal.map(str::to_string)
    } else {
        None
    }
}

/// A leading python string literal and the text after it; `(None, text)` when
/// the text does not start with one.
fn quoted(text: &str) -> (Option<&str>, &str) {
    let t = text.trim_start();
    let t = t.trim_start_matches(['r', 'b', 'f', 'R', 'B', 'F']);
    // Inside a shell's double quotes a python `"` arrives as `\"`.
    let (escaped, t) = match t.strip_prefix('\\') {
        Some(rest) => (true, rest),
        None => (false, t),
    };
    let Some(q) = t.chars().next().filter(|c| matches!(c, '\'' | '"')) else {
        return (None, text);
    };
    let body = &t[1..];
    let close = if escaped { format!("\\{q}") } else { q.to_string() };
    match body.find(&close) {
        Some(end) => (Some(&body[..end]), &body[end + close.len()..]),
        None => (None, text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::door::shell_write::{written_paths, A_FILE};

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
    fn python_modes_that_write_are_read_and_reads_are_not() {
        for cmd in [
            "python3 -c \"open('a.py','a').write('x')\"",
            "python -c \"open(\\\"a.py\\\", 'x')\"",
            "python3 -c \"open('a.py', mode='wb')\"",
            "python3 -c \"open('a.py', 'r+')\"",
            "python3 - <<EOF\nf = open(\"a.py\", \"wt\")\nEOF",
            "/usr/bin/python3 -c \"open('a.py','w')\"",
            "python3 -c \"Path(r'a.py').write_bytes(b'x')\"",
        ] {
            assert_eq!(paths(cmd), ["a.py"], "{cmd}");
        }
        for cmd in [
            "python3 -c \"open('a.py').read()\"",
            "python3 -c \"open('a.py', 'r').read()\"",
            "python3 -c \"open('a.py', 'rb').read()\"",
            "python3 -c \"Path('a.py').read_text()\"",
            "python3 -m pytest -q tests/test_a.py",
            "python3 script.py",
        ] {
            assert_eq!(paths(cmd), Vec::<String>::new(), "{cmd}");
        }
        assert_eq!(written_paths("python3 -c \"open(name, 'w')\""), [Written::Unknown], "a path the text does not carry");
        assert_eq!(paths("node -e \"fs.writeFileSync('a.py', 'x')\""), Vec::<String>::new(), "not python: not read");
    }

    #[test]
    fn a_heredoc_body_is_not_shell_text() {
        let cmd = "cat > a.py << 'EOF'\nprint(1 > 0)\ncp x y\nEOF\necho done > b.txt";
        assert_eq!(paths(cmd), ["a.py", "b.txt"]);
        let (shell, bodies) = split_heredocs(cmd);
        assert_eq!(shell, "cat > a.py << 'EOF'\necho done > b.txt");
        assert_eq!(bodies, ["print(1 > 0)\ncp x y"]);
        assert_eq!(heredoc_delimiter("cat <<-\"END\" > f"), Some("END".into()));
        assert_eq!(heredoc_delimiter("cat <<< here-string"), None);
        assert_eq!(heredoc_delimiter("cat f"), None);
    }

    #[test]
    fn the_words_keep_quotes_in_mind_and_split_the_operators() {
        let w = |t: &str| shell_words(t).into_iter().map(|w| (w.text, w.quoted)).collect::<Vec<_>>();
        assert_eq!(
            w("familiar-ai claim intent_submitted --text 'x > y'"),
            [("familiar-ai", false), ("claim", false), ("intent_submitted", false), ("--text", false), ("x > y", true)]
                .map(|(t, q)| (t.to_string(), q))
        );
        assert_eq!(w("cat > \"my file.py\""), [("cat".to_string(), false), (">".to_string(), false), ("my file.py".to_string(), true)]);
        assert_eq!(w("a;b|c||d&&e&f (g)").iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["a", ";", "b", "|", "c", "||", "d", "&&", "e", "&", "f", "(", "g", ")"]);
        assert_eq!(w("cmd 2>&1 &> both").iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["cmd", "2>&1", "&>", "both"]);
        assert_eq!(w("echo a\\ b").iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["echo", "a b"]);
        assert_eq!(w("x ''").iter().map(|(t, q)| (t.as_str(), *q)).collect::<Vec<_>>(), [("x", false), ("", true)]);
    }

    #[test]
    fn sed_perl_and_the_copying_forms_skip_their_flags_and_scripts() {
        let a = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(sed_files(&a(&["-i", "-e", "s/a/b/", "-e", "s/c/d/", "x", "y"])), ["x", "y"]);
        assert_eq!(sed_files(&a(&["--in-place=.bak", "--expression=s/a/b/", "x"])), ["x"]);
        assert_eq!(sed_files(&a(&["-i", "-f", "script.sed", "x"])), ["x"]);
        assert_eq!(sed_files(&a(&["-E", "s/a/b/", "x"])), Vec::<String>::new());
        assert_eq!(perl_files(&a(&["-i", "-p", "-e", "s/a/b/", "x"])), ["x"]);
        assert_eq!(perl_files(&a(&["-pi", "-e's/a/b/'", "x"])), ["x"]);
        assert_eq!(perl_files(&a(&["-i", "fix.pl", "x"])), ["x"]);
        assert_eq!(destination(&a(&["-r", "a", "b", "c"])).as_deref(), Some("c"));
        assert_eq!(destination(&a(&["--target-directory=d", "a"])).as_deref(), Some("d"));
        assert_eq!(destination(&a(&["--", "-a", "b"])).as_deref(), Some("b"));
        assert_eq!(destination(&a(&["a"])), None);
        assert_eq!(truncate_files(&a(&["-s", "0", "a", "b"])), ["a", "b"]);
        assert_eq!(truncate_files(&a(&["--size", "1K", "-r", "ref", "a"])), ["a"]);
    }

    #[test]
    fn a_python_variable_path_is_the_last_literal_assigned_to_it() {
        assert_eq!(paths("python3 - <<EOF\np = 'src/x.py'\nopen(p, 'w').write('x')\nEOF"), ["src/x.py"]);
        assert_eq!(paths("python3 -c \"p = 'a.py'; p = 'src/x.py'; open(p, 'w')\""), ["src/x.py"], "the last one");
        assert_eq!(paths("python3 - <<EOF\npath = \"src/y.py\"\nopen(path, mode='a')\nEOF"), ["src/y.py"]);
        assert_eq!(paths("python3 - <<EOF\np = 'src/x.py'\nopen(p).read()\nEOF"), Vec::<String>::new(), "a read");
        for body in ["p = f'src/{n}.py'", "p = os.path.join(d, 'x.py')", "p = 'a' + n", "q = 'src/x.py'", "p = 'src/x.py'\np = name"] {
            assert_eq!(paths(&format!("python3 - <<EOF\n{body}\nopen(p, 'w')\nEOF")), [A_FILE], "{body}");
        }
    }

    #[test]
    fn a_newline_separates_commands() {
        assert_eq!(paths("echo hi\ncp a src/x.py"), ["src/x.py"]);
        assert_eq!(paths("echo hi\nsed -i 's/a/b/' src/x.py"), ["src/x.py"]);
        assert_eq!(paths("cat a\ncp b c.py\ncat d"), ["c.py"], "the third line is not cp's");
        let w = shell_words("a b\nc 'd\ne'");
        assert_eq!(w.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["a", "b", "\n", "c", "d\ne"]);
        assert!(is_separator("\n"));
    }

    #[test]
    fn a_dollar_target_resolves_through_an_earlier_assignment_else_is_a_file() {
        assert_eq!(paths("F=src/x.py\ncat > $F"), ["src/x.py"]);
        assert_eq!(paths("F=src/x.py; echo hi | tee ${F}"), ["src/x.py"]);
        assert_eq!(paths("D=src\nF=$D/x.py\ncp a \"$F\""), ["src/x.py"], "through another variable");
        assert_eq!(paths("export F=src/x.py\ncat > $F"), ["src/x.py"]);
        assert_eq!(paths("F=a.py\nF=src/x.py\ncat > $F"), ["src/x.py"], "the last one");
        assert_eq!(paths("cat > $F\nF=src/x.py"), [A_FILE], "not yet assigned");
        assert_eq!(paths("cat > $UNSET"), [A_FILE]);
        assert_eq!(paths("F=$(mktemp)\ncat > $F"), [A_FILE]);
        for t in ["$HOME/x", "${HOME}/x"] {
            assert_eq!(paths(&format!("cat > {t}")), [t], "outside the project, as ~ is");
        }
        for t in ["$TMPDIR/x", "${TMPDIR}/x"] {
            assert_eq!(paths(&format!("cat > {t}")), [A_FILE], "a temp directory is not outside the project");
        }
        assert_eq!(paths("HOME=src\ncat > $HOME/x"), ["src/x"], "unless the text assigns it");
        assert_eq!(paths("cat > $HOMEDIR/x"), [A_FILE], "another name");
        assert_eq!(paths("cat > ~/x"), ["~/x"], "outside the project, as before");
    }
}
