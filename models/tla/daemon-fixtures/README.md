# Daemon trace regressions

These are complete single-path projections of real S9 traces from
`smarter/pigeon` Actions run 38821834366, job 116721582090, artifact
12014443541, at da3d7a2. They retain the original observations and clocks;
gaps are events for other paths or specifications in the full log.

- `write-during-keep-index.ndjson`: S9-random-001, a person finishes a write
  while keep's asynchronous index step runs. Before/after enclose exactly
  one driver action, with person/time interleavings.
- `recycled-stage-inode.ndjson`: S9-random-015, a freed inode is released
  and its physical number reused for a later keep's stage. Allocation must
  still refuse any inode linked or held open in the model.

`scripts/check-driver-traces` checks both with TLC, plus removal of the
release boundary, which must fail. Global clock continuity is checked on
full logs by `check-lab-traces`, rather than on these path projections.
