# Parameterized purge safety

`PurgeProof` imports the existing `Purge` actions and proves:

```tla
Spec => [](KeepWins /\ WaitingPeriod /\ NoResurrection)
```

The theorem covers any nonempty custodian set, disjoint laptop set, decider
in their union, initial copies on the configured custodians and laptops
with at least one initial copy, natural waiting period `R` and natural
`MaxTime`, with `Mode = "full"`. No fixed device count, finite-set
cardinality or execution-length bound is assumed. Time still has the
model's discrete ticks and the chosen `MaxTime` bound; the proof holds for
every such bound.

The claim is about the protocol of [08 §8.6](spec/08-custody-loops.md).
Purge is not implemented yet. It does not prove an erase command, physical
erasure, fault tolerance for lost/corrupt copies, changing group membership,
multiple keys, references outside this model, or liveness.

The induction invariant records:

- Monotone decision/restore/cancellation knowledge, well-typed reports and
  a custodian's elapsed waiting time.
- Seals are made only after the waiting period. A device knows only seals
  that exist, and a seal's reference/cancellation flags imply its maker's
  corresponding history.
- A timely keep has a custodian witness: either it has not sealed, or its
  seal reports the keep. Full release must merge that sealer's history,
  so every custodian sealing cannot erase this witness.
- Every past drop had all seals with neither keep flag, respected the
  waiting period, and excludes a timely keep. Before the first drop, an
  initial copy still exists.
- A fetch after a drop requires an uncovered reference. Already recorded
  references and cancellations never disappear.

Every original action, including report/history delivery in either order,
local-state restart and stuttering, preserves this invariant. A lemma
reduces the original full-mode release predicate; there is no replacement
transition relation.

`scripts/check-proofs` checks the root module from scratch, proves its
theorem implies the actual invariants named by `Purge_full3.cfg`, and
evaluates the proof's own parameter expression against those constants.
Three removed protocol guards must both leave TLAPS obligations unproved
and produce an actual TLC counterexample to one of the original safety
properties: merging sealer histories, waiting before sealing, and fetching
only wanted content. Invalid release modes, no custodians and no initial
copies must fail assumption coverage. Tool/process failures cannot count
as detected proof mutations.

## Independent checks and timing

Both `Purge_full` and `Purge_full3` retain their existing TLC frequencies
while the proof and its cost are evaluated. The smaller positive instance,
existing negative protocol configurations, E14 and implementation checks
remain independent. No test is removed on the strength of an unchecked
proof or an assumed speedup.

Run `scripts/check-proofs --benchmark-purge` for the roots, property
bridges, configuration checks and all mutants, plus the large finite purge
graph with one TLC worker. Logs and per-command timings go to
`target/formal-proofs`. The draft PR runs this comparison in CI because
the development container cannot reliably start solver processes.
Measurements will determine whether moving only the large instance to
the existing weekly/release/manual audit set is justified.
