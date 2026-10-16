# Report: release validation after an interrupted Keep

16 October 2026. Version 0.5.2 is the next publication candidate for M4,
on current main including the merged Nextcloud bridge and shorter CI. The
v0.5.0 and v0.5.1 tags remain immutable and unpublished.

## What stopped v0.5.1

The Linux checks, macOS Rust workspace, lab and cross-runner reach passed.
The macOS random-crash job 116920459974 in slice run 38884993562 completed
all 25 schedules without data loss or duplication. Its subsequent trace
refinement rejected five observations after Keep was interrupted between
the rename and the next checkpoint. The full workflows did not all pass;
remaining superseded jobs were cancelled to preserve runner capacity.

Git can clear skip-worktree when a path becomes visible. In these traces,
the old process renamed the staged file, crashed, and the next process's
index read cleared the bit before its first recovery checkpoint. The
adapter already allowed that exact Git transition between operations, but
not inside an observation pair spanning a crash. It now permits it there,
with the same visible-path guard. This clears one bit; it cannot set Free's
bit, change a key, invent a rename, prove content or finish a journal.

The release's Linux system-install job also stopped before the service-user
doctor check. Its expected root-client refusal was emitted correctly on
stderr, while the script searched a stdout-only file. Both channels are now
captured. A successful real service-user and uninstall check is still
required from the next tag; the failed job is not such evidence.

## Local evidence

Replaying the exact captured trace against the original model reproduced
`trace:rejected:39`. The corrected adapter accepts all five failing path
streams. The retained original path projection is part of the regular
driver regression checks. Mutations that clear skip-worktree without a
visible file or omit durable journal completion are rejected. The older
recorded write-during-index and recycled-inode cases still pass, with the
missing-release mutation still rejected.

Fresh real driver traces and their full negative suite pass on current
main, including the new recorded restart cases. Workspace Clippy with lab
and core/daemon TLA features passes with warnings denied, as do the 15
packaging tests and actionlint on both changed release/CI workflows. The
CLI error-channel check reproduces an empty stdout and the expected JSON
problem on stderr, and the corrected capture finds that problem.

Hosted PR checks and the new tag's platform/release gates are still needed;
these local checks do not replace the real service-user proof.
