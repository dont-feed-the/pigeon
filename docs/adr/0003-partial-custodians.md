# 0003. Custodians may be partial

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview:
  git-annex's `sizebalanced` with capacity warnings; a pigeon planner only if
  needed later). Proposed on 12 October 2026 in answer to Laurent.

## Context

Members' machines differ in capacity, and the collection may exceed any one of
them. Protection is defined per version (ADR 0002), so a custodian need not
hold everything. E10 (three custodians of 4.5 MB each for 10 MB at K = 2)
protected 6 of 10 files where 8 were possible: git-annex's placement is
heuristic, even with `--rebalance`. Repair after a custodian loss worked
within the remaining capacity, and history was kept (E10c).

## Decision

Custodians use `wanted = sizebalanced=custodian:K` with a `maxsize` each.
Versions that do not reach K custody copies stay *published*, are shown as
unprotected, and cannot be evicted from laptops.

## Consequences

- Total custody capacity must be at least K × (collection + retained
  history); a custodian is useful only up to one copy of everything.
- Keep about 25 % headroom, because placement is heuristic and `maxsize`
  is advisory. A pigeon placement planner is a possible later replacement.
- pigeon warns, in the words of ADR 0027, when custody capacity falls below
  K × (collection + retained history) plus the headroom, and says how many
  GB are missing and on which machines.
