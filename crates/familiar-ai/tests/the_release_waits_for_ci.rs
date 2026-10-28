//! A tag's release publishes only after CI passes on that tag (the owner, 2026-10-28): the
//! release workflow runs ci.yml as one of its own jobs and creates the GitHub release only when
//! both the build and that CI job succeed. CI runs on a tag once, inside the release, not again
//! from its own push trigger.

fn workflow(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../../.github/workflows/{name}", env!("CARGO_MANIFEST_DIR"))).expect(name)
}

/// The lines of the top-level job `job`, up to the next top-level job.
fn job<'a>(text: &'a str, job: &str) -> Vec<&'a str> {
    let head = format!("  {job}:");
    text.lines()
        .skip_while(|l| *l != head)
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with("   "))
        .collect()
}

#[test]
fn the_release_runs_ci_as_its_own_job() {
    let release = workflow("release.yml");
    let ci = job(&release, "ci");
    assert!(ci.iter().any(|l| l.trim() == "uses: ./.github/workflows/ci.yml"), "release.yml has no ci job calling ci.yml:\n{release}");
}

#[test]
fn the_release_is_created_only_after_the_build_and_ci_pass() {
    let release = workflow("release.yml");
    let create = job(&release, "release");
    assert!(create.iter().any(|l| l.trim() == "needs: [build, ci]"), "the release job does not need both build and ci:\n{release}");
}

#[test]
fn ci_can_be_called_and_runs_on_a_tag_only_through_the_release() {
    let ci = workflow("ci.yml");
    let on: Vec<&str> = ci.lines().skip_while(|l| *l != "on:").skip(1).take_while(|l| l.is_empty() || l.starts_with(' ')).collect();
    assert!(on.iter().any(|l| l.trim() == "workflow_call:"), "ci.yml cannot be called:\n{ci}");
    let push: Vec<&str> = on.iter().skip_while(|l| l.trim() != "push:").skip(1).take_while(|l| l.starts_with("    ")).copied().collect();
    assert!(push.iter().any(|l| l.trim() == "branches:"), "ci.yml's push runs on tags too, so a tag runs CI twice:\n{ci}");
}
