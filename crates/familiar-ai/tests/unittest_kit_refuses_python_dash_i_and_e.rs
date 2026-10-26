//! Lane AKunittest (plan of record 2026-10-19, A0 spike correction 2): `python -I` and
//! `python -E` ignore every `PYTHON*` variable, so the kit, which rides on `PYTHONPATH`, cannot
//! load under them. The reporter reader is refused up front for a command that runs python with
//! either, instead of spending a dry-run probe to find out (the run itself is safe: the spike's
//! `isolated` setup wrote no file and left the person's exit and output as they were).
//!
//! ```text
//!   python -I -m unittest        ─▶ refused (-I)         python -m unittest          ─▶ the kit
//!   python -Im unittest          ─▶ refused (-I, in a cluster)
//!   python -W error -m unittest  ─▶ the kit  (-W's value is not a flag)
//!   python -m unittest -I        ─▶ the kit  (after -m, the flags are unittest's)
//! ```
//!
//! The words are the quote-aware shell words the other command readers use; python's own options
//! end at the first word that is not one (a script), at `-c`/`-m` and its argument, at `-`, at
//! `--`, or at the next separator (`&&`, `;`, `|`).

use familiar_ai::door::kits::unittest::refused;

#[test]
fn a_command_that_runs_python_isolated_or_without_the_environment_is_refused() {
    for (command, flag) in [
        ("python -I -m unittest", "-I"),
        ("python3 -E -m unittest", "-E"),
        ("/usr/bin/python3.11 -I -m unittest discover -s tests", "-I"),
        ("python -Im unittest", "-I"),
        ("python -bI -m unittest", "-I"),
        ("python3 -sE -m unittest", "-E"),
        ("python -W error -I -m unittest", "-I"),
        ("python -Werror -I -m unittest", "-I"),
        ("python -X dev -E -m unittest", "-E"),
        ("cd tests && python -I -m unittest", "-I"),
        ("PYTHONPATH=. python -I -m unittest", "-I"),
        ("python -m unittest; python3 -I -c 'pass'", "-I"),
        ("python -I tests/run_all.py", "-I"),
        ("python -m unittest | tee out.txt && python -E -m unittest", "-E"),
    ] {
        let why = refused(command).unwrap_or_else(|| panic!("{command}: not refused"));
        assert!(why.contains(flag), "{command}: the reason {why:?} does not name {flag}");
    }
}

#[test]
fn any_other_command_is_not_refused() {
    for command in [
        "python -m unittest",
        "python3 -m unittest discover -s tests",
        "python -W error -m unittest",
        "python -X dev -m unittest",
        "python -OO -m unittest",
        "python -bB -m unittest tests.test_a",
        "uv run python -m unittest tests.test_a",
        ".venv/bin/python -m unittest -v",
        "python -c \"import unittest; unittest.main()\" -I",
        "python -m unittest -I",
        "python tests/run_all.py -E",
        "python - < tests/run_all.py",
        "python -m unittest && ls -I",
        "cd tests && python -m unittest && echo -E",
        "make test",
        "tox -e py312",
        "",
    ] {
        assert_eq!(refused(command), None, "{command}: refused though python's own options are untouched");
    }
}
