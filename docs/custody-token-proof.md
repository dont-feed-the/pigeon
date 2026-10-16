# Custody-token safety proof and full-model scheduling

> Mission: state exactly what the token proof covers and how it reduces
> repeated finite checking without reducing other verification.

## Scope

`CustodyTokensProof.Safety` proves all four invariants of the original
`CustodyTokens` model: `TokensNeverLost`, `TokenHolderHasContent`, `KCopies`
and `AtMostOneToken`. It extends the original `Init`, `Next` and predicates;
there is no replacement transition system or additional history state.

The theorem assumes natural `N`, `K <= N` and `MaxOffers`, `Nonce = TRUE`
and `IdempotentAck = FALSE`. Each execution may have arbitrarily many
steps, including changes of mind, fetches, deletes and message replays.
Offers remain bounded by each instance's `MaxOffers`; the proof covers
every natural choice of that bound. It uses the model's original initial
state, where every node has the content and nodes 1 through K hold tokens.
It proves safety, not delivery, hand-off completion or fairness.

This is the exploratory token protocol. Pigeon's implementation uses
git-annex verified drops, covered separately by `AnnexDrop`. The token
proof does not establish anything about git-annex locks, corruption,
filesystem durability, the Nextcloud bridge, or Rust refinement.

## Why the invariant is inductive

For every token, `Witnesses` requires a holder whose pending offer, if any,
has an ID newer than every acknowledgement for that token still in the
message set. This is a property of the original state, not ghost history.

- Initially, each token has a holder with no pending offer.
- A fresh offer uses `next`, greater than every message ID. The holder
  continues to qualify as a witness.
- An accepted offer requires an empty token slot. `ActiveOffers` establishes
  that this recipient has no pending offer, so it becomes a fresh witness
  even when the received offer is old. Other tokens keep their witnesses.
- An acknowledgement can remove a token only when its ID matches the
  current offer. It cannot remove the witness: the witness's offer would
  have to be both newer than this acknowledgement and equal to it.
- A rejection clears an offer without removing a token. Deletion requires
  an empty token slot; fetching and changing one's mind preserve tokens.

The type invariant and `SingleTokens` ensure one token at most per node.
`TokenHolderHasContent` follows from the receive and delete guards. Choosing
one holder for each preserved token gives an injection from K tokens into
content holders; the standard finite-set theorem yields at least K copies.

## Independent validation

The checker proves the root with `--nofp`, separately proves that its safety
theorem implies every invariant named in the actual `T_n42o4.cfg`, and uses
TLC to check the theorem's own assumptions on those constants. All four
names in the plural `INVARIANTS` directive are included. The initial-state
check permits multiple initial states but refuses an empty initial set.

Removing the reply's nonce match or allowing duplicate acknowledgements
must leave actual unproved proof obligations and produce a TLC
`TokensNeverLost` counterexample in a small four-offer instance. Enabling
either unsupported flag in the original configuration must fail assumption
coverage. Syntax errors, missing tools and timeouts are not accepted as
detected mutations. Smaller positive and negative token cases still run
with TLC on every PR/main push. The large unmodified case remains a weekly
and manual independent audit.

Fresh local checks on 16 October 2026 used TLAPS 1.5.0, TLC 1.7.4, Java 21,
Linux x86_64 and four available CPUs. TLAPS used two threads; another large
TLC exploration was running concurrently. These are single samples with
installed tools:

| Check | Seconds |
|---|---:|
| Root proof, all 86 obligations | 11.867 |
| Actual configured-property theorem | 0.558 |
| Actual constants and nonempty initial-state coverage | 1.025 |
| Token proof and coverage total | 13.450 |
| Original `T_n42o4` finite exploration, four workers, 2 GiB heap | 592.6 |
| Full four-family suite including mutations, 37 commands | 188.964 |

The large original finite exploration completed successfully. It shared
the workspace with proof checks; its timing is not directly comparable to
the CI runner's 380.2 seconds below.

## Effect on full shards

Completed [CI run 38900739734](https://github.com/dont-feed-the/pigeon/actions/runs/38900739734)
provides the scheduling baseline. `T_n42o4` took 380.2 seconds;
the already-proved `Purge_full3` and `AnnexDrop_design4` took 145.4 and
85.3 seconds. These three graphs previously ran on every release.
They now run in a separate Monday/manual audit job, while release tags
still check fresh proofs. Monday's audit runs even if main has not moved.
This removes 610.9 seconds of finite graph traversal from a release's
combined model work, with the new proof and its mutations adding roughly
44 seconds to the proof job in this local sample. Existing proof families
were already prerequisites.

Simply deleting these rows would leave a long round-robin shard.
`scripts/run-tlc --balanced` therefore places the largest estimated costs
first, breaking ties deterministically, then executes each shard in manifest
order. `models/tla/costs.tsv` records cases taking at least 20 seconds in
that run; every new/unmeasured case receives a two-second scheduling estimate.
These are placement hints, never timeouts, bounds or exclusion rules.

| Sum of recorded case times | Shard 0 | Shard 1 | Shard 2 | Shard 3 |
|---|---:|---:|---:|---:|
| Previous release selection and partition | 1202.4 s | 616.8 s | 1233.3 s | 1010.6 s |
| Remaining cases, balanced partition | 863.0 s | 856.6 s | 840.0 s | 892.6 s |

This predicts a longest shard of 14.9 rather than 20.6 minutes, about 28%
less. It is a replay of recorded costs, not a measured new CI wall time;
runner speed, proof time, setup, queues and native tests still contribute.
`models-full` is currently separate from the release publication gate, so
this improves verification time and runner demand without promising the
same reduction in publication time.

The remaining long safety cases need their own complete proofs:
`Evict_flip` took 550.3 seconds and `Authority_decide4` 460.2 seconds.
The eviction configuration's `VIEW` requires a coverage argument too.
Eviction liveness, implementation traces and crash tests retain their
checks; safety proofs do not establish their claims.

Reproduce:

```sh
scripts/check-proofs
scripts/check-proofs --no-mutants --benchmark-tokens
python3 -B -m unittest discover -s scripts/tests -p 'test_proof_*.py'
scripts/run-tlc T_n42o4 --workers 4 --heap 2g
scripts/run-tlc --audit-only
scripts/run-tlc --full --balanced --shard 0 4
```
