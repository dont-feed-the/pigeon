# Daemon trace regressions

These are complete single-path projections of real S9 traces. The first
two are from
`smarter/pigeon` Actions run 38821834366, job 116721582090, artifact
12014443541, at da3d7a2. They retain the original observations and clocks;
gaps are events for other paths or specifications in the full log.

- `write-during-keep-index.ndjson`: S9-random-001, a person finishes a write
  while keep's asynchronous index step runs. Before/after enclose exactly
  one driver action, with person/time interleavings.
- `recycled-stage-inode.ndjson`: S9-random-015, a freed inode is released
  and its physical number reused for a later keep's stage. Allocation must
  still refuse any inode linked or held open in the model.
- `keep-restart-visible.ndjson`: `dont-feed-the/pigeon` v0.5.1 run
  38884993562, job 116920459974, artifact 12040284242, S9-random-015.
  Keep renames its staged file, crashes before its next checkpoint, and
  Git clears skip-worktree during restart because the file is visible.
  The complete path observations and original clocks are retained.

`scripts/check-driver-traces` checks all three with TLC. Removing the
release boundary, clearing skip-worktree without a visible path, and
omitting durable journal completion must fail. Global clock continuity is checked on
full logs by `check-lab-traces`, rather than on these path projections.
