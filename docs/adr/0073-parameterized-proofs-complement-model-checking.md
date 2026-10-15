# 0073. Parameterized safety proofs complement model checking

- **Status:** Proposed, 15 October 2026. Guillaume asked to evaluate TLAPS
  and Apalache, strengthen guarantees, and reduce time spent running tests.

## Context

TLC exhausts a finite state graph, while the number of custodians, versions
and protocol steps can grow beyond its bounds. Repeating a large finite
instance is expensive. A proof over the original actions can cover every
size and execution length, provided its assumptions match the configuration.
This concerns model safety; testing the implementation against the model
and the real storage boundaries is still necessary.

## Decision

- Check `CaptureTransactionProof` and `AnnexDropProof` with pinned TLAPS
  1.5.0 on each pull request and model push. Import the existing model
  actions; do not maintain a separate proof implementation of the protocol.
- Check all proof obligations from scratch, reject omitted proofs, and
  require the `Safety` theorem. Check mutations that remove safety guards.
- Evaluate the proof's own `ProofParameters` expression with TLC and the
  original configuration. Prove that the configured invariants follow from
  `Safety`, checking the root proof before this generated theorem because
  TLAPS trusts imported theorems. Unsupported assumptions or claims fail
  before a finite check can be moved out of routine CI.
- Move only `AnnexDrop_design4`, covered by the parameterized no-expiry
  proof, to an audit set: scheduled/manual TLC and explicit local runs.
  Other safety, negative, liveness, trace, law, contract and fault-injection
  checks keep their frequency. Proofs are prerequisites of the model jobs.
- Keep Apalache 0.62.4 as an opt-in, reproducible benchmark and induction
  cross-check. Its bounded checks alone cannot replace exhaustive TLC;
  on the tested capture model it was slower than TLC and weaker than the
  parameterized TLAPS proof. Do not add it to routine CI for this workload.

## Consequences

- The two stated model safety claims hold without a fixed domain size or
  execution length. They do not prove Rust refinement, storage durability,
  crash/power-loss behavior absent from the model, or liveness.
- The drop proof does not cover lock expiry or ranked P2P checks. The
  expiry-enabled design configurations remain on their existing schedule.
- TLAPS adds a Linux proof job and a pinned tool installation, cached
  between runs. Its proof manager, SMT solver and standard mathematical
  library remain trusted components. The binary installer builds and
  self-tests Isabelle; local alternate installations must match the version.
- Commands, measurements, proof assumptions and tool limitations live in
  [model-proofs.md](../model-proofs.md). Future reductions require another
  explicit coverage argument; a proof of the model cannot justify dropping
  checks of the implementation's correspondence to it.
