# Adoption-permission proof and remaining Authority checks

> Mission: state the exact Authority contract proved, the evidence for the
> fold reformulation, and which expensive checks still require TLC.

## What is proved

`AuthorityDecidersProof.Safety` establishes `[]DecidersCanAdopt` under
`ForceAdopt = TRUE`, `DecidersAt` in `{"decision", "mixed"}` and
`ConcreteNames = FALSE` (the original abstract-path model). It extends
the actual `Authority` model and uses its policy definitions directly.
No transition relation or policy implementation is duplicated for proof.

This is an unconditional state theorem: under those flags, every person
selected as a decider satisfies the model's adoption-permission predicate.
It does not depend on a commit bound, history length, hostile-device budget,
operation selection or fairness. The model still fixes its persons, devices
and two paths; this is not a proof for arbitrary membership structures.
Concrete-name trace validation uses its separate pointwise adoption checks;
this abstract-path theorem does not certify those traces. Configuration
coverage must reject enabling concrete names for the abstract theorem.

For the decision policy, deciders satisfy the exact permission check of
the adoption's diff, or are fallback admins whose Force grants permission.
For the mixed policy, the adoption changes a subset of the proposal's
paths. An unchanged base uses the proposal's already-permitted replacement;
otherwise the adoption unions old and proposed versions. That preserves
the old versions and is an allowed root addition. Folder ownership still
applies, and fallback admins still need Force.

The theorem proves permission, not that the complete adoption action can
execute or will eventually execute. It does not establish authorized
content, authorized decisions, retention of adopted content, revocation,
catalogue consistency, merge acceptance or convergence.

## Exactly which graphs move

`Authority_mixed.cfg` names only `DecidersCanAdopt`. Its complete configured
contract is therefore covered and its large finite graph becomes a weekly
and manual audit. Its actual constants and actual configured invariant
are checked separately from the root theorem. The generated configuration
theorem is unconditional too, a stronger requirement than a theorem
assuming `Spec`.

`Authority_decide4`, `Authority_faulty2` and `Authority_revoke2` remain full
TLC checks. They name additional invariants that this theorem does not
establish. The smaller positive Authority cases, all negative cases,
liveness and implementation trace checks retain their frequency.

The existing token proof additionally covers `T_n31o5` and `T_n32o5`.
Each gets its own proof of its actual configured properties and a check
of its own constants. The shared root theorem is checked once. `T_nonce`,
`T_n42o3`, `T_idem` and `T_nononce` remain routine finite cross-checks.

## Tool compatibility and semantic checks

Pinned TLAPS 1.5.0 rejects recursive operator definitions. Authority's
three loops now use recursive functions over natural-number indices:

- `ApplyAll` repeatedly chooses and removes the same edit, updating the
  accumulator, for exactly `Cardinality(E)` iterations.
- `FoldUpTo` visits the same increasing commit indices with the same
  accumulator updates, for `Len(cm) - i + 1` iterations, or zero past the end.
- `CountUpTo` likewise visits those indices and applies the same revocation
  and roster-membership conditions to the preceding counted set.

`scripts/check-authority-folds` reconstructs a reference model by replacing
only these three definitions with the original equations retained in
`models/tla/reference/AuthorityRecursiveFolds.tla`. All actions, guards and
claims come from the current model. TLC explores either transition
relation, requiring both to admit each observed transition, and checks
that every stored history's roster and catalogue agree. The small graph
enables every operation kind, including removals and hostile additions.
A deliberately broken fold that omits the newest commit must produce
an actual `HistoryViewsAgree` counterexample. Merely rejecting a malformed
model or exiting with a tool error does not count.

The checked comparison completed with 4,866 distinct states; the broken
fold produced its counterexample after four states.

This is finite migration evidence, not an unbounded equivalence proof.
The policy theorem itself is independent of the folds' particular results.
All 19 pre-existing Authority configurations were also rerun to completion,
including the three large safety cases and all expected counterexamples.
The original and new single-worker `Authority_decide` checks took 13.28
and 13.64 seconds locally; both visited 22,442 distinct states. An earlier
old/new transition comparison of that case also passed all 22,442 states.
These are single samples, not a performance guarantee for other cases.

Mutation checks remove the decider permission filter and require both
an unproved TLAPS obligation and a real `DecidersCanAdopt` counterexample.
Disabling Force or selecting the proposal-era policy must fail assumption
coverage. Adding an initially true but unproved `NoNewCommits` invariant
must fail the configured-property theorem. Formal-input hashes include
the comparison script, reference equations and coverage tests.

## Effect on runtime

In completed [CI run 38900739734](https://github.com/dont-feed-the/pigeon/actions/runs/38900739734),
`Authority_mixed` took 170.2 seconds and the two additional token cases
took 95.0 and 116.7 seconds. Moving those graphs to audits removes
381.9 seconds, about 6.4 minutes, of combined TLC work from routine runs.
The new proof/configuration/mutation checks add work to the proof job;
the token root was already required.

Fresh local checks on 16 October 2026 used pinned TLAPS 1.5.0 and TLC
1.7.4, Java 21 and Linux x86_64, with installed tools:

| Check | Seconds |
|---|---:|
| Authority root, all 27 obligations | 7.967 |
| Actual `Authority_mixed` configured-property theorem | 16.246 |
| Actual policy parameters and nonempty initial-state coverage | 1.023 |
| Additional token configurations, both property/parameter checks | 2.709 |
| Old/new fold comparison and rejected fold mutant | 15.227 |
| Complete proof suite, 50 commands with required verdicts | 242.611 |

All 19 formal-runner unit tests, Authority trace fixtures, workflow lint
and repository checks also passed. These timings are single samples;
the large Authority finite checks ran separately in two competing shards.

The larger audit set is balanced over two Ubuntu runners on Mondays and
manual runs, so the added independent cross-checks do not accumulate in
one long sequential audit job. Every selected audit still runs once.

Replaying the same recorded costs with the remaining balanced full shards
gives 763.4, 753.5, 751.6 and 801.8 seconds: a longest shard near 13.4
minutes, versus 14.9 after the first token-proof reduction. These are
scheduling estimates before proof/setup time and runner variability,
not measured CI wall-time improvements or release-publication savings.

The larger potential Authority savings still require complete inductive
proofs over the original actions. History-prefix stability, revocation,
adoption witnesses and quiet-state agreement remain substantial work.
No graph is removed on the strength of an incomplete proof.

Reproduce with installed pinned tools:

```sh
scripts/check-proofs
scripts/check-authority-folds
scripts/run-tlc Authority_mixed
scripts/run-tlc T_n31o5 T_n32o5
python3 -B -m unittest discover -s scripts/tests -p 'test_proof_*.py'
```

Proof logs and comparison evidence are saved under `target/formal-proofs`.
The Rust implementation still needs its independent trace, contract and
fault-injection checks; these model theorems do not establish refinement.
