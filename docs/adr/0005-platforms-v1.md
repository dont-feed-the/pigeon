# 0005. v1 targets Linux and macOS; native Windows in v1.1

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q1).
  For backup machines, narrowed to Linux by ADR 0032.

## Context

git-annex's Windows port is beta and forces unlocked files. The published
iroh helper uses `dumbpipe listen-unix`, compiled on Unix only. Windows file
sharing flags make rename and delete fail while a file is open, so capture
and install need their own test suite. The proposal of 12 October put native
Windows in week 2 of the feasibility study.

## Decision

pigeon v1 supports Linux and macOS. Native Windows comes in v1.1, after a
dedicated experiment (E11: git-annex on Windows, the iroh helper, the
save-pattern suite). WSL does not count as Windows support.

## Consequences

- The feasibility study drops its Windows week; Windows becomes the v1.1
  study.
- The catalogue keeps the portable naming rules from day one (NFC, case-fold
  collisions, Windows reserved names), so Windows nodes can join later without
  renaming history.
- Nothing in v1 may assume Unix-only primitives in its protocol or data
  formats; Unix-only code stays behind a platform layer.
