//! Laws of the scenario algebra and of the model interpreter (pigeon-lab).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{actions, int, schedule, steps, world};
use hegel::TestCase;
use pigeon_lab::model::{Model, ModelRun, model_after};
use pigeon_lab::run::{Verdict, run};
use pigeon_lab::scenario::{Presence, Scenario, Steps, embeds, project, validate, validate_from};
use pigeon_lab::slice;

#[hegel::test(test_cases = 1000)]
fn then_is_a_monoid(tc: TestCase) {
    let g = world();
    let (a, b, c) = (steps(&tc, &g, 4), steps(&tc, &g, 4), steps(&tc, &g, 4));
    assert_eq!(
        a.clone().then(b.clone()).then(c.clone()),
        a.clone().then(b.then(c))
    );
    assert_eq!(Steps::empty().then(a.clone()), a);
    assert_eq!(a.clone().then(Steps::empty()), a);
}

#[hegel::test(test_cases = 1000)]
fn validation_is_compositional(tc: TestCase) {
    let g = world();
    let (a, b) = (steps(&tc, &g, 5), steps(&tc, &g, 5));
    let start = Presence::initial(&g.world);
    let whole = validate_from(&g.world, &mut start.clone(), &a.clone().then(b.clone())).is_ok();
    let mut mid = start;
    let first = validate_from(&g.world, &mut mid, &a).is_ok();
    let second = first && validate_from(&g.world, &mut mid, &b).is_ok();
    assert_eq!(whole, second);
}

#[hegel::test(test_cases = 1000)]
fn projection_is_a_homomorphism(tc: TestCase) {
    let g = world();
    let (a, b) = (steps(&tc, &g, 6), steps(&tc, &g, 6));
    let mut both = project(&a);
    both.extend(project(&b));
    assert_eq!(project(&a.then(b)), both);
    assert!(project(&Steps::empty()).is_empty());
}

#[hegel::test(test_cases = 300)]
fn scenarios_round_trip_through_json(tc: TestCase) {
    let s = common::scenario(&tc, 8);
    let back: Scenario = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.to_string(), s.to_string());
}

#[hegel::test(test_cases = 300)]
fn a_prefix_runs_the_same_whatever_follows(tc: TestCase) {
    let mut s = common::scenario(&tc, 6);
    if validate(&s).is_err() {
        return;
    }
    let cut = int(&tc, 0, s.steps.0.len() as i64) as usize;
    let whole = run(&s, &mut ModelRun::default());
    s.steps.0.truncate(cut);
    let prefix = run(&s, &mut ModelRun::default());
    let steps = |r: &pigeon_lab::run::Report| -> Vec<_> {
        r.checks
            .iter()
            .filter(|c| c.step > 0 && c.step <= cut)
            .cloned()
            .collect()
    };
    assert_eq!(steps(&whole), steps(&prefix));
}

#[hegel::test(test_cases = 300)]
fn settling_is_idempotent(tc: TestCase) {
    let g = world();
    let mut m: Model = model_after(&g.world, &actions(&tc, &g, 10));
    m.settle();
    let once = m.clone();
    m.settle();
    assert_eq!(m, once);
}

#[hegel::test(test_cases = 300)]
fn the_model_logs_what_the_steps_project(tc: TestCase) {
    let g = world();
    let mut m = Model::new(&g.world);
    let mut met = Vec::new();
    for s in actions(&tc, &g, 10) {
        if m.perform(&g.world, &s) == Verdict::Pass {
            met.push(s);
        }
    }
    m.settle();
    assert!(embeds(&project(&Steps(met)), &m.trace));
}

#[hegel::test(test_cases = 1000)]
fn crashes_at_named_points_change_no_outcome(tc: TestCase) {
    let plan = schedule(&tc);
    let crashed = run(&slice::s9("S9", &plan), &mut ModelRun::default());
    assert!(crashed.passed(), "{plan:?}: {:?}", crashed.failure());
    let s = slice::slice();
    let (work, _) = slice::s9_workload(&s);
    let calm = model_after(&s.world, &work);
    let mut stormy = Model::new(&s.world);
    let full = slice::s9("S9", &plan);
    for step in &full.steps.0 {
        stormy.perform(&s.world, step);
    }
    assert_eq!(stormy.observation(), calm.observation(), "{plan:?}");
}
