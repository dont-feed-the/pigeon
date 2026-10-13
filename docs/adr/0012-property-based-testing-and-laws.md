# 0012. Property-based tests first; the core is built from algebras with laws

- **Status:** Accepted, 13 October 2026, as a requirement from Guillaume
  (chat): prefer property-based testing where it makes sense, design the
  system around algebras with sound laws, use `hegel-rust`.

## Context

pigeon's guarantees (P1–P6) are statements about *all* histories, arrival
orders and crash points, not about chosen examples. Most of pigeon's logic
is a function of values that git already makes immutable (commits, trees,
rosters) or of observations that only grow. Such functions have algebraic
laws, and laws are what property-based tests check best.
`hegel-rust` (crate `hegeltest`, 0.49.4 on 13 October 2026) is a Rust
property-based testing library built on Hypothesis, with shrinking and
stateful (state-machine) testing. It is in beta and may break between
0.N releases. It ships prebuilt engines for Linux and Apple Silicon, and
builds from source on Intel macOS.

## Decision

1. **Functional core, imperative shell.** Policy is pure functions over
   values; git, git-annex, the filesystem and iroh sit in a thin shell
   behind traits. A core module may not perform I/O.
2. **Every core structure states its laws** in its documentation, and each
   law has a hegel property:

   | Structure | Laws |
   |-----------|------|
   | Fetched history (ancestor-closed commit sets, union) | join-semilattice: associative, commutative, idempotent |
   | Acceptance `accepted(S)` (P4) | homomorphism: `accepted(S ∪ T) = accepted(S) ∪ accepted(T)`; a commit's verdict depends only on its ancestors |
   | Logical catalogue of a merge (P5) | merge commutative, associative, idempotent; conflict names independent of arrival order |
   | Portable names (P5) | normalisation idempotent; injective within a folder; names of non-colliding files unchanged |
   | Custody observations (P6) | per (custodian, key) latest-observation join; `protected` monotone in presence observations |
   | Released keys and retention (P3, purge) | grow-only set; drop eligibility monotone in time |
   | Trace merge (ADR 0008) | Lamport merge associative and commutative; per-device order preserved |
   | Encodings (roster TOML, NDJSON, git objects) | round-trip: `parse(print(x)) = x`; our parser agrees with git |
   | Placement planner | never exceeds a `maxsize`; reaches K whenever capacity allows; matches brute force on small instances |

3. **The shell is tested statefully.** hegel's state-machine tests drive real
   git and git-annex repositories through random operations and fault
   schedules (kill -9, disk full, partitions, custodian loss), compare them
   with a model, and validate each run's trace against TLA+ (ADR 0008).
4. **Example tests** remain for regressions found by properties (stored with
   the shrunk input) and for fixed scenarios of the spec (§5 E-cases).
5. Pin `hegeltest` to one 0.N minor version; upgrades are deliberate.

## Consequences

- Designing the core as algebras comes first; code that has no law to state
  is a hint that it belongs in the shell.
- The TLA+ models and the hegel models must agree; the trace vocabulary
  (ADR 0008) is shared by both.
- Beta dependency risk is limited to tests: `hegeltest` is a
  dev-dependency only.
