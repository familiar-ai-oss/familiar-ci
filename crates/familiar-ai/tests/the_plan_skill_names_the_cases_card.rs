//! The plan skill says what to do around the cases card (the owner's plan of 2026-10-25, step 4)
//! [draft names]: the case-finder released through the Agent tool, the card the person answers, and
//! the cases they asked for claimed into the promise; within its 70 lines.

#[test]
fn the_plan_skill_names_the_case_finder_and_the_cases_card() {
    let skill = std::fs::read_to_string(format!("{}/../../plugin/skills/plan/SKILL.md", env!("CARGO_MANIFEST_DIR"))).expect("plan skill");
    assert!(skill.lines().count() <= 70, "{}", skill.lines().count());
    let description = skill.lines().find(|l| l.starts_with("description:")).unwrap_or("");
    assert!(description.contains("cases_unconfirmed"), "the description's cards name the cases card: {description}");
    let flat = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    for want in ["familiar-ai:case-finder", "spawn the Agent tool", "The cases card (`cases_unconfirmed`)", "claim the promise again with the cases the person asked for"] {
        assert!(flat.contains(want), "{want}");
    }
}
