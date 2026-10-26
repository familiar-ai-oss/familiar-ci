//! A `new_work_no_spot` claim requires `integration_point`, `<file>[:<symbol>]`,
//! naming where net-new work is reached from or stored. See
//! docs/decisions.md#new-work-names-its-integration-point

mod common;

use common::Scratch;

fn planned(name: &str) -> Scratch {
    let p = Scratch::new(name);
    p.git_init();
    assert_eq!(p.run(&["setup"]).0, 0);
    for args in [
        vec!["claim", "intent_submitted", "--for", "0", "--text", "greet with hello"],
        vec!["claim", "criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"],
    ] {
        let (code, out, err) = p.run(&args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    p
}

#[test]
fn the_form_names_the_integration_point_with_an_example() {
    let p = Scratch::new("ip-form");
    let (code, out, err) = p.run(&["claim", "new_work_no_spot"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("--integration_point  text  required"), "{out}");
    assert!(out.contains("where the new work is reached from"), "{out}");
    assert!(out.contains("familiar-ai claim new_work_no_spot --criterion_id c1 --integration_point src/app.py:main"), "{out}");
}

#[test]
fn a_greenfield_without_its_integration_point_is_refused_naming_it() {
    let p = planned("ip-missing");
    let (code, out, err) = p.run(&["claim", "new_work_no_spot", "--criterion_id", "c1"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("integration_point"), "{err}");
    assert!(err.contains("--integration_point src/app.py:main"), "the form follows the refusal: {err}");
    assert!(!p.read(".familiar/plan").contains("new_work_no_spot"), "nothing claimed");
}

#[test]
fn the_plan_line_carries_the_integration_point() {
    let p = planned("ip-claimed");
    let (code, out, err) = p.run(&["claim", "new_work_no_spot", "--criterion_id", "c1", "--integration_point", " src/app.py:main "]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        p.read(".familiar/plan").contains(r#"{"kind":"new_work_no_spot","criterion_id":"c1","integration_point":"src/app.py:main"}"#),
        "{}",
        p.read(".familiar/plan")
    );
}
