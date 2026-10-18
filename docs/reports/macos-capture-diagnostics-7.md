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
5.115 seconds without status polling. Hosted macOS results are still pending;
Linux success does not diagnose the original macOS deletion failures.
