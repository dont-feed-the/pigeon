# 0031. Space is freed per folder, with git sparse checkout

- **Status:** Proposed, 13 October 2026, by the agent in the UX study, on the
  evidence of experiment E15 (`lab/git-annex/ga-e15-sparse-folders.sh`).

## Context

git-annex frees space by dropping content and leaving a small pointer file
in its place. To someone who is not technical, a 102-byte file named like
their photo is a broken photo (proposal §6, UX risk). Files must be either
there, complete, or not on disk at all, and still visible in pigeon.

## Decision

- In v1, people choose per top-level folder: *Keep on this computer* or not.
  A folder not kept is absent from disk (`git sparse-checkout`) and listed in
  the web interface and in `pigeon status`.
- Freeing a folder: verified `drop` of its content while it is still checked
  out, only for protected versions; then hide it. Versions not yet protected
  keep their space until they are, and the folder row says so.
- Getting a folder back: `get` its keys first, then show it, then `git annex
  smudge --update` and `restage`, so files appear complete, never as
  pointers.
- E15 shows each step works on git-annex 10.20261006, and that path-based
  commands skip hidden folders, so pigeon works by key for them.

## Consequences

- No per-file freeing in v1; a large single file in a kept folder stays.
- Sparse patterns become part of the local state of `pigeond`, restored at
  start.
- E15 must join CI with the git-annex lab, since the protocol depends on
  details of git-annex behaviour.
