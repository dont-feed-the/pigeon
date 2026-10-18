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


## Corrected-head bridge observation and native cancellation

Corrected head `e7fcf78faa9f9e7d09273ba6c3f4cae4a018d352` passes CI
39105236302 and lab 39105236312. Its bridge run 39105236313 fails the
custodian shared-storage fixture at the offline regular-peer save. Original
job 117609883335 retains artifact 12128501718, 13,645,289 bytes, SHA-256
`221eda02a0ad471adc37b7e794b283efa2b9f562d79de84903909e2eea03c151`.
The working bytes survive, capture.json names the new file, and annex add
logs a slow operation without completing a capture. The retained annex
index.lock alone cannot identify its live owner after fixture shutdown.
There is no unchanged-source retry of this observation.

A separate deterministic native regression reproduces a scheduler deadlock:
a transfer in the same repository holds a real flock, capture registers,
and the controller suspends that transfer while capture waits for its lock.
Capture fails its original one-second active deadline in 1.016 seconds.
With cancellation, capture completes in 0.031 seconds. This proves the
mechanism; the hosted artifact does not prove the exact original lock owner.

Pending capture now cancels lower native work in its own repository rather
than leaving a suspended lock owner. One-shot children resume, receive TERM,
then drain and reap through the existing cleanup grace. Batched children use
their existing cancellation cleanup. Interrupted operations return an error
and retry from their normal authority checks; no receipt or completion is
invented. Metadata remains available, environmental waits keep their prior
policy, and priority suspension across different repositories is unchanged.
Capture claims are counted and removed when the last operation exits.
No new persistent format or accepted ADR is introduced.


The final local correction passes all 181 daemon-library cases in 17.640
seconds and all 19 Git-driver library cases in 0.429 seconds. The native
custodian shared-storage fixture passes inode reuse, retained versions,
concurrent client saves, offline regular-peer capture and bidirectional
convergence, then provider removal and bridge restart with stable identities.
All-target Clippy has zero warnings; formatting and repository checks pass.
Hosted validation still runs on the final exact source head before merge.

## Integrated release validation

PR #98, head `8881da11da4088ef8c2181e3e02c1173703cf847`, integrates main `d4e4e7ff65a6cc0e6b8dcb310c3406aac859a51c` before
validation. CI 39106911822, lab 39106911787 and bridge
39106911796 pass. v0.6.28 was published and independently
verified before this candidate. The complete joining/signing review and
M6/M7 resource and elapsed gates remain open. Fresh v0.6.29 tag gates and
independent publication/install checks are required before it is released.

The integrated rollup passes 181 daemon-library cases, 19 Git-driver library cases, eight native
invitation/retry/crash-resume cases and 118 script contracts. The short lab
diagnostic stops unmeasured and retains both live-peer snapshots. The full
native reference result remains independent evidence, not a passing budget.

The original final-head native shard 3 miss is retained from CI 39106911822,
job 117614668013, artifact 12129121185, SHA-256
`0e4445f2ffdccac74471a81659966e385e5b7d1d841f43308d5f331b79287d6b`. Its initial Protected wait expires before deletion,
with all 102 source objects and only 26 backup objects independently
hash-verified; both peers accepted the same captured main. Logs record
repeated I/O-pressure waits and continued placement. The exact live transfer
state is unproved. One unchanged-source retry of only that shard and its
selection aggregate passes; no assertion, wait budget or source changes.
This original receives no further retries.
