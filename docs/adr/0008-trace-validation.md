# 0008. Implementation traces are validated against the TLA+ specifications

- **Status:** Accepted, 13 October 2026, as a requirement from Guillaume
  (chat), in the spirit of Cirstea, Kuppe, Loillier and Merz, *Validating
  Traces of Distributed Programs Against TLA+ Specifications*,
  arXiv:2404.16075 (2024).

## Context

The TLA+ models ([`models/tla/`](../../models/tla/)) check the protocol, not the code. A model can be
correct while the implementation drifts from it. The paper reduces the
question "is this execution a behaviour of the spec?" to constrained model
checking with TLC: the program logs only *updates* of specification
variables, possibly of some variables only, and TLC searches for a behaviour
of the spec that matches the log. The authors found discrepancies in every
program they tried.

## Decision

- Every protocol pigeon implements has a TLA+ specification that defines its
  event vocabulary: action names, variable names, value encodings. At least
  capture/install, accept (authorization), evict, purge, and repair.
- pigeon emits these events as NDJSON, one line per spec action, with the
  device ID, a Lamport clock, the action and the variable updates. The Rust
  side is one typed enum per spec (one variant per action) behind a cargo
  feature `tla-trace`; a test checks that the names match the spec.
- Logs from several devices are merged in Lamport order, ties broken by
  device ID, which gives an interleaving consistent with causality.
- Each spec has a trace module (`*Trace.tla`) that reads the log with
  `ndJsonDeserialize` (TLC's standard `Json` module, present in our
  `tla2tools.jar`) and constrains `Next` to the logged step; variables not
  traced stay non-deterministic.
- Every fault-injection run in CI is trace-validated. A mismatch fails the
  build and reports the first log line no behaviour of the spec can explain.

## Consequences

- The specs become an interface: renaming an action changes both sides.
- git-annex is not instrumented (that would need a Haskell patch). Its
  effects are traced at the boundary: pigeon logs each git-annex command, its
  JSON result, and snapshots of which custodians hold which keys, taken by
  the test harness.
- Trace validation shows that the observed runs conform. It is not a proof
  of the code; the models remain bounded (N ≤ 4, K ≤ 2 so far).
- In production the feature is off by default; a user may turn it on to
  attach a trace to a bug report.
