//! The laptop's commit reader and CI's verify observe each criterion's tests
//! through one helper, so verify in a fresh clone stamps the fingerprint the
//! laptop stamped, also for two criteria in one commit.

mod common;
mod observed;
mod way_back;

use familiar_ai_rules::lifecycle::same_proof;
use familiar_ai_rules::View;

#[test]
fn the_pipeline_stamps_the_fingerprint_the_laptop_stamped() {
    let p = way_back::Sealed::green("same");
    p.commit_plan();
    let laptop = observed::stamps(&observed::facts_at(&p.repo()), "c1", "laptop");
    let proof = laptop.last().expect("the laptop stamped").1.clone();
    let clone = p.dir.join("clone-same");
    let (code, out) = p.verify_in_clone("same");
    assert_eq!(code, 0, "{out}");
    let ci = observed::facts_at(&clone);
    let pipeline = observed::stamps(&ci, "c1", "pipeline");
    assert!(!pipeline.is_empty(), "{out}");
    assert!(pipeline.iter().all(|(_, h)| *h == proof), "the pipeline's {pipeline:?} = the laptop's {proof}");
    assert!(same_proof(&View::new(&ci), "c1"), "same_proof reads true on CI's record");
}

#[test]
fn two_criteria_in_one_commit_are_each_proven_under_their_own_fingerprint() {
    let p = observed::Heads::both_green("two");
    p.commit_plan();
    let log = p.facts();
    let (l1, l2) = (observed::stamps(&log, "c1", "laptop"), observed::stamps(&log, "c2", "laptop"));
    let (p1, p2) = (l1.last().expect("c1 stamped").1.clone(), l2.last().expect("c2 stamped").1.clone());
    assert_ne!(p1, p2, "each criterion its own fingerprint");
    let seal = &l1[0].0;
    assert_eq!(p1, observed::hash_of(&p.repo(), seal, observed::T1), "c1's over c1's file alone");
    assert_eq!(p2, observed::hash_of(&p.repo(), seal, observed::T2), "c2's over c2's file alone");
    let (code, out, clone) = p.verify_in_clone("two");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"a says hello\" · proven") && out.contains("\"b says bye\" · proven"), "{out}");
    let ci = observed::facts_at(&clone);
    for (c, proof) in [("c1", &p1), ("c2", &p2)] {
        let pipeline = observed::stamps(&ci, c, "pipeline");
        assert!(!pipeline.is_empty() && pipeline.iter().all(|(_, h)| h == proof), "{c}: the pipeline's {pipeline:?} = the laptop's {proof}");
        assert!(same_proof(&View::new(&ci), c), "{c}: same_proof");
    }
}
