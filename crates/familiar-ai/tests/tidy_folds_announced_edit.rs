//! tidy folds the announced edit into the fix commit, and the recheck accepts
//! a fix commit that touches the announced test.

mod common;
mod way_back;
use way_back::*;

#[test]
fn the_announced_edit_folds_into_the_fix_and_the_recheck_passes() {
    let p = Sealed::way_back("tidy");
    let old_head = p.git(&["rev-parse", "HEAD"]);
    let before_tree = p.git(&["rev-parse", "HEAD^{tree}"]);
    assert_eq!(p.log(), ["base", "familiar-ai setup", "c1 tests", "c1 the fix", "c1 the test's wording"]);
    let (code, out, err) = p.run(&["tidy"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(p.log().len(), 4, "one seal commit and one fix commit: {:?}", p.log());
    assert_eq!(p.log()[2], "c1 tests");
    let fix_files = p.git(&["diff", "--name-only", "HEAD~1", "HEAD"]);
    assert!(fix_files.contains("tests/test_c1.sh") && fix_files.contains("src/app.sh"), "the fix touches the announced test: {fix_files}");
    assert_eq!(p.git(&["rev-parse", "HEAD^{tree}"]), before_tree, "the same files at the end");
    assert!(out.contains("re-checked \"greet says hello\": the seal ") && out.contains("its tests fail there; they pass at the fix"), "{out}");
    let kept = p.git(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/familiar-ai/pre-tidy"]);
    assert!(kept.starts_with("refs/familiar-ai/pre-tidy/") && kept.ends_with(&old_head), "the pre-tidy ref is kept: {kept}");
    assert!(p.status().contains("\"greet says hello\" · "), "{}", p.status());
    assert!(!p.status().contains("held"), "{}", p.status());
}
