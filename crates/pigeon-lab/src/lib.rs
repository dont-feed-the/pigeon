//! pigeon-lab: the slice's scenarios as typed values, and their interpreters
//! (13 §13.12).
//!
//! A [`scenario::Scenario`] is a [`world::World`] of typed device handles and
//! a sequence of [`step::Step`]s: what people do, what devices undergo, and
//! what must be observed, with deadlines. [`run::run`] folds the steps
//! through an [`run::Interpreter`]: the pure [`model`], which computes with
//! pigeon-core what the specification predicts, or the process executor of
//! [`exec`], which runs `pigeond` and `pigeon --json` on Linux namespaces or
//! macOS disk images. [`slice`] holds S1–S12.

pub mod exec;
pub mod model;
pub mod run;
pub mod scenario;
pub mod slice;
pub mod step;
pub mod world;
