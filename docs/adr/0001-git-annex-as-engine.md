# 0001. pigeon v1 is a policy and UX layer over git-annex

- **Status:** Accepted, 12 October 2026, by the group in chat, after
  Guillaume challenged the first verdict ("git-annex for prototyping only").
  Recorded in the proposal of 12 October 2026.

## Context

Draft 1 of the spec ([`docs/design/spec-draft-1.md`](../design/spec-draft-1.md)) implied a custom sync engine: version set,
custody tokens, blob transfer. The review ([`docs/design/REVIEW.md`](../design/REVIEW.md), N1–N7) found most of
it re-invented. git-annex already tracks where every object lives, refuses
drops below `numcopies`, locks content against concurrent drops, keeps
deleted content until `dropunused`, scrubs with `fsck`, and has an iroh
transport. Experiments E1–E10 confirmed this on git-annex 10.20261006:
40/40 concurrent drops lost nothing (E1), stale location logs are not
trusted (E5), a propagated `rm -rf` is revertible (E2/E4), and partial custody
works (E10).

## Decision

pigeon v1 is a policy and UX layer over git-annex. pigeon owns what git-annex
lacks: the signed roster and authorization, capture and install, proposals,
the three version states, and honest status. git-annex owns objects, location
tracking, transfers, verified drops and scrubbing. The spec stays
engine-neutral: it states guarantees, not git-annex commands.

## Consequences

- No custom engine in v1. If the feasibility study fails, the next step is a
  requirements review, not an engine.
- pigeon inherits git-annex's limits: Haskell, one main maintainer, AGPL-3.0,
  a beta Windows port, pointer files for absent content, heuristic placement.
- Every git-annex default that bypasses pigeon must be switched off and
  tested (E9: `git annex sync` auto-commits, so `annex.autocommit=false`).
