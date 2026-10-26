//! The owner, 2026-10-13: "k6 is a second red seal, that requires green".
//! After the way back went green (kw), the test put back (k4), code on the
//! greeting (k5), the reworded test committed alone again and stamped (k6),
//! then the fix (k7), stamped: verify in a fresh clone reads the part from
//! k6, red there and green with teeth at k7. With k6 not red, it exits 1
//! naming k6.
//!
//! The laptop's hooks ask for a new intent and criterion once the part is
//! green, so k4..k7 are landed as a script lands them, and the laptop's two
//! stamps (k6, k7) are written into the plan as the binary writes stamps:
//! the plan carries stamps, never the laptop's results.

mod common;
mod way_back;
use way_back::*;

const BROKEN: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";
const FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
/// Another change on the greeting that still says hello: k6 passes on it.
const STILL_HELLO: &str = "#!/bin/sh\ngreet() { echo 'hello'; }\ngreet\n";

/// The way back, then k4..k7 with the laptop's stamps at k6 and k7 in the
/// plan; `k5` is the code k6 runs against. Returns k6's sha.
fn second_seal(name: &str, k5: &str) -> (Sealed, String) {
    let p = Sealed::way_back(name);
    let plan = p.read(".familiar/plan");
    let edited = plan
        .lines()
        .rfind(|l| l.contains("\"kind\":\"fingerprint_stamped\""))
        .and_then(|l| l.split("\"proof_not_tampered\":\"").nth(1))
        .and_then(|s| s.split('"').next())
        .expect("the way back's stamp")
        .to_string();
    p.commit_plan();
    p.write("tests/test_c1.sh", TEST);
    p.land("c1 the test put back");
    p.write("src/app.sh", k5);
    p.land("c1 the greeting changed");
    p.write("tests/test_c1.sh", REWORDED);
    let k6 = p.land("c1 the reworded test again");
    p.write("src/app.sh", FIXED);
    let k7 = p.land("c1 the fix again");
    let stamp = |id: &str| format!("{{\"kind\":\"fingerprint_stamped\",\"criterion_id\":\"c1\",\"commit\":\"{id}\",\"proof_not_tampered\":\"{edited}\",\"ran_in\":\"laptop\"}}\n");
    p.write(".familiar/plan", &format!("{}{}{}", p.read(".familiar/plan"), stamp(&k6), stamp(&k7)));
    p.commit_plan();
    (p, k6)
}

#[test]
fn red_at_k6_and_green_with_teeth_at_k7_is_proven_from_k6_exit_0() {
    let (p, k6) = second_seal("second-seal", BROKEN);
    let (code, out) = p.verify_in_clone("second-seal");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&format!("\"greet says hello\" · proven · red at {}", &k6[..7])), "{k6} {out}");
    assert!(out.contains("fails with the change taken back out (exit 1)"), "{out}");
}

#[test]
fn k6_not_red_is_not_proven_exit_1_naming_k6() {
    let (p, k6) = second_seal("second-seal-not-red", STILL_HELLO);
    let (code, out) = p.verify_in_clone("not-red");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&k6[..7]), "{k6} {out}");
    assert!(out.contains("not proven: its tests passed at the tests-first commit"), "{out}");
}
