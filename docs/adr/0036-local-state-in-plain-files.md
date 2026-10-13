# 0036. `pigeond` keeps its local state in plain files

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview).

## Context

Almost everything pigeon knows is in git and git-annex: the catalogue, the
roster, suggestions and decisions, where each copy is and when it was last
checked. "git status is the truth" (ADR 0013). What remains is small and
local to one machine: settings, the install journal that makes installing
crash-recoverable (P2), the table of conflicts (ADR 0034), the Lamport clock
of trace events (ADR 0008), invitations issued and web sessions (ADR 0021).
Candidates: plain files, `redb` 4.3 (embedded transactional key-value store,
pure Rust), SQLite through `rusqlite` 0.40.

## Decision

- Per-user settings: `config.toml` in the configuration directory
  (`$XDG_CONFIG_HOME/pigeon` on Linux, `~/Library/Application Support/pigeon`
  on macOS), found with the `dirs` crate, read with `toml` and `serde`.
- Per-repository state lives in `.git/pigeon/` of the repository it belongs
  to, so it moves with it and git never shares it: `conflicts.json`,
  `clock.json`, `invitations.json`, and the install journal
  `install.ndjson`.
- Whole-file state is written atomically: write a temporary file in the same
  directory (`tempfile`), fsync it, rename it over the old one, fsync the
  directory. The journal is appended one fsynced line per step, replayed at
  start, and truncated once every entry is complete.
- Every file carries `format = N`; `pigeond` refuses a newer format with a
  `Problem` (ADR 0023) instead of guessing.
- `pigeond` is the only writer (ADR 0011). The CLI and the web interface ask
  it; they never write these files.
- Anything that can be rebuilt from git or git-annex is not stored.

## Consequences

- No database dependency; a person helping can read every file with a text
  editor.
- Laws for property tests (ADR 0012): write then read gives back the value;
  a crash at any point of a write leaves the old or the new file, never a
  mix; replaying the journal twice equals replaying it once.
- If the state outgrows whole-file rewrites (for example, per-file data for
  very large collections), a new ADR can move that part to `redb`.
