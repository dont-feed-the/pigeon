//! Running a scenario: the interpreter interface and its report.
//!
//! An [`Interpreter`] gives each step a meaning; [`run`] folds a scenario's
//! steps through it, in order, and stops at the first failed check, since
//! later steps assume earlier ones. Two interpreters exist: the model of
//! [`crate::model`], pure and fast, and the process executor of
//! [`crate::exec`], which runs `pigeond` and `pigeon`. Both report the same
//! [`Report`], so their verdicts can be compared step by step.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::scenario::{Label, Scenario, embeds_per_device, project, step_text, validate};
use crate::step::Step;
use crate::world::World;

/// The verdict on one check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// It held.
    Pass,
    /// It did not; why.
    Fail(String),
    /// It was not checked; why.
    Skip(String),
}

/// One check of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Check {
    /// The step, from 1; 0 for checks of the whole run.
    pub step: usize,
    /// What was checked, in words.
    pub what: String,
    /// The verdict.
    pub verdict: Verdict,
}

/// One disk measure (S11): bytes per part (`folders`, `.git`, `annex`,
/// `annex-tmp`, `state`, `ballast`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Measure {
    /// The device's name.
    pub device: String,
    /// The scenario's label.
    pub label: String,
    /// Bytes per part.
    pub parts: BTreeMap<String, u64>,
}

/// What a run found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The scenario's id.
    pub scenario: String,
    /// The interpreter, such as `model` or `exec linux-namespaces`.
    pub interpreter: String,
    /// Every check, in order.
    pub checks: Vec<Check>,
    /// Disk measures.
    pub measures: Vec<Measure>,
}

impl Report {
    /// Whether no check failed.
    pub fn passed(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|c| matches!(c.verdict, Verdict::Fail(_)))
    }

    /// The first failed check, if any.
    pub fn failure(&self) -> Option<&Check> {
        self.checks
            .iter()
            .find(|c| matches!(c.verdict, Verdict::Fail(_)))
    }
}

/// A meaning for steps.
pub trait Interpreter {
    /// Its name in reports.
    fn name(&self) -> String;
    /// Sets up the world's devices; the present ones run and are in the group.
    fn begin(&mut self, world: &World) -> Result<(), String>;
    /// Performs one step: the verdict on its expectation or condition.
    fn step(&mut self, world: &World, step: &Step) -> Verdict;
    /// The disk measures so far.
    fn measures(&mut self) -> Vec<Measure>;
    /// The trace labels the devices logged, merged in clock order; `None`
    /// when this interpreter has no trace.
    fn trace(&mut self) -> Option<Vec<Label>>;
    /// Tears the world down.
    fn end(&mut self);
}

/// Runs `scenario` through `interp`.
pub fn run<I: Interpreter>(scenario: &Scenario, interp: &mut I) -> Report {
    let mut report = Report {
        scenario: scenario.id.clone(),
        interpreter: interp.name(),
        checks: Vec::new(),
        measures: Vec::new(),
    };
    if let Err(e) = validate(scenario) {
        report.checks.push(Check {
            step: e.step + 1,
            what: "the scenario is well formed".into(),
            verdict: Verdict::Fail(e.why),
        });
        return report;
    }
    let w = &scenario.world;
    if let Err(e) = interp.begin(w) {
        report.checks.push(Check {
            step: 0,
            what: format!("set up {w}"),
            verdict: Verdict::Fail(e),
        });
        interp.end();
        return report;
    }
    let mut failed = false;
    for (i, s) in scenario.steps.0.iter().enumerate() {
        let verdict = if failed {
            Verdict::Skip("an earlier check failed".into())
        } else {
            interp.step(w, s)
        };
        failed |= matches!(verdict, Verdict::Fail(_));
        report.checks.push(Check {
            step: i + 1,
            what: step_text(w, s),
            verdict,
        });
    }
    // Preserve the observations even when a behavioral check failed.
    let trace = interp.trace();
    if !failed {
        let verdict = match trace {
            None => Verdict::Skip("no trace".into()),
            Some(trace) => {
                let wanted = project(&scenario.steps);
                if embeds_per_device(&wanted, &trace) {
                    Verdict::Pass
                } else {
                    Verdict::Fail("the trace lacks an action the scenario performs".into())
                }
            }
        };
        report.checks.push(Check {
            step: 0,
            what: "the trace logs the scenario's actions in order".into(),
            verdict,
        });
    }
    report.measures = interp.measures();
    interp.end();
    report
}
