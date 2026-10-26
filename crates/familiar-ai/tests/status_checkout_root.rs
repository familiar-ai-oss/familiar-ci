//! The pane answers another checkout's card by running `familiar-ai approve`
//! there (lane S9e): `status --json` names each other checkout's root, the
//! folder its binary runs in.

mod common;

use common::Project;
use serde_json::Value;

#[test]
fn each_other_checkout_names_its_root() {
    let p = Project::builder("checkout-root").no_session().create();
    let at = p.dir.join("lane");
    p.git(&["worktree", "add", "-q", "-b", "lane", &at.to_string_lossy()]);
    let (code, out, err) = p.run_in(&at, &["setup"], "");
    assert_eq!(code, 0, "setup in the worktree: {out}{err}");
    let (code, out, err) = p.run(&["status", "--json"], "");
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("status json");
    let checkouts = v["checkouts"].as_array().expect("checkouts");
    let lane = checkouts.iter().find(|o| o["branch"] == "lane").unwrap_or_else(|| panic!("{checkouts:?}"));
    let root = lane["root"].as_str().expect("a root");
    assert_eq!(std::fs::canonicalize(root).expect("root"), std::fs::canonicalize(&at).expect("worktree"));
}
