# 0015. The new design does not build on pigeon 0.9.0

- **Status:** Accepted, 13 October 2026, by Guillaume (chat: "You can ignore
  the previous implementation of pigeon").

## Context

The repository `github.com/mariogeiger/pigeon` already holds pigeon 0.9.0
(3 October 2026): about 34 000 lines of Rust in five crates, with its own
sync engine (iroh, iroh-blobs, redb), suggestions, a web interface and an
installer. The spec, the review, the models and ADRs 0001–0014 were written
without reference to it, and ADR 0001 makes git-annex the engine.

## Decision

The new specification and its ADRs are written for a new implementation on
git-annex. pigeon 0.9.0 is neither a base nor a constraint: its code, wire
protocol (`pigeon/sync/10`) and data formats need not be kept compatible.

## Consequences

- No migration path from 0.9.0 groups is promised; if one is wanted later, it
  is a separate decision.
- Lessons of 0.9.0 may still be borrowed by reading it (for example its
  portable-name rules and its pause when the root disappears), but nothing
  is inherited by default.
- Where the new code lives in the repository (replacing 0.9.0 on `main`, a
  new branch, or a new directory) is decided by the repository's owner.
