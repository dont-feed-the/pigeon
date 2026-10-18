# Joining review: preserve failed laptop joins

This checkpoint extends the backup-preservation review to laptop failures
(spec 09 §9.6). It does not close the complete joining/signing review or
resource/elapsed release gates.

## Reproduced failure

A laptop rollback runs after the initial empty-folder check and an awaited
join. It tries to detect changed working files, then recursively removes
`.git`, `.pigeon` and `.gitattributes` by name. A later writer can own files
under those paths. The prior checks also do not make a later unlink atomic
with that writer's edits.

Two real cleanup regressions create project history, metadata, attributes
and notes after the initial check, for both a newly created folder and a
previously existing empty folder. Both lose the generated project history
under the old implementation (0.018 seconds). This is a reproduced local
data-loss mechanism, separate from any unproved original hosted timing miss.

## Correction

Both roles now retain their complete unfinished repository after a refused
or failed join. They use the existing joining marker and annex UUID when
the same group is retried. There is no recursive cleanup or snapshot-based
unlink. An unfinished group is still not configured or served before its
welcome verifies. No new persistent format or accepted ADR is introduced.
The earlier backup checkpoint's laptop rollback description is historical;
this checkpoint supersedes that behavior.

All four role/folder preservation regressions pass in 0.008 seconds after
correction. Native retry, final base integration and hosted PR results are
recorded separately before this checkpoint is released.

## Integrated local validation

Current main, the v0.6.27 candidate
`2b9670765248e56274d7e569e9f93145c9468f5c`, is integrated before this
validation. All 179 daemon-library cases pass in 18.043 seconds. Eight
native cases pass in 8.477 seconds: both role retries, four invitation
contracts and both crash-resume fixtures. The laptop test retains its
joining marker and original UUID through two ordinary refusals, keeps exact
working and private-metadata note bytes, then joins with the valid
invitation. Only the marker and resumable-UUID record are cleared on
success. The backup case retains the same guarantees.

Formatting, all-target workspace Clippy with zero warnings, headers/length,
ADR index and specification references pass. Full joining/signing review,
resource budgets and real elapsed gates remain separate.

The combined follow-up integrates current main
`d4e4e7ff65a6cc0e6b8dcb310c3406aac859a51c` (v0.6.28 candidate) before
PR validation. All 179 daemon-library cases pass again in 17.610 seconds;
the eight native join, invitation and crash-resume cases pass in 8.742
seconds. All-target Clippy, formatting and repository rules pass. The
measurement diagnostics and expiring scheduled-night policy share this
rollup's single hosted validation; no extra native performance pass or
elapsed-night result is claimed.

## Original PR S2 observation and corrected precondition

PR #98's first head `8a7ed457be15a0d3af38160a1edb509b69a6e64b` fails
native S2's later file-page assertion, after Free, in 85.935 seconds.
Original CI 39104328355/job 117606870220 retains artifact 12127718640,
14,906,511 bytes, SHA-256
`925dfa1dcb1a518a54dd5dfd527a7e30b6011a0690eef3aa6324ada113d9bc15`.
The working name is absent, but the actual local annex still contains both
the valid 300,000-byte big file and the valid six-byte text file. The page
says Safe, correctly reflecting local cached content. Hiding the working
name does not prove the later annex drop completed.

The fixture now uses its original 60-second Free wait to require both the
missing working name and the file page's fully freed state. It logs in
before requesting Free and keeps all subsequent folder/file assertions and
the same wait budget. No production status code or timing policy changes.
The original head is preserved without an unchanged-source retake; corrected
source receives a fresh combined PR validation. This is separate from the
earlier captured-path and concurrent-index observation corrections.

The corrected native S2 fixture passes in 19.773 seconds, with the original
60-second completed-eviction wait and every subsequent file/folder assertion
intact. Final all-target Clippy and repository checks pass.
