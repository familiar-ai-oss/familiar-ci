//! Normalise and check a `test_stack_detected` claim: its `project_root` and
//! the file paths its test command names.

use std::path::{Component, Path};

/// `project_root` as stored: relative, with a trailing `/` on a folder and
/// empty for the whole project. The rules match it as a string prefix of the
/// anchor's path, so any other spelling never matches.
pub fn normalise_project_root(project: &Path, given: &str) -> Result<String, String> {
    let given = given.trim();
    let path = Path::new(given);
    let relative = if path.is_absolute() {
        inside(project, path).ok_or_else(|| {
            format!("project_root {given} is outside the project at {}; the root names a folder inside it (. for the whole project)", project.display())
        })?
    } else {
        path.to_path_buf()
    };
    let mut parts: Vec<String> = Vec::new();
    for c in relative.components() {
        match c {
            Component::CurDir => {}
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(format!(
                        "project_root {given} climbs out of the project; the root names a folder inside it (. for the whole project)"
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Ok(if parts.is_empty() { String::new() } else { format!("{}/", parts.join("/")) })
}

/// `path` relative to the project, literally or once resolved; `None` if outside.
fn inside(project: &Path, path: &Path) -> Option<std::path::PathBuf> {
    if let Ok(rel) = path.strip_prefix(project) {
        return Some(rel.to_path_buf());
    }
    let resolve = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    resolve(path).strip_prefix(resolve(project)).ok().map(Path::to_path_buf)
}

/// Extensions that mark a command word as a file, besides any word with a `/`.
const TEST_FILE_EXTENSIONS: [&str; 34] = [
    "py", "sh", "bash", "rs", "js", "mjs", "cjs", "ts", "tsx", "jsx", "go", "rb", "java", "kt", "kts", "php", "ex",
    "exs", "lua", "pl", "swift", "dart", "scala", "clj", "hs", "ml", "zig", "nim", "cs", "fs", "cpp", "cc", "c", "mm",
];

/// The words of a test command that name files, relative to the root, with a
/// pytest `::test` suffix dropped.
pub fn path_words(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|w| w.trim_matches(|c| c == '\'' || c == '"'))
        .filter(|w| !w.is_empty() && !w.starts_with('-') && !w.contains('='))
        .filter(|w| !w.contains(['<', '>', '|', '&', ';', '$', '*', '?', '[', '(', ')']) && !w.contains("..."))
        .map(|w| w.split("::").next().unwrap_or(w))
        .map(|w| w.strip_prefix("./").unwrap_or(w))
        .filter(|w| {
            w.contains('/')
                || Path::new(w)
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| TEST_FILE_EXTENSIONS.contains(&e))
        })
        .map(str::to_string)
        .collect()
}

/// Refuse a command that names a path neither in the tree nor planned; a
/// planned test file counts because it does not exist yet at claim time.
pub fn check_command(project: &Path, command: &str, planned: &[String]) -> Result<(), String> {
    let missing: Vec<String> = path_words(command)
        .into_iter()
        .filter(|w| !project.join(w).exists() && !planned.iter().any(|p| p.strip_prefix("./").unwrap_or(p) == w))
        .collect();
    let Some(first) = missing.first() else {
        return Ok(());
    };
    let run_these = if planned.is_empty() {
        "plan the tests first (test_planned, line 6), then name the command that runs them".to_string()
    } else {
        format!("name the command that runs {}", planned.join(" "))
    };
    if command.trim() == example_command() {
        return Err(format!("test_command '{command}' is the example; {run_these}"));
    }
    Err(format!(
        "test_command '{command}' names {first}, which is neither in the working tree nor a planned test file; {run_these}"
    ))
}

/// The form's example command, kept equal to `form::example_args`: `python -m pytest`, or
/// `python3 -m pytest` when no `python` is on PATH (the owner, 2026-10-15).
pub fn example_command() -> &'static str {
    static COMMAND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    COMMAND.get_or_init(|| format!("{} -m pytest -q tests/test_greet.py", crate::teach::example::python()))
}

/// The default `codebase_id`: the repository folder's name, else the
/// project's. It is only a label.
pub fn codebase_id_of(project: &Path) -> String {
    let worktree = familiar_ai_git::worktree_root(project).ok().flatten();
    worktree
        .as_deref()
        .unwrap_or(project)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "project".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_whole_project_is_the_empty_prefix() {
        let p = PathBuf::from("/w/repo");
        for given in ["", ".", "./", "/w/repo", "/w/repo/", "src/..", "./src/../"] {
            assert_eq!(normalise_project_root(&p, given).as_deref(), Ok(""), "{given:?}");
        }
    }

    #[test]
    fn a_folder_gets_its_trailing_slash_and_an_absolute_inside_is_made_relative() {
        let p = PathBuf::from("/w/repo");
        for (given, want) in [
            ("src", "src/"),
            ("src/", "src/"),
            ("./src", "src/"),
            ("src/sub", "src/sub/"),
            ("src/./sub/", "src/sub/"),
            ("/w/repo/src", "src/"),
            ("/w/repo/src/sub/", "src/sub/"),
            ("pydantic", "pydantic/"),
        ] {
            assert_eq!(normalise_project_root(&p, given).as_deref(), Ok(want), "{given:?}");
            assert!("pydantic/json_schema.py".starts_with(want) || !want.starts_with("pydantic"));
        }
    }

    #[test]
    fn outside_the_project_is_refused_with_one_plain_line() {
        let p = PathBuf::from("/w/repo");
        for given in ["/elsewhere", "/w/other", "..", "../repo", "src/../../x"] {
            let err = normalise_project_root(&p, given).expect_err(given);
            assert!(err.contains(given) && err.contains("the project"), "{err}");
            assert_eq!(err.lines().count(), 1);
        }
    }

    #[test]
    fn the_codebase_id_is_the_folder_name_outside_any_repository() {
        assert_eq!(codebase_id_of(Path::new("/nowhere/at/all/pydantic")), "pydantic");
    }

    #[test]
    fn the_path_words_of_a_command_are_its_file_paths_only() {
        let words = |c: &str| path_words(c);
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert_eq!(words("python -m pytest -q tests/test_greet.py"), ["tests/test_greet.py"]);
        assert_eq!(words("python -m pytest 'tests/test_a.py::test_x' tests/test_b.py -k foo"), ["tests/test_a.py", "tests/test_b.py"]);
        assert_eq!(words("sh ./tests/test_c1.sh"), ["tests/test_c1.sh"]);
        assert_eq!(words("pytest -q"), Vec::<String>::new());
        assert_eq!(words("cargo test -p familiar-ai"), Vec::<String>::new());
        assert_eq!(words("go test ./..."), Vec::<String>::new());
        assert_eq!(words("npm test -- --grep foo"), Vec::<String>::new());
        assert_eq!(words("PYTHONPATH=src pytest --cov=src/ tests/ 2>/dev/null"), ["tests/"]);
        assert_eq!(words("make test"), Vec::<String>::new());
        assert_eq!(words("pytest -q test_root.py"), ["test_root.py"]);
        assert_eq!(words("python3.12 -m pytest"), Vec::<String>::new());
    }

    #[test]
    fn a_command_naming_a_file_neither_in_the_tree_nor_planned_is_refused_naming_it() {
        let root = std::env::temp_dir().join(format!("familiar-ai-stack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("tests")).expect("dir");
        std::fs::write(root.join("tests/test_old.py"), "").expect("file");
        let planned = vec!["tests/test_mutable_sequence.py".to_string()];
        assert_eq!(check_command(&root, "pytest -q", &planned), Ok(()));
        assert_eq!(check_command(&root, "pytest -q tests/", &planned), Ok(()), "a folder in the tree");
        assert_eq!(check_command(&root, "pytest -q tests/test_old.py", &planned), Ok(()), "a file in the tree");
        assert_eq!(check_command(&root, "pytest -q tests/test_mutable_sequence.py", &planned), Ok(()), "a planned file, not written yet");
        assert_eq!(check_command(&root, "pytest -q ./tests/test_mutable_sequence.py::test_x", &planned), Ok(()));
        let err = check_command(&root, example_command(), &planned).expect_err("the example");
        // definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert_eq!(err, format!("test_command '{} -m pytest -q tests/test_greet.py' is the example; name the command that runs tests/test_mutable_sequence.py", crate::teach::example::python()));
        let err = check_command(&root, "pytest -q tests/test_c2.py", &planned).expect_err("missing");
        assert_eq!(err, "test_command 'pytest -q tests/test_c2.py' names tests/test_c2.py, which is neither in the working tree nor a planned test file; name the command that runs tests/test_mutable_sequence.py");
        let err = check_command(&root, example_command(), &[]).expect_err("the example, nothing planned");
        assert!(err.ends_with("is the example; plan the tests first (test_planned, line 6), then name the command that runs them"), "{err}");
        // The example is accepted when it is what was planned.
        assert_eq!(check_command(&root, example_command(), &["tests/test_greet.py".to_string()]), Ok(()));
        assert_eq!(crate::teach::form::example_args("test_stack_detected").and_then(|a| a.iter().find(|(f, _)| *f == "test_command").map(|(_, v)| *v)), Some(example_command()));
        let _ = std::fs::remove_dir_all(&root);
    }
}
