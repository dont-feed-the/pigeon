# Native macOS capture diagnostics

> Mission: preserve failed evidence and distinguish fixture portability from capture faults.

## Original background observations

The periodic run 39120772097 checks v0.6.35, source
`080a1b78274de8a9f6c99bc6ade5f564bd228b0d`. The workspace passes
834 tests and fails one same-repository capture/preemption fixture. Native
shards 0 and 3 each fail a deletion assertion; shards 1 and 2 and packaging
pass. This is background evidence, not a full macOS pass.

Original workspace job 117657528220 fails after capture has returned:
the lower transfer has not completed within two seconds. The fake Git
program invokes external `flock` without checking its exit status. macOS
supplies no such command by default. A controlled Linux negative control
makes that command return 127, reproducing the same completion assertion
in 2.018 seconds. This proves a portability defect in the fixture; it does
not establish whether that exact mechanism caused the original Mac miss.

The fixture now uses Python's native `fcntl.flock` in the long-lived child
process itself. Its ready marker is written only after acquiring the lock;
the capture must contend with that live owner. Signal handling is installed
before publishing readiness. With `flock` unavailable, the corrected case
passes in 0.047 seconds. All 15 priority tests pass together in 4.749 seconds.
No production timeout or scheduler policy changes.

The other failed jobs retain independent native fixture archives:

| Job | Artifact | Original archive SHA-256 |
| --- | --- | --- |
| 117657528268 | 12134913225 | `0d3ffcc8a6980e1ec29b0e3754a35559e89ce0890579934188c9a80bec065601` |
| 117657528315 | 12134322268 | `bb255b4bbc65a7c162033c74be8527a9d7e6d665c5e01a448ba52dc122c02976` |

Their final candidate observations report respectively two and one pending
changes; catalogue paths are still present and historical object bytes
remain retained. Both jobs also pass native creation-watch cases. No
original mechanism is proved, no data loss is observed, and the originals
are preserved before any follow-up. No unchanged-source retry is used.

## Bounded follow-up

Debug admission events now record the native watcher revision, health,
settling flag, monotonic observation clock and whether a full background
observation is due. They contain no working-tree paths or contents. This
separates a missing hint from an observation admitted but unable to capture.
The native watcher case now removes its saved file and waits for the
catalogue deletion, using only Git reads and no status API polling. Existing
creation and new-folder checks remain.

`macos-background` accepts `capture_diagnostics_only=true`. It occupies one
Mac runner and runs the native preemption case, both watch cases and the two
original deletion fixtures with matching lab programs and the required
verified patched git-annex. It restores main's dependency cache, does not
run the ordinary full background workflow at the same time, and retains
failed fixture directories. A focused pass cannot count as a complete
macOS suite, a qualification night or a resource-budget result.

Local validation passes both watch cases and both original deletion
fixtures together in 19.565 seconds. The watcher save/delete case takes
5.115 seconds without status polling. The first focused macOS result is recorded below; Linux success alone
does not diagnose the original macOS deletion failures.

## Preserve completed compilation caches

The original workspace job reports no matching Mac Rust dependency cache.
Clippy takes 2 minutes 28 seconds and test compilation another 3 minutes
6 seconds before 835 tests start. The main-only cache save is skipped
when the later runtime assertion fails, so that completed compilation
cannot warm the next background run.

The existing workspace cache save now follows successful Clippy and
precedes runtime tests. The native cache save follows the existing seam
build and precedes that shard's native fixtures. These are pure step
reorders: every job, step, save condition, cache path and key is preserved,
including main-only and native shard-0-only restrictions. Pull requests
still restore main's caches and cannot save them. Failed builds cannot
reach the save; failed later tests retain their original failure verdict.
Actual warm-cache and end-to-end timing effects remain to be measured.

## Focused macOS result and deletion recovery

Focused run 39124778503/job 117667506936 checks exact source
`226cd00d210a297074a71d5ddc2bede973f303dc`. Four of five cases pass:
the native lock fixture (0.199 seconds), new-folder watching (8.036),
save/delete watching (10.955) and time-based restore (34.743). The free/keep
case again fails its deleted-file view after 123.842 seconds. This is not
a full or focused passing Mac run.

Its new admission diagnostics show watcher revision 14 acknowledged before
the deletion, then remaining at 14 with `due=false` while authoritative
candidate scans report one pending change. This identifies a skipped
background observation after a lost hint; why the native event was lost
remains unproved. Original artifact 12135782659 is 3116478 bytes, SHA-256
`d6be2393b5afdbebae4107498ac8f456d608c28eb17706e7aeda42f704c72a0b`.
The archive and complete job log are preserved before any further run.

The correction reuses actual pending-deletion observations from existing
history, file-view and telemetry reads to wake the owner. Ordinary pending
saves retain quiet/hot scheduling. Held deletions and unsettled free/install
journals do not request a wake. A small core algebra coalesces repeated
observations across folder scopes, clears requests after captured removals
and rearms paths after clean observations or restoration. Four new laws
run 1000 cases each. Existing capture and authority checks still decide
what is committed; these requests are never positive deletion authority.

Failure retries keep their delay. A new invalidation arriving during an
observation survives its completion. The test-only frozen-revision seam
is compiled out of ordinary daemons. With native revision frozen, the
read-only history case fails before the correction in 16.911 seconds;
the public `status` command is unsuitable as a negative control because
it already captures explicitly. The initial pilot assertion assumed one
history response, but history may include a separate explanation; it is
corrected before the causal RED result. A later fixture guard assumed no
other pending observer could win; telemetry can correctly discover the
deletion first, so the final check instead requires a new recorded
invalidation with the native revision still frozen. All pilots are retained.

Final local and hosted validation of the coalesced correction follow below;
the earlier four passing Mac cases do not verify this new correction.

## Coalesced local validation

The integrated correction passes all ten change-scan laws (1000 cases
each, including the four new coalescing laws), 194 daemon library tests
and all eight focused native cases in 36.395 seconds. The loss-injection
case completes in 7.708 seconds with a new authoritative invalidation
and the native revision still frozen. A separate broader run passes
59 install, backup-history and suggestion cases. All 139 script contracts,
workflow actionlint, zero-warning workspace/all-target Clippy and repository
checks pass. These checks use current main v0.6.37 integrated before
submission. Fresh hosted Linux and corrected focused Mac results remain
required evidence before claiming their respective passes.

## Integrated release validation

PR #107, head `e5c7f7601647731c59dc374aa1610db95f535544`, integrates main `79d290b6f3d7885f76be199630d29e6faeaba1bb` before validation.
CI 39126619540, lab 39126619545 and bridge
39126619537 pass. v0.6.37 is independently published
and verified first. Local validation passes ten 1000-case change-scan laws,
194 daemon library tests, eight focused native and 59 broader native
cases, 139 script contracts, actionlint, zero-warning Clippy and repository
checks. Fresh v0.6.38 tag gates and independent archive/install verification
remain required. Corrected Mac evidence is recorded separately; no full Mac
suite, reference/HDD budget or actual elapsed gate is inferred.

## Corrected hosted Mac result

Run 39126618628/job 117673006519 checks integrated head
`e5c7f7601647731c59dc374aa1610db95f535544` and passes all six selected
cases in 39.685 seconds, without a retry. The lock fixture takes 0.089
seconds; frozen-hint deletion recovery 8.809; time-based restore 21.248;
save/delete watching 30.677; and the previously failing free/keep case
39.683. The new-folder case also passes. Original failures remain retained.

PR #107 merges after CI 39126619540, lab 39126619545 and bridge 39126619537
pass, then ships in independently verified v0.6.38. The focused result is
linked from its release notes. Full periodic Mac run 39128408817 uses source
`24dcd39b382b7e3a961b8554c5002c6a15d04a4d`; it is pending and cannot
be replaced by the focused pass. Resource and elapsed gates remain open.
