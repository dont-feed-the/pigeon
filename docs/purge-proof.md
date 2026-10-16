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

`Purge_full3` is a proof-covered audit: TLC runs it on Monday's nightly
run, every release tag, manual full checks, and when explicitly named.
`Purge_full` and the negative protocol configurations remain in the fast
set. E14, implementation traces and crash tests retain their checks.

The full-model workflow requires successful proof verification before its
model jobs. The proof runner already checked `Purge_full3`'s root theorem,
actual configured invariants, constants and guard mutations before this
schedule change. Moving the row to `audit` therefore adds no recurring
proof work. The existing audit-coverage check refuses a row without a
registered, checked proof; changed formal inputs invalidate the verification
cache. Scheduled, manual and release proof checks stay fresh.

On 16 October 2026, fresh checks completed in this workspace with pinned
TLAPS 1.5.0 and TLC 1.7.4, OpenJDK 21.0.12.1, Python 3.12.14, Linux x86_64
and four CPU cores. Proof commands used `--nofp --threads 2`; the finite
check used four workers and a 4 GiB maximum heap. Single samples, excluding
tool installation:

| Check | Wall time |
|---|---:|
| Purge root theorem | 11.161 s |
| Actual configured-property theorem | 0.201 s |
| Actual configuration's parameter coverage | 0.782 s |
| Purge proof and coverage total | 12.144 s |
| `Purge_full3` finite exploration | 203.2 s |
| All three proof families, coverage and all mutations | 122.748 s |

All 28 proof-suite commands returned their required verdicts. In particular,
each of the three purge guard mutations left real TLAPS obligations
unproved and produced a TLC counterexample to the original safety claims.
The four unsupported purge configurations were rejected. The unmodified
large finite configuration completed with no invariant violation.

This removes one large graph traversal from each ordinary non-audit
nightly run. The complete proof suite was already required; its mutation
cost is unchanged. PR model timings are unchanged because the large purge
case was not in their fast set. These measurements describe this workspace,
not a prediction of CI timings or total workflow savings.

Reproduce the checks with:

```sh
scripts/check-proofs
scripts/run-tlc Purge_full3 --workers 4 --heap 4g
scripts/run-tlc Purge_full Purge_seals Purge_none
```

Proof logs and per-command times go to `target/formal-proofs` by default.
The older `scripts/check-proofs --benchmark-purge` option uses one TLC
worker and a 1 GiB heap; its earlier ten-minute CI timeout was not a
completed finite baseline. The completed measurement above uses the
explicit worker and heap settings shown here.
