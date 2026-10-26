//! A run in a scratch copy imports the copy's Python, never the working
//! tree's: neither the linked `.venv`'s editable install (a `.pth` line or a
//! setuptools finder's `MAPPING`) nor a `PYTHONPATH` naming the tree reaches
//! past the copy. The fix lives only in the tree's head, so a run at the
//! tests-first commit, and the teeth run at the fix, are red.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use familiar_ai::door::verify_run::{run_at, run_commit_watched, At, Place};
use familiar_ai_git::Anchor;

/// `PYTHONPATH` is read from this process; one case sets it, so the cases run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

const BUGGY: &str = "def add(a, b):\n    return a - b\n";
const FIXED: &str = "def add(a, b):\n    return a + b\n";
const CHECK: &str = "import pkg\nassert pkg.add(1, 2) == 3, pkg.__file__\n";
const COMMAND: &str = ".venv/bin/python tests/check.py";

/// How the tree's package reaches a run.
enum Pointer {
    /// `site-packages/__editable__.pkg-0.pth` holds the line `<tree>/src`, as `pip install -e .` writes it.
    Pth,
    /// `__editable___pkg_0_finder.py` maps `pkg` to `<tree>/src/pkg`, as setuptools' strict editable mode writes it.
    Finder,
    /// No editable install: the package sits at the top and the hook exports `PYTHONPATH=<tree>`.
    PythonPath,
}

struct Tree {
    dir: PathBuf,
    tests_first: String,
    fix: String,
    package: &'static str,
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "git {args:?}");
}

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, text).expect("write");
}

fn head(dir: &Path) -> String {
    familiar_ai_git::head_commit(dir).ok().flatten().expect("head")
}

fn site_packages(dir: &Path) -> PathBuf {
    let lib = dir.join(".venv/lib");
    let python = std::fs::read_dir(&lib).expect("lib").flatten().find(|e| e.file_name().to_string_lossy().starts_with("python")).expect("lib/python*");
    python.path().join("site-packages")
}

/// A base with the bug, the tests-first commit, then the fix at the head;
/// a `.venv` beside it, untracked, pointing at the tree as `pointer` says.
fn tree(name: &str, pointer: &Pointer) -> Tree {
    let dir = std::env::temp_dir().join(format!("familiar-ai-imports-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("folder");
    let package = match pointer {
        Pointer::PythonPath => "pkg/__init__.py",
        _ => "src/pkg/__init__.py",
    };
    git(&dir, &["init", "-q"]);
    put(&dir, ".gitignore", ".venv/\n__pycache__/\n");
    put(&dir, package, BUGGY);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    put(&dir, "tests/check.py", CHECK);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "tests first"]);
    let tests_first = head(&dir);
    put(&dir, package, FIXED);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "fix"]);
    let fix = head(&dir);

    let venv = Command::new("python3").args(["-m", "venv", "--without-pip", ".venv"]).current_dir(&dir).status();
    assert!(venv.is_ok_and(|s| s.success()), "python3 -m venv: this test needs python3");
    let site = site_packages(&dir);
    let src = dir.join("src");
    match pointer {
        Pointer::Pth => put(&site, "__editable__.pkg-0.pth", &format!("{}\n", src.display())),
        Pointer::Finder => {
            put(&site, "__editable__.pkg-0.pth", "import __editable___pkg_0_finder; __editable___pkg_0_finder.install()\n");
            put(&site, "__editable___pkg_0_finder.py", &finder(&src.join("pkg")));
        }
        Pointer::PythonPath => {}
    }
    Tree { dir, tests_first, fix, package }
}

/// The shape of setuptools' `__editable___<pkg>_finder.py`: a `MAPPING` from
/// the top-level name to its folder, and a finder appended to `sys.meta_path`.
fn finder(pkg: &Path) -> String {
    format!(
        "import sys\nfrom importlib.machinery import PathFinder\n\n\
         MAPPING: dict[str, str] = {{'pkg': '{}'}}\n\n\
         class _EditableFinder:\n    @classmethod\n    def find_spec(cls, fullname, path=None, target=None):\n        \
         if fullname in MAPPING:\n            import os\n            \
         return PathFinder.find_spec(fullname, [os.path.dirname(MAPPING[fullname])])\n        return None\n\n\
         def install():\n    if _EditableFinder not in sys.meta_path:\n        sys.meta_path.append(_EditableFinder)\n",
        pkg.display()
    )
}

/// The tests-first commit's run and the teeth run at the fix, in copies, by
/// verify's road and the hook's; and the head's run in the tree.
fn runs(t: &Tree) {
    let p = Place { worktree: &t.dir, root: &t.dir, head: &t.fix, wait: Duration::from_secs(60) };
    let here = run_at(&p, COMMAND, &At::Here);
    assert!(here.passed, "the head, in the tree, is green: {here:?}");

    let first = run_at(&p, COMMAND, &At::Commit(&t.tests_first));
    assert!(!first.passed && first.not_run.is_none(), "the tests-first commit's copy imports its own bug: {first:?}");
    let watched = run_commit_watched(&p, COMMAND, &t.tests_first, None, &mut |_| true).expect("ran");
    assert_eq!(watched.out.end, familiar_ai_run::End::Exited(1), "the hook's copy run imports its own bug: {}", watched.out.stderr);

    let anchor = Anchor { criterion_id: "c1".into(), file_path: t.package.into(), line_start: 1, line_end: 2, sha: t.tests_first.clone() };
    let anchors = [anchor];
    let teeth = run_at(&p, COMMAND, &At::PutBack(&t.fix, &anchors));
    assert!(!teeth.passed && teeth.not_run.is_none(), "the teeth run imports the lines put back, so the part has teeth: {teeth:?}");
    assert_eq!(std::fs::read_to_string(t.dir.join(t.package)).expect("tree"), FIXED, "the tree is never changed");
}

#[test]
fn an_editable_pth_naming_the_tree_does_not_reach_the_copys_run() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let t = tree("pth", &Pointer::Pth);
    let pth = std::fs::read_to_string(site_packages(&t.dir).join("__editable__.pkg-0.pth")).expect("pth");
    runs(&t);
    assert_eq!(std::fs::read_to_string(site_packages(&t.dir).join("__editable__.pkg-0.pth")).expect("pth"), pth, "the venv is never written");
    let _ = std::fs::remove_dir_all(&t.dir);
}

#[test]
fn an_editable_finder_mapping_the_tree_does_not_reach_the_copys_run() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let t = tree("finder", &Pointer::Finder);
    runs(&t);
    let _ = std::fs::remove_dir_all(&t.dir);
}

#[test]
fn a_pythonpath_naming_the_tree_does_not_reach_the_copys_run() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let t = tree("pythonpath", &Pointer::PythonPath);
    let elsewhere = std::env::temp_dir().join("familiar-ai-imports-elsewhere");
    std::env::set_var("PYTHONPATH", format!("{}:{}", t.dir.display(), elsewhere.display()));
    let outcome = std::panic::catch_unwind(|| runs(&t));
    std::env::remove_var("PYTHONPATH");
    let _ = std::fs::remove_dir_all(&t.dir);
    if let Err(e) = outcome {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn a_pointer_into_an_untracked_folder_has_no_twin_and_is_left_as_it_is() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let t = tree("untracked", &Pointer::Pth);
    put(&t.dir, "src/generated/made.py", "MADE = 'made'\n");
    put(&site_packages(&t.dir), "made.pth", &format!("{}\n", t.dir.join("src/generated").display()));
    let p = Place { worktree: &t.dir, root: &t.dir, head: &t.fix, wait: Duration::from_secs(60) };
    let command = ".venv/bin/python -c 'import made; print(made.MADE)'";
    let watched = run_commit_watched(&p, command, &t.tests_first, None, &mut |_| true).expect("ran");
    assert_eq!(watched.out.stdout.trim(), "made", "the untracked folder is still read from the tree: {}", watched.out.stderr);
    let _ = std::fs::remove_dir_all(&t.dir);
}
