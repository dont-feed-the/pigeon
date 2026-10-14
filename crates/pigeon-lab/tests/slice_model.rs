//! Every scenario of the slice is well formed and passes in the model: the
//! expectations it writes are what pigeon-core predicts (13 §13.12).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pigeon_lab::model::ModelRun;
use pigeon_lab::run::{Verdict, run};
use pigeon_lab::scenario::validate;
use pigeon_lab::slice;
use pigeon_lab::step::CrashPoint;

#[test]
fn every_scenario_passes_in_the_model() {
    let all = slice::all();
    assert_eq!(all.len(), 10 + CrashPoint::ALL.len());
    for s in &all {
        validate(s).unwrap_or_else(|e| panic!("{}: step {}: {}", s.id, e.step, e.why));
        let report = run(s, &mut ModelRun::default());
        assert!(report.passed(), "{}\n{s}\n{:#?}", s.id, report.failure());
        let trace = report.checks.last().unwrap();
        assert_eq!(trace.verdict, Verdict::Pass, "{}: {trace:?}", s.id);
    }
}

#[test]
fn scenarios_print_and_round_trip_through_json() {
    for s in slice::all() {
        let text = s.to_string();
        assert!(text.starts_with(&format!("{}: ", s.id)), "{text}");
        let json = serde_json::to_string(&s).unwrap();
        let back: pigeon_lab::scenario::Scenario = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

#[test]
fn a_wrong_expectation_fails_in_the_model() {
    use pigeon_lab::scenario::{Handle, wait};
    use pigeon_lab::step::Kind;
    let mut s = slice::s4();
    let sl = slice::slice();
    let a = sl.docs.file("letter.txt");
    // With backup-2 stopped, the letter can never be protected.
    s.steps.0.truncate(3);
    s.steps.0.push(wait(sl.l.sees(&a, Kind::Protected), 10));
    let report = run(&s, &mut ModelRun::default());
    assert!(!report.passed());
    assert_eq!(report.failure().unwrap().step, 4);
    let _ = sl.c1.stop();
}

#[test]
fn each_s9_crashes_where_it_says_and_recovers() {
    for p in CrashPoint::ALL {
        let s = slice::s9_point(p);
        let mut m = ModelRun::default();
        let report = run(&s, &mut m);
        assert!(report.passed(), "{}: {:?}", s.id, report.failure());
        let model = m.model.unwrap();
        let crashes = model.trace.iter().filter(|l| l.action == "Crash").count();
        assert_eq!(crashes, 1, "{}: the plan never crashed the laptop", s.id);
        assert!(
            model.laptops[0].journal.is_empty(),
            "{}: the journal was not truncated",
            s.id
        );
        assert!(model.laptops[0].stages.is_empty() && model.laptops[0].asides.is_empty());
    }
}
