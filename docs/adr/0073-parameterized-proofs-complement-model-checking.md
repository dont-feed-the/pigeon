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

- Check `CaptureTransactionProof`, `AnnexDropProof`, `PurgeProof` and
  `CustodyTokensProof`, plus `AuthorityDecidersProof`, with TLAPS
  1.5.0 on each pull request and main push. Import the existing model
  actions; do not maintain a separate proof implementation of the protocol.
- Fresh checks prove all obligations from scratch, reject omitted proofs, and
  require the `Safety` theorem. Check mutations that remove safety guards.
- Evaluate the proof's own `ProofParameters` expression with TLC and the
  original configuration. Prove that the configured invariants follow from
  `Safety`, checking the root proof before this generated theorem because
  TLAPS trusts imported theorems. Unsupported assumptions or claims fail
  before a finite check can be moved out of routine CI.
- Move `AnnexDrop_design4`, covered by the parameterized no-expiry proof,
  `Purge_full3`, covered by `PurgeProof`, and `T_n42o4`, covered by
  `CustodyTokensProof`, to an audit set: a separate Monday job, manual TLC
  and explicit local runs. Monday's audit runs even without new commits;
  release tags check fresh proofs without repeating the proved graphs.
  The smaller positive purge and token cases remain in the fast set.
  The same token root proof also covers `T_n31o5` and `T_n32o5`, each with
  separate configuration checks. `Authority_mixed` is an audit under the
  unconditional adoption-permission theorem; the larger Authority cases
  keep all their checks until their complete contracts have proofs.
  Other safety, negative, liveness, trace, law, contract and fault-injection
  checks keep their frequency. Proofs are prerequisites of the model jobs.
- Balance the remaining full-model shards using recorded execution costs.
  Estimates affect placement only; every selected configuration still runs
  once, and new configurations need no estimate to be included.
- Keep Apalache 0.62.4 as an opt-in, reproducible benchmark and induction
  cross-check. Its bounded checks alone cannot replace exhaustive TLC;
  on the tested capture model it was slower than TLC and weaker than the
  parameterized TLAPS proof. Do not add it to routine CI for this workload.

- Reuse successful main verification only for an exact hash of formal input
  paths and bytes, including configurations, runners, tool pins and workflow.
  PRs cannot save it; a miss checks fresh, and scheduled/manual/release runs
  always check fresh. Validate and cite the source main run. The standalone
  proof runner continues checking every obligation without fingerprints.

## Consequences

- The stated model safety claims hold without a fixed small domain size or
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
