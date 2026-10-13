# 0010. pigeon reaches git through the git command line only

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q5).

## Context

pigeon reads commits and their parents' trees to validate authority (ADR
0009), checks the `gpgsig` header of each commit (ADR 0007), and writes signed
commits, refs and merges. git-annex already requires the git CLI and uses it.
Rust options: `gix` (gitoxide, 0.89 on 8 October 2026, API still changing),
`git2` (libgit2 bindings, its own implementation of git's behaviour), or the
git CLI.

## Decision

pigeon uses the git CLI only:

- reads through one long-lived `git cat-file --batch` process per repository,
  parsing raw commit and tree objects in pigeon;
- writes with plumbing: `commit-tree -S`, `update-ref`, and
  `merge-tree --write-tree` (git ≥ 2.38), which merges without touching the
  working copy;
- fetch and push with `git fetch` / `git push` on the remotes the transport
  helper serves (ADR 0006).

## Consequences

- One source of git behaviour, shared with git-annex; no pre-1.0 dependency.
- pigeon owns a small parser for commit and tree objects, with tests against
  objects produced by real git.
- Minimum git version 2.38 (for `merge-tree --write-tree`).
- `gix` may later replace the read path as an optimisation, behind the same
  interface.
