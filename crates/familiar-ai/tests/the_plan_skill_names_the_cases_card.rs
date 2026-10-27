//! The plan skill says what to do around the cases card (the owner's plan of 2026-10-25, step 4)
//! [draft names]: the lens panel released through the Agent tool (the owner's plan of 2026-10-26;
//! updated with the definition, it was the case-finder), the card the person answers, and the cases
//! they asked for claimed into the promise; within its 70 lines.

#[test]
fn the_plan_skill_names_the_lens_panel_and_the_cases_card() {
    let skill = std::fs::read_to_string(format!("{}/../../plugin/skills/plan/SKILL.md", env!("CARGO_MANIFEST_DIR"))).expect("plan skill");
    assert!(skill.lines().count() <= 70, "{}", skill.lines().count());
    let description = skill.lines().find(|l| l.starts_with("description:")).unwrap_or("");
    assert!(description.contains("cases_unconfirmed"), "the description's cards name the cases card: {description}");
    let flat = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(!flat.contains("case-finder"), "the case-finder is gone");
    for want in ["the lens panel", "familiar-ai:lens", "spawn the Agent tool once per lens", "then once for the synthesis", "The cases card (`cases_unconfirmed`)", "claim the promise again with the cases the person asked for"] {
        assert!(flat.contains(want), "{want}");
    }
}
