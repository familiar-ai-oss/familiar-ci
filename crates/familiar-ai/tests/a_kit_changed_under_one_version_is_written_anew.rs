//! The kit's folder is written once and its files kept; a binary whose kit changed under the same
//! version (a dev build, `0.1.0-dev`) must not run the older kit left in the folder: the call
//! recorder of `familiar-ai try` lives in the kit, and a stale reporter never records a call.
//! Found building `familiar-ai try` (plan test-first-spot, change C): the temp folder held the
//! pytest reporter of three days before, with no recorder, and every try listed nothing.

use familiar_ai::door::readers::kit_dir_in;

const SHIPPED: &str = include_str!("../kit/pytest/familiar_ai_pytest.py");

#[test]
fn a_kit_folder_left_by_an_older_build_of_the_same_version_is_not_the_one_run() {
    let base = std::env::temp_dir().join(format!("familiar-ai-kit-stale-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    // an older build of this version left its reporter where this version's folder was
    let old = base.join(format!("familiar-ai-kit-{}", env!("CARGO_PKG_VERSION"))).join("pytest");
    std::fs::create_dir_all(&old).expect("the old folder");
    std::fs::write(old.join("familiar_ai_pytest.py"), "# an older reporter\n").expect("the old reporter");
    let dir = kit_dir_in(&base).expect("the kit");
    assert!(dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("familiar-ai-kit-")), "{}", dir.display());
    let text = std::fs::read_to_string(dir.join("pytest/familiar_ai_pytest.py")).expect("the reporter");
    assert!(text == SHIPPED, "the reporter run is the one this binary ships, not the older one: {}", text.lines().next().unwrap_or(""));
    // the same binary finds its own folder again
    assert_eq!(kit_dir_in(&base).expect("the kit again"), dir);
    let _ = std::fs::remove_dir_all(&base);
}
