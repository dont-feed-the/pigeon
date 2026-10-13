# 0004. pigeon is written in Rust and drives git-annex as a subprocess

- **Status:** Proposed, 13 October 2026, discussed with Mario in chat
  (Rust or Haskell). To be confirmed during the tooling interview.

## Context

git-annex is a Haskell program with no library API, no FFI and no Rust crate
(crates.io searched on 13 October 2026). It offers three stable interfaces:
the command line with `--json` and `--batch`, the external special remote
protocol, and the generic P2P transport. Writing pigeon in Haskell would mean
forking or extending git-annex, inheriting AGPL for pigeon's own code, the
GHC toolchain, and a thinner networking ecosystem; iroh is Rust. The group
does not read Haskell, and code will be maintained by agents from written
traces (spec, models, tests), so the reviewers must be able to read it.

## Decision

pigeon is written in Rust. It runs `git annex` subprocesses, with long-lived
`--batch --json` processes where it issues many requests, and parses their
JSON with serde. It does not read git-annex's internal branch formats
directly. A missing hook is sent upstream as a patch, not worked around.

## Consequences

- pigeon's licence is independent of git-annex's (subprocess boundary).
- A pinned minimum git-annex version and a JSON contract test suite are
  needed, because the CLI output is the API.
- Per-call process cost is paid on short commands; batch mode amortises it.
