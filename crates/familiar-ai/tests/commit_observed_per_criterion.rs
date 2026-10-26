//! The laptop's commit reader writes one `tests_observed` per criterion per
//! landed commit, each over that criterion's own test files.

mod common;
mod observed;
use observed::*;

#[test]
fn a_commit_that_touches_both_criteria_s_tests_gives_each_its_own_fingerprint() {
    let p = Heads::two_heads("both");
    std::fs::write(p.repo().join(T1), TEST1).expect("test");
    std::fs::write(p.repo().join(T2), TEST2).expect("test");
    let k = p.commit("both criteria's tests");
    let log = p.facts();
    let at = log.iter().position(|f| f.kind == "commit_seen" && text(f, "commit") == k).expect("the commit is seen");
    assert!(!log[at].fields.contains_key("tests_hash"), "commit_seen carries no hash: {:?}", log[at].fields);
    let kinds: Vec<&str> = log[at + 1..at + 3].iter().map(|f| f.kind).collect();
    assert_eq!(kinds, ["tests_observed", "tests_observed"], "the observations follow their commit");
    let (h1, h2) = (hash_of(&p.repo(), &k, T1), hash_of(&p.repo(), &k, T2));
    assert_ne!(h1, h2);
    assert_eq!(observations(&log, &k), [("c1".to_string(), h1), ("c2".to_string(), h2)], "each over its own file, plan order");
}

#[test]
fn an_edit_of_one_criterion_s_test_changes_only_its_fingerprint() {
    let p = Heads::two_heads("one");
    std::fs::write(p.repo().join(T1), TEST1).expect("test");
    std::fs::write(p.repo().join(T2), TEST2).expect("test");
    let k1 = p.commit("both criteria's tests");
    std::fs::write(p.repo().join(T1), "sh src/a.sh | grep -x -q hello\n").expect("edit");
    let k2 = p.commit("c1's test reworded");
    let log = p.facts();
    let (o1, o2) = (observations(&log, &k1), observations(&log, &k2));
    assert_eq!(o2.len(), 2, "both criteria observed at a commit of one: {o2:?}");
    assert_ne!(o1[0].1, o2[0].1, "c1's changed");
    assert_eq!(o1[1].1, o2[1].1, "c2's the same");
    assert_eq!(o2[0].1, hash_of(&p.repo(), &k2, T1));
    for f in log.iter().filter(|f| f.kind == "commit_seen") {
        assert!(!f.fields.contains_key("tests_hash"), "no commit_seen carries a hash now");
    }
}

#[test]
fn a_criterion_with_no_file_at_a_commit_is_not_observed() {
    let p = Heads::two_heads("none");
    std::fs::write(p.repo().join(T1), TEST1).expect("test");
    let k = p.commit("c1's tests");
    let log = p.facts();
    let o = observations(&log, &k);
    assert_eq!(o.len(), 1, "{o:?}");
    assert_eq!(o[0].0, "c1");
}
