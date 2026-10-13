# 0019. pigeon v2 is licensed MIT OR Apache-2.0

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview,
  Q12, asked of Mario as the repository's owner, who may still override it).

## Context

pigeon 0.9.0 was MIT OR Apache-2.0, the usual dual licence of Rust crates.
git-annex is AGPL-3.0. pigeon runs git-annex as a separate program (ADR
0004), so pigeon's code is not a derivative of git-annex.

## Decision

pigeon v2 is licensed under MIT OR Apache-2.0, at the user's option.

## Consequences

- Any Rust dependency must be compatible with this dual licence; CI checks
  dependency licences once there is Rust code.
- If pigeon's installer or release ships a git-annex build, that component
  stays under its own licence, with its source offer, as an aggregate.
