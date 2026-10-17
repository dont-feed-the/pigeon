# Authority concrete-path trace checkpoint

The 17 October [nightly models-full run](https://github.com/dont-feed-the/pigeon/actions/runs/39007114840)
failed the good Authority trace at seed 104, event 14. This is outside the
first 50 seeds exercised by the successful v0.6.0 candidate run. It remains
a retained historical M7 failure; seven green nights are not claimed.

The generated trace is reproducible with `PIGEON_TRACE_SEEDS=104` and
`PIGEON_TRACE_DIR` when running `pigeon-core`'s `authority_trace` target with
nextest and the `tla-trace` feature. Seed 104 contains an accepted adoption
of a root-file proposal after the original root path was deleted. Its next
ordinary commit by device 4 is accepted by Rust; the two-slot Authority
model cannot consume that event. The trace adapter groups a concrete path
and all its conflict-copy names into one set of versions, while the native
judge evaluates additions and modifications at concrete names.

The original aggregate trace is retained as [failed evidence](evidence/authority-104-abstract-failure.ndjson).
Replaying the original model rejects it at event 14. The native history
confirms that the original `r` is absent while `r (person 2, ...)` exists.
Person 2 may remove their own copy and add an absent original. The original
name's older writer must not be attributed to the separate conflict copy.

## Refinement and scope

The trace now records every concrete content name and version. Its model
domain includes those names, with an independent contribution/writer history
per name. Pointwise adoption applies a replacement on an unchanged main or
adds a conflict copy beside a changed main. Decider and rule-7 checks retain
the existing permission rules. Trace rows must have unique concrete names.
This fixture world uses ASCII names without extensions; it is not a proof
about every portable filename or arbitrary membership structure.

The original two-slot state exploration keeps its explicit path constants
and `ConcreteNames = FALSE`. Its TLAPS theorem is explicitly limited to that
abstract variant. All 27 obligations, its configured theorem and coverage
check passed; turning concrete names on is rejected by the coverage check.
Removing the decider guard still produces genuine unproved obligations.
The concrete trace result is separate evidence, not an extension of that proof.

## Validation

All 200 nightly-range Authority histories passed the concrete trace model;
all 200 mutants reversing the final verdict were rejected. The environment
restart interrupted the first batch: completed seeds 1–102 were retained
and 103–200 rerun to completion, with the complete seed sets checked. No
failed verdict became a pass. Earlier first-verdict mutant experiments are
not the evidence for this final-verdict result.

A 1000-case native law checks allowed new originals and removal of one's own
copy, refusal of another person's original/copy changes, and additions beside
an unrelated copy. Normal 20/50-seed checks now also include seed 104; the
200-seed range already includes it. The single seed-1 fixture generation
remains deterministic. The trace runner uses nextest.

Committed good/bad traces passed. The abstract `Authority_decide` and
`Authority_part` graphs passed, while proposal-time deciders, missing Force
and fake adoption still produced their expected invariant violations.
Fold migration equivalence passed and its shifted-index mutant failed the
expected history-view invariant. End-to-end trace-driver retakes,
current-main integration and exact-head CI still precede merging. The broader law/trace audit,
seven-night, soak, upgrade and security gates remain open; M7 is not complete.

## Merge validation

Final combined source `b92d8deba60b8c52674a39b26b9eaafefa033a48` passed exact-head CI 39060412068, lab
39060412108 and bridge 39060412065. Main cache producers completed
their save steps before tagging. The full 760-check implementation run,
13 current-version input/role/restore checks, concrete-name law, 200
positive Authority histories and 200 final-verdict mutants, abstract TLAPS
checks, fold equivalence/mutant and nine-history end-to-end trace-driver
retake remain distinct evidence. The first combined CI head failed before
executing traces because nextest setup was missing; both calling workflows
now install the existing pinned action. No elapsed M6 or M7 gate is inferred.
