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
