# Protection priority between native batch keys

> Mission: keep unfinished protection visible to the daemon's scheduler,
> and release its priority on completion, failure teardown or cancellation.

The native batch driver scopes its registration to one request/response.
A worker reuses that batch for subsequent keys, revalidating authority in
between. During that interval the scheduler formerly saw no pending transfer,
so another group's scrub could run despite unfinished protection work.

Two native regressions fail against the original implementation: after the
first exact batch answer, a two-second scrub runs about 180 heartbeats and
spends its deadline while the caller still has a second key. The preserved
original run is 0/2, 2.211 seconds. The fake executable speaks one JSON answer
per key; the lower task is a real owned native process group.

The pool now owns an additional pending registration from its first request
through stop or drop. Batch processes drop before that registration. Stop
releases it after all sessions stop, even if the pool itself stays alive;
reusing that pool creates a fresh registration. The registration uses the
repository's control view, so optional downloads retain Download priority.
No future key is written before its caller's authority revalidation, and
transport authorization continues throughout each active transfer.

All 13 priority cases pass in 4.769 seconds. The new stop and cancellation
cases hold scrub beyond its ordinary active deadline, require metadata to
remain available, then observe native scrub resume with exact output. The
third new case verifies optional-download priority, stop, reuse and drop.
The existing cases still cover capture preemption, shared-class progress,
repair and active-download lock safety. Workspace all-target Clippy and
repository rules pass. Broader native validation is recorded when complete.

Scrub explicitly runs without the worktree busy lock. Protection validation
still rechecks retained references, identity and revision; this change does
not introduce a busy-lock wait in a paused scrub. The original v0.6.21
incomplete-protection failure has no established cause, and this local
priority regression does not establish a fix for that hosted observation.
The complete global within-class ordering and measured HDD budgets remain
open M6 work.

## Broader local validation

All 160 daemon-library cases pass in 15.291 seconds. All nine selected
real-peer cases pass in 101.455 seconds, covering erasure, refounding, held
deletions, declined suggestions/fresh join, capacity, returning backups,
free and keep. The held lifecycle passes in 54.981 seconds with its existing
deadline. The 13 priority cases overlap the library run; these counts are
not summed into a larger unique total. The CLI, network helper and daemon
are rebuilt for the integrated runtime before those native cases.

This branch starts from PR #91's current-main-integrated head. It will
integrate the resulting main/release commit and rerun final checks before
opening its own PR; the earlier PR's CI is not restarted for this change.
