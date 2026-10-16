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

- `keep-recovery-retries-a.ndjson` and `keep-recovery-retries-b.ndjson`:
  `dont-feed-the/pigeon` v0.5.13 run 38963802150, job 117185158655,
  artifact 12075336489, S9-random-015 at 2101edb. Keep crashes during
  index update; a stale Git lock leaves repeated failed recovery attempts
  before recovery succeeds. The two complete path projections retain all
  observations, including cancellation, clocks and retries. These took
  most of that shard's 16-minute trace replay.
- `release-hidden-writer.ndjson`: the native
  `tracing::driver_traces_release_changed_aside` integration test at
  5df9b31, generated with `--features tla-trace,lab`. A retained inode is
  written through a handle the scan did not see. The eager search cannot
  explain it, and must retry the full model, which accepts it.

`scripts/check-driver-traces` checks the first three with TLC;
`models/tests/test_install_witness.py` checks the recovery retry histories.
Removing the
release boundary, clearing skip-worktree without a visible path, and
omitting durable journal completion must fail. Global clock continuity is checked on
full logs by `check-lab-traces`, rather than on these path projections.
