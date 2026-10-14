# 0065. Runs of pigeon-core are checked by TLC before the drivers exist

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"; and his standing requirement to
  validate implementation traces against TLA+). Applies ADRs 0008 and 0053
  to the code before the fault-injection runs of 13 §13.5.

## Context

ADR 0008 validates traces of the running system against the TLA+ models, and
ADR 0053 reads a log as a module TLC 1.7.4 can check. The drivers, loops and
lab scenarios come later in the slice (ADR 0057), but `pigeon-core` already
computes what the models decide: which roster changes a device may sign,
how changes fold, what a merge writes. The hegel laws of the fold check its
algebra, not that it is the model's: a canonical order with the signer
before the time passes every law of the fold.

## Decision

1. **Simulated runs.** For a model whose decisions live in the core, a test
   runs a simulated environment that takes the model's steps and asks
   `pigeon-core` for every decision, and logs each step with the
   parameters and what the code computed, as trace events of 12 §12.3. One
   process plays every device, so a step counter is the Lamport clock. The
   first is `Roster`: `crates/pigeon-core/tests/common/roster_sim.rs` takes
   `Commit(r, t)` with an op of the model's `Ops`, filtered by
   `pigeon_core::roster::apply` and asserted allowed by `may_sign`, and
   `Merge(r, y)` with the rosters of `History::fold`.
2. **The trace spec.** `models/tla/RosterTrace.tla` extends `Roster`, takes
   exactly the logged step, and requires the model's roster to equal the
   code's at every commit and merge (a fast-forward writes none).
3. **The check.** `scripts/check-code-traces` runs the test with
   `PIGEON_TRACE_DIR`, converts each log with `scripts/trace-to-tla`, and
   runs TLC: every run must give `trace:accepted`, and a mutant of it, with
   the roster of its last checked step altered, `trace:rejected`. `ci` runs
   20 seeds, `models-full` 200. Seed 1 and its mutant are committed as
   `RosterTrace_good` and `RosterTrace_bad`, in the fast set of
   `scripts/run-tlc`, and `--fixtures` rewrites them.
4. **Next models.** `CaptureInstall`'s install and recovery and `AnnexDrop`'s
   drop follow the same pattern once their decisions are in the core; their
   simulated environments then give way to the lab's real runs (13 §13.5).

## Consequences

- A divergence between the core and a model fails CI with the first line no
  behaviour explains; the canonical-order mutation above is caught at line
  4 of one run in 20.
- The simulated environment is test code that mirrors the model: an error
  shared by both passes. The lab's runs against real git-annex remain the
  evidence for the drivers.
- Each run costs a TLC start, about 2 seconds: 20 runs and their mutants
  take about a minute in CI.
