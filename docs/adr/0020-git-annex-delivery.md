# 0020. git-annex comes from its official builds, with a checked minimum version

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview,
  Q13).

## Context

pigeon needs recent git-annex features: the generic P2P transport (2025),
`sizebalanced` preferred content, and the JSON output of the commands it
drives (ADR 0004). Distribution packages lag: Ubuntu 24.04 ships a version
from January 2024. git-annex publishes a standalone Linux build that bundles
its own git, and Homebrew carries current releases for macOS. Our
experiments ran on 10.20261006.

## Decision

- On Linux, pigeon's installer fetches the official standalone build into
  pigeon's own directory and uses the git it bundles, which also meets ADR
  0010's git ≥ 2.38.
- On macOS, the installer uses Homebrew (`brew install git-annex`).
- `pigeond` checks `git annex version` and `git --version` at start-up and
  refuses to run below the minimum versions that CI tests. Until CI tests
  older ones, the minimum is 10.20261006.

## Consequences

- Upgrades of git-annex are deliberate: a new minimum is raised only after
  CI passes on it, and the JSON contract tests (ADR 0004) catch output
  changes.
- The standalone build publishes "current" only; CI records the version it
  tested, and the installer verifies the version it got.
- Native Windows (v1.1) will need its own delivery decision.
