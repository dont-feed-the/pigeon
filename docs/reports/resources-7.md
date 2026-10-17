# M6 resource-control progress

M6 is not complete. Its controller components, configuration validation and
native content shaping ship incrementally in v0.6.x patches; the milestone
release remains v0.7.0. No reference-tree or foreground HDD resource budget
has been claimed to pass.

## Controller components

`pigeon-core::resources` supplies the Gentle/Balanced/Fast defaults,
explicit power/network waiting decisions, the size-dependent capture quiet
period, disk back-off with capture's progress floor and quiet recovery,
delay-based rate updates, and a shared bounded token bucket. All timing is
supplied monotonic input. Manual ceilings win over automatic rate floors;
backwards clocks and rate changes cannot manufacture past credit. Oversize
reservations are refused for splitting rather than silently undercharged.

Fourteen hegel laws run 1000 cases each, including extreme integer clock/rate
ranges. The active-time clock separately proves that arbitrary pauses spend no
operation budget, backwards clocks refund nothing, and splitting an active
interval changes no charge. The next native scheduling checkpoint wires that
clock into heavy one-shot children. These are pure controller laws, not
measurements of foreground workload interference or real network delay.

The P2P filter can distinguish content DATA bodies from protocol and Git
history bytes without changing their order or bytes. Two more 1000-case
laws check both directions and arbitrary transport chunk boundaries.
Both native relay directions now enable classification and use a single
upload bucket and download bucket for the whole daemon. Existing filter and purge laws still pass, for 22
selected core tests in total. A Clippy failure on the new fixture's constant
assertion was corrected to an explicitly justified fixture panic.

`config.toml` parses resource presets, exact decimal/binary byte rates,
battery/metered overrides and local full-speed/scrub windows. Invalid
readings are rejected whole instead of silently becoming unlimited values.
Missing details follow the preset; explicit choices are independent.
Five configuration checks pass, including comment/unknown-setting
preservation and overflow/fractional-rate cases. The daemon reloads complete valid settings every two seconds, retaining
its last valid reading after an invalid edit. CLI and browser changes
validate a candidate before writing it; manual details override presets.

## Native content boundary

Authenticated selected QUIC paths provide smoothed RTT and a cwnd/RTT
capacity estimate. The controller updates every 200 ms, with a ten-minute
minimum baseline, bounded 64 KiB reservations and explicit ceilings even
inside full-speed windows. Control/history frames do not reserve tokens.
Each connection/path/address keeps its own rolling minimum; adding a distant
peer cannot turn its propagation time into congestion on a local path.
Queued content rechecks the existing erasure/admission filter at most every
200 ms and immediately before forwarding. No new authorization rule or
positive custody proof is introduced.

macOS probes read IOKit power sources and NSProcessInfo low-power mode,
with a serial Network.framework monitor for expensive/constrained paths.
The native module and its lifecycle test cross-compile and pass all-target
Clippy for Apple Silicon. The actual periodic Apple Silicon run
[39029055613](https://github.com/dont-feed-the/pigeon/actions/runs/39029055613)
also passed workspace Clippy and `monitor_moves_between_threads_and_samples_native_apis`.
That workspace run had 599 passes and four failures: three overly long
bridge socket fixtures and the root-link path guard below. It is not a
passing macOS suite or a foreground resource-budget measurement.

Linux probes reuse D-Bus for UPower, power profiles and NetworkManager,
with a two-second sampling deadline. Unknown observations use ADR 0037's
fallback. Linux I/O pressure pauses pending content and subsequent scrub
slices, with quiet recovery. The later native scheduling checkpoint adds mid-file child pausing.
Own-read latency measurement remains outstanding; this is not the full disk
controller.

137 selected law/native/UI/configuration tests passed locally, including a real
QUIC regression that queues bytes waiting for power, then erases their key:
no queued bytes reach the client. This is finite evidence, not an elapsed
resource-budget measurement. English and French messages explain waits
without replacing Missing or Needs you; Resume remains a person's pause.

## Remaining

- Extend the staged work queue and validate native pausing against the
  foreground HDD workload, including explicit request paths.
- Integrate file quiet/hot-path scheduling, native priorities and supervisor
  behavior. Validate foreground disk contention and read-latency feedback.
- Measure the reference-tree idle/scan/memory budgets in CI and the E17
  foreground video workload on the required HDD/platform combinations.
- Validate supported filesystems, upgrade and crash behavior, integrate
  current main, complete the release gates and publish v0.7.0.

No 10-minute budget, 30-minute HDD run, 72-hour soak, seven-night result or
two-week dogfood result is inferred from these finite tests.

## Native child scheduling checkpoint

Heavy one-shot annex commands use owned process groups, STOP/CONT and active
deadlines. Cancellation resumes and kills the whole unreaped group; after
waiting reaps the leader, the guard disarms before another suspension point
to avoid signalling a reused PID. Metadata calls bypass the controller.
Capture ignores power/network waits and keeps its disk share. Each group's
local stream waits exclude time only from that group's transfer budget.

Twenty-eight selected driver and native checks pass, including real parent
and helper heartbeats frozen beyond the ordinary deadline, exact output
after resuming, dead-descendant assertions after cancellation and genuine
timeout, and independent groups' deadline accounting. These finite process
tests do not establish the HDD workload budget.

The first integration check found a real scheduling gap: content work held
the group lock and the round while waiting, blocking later capture. A native
256 KiB transfer capped at 1 B/s reproduced the failure; a first 64 KiB
fixture was insufficient because the allowed initial burst completed it.
Placement now plans under the lock and executes key-only transfers outside
it. Normal install and keep passes use available local/provider objects;
kept current-version downloads follow protection work outside the lock.
Every key rechecks group identity, membership and retained authority, sharing
only a complete revision-bound retained-key cache. Active stream filtering
remains unchanged. Stopping or moving a group aborts and joins its worker.

The first integrated driver/native run passed 37 checks, including the live
capture/history regression, erasure lifecycle and refounding. This is not
the complete cross-group priority queue or a measured HDD budget.
Explicit user-request download paths still need scheduling review.

The broader run passed 45 checks, including free/keep, returning backup,
absent restores, queued erasure and the existing positive-answer barrier.
Native S2 (11 checks) and S13 (7 checks) passed; their filesystem and roster
traces were accepted by TLC. A final 12-check pass covers the alias/root-link
guards, shortened bridge fixtures, child pause policy and a live later-save
capture during a person's pause. These are finite observations.

The next CI head (`ff93370`) still failed the 102-file protection deadline
and a portal suggestion assertion. The saved, checksum-verified fixture has
all 102 backup objects matching their sizes and SHA-256 keys; the laptop's
fetched annex metadata is older. Cold location planning now runs outside
the worktree lock, and complete unchanged retention readings need no lock
per file. A controlled native location probe proves a new signed capture
finishes before that probe is released.

Location claims are read in bounded 64-key native batches instead of starting
annex and its helper per key. Missing, duplicate, malformed or mismatched
answers fail the entire batch closed. A real 67-key fixture crosses the
batch boundary and includes a missing key. Nine selected checks pass,
including the 102-file lifecycle in 60.4 seconds, normal free/keep and the
queued-content erasure barriers. This is a finite local observation, not a
controlled CI speedup or a reference-tree budget. The portal lifecycle also
passes locally without NEXTCLOUDCMD; the real-client CI check remains required.

The same head's actual Mac workspace passed all but the suspension fixture:
twelve resumed shell/sleep forks could consume its 600 ms active budget.
The fixture now requests completion at the first resumed checkpoint while
keeping the 800 ms pause and 600 ms budget. The genuine active-timeout and
dead-descendant checks remain. Missing Linux D-Bus services no longer create
fresh property-change subscriptions or repeated cache warnings every probe;
properties are polled directly, preserving unknown-value fallback.

Upstream `Annex/Transfer.hs` chooses an independently grouped transferrer
when explicit stall detection is enabled (`Annex/TransferrerPool.hs`).
Managed repositories and peers now disable all three stall options; the
driver retains its bounded active deadline. `Types/StallDetection.hs`
defines `false` as disabled. The source tarball was checked against the
repository's pinned SHA-256. E17 remains required for real transfer-tree
pausing under disk contention.

PR #70's first CI also found a cancellation preview invalidated by incoming
metadata and `read-tree` racing an index lock during refounding. The fixture
performs a fresh explicit review only for `erase-stale`; production consent
never retries automatically. Atomic `read-tree` uses the existing one-second,
caller-deadline-bounded index retry. Real Git fixtures show a short owner-held
lock succeeds after its owner removes it and a persistent lock stays exact.

## Native transfer batches and local-content observations

The next head (`47974c5`) again missed initial 102-file protection in CI.
Its checksum-verified saved fixture holds 102 correct laptop objects but
only 80 on the backup, unlike the previous stale-metadata fixture. Logs
show duplicate transfer-lock conflicts and interrupted fetches. Status calls
also spend 1–4 seconds starting per-key local-content queries.

Placement now reuses native `get` and per-peer `copy` batches, feeding only
one key after revalidating its current authority. Batches use the same owned
process-group controller and active clock as one-shot operations. Cancelling
an exchange kills its process tree and discards the session, so the next key
cannot consume a stale answer. Native process tests cover reuse, an 800 ms
deliberate pause within a 600 ms active budget, real timeout and cancellation
of stopped descendants. All 21 selected checks pass locally, including the
102-file lifecycle in 28.5 seconds before batching local-content observations.
That timing is finite local evidence, not a controlled CI speedup.

Local presence observations now use bounded 64-key `contentlocation --batch`
queries, preserving blank answers and rejecting incomplete batches. Unusual
encoded key names use the scalar query. Upstream `Command/ContentLocation.hs`
declares the command batchable and checks the native object exists. Location
claims remain separate from durable positive-copy proofs and erasure authority.
With both batches integrated, 65 selected native checks pass, including full
install/crash recovery, protection/free/keep, a returning backup, content
erasure barriers and controlled process lifetimes. The 102-file lifecycle
took 30.5 seconds in that concurrent run. The real 67-key/missing-key fixture
also checks batched local-presence answers across the boundary.

## Repeated-save safety regression found during scheduling

A new native hot-file fixture exposed a pre-existing problem on `a30376d`:
four ordinary in-place saves followed by another write lost or changed
unprotected captured objects. The regression failed locally in 0.85 seconds
without the new scheduler. Automatic merge and release preparation were
stopped before any tag was created.

Git status and index refresh can run the annex clean filter before capture's
explicit `annex add`. Upstream `Command.Smudge.clean'` calls
`Annex.Ingest.ingest'`, whose `linkToAnnex` honors `annex.thin`. A per-add
`annex.thin=false` override therefore misses the earlier ingestion. All
managed Git/annex children now inherit the immutable-copy override, including
their filter children; the existing explicit re-thinning step still requires
protection. The regression checks every captured object's exact bytes after
later saves, index refresh and a pending-status observation.
It passes with the fix, alongside 79 selected native install, contract,
recovery, thinning and driver checks. Full all-target Clippy passes locally.

## File scheduling checkpoint

The native background capture adapter uses monotonic elapsed time, two equal
stat observations and the size-dependent 2–60 second quiet period. Seven
distinct captures in an hour space later captures of that path by ten minutes;
an hour without changes cools it. One earliest deadline wakes the normal round,
without a sleeping task per file. Explicit capture requests remain immediate.
Actual successful proposals and concurrent saves are recorded separately from
failed attempts; a remembered near-install classification survives settling.

Twelve pure laws run 1000 cases each, including oldest-observed-change
ordering and fingerprint changes that move a save behind older candidates.
The clock uses Linux CLOCK_BOOTTIME and Darwin CLOCK_MONOTONIC_RAW, so sleep
counts toward quiet and hot-path waits. The native clock API cross-compiles
for Apple Silicon; a short elapsed check is not a physical suspend test.

The native same-size edit fixture initially failed because Git status invoked
a clean filter before settling. Candidate discovery, explicit capture, status
and install's dirty observation now use the existing index parser and stat
observations, plus a names-only untracked query. An install-wide index refresh
also ingested unrelated saves; that global refresh has been removed.

A same-size save with its modification time preserved exposed another native
shortcut: non-batch annex add can skip the file in withFilesMaybeModified.
The NUL-delimited batch path in Command/Add.hs enters start/ingestAdd directly.
Replies are checked by unique literal file name, without assuming completion
order. Unportable new names remain held. Native captures preserve immutable
objects and executable modes.

Annex's own temporary hardlink changes ctime during ingestion. Exact captured
byte/stat/index observations are therefore made after ingestion with controlled
calckey reads. A read-only kernel timestamp fence is read before hashing; only
an older ctime and a complete unchanged stat are retained. Verification does
not change the index timestamp to manufacture that fence. Native Git cache
entries also require both their modification and change timestamps to predate
the index; an old preserved mtime alone cannot clear a current-tick ctime.
The focused native-cache regression failed before this check. Their private cache
survives a restart, includes ctime and the
exact staged pointer/mode, and is invalidated by later edits. Missing, malformed
or future cache formats grant no shortcut. An unchanged suggestion also binds
that observation to its exact signed proposal, so a later same-size edit or
executable-mode change cannot hide behind its old modification time. Rebuilding
a missing observation restores the accepted index pointer without creating an
empty proposal. The install-window classification uses ctime, retaining an
application's original mtime without making a recent save look old or a later
save look concurrent. These regressions were reproduced before the correction.
The cache authorizes no transfer, overwrite or deletion; install still proves
displaced bytes and authority separately.
Only paths just installed receive a completion observation, so another path's
unsettled save is left alone. Normal idle recovery, keep, thinning and metadata
commits no longer refresh every working file as a side effect. The full native
metadata includes ctime, but the durable journal's tuple remains unchanged.
After a restore's own rename, identity/mode/mtime and the independent old-byte
proof still apply; the full ctime comparison remains before the move.
Conservative verification of unchanged foreign bytes must create no empty
suggestion. A genuinely unborn repository may have no index; a missing index
with an existing HEAD remains an error. These extra reads and cache writes need reference
budget measurements; no idle, memory, scan or HDD budget is inferred here.

The initial 58-check checkpoint covered index formats 2/3/4, conflicts, crash
recovery, seven actual hot-file captures, unrelated-file progress and pre-quiet
observation. The broader run found fresh repositories without an index,
post-rename ctime comparisons and empty proposals from unchanged foreign bytes;
those implementation failures were corrected. The next workspace run passed
714 of 715 checks; its malformed-cache fixture had left a valid native Git stat
cache in place. Clearing that independent cache without changing its pointer
makes the intended fallback test meaningful, and the corrected fixture passes.
After integrating current main at `3f5e5ef`, the workspace checkpoint passed
all 718 checks. A subsequent 69-check native run passed install, suggestions,
restores and shared-pool regressions on the final pre-hash fence. The final
native-cache ctime correction then passed both focused timestamp-fence checks;
all-target Clippy and repository checks pass. Fresh generation of all 13 driver
cases passed TLC and the corruption, missing-step, cancellation and pipeline
negative checks. Native S2 passed 11 checks in 41 seconds and S3 passed 14
checks in 13 seconds; TLC accepted both filesystem and roster streams.

A subsequent API review found that CLI status still used immediate capture;
its native handler regression failed without any wall-clock race. Status now
uses the background policy and wakes the round to recalculate its earliest
observation deadline. Explicit data-changing requests retain immediate capture.
The corrected handler regression passes. Updated-head native validation and CI
remain required before merging. Native priorities, the complete work queue
and the resource budgets remain outstanding.

## Periodic macOS failures

The v0.6.0 periodic run found a missing-file history lookup through macOS's
`/tmp` alias. A Linux symlink fixture reproduced the same `not-in-group`
response before the fix. Group lookup now resolves existing ancestors of
missing paths; shared-path validation resolves an alias of the worktree
while retaining and checking every component below it. Dangling links,
non-directories and ambiguous missing `..` paths are refused. The native
migration/history regression and folder-restore checks pass locally.

The broader workspace check caught an in-tree link back to the root, which
the first alias change mistakenly treated as a new root alias. Resolution
now chooses the outermost root spelling and retains that link in the checked
suffix. Existing and outer-alias root-link regressions pass. Three bridge
lifecycle fixtures now use the established short `/tmp` socket root.

The Linux-only scrub fixture's entire module is now conditional, avoiding
unused imports under macOS Clippy. The subsequent periodic run
[39037709329](https://github.com/dont-feed-the/pigeon/actions/runs/39037709329)
on v0.6.2 passed workspace CI and all four native daemon shards, including the
previously failing mass-deletion case, and advanced through lab to slice.
The whole periodic suite is not yet reported complete. This does not establish
the newer file scheduler's Mac runtime or foreground HDD budgets.


## Independent settings usability

English and French sessions used real isolated daemon HTTP pages and forms,
without reading implementation source. They found raw catalogue IDs in
confirmations, ambiguous full-speed ceilings, missing automatic-resume
explanation, and CLI-oriented validation errors. Focused router regressions
cover the observed confirmations and localized validation errors. 99 web
and translation checks pass after the fixes.

Both focused retakes understood the preserved ceiling, automatic waits and
web recovery; invalid values left prior settings intact. The remaining
clock/window questions were clarified to name this computer's clock and
retain power/metered waits during full-speed windows. These sessions did
not test JavaScript presentation, visuals, hardware transitions or budgets.
Original sessions and retakes are recorded in
[m6-usability-english.md](m6-usability-english.md),
[m6-usability-english-retake.md](m6-usability-english-retake.md),
[m6-usability-french.md](m6-usability-french.md), and
[m6-usability-french-retake.md](m6-usability-french-retake.md).

Nine real git-annex integration tests also passed locally in 35.7 seconds:
protection, free/keep, returning backup, restores and explicit erasure. This
finite result does not establish the ten-minute resource budgets or E17.


## v0.6.2 publication checkpoint

PR #70 merged after exact-head CI, lab and all three bridge cases passed.
The previously failing 102-file lifecycle passed in CI in 76.9 seconds.
[v0.6.2](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.2)
then published on 17 October at 13:42:34 UTC, source
`3439d9d67fc8b1a6368f4eb15e2633a28a2ee6dd`, with both platform archives,
pinned dependencies, installer and checksums verified. Its two first-attempt
misses and targeted follow-up fixes are recorded in the
[publication KPI](../release-kpi.md#v062-observation). This patch releases
the completed controller and safety work; it does not complete M6 or its
reference-tree, filesystem, foreground HDD or native-priority gates.


## Own-read cache advice continuation

Native SHA-256 verification now uses a bounded streaming reader. Linux requests
DONTNEED for each completed page range while the hash runs, and for the last
partial page only after an actual EOF; empty-buffer reads never mean EOF.
macOS opens the reader with F_NOCACHE. Bridge hashes share this reader instead
of a second cached implementation. All actual bytes still pass through SHA-256;
cache advice grants no integrity, authority or custody proof. The kernel may
retain pages despite advice; no eviction or foreground HDD budget is inferred.

The first native checkpoint read 1,048,613 exact bytes in 257-byte reads,
including empty-buffer calls, and checked that completed-page advice does not
wait for the whole file. Apple Silicon API checking passed. The initial
Clippy run found a leftover bridge import, which was removed. Final validation
of the EOF-tail refinement and integration still precedes publication.

Independent CLI agent sessions observed final bytes through normal read-only
views. The corrected status handler visibly distinguished pending changes from
captured local versions; no false safe claim was observed. The retake found
that saying a change was waiting to be saved could be confused with an editor
save. English and French now distinguish pigeon's recorded version. The guide
explains rapid-save history granularity. Sessions and limits are recorded in
[m6-capture-usability-english.md](m6-capture-usability-english.md) and
[m6-capture-usability-english-retake.md](m6-capture-usability-english-retake.md).


## Final rolled-up validation before v0.6.4

The corrected status handler passed the complete 720-check workspace run,
then native S2 (11 checks) and S3 (14 checks), with TLC accepting both
filesystem and roster streams. CI on `7951c52` separately missed recovery-sheet
protection: its artifact contains both expected exact objects, matching
location logs and backup main, while the last live status stayed Published.
Artifact 12105134222 has SHA-256
`09aa0cc908deb1076038ff965029f24ce0513389004ea6440eadf70cfb3979b8`.
A targeted retry was cancelled after the independent dependency audit required
a new head. The sibling-merge cause was then reproduced below.

The audit rejected the retired D-Bus support crates in RUSTSEC-2026-0370,
0371 and 0372. zbus 6 removes them. Linux notifications now use its standard
D-Bus interface, while macOS retains notify-rust without its Linux backend
(ADR 0082). Three private native D-Bus tests pass, including immediate action,
a foreign notification ID and closure. The full dependency audit passes
without new exceptions.

The separate cache-reader checkpoint passed 71 of 72 native checks; the
remaining recent-foreign-save trace again lacked a completed install. A
fresh-ctime regression reproduced the cause: the real coarse clock fence
had not advanced yet. Verification now asynchronously waits up to 50 ms for
that actual clock reading before hashing. Every path still needs an older
ctime, a complete unchanged stat and the exact hash/pointer/mode. Future
stamps remain unknown; other eligible files in the batch can progress.
Neither index timestamps nor proof comparisons are relaxed. Eight consecutive
fresh observations and the bounded future-stamp check pass.

The maintained D-Bus adapter, cache reader, status handler and clock-fence
regressions pass together (seven checks), and all-target Clippy passes.
The completed components are rolled up for one new PR CI run. Fresh full
workspace/native/trace validation still precedes merging and publication.


## Historical status after independent merges

The final `5263f7a` local workspace run passed 724 of 725 checks; recovery-sheet
again missed its protection deadline. Its preserved fixture contains the exact
old and new objects and matching location logs on all peers. Bob and the backup
had independently merged the same two parents: their merge IDs differed, while
their accepted recipe and both historical captures matched. The earlier
`7951c52` CI artifact has the same shape: Bob's `70c441f` and the backup's
`208f22a` have the same two parents.

Current-file status already accepted the shared recipe, but retained-history
status required the backup to contain Bob's entire latest merge commit. That
condition could never become true when identical trees needed no further merge.
The group therefore stayed Published with an empty current-file problem list.
A native Git DAG regression failed deterministically on that condition.

For groups without purge or refounding history, historical evidence follows all common
ancestry roots and their actual retained pointer history. A backup can prove a shared capture without adopting
an equivalent sibling merge. Unrelated histories with identical keys prove
nothing, captures absent from the peer do not count, and an unreadable ancestry
remains unknown. Actual annex location claims, recency, checking/lost state,
custodian role and purge exclusions still apply. No timeout was raised.
Final native recovery, health, traces and exact-head CI precede publication.


The periodic macOS run on v0.6.2 (39037709329) passed workspace, all four
daemon shards and all lab groups. Its serial remaining-scenarios job passed
S1, S2 and S4–S16, but S3 failed the already-completed-restore trace assertion.
That source predates v0.6.3's VerifyRestore fix; it is retained as a background
failure, not reported as validation of the current source. The other macOS
slice jobs were still running at this checkpoint.


Review before merging added a fresh-reference guard. A fifth native DAG test
showed that shared old history alone could acknowledge a historical key whose
fresh post-erasure reference the backup had not seen. The test failed before
the guard. Groups with armed purge history therefore retain the previous
complete-head requirement; this patch does not relax it. Per-reference live
birth evidence for equivalent merges in those groups remains future work.
The four ordinary shared-history/negative regressions and the guard are checked
together. The parent `03b973d` passed all 729 workspace tests, all 13 fresh
native driver/TLC scenarios and negative checks, and native S2/S3 plus TLC.
Final guard and affected purge/restore checks, followed by new exact-head CI,
precede publication; the prior green CI is not reused for a changed commit.


## Restore requests leave capture available while receiving

File and folder restores now release the group worktree lock while their
key-only content requests wait or run. They keep the exact pre-download
filesystem baselines. After receiving, they refresh the complete purge reading,
reacquire the lock, check the current group and content policy, and recheck
staging space before installation. A newer captured save is never adopted as
the replacement baseline. The request still retains its ordinary signed
reference before asking for missing bytes.

Three native interleaving regressions first failed at an actual get waiting for
production resource admission: the group lock remained held. After the fix,
later saves can be captured while that get waits, real status API requests
return, unchanged files/folders restore after resuming, and removing the group
prevents the old request from installing. Four existing baseline/bytes/mode
checks and the five historical-acknowledgement checks pass alongside them
(14 focused checks). These fixtures block admission before get starts; they
are not evidence of a mid-transfer SIGSTOP workload or the HDD budget.

Eight native integration checks passed on the initial follow-up: time/folder
restores, missing contents and durable retry, health evidence and a limited
transfer that leaves later capture available. Native S3 passed all 14 checks
and TLC accepted its filesystem and roster streams. Distinct dates in the
small unit fixture select an old folder snapshot; no retention period or
resource interval is claimed elapsed. Full current-main CI still precedes
merging. The complete cross-group queue, native priorities, read-latency
feedback, filesystem admission and measured budgets remain open.


## Trace observation retry fairness

The first v0.6.4 tag slice passed all 25 native S9 shard-0 schedules but
rejected a trace's failed filesystem observation at clock 49 during Index:
three immediate stat/hash attempts all saw a changing file. Original artifact
12107900725 is 218271 bytes, with verified SHA-256
`ffb8bc45e31354827b05d7a1e7ce59aed9e5716da6ff948b9bde917750ac15db`.
One bounded retry repeats seed 156199965296 and the identical 25 schedules;
the retry passed its complete native and model checks. The failed observation
remains retained in [the original trace](evidence/v064-s9-observation-failure.ndjson).
No model verdict or release gate was waived.

The follow-up collector gives concurrent writers one millisecond between
unsuccessful observations. It still makes at most three attempts, compares
the full native stat around the actual hash, and reports persistent changes
as failed observations. A deterministic native-file interleaving failed
before the yield: the writer could never finish before the next attempt.
The regression requires the recorded hash to name the completed bytes;
the existing continuously changing-file check must still fail closed.
This is retry fairness, not evidence that every S9 observation race is gone.

The restore patch passed all 735 parallel workspace checks before version-only
integration of v0.6.4 main; its exact restore-source patch remained unchanged.
Focused current-version restore/history/trace checks and new exact-head CI
precede merging this follow-up.


The retained keep-index schedule now runs as a named S9 regression. It passed
locally in 26.4 seconds with the current-version binaries; TLC accepted both
filesystem streams and the roster stream. The focused current-version set
passed all 17 checks, including persistent-change refusal and the new yield
interleaving. Full all-target Clippy and structural checks passed.

## v0.6.4 publication checkpoint

PR #72 merged at `45a2106`; [v0.6.4](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.4)
published at 16:58:43 UTC from `fa00dcc`, with all assets and the Linux install
independently verified. The bounded S9/publish retries and missed publication
gap are recorded in [the KPI](../release-kpi.md#v064-observation).
Background macOS run 39051800511 targets this exact release. The preceding
v0.6.2 run completed with every workspace, daemon, lab, random-crash and S5
job green; only its old S3 restore-action assertion failed. Neither result
is represented as current-source macOS completion or measured M6 budgets.

## Filesystem preflight follow-up

The [filesystem checkpoint](filesystems-7.md) records native volume identity,
strict role/platform admission, create/join refusal before repository writes,
doctor diagnostics, and the accepted English/French usability retakes.
Fourteen focused native checks passed, including preservation of an existing
binary file on refusal. CI adds actual ext4, btrfs and xfs fixtures to one
existing daemon shard; their first results remain pending at this checkpoint.
The reference-tree/HDD budgets, complete cross-group priority queue and
remaining resource measurements still prevent M6 from closing.


## Cross-group heavy-work priority checkpoint

The three existing native child classes now share daemon-wide admission:
capture precedes transfer, which precedes scrub. Pending operations count
before launch and during suspension; same-class operations may run together.
Registration is released on success, refusal, failure and cancellation.
The existing process-group controller observes priority within its normal
20 ms poll and excludes deliberate suspension from active deadlines. Capture
retains its existing disk floor and power/network exception; metadata and
history calls still bypass heavy-work admission.

Content DATA waits behind capture through the existing shared controller.
Its ordinary erasure/admission check still runs during the wait and before
forwarding. Priority neither grants content authority nor proves custody.
This checkpoint orders three native classes, not all six placement classes,
individual keys or folders in the complete queue.
Review found that serving-side `AnnexSyncer` constructed an independent
repository for repair. It now shares the daemon's child controller and
native environment; metadata lookups remain unshaped and the durable/hash
positive-answer barrier is unchanged. Its own hash reads are not yet
latency-controlled, so this is not the complete read/child queue.

Three hegel laws run 1000 cases each: no pending count denies capture,
nonempty higher classes hold lower classes, and releasing higher work restores
admission independently of same/lower counts. The direct controller regression
failed before priority was connected. Eleven focused controller/native/QUIC
checks pass after the change. Real owned shell-process groups demonstrate
cross-group capture/transfer and transfer/scrub suspension for 2.2 seconds
against a two-second active budget, higher-child progress, exact output after
resumption, metadata availability, cancellation release and concurrent
same-class progress. These use controlled child fixtures, not an HDD workload.
The first fixtures used annex argument forms the production allow-list refused;
the fixtures were corrected, and the allow-list remains unchanged.

A real QUIC fixture registers capture in another group, observes the production
payload wait, then erases the key. No queued bytes reach the receiver. Its
capture registration is controlled; it is separate evidence from the native
process-group fixtures. Existing active-release and power-wait erasure checks
also pass.

English and French simulated wording retakes accept explicit waiting, saving
as the reason, and automatic resumption without manual action or a claim of
protection. The first shorter headlines left waiting/resumption implicit;
both catalogues now explain them. These are wording reviews, not native macOS
or foreground-performance measurements. Full workspace and current-main CI
results still precede merging this checkpoint.

The serving-side regression reproduces native repair starting despite low
battery before the controller was connected. An earlier counter-checkpoint
fixture also failed; the stronger heartbeat fixture records actual execution
instead. The retake covers power wait, another group's capture, metadata
availability, eventual repair and cancellation before launch. These controlled
child fixtures do not measure real-media repair performance.
The combined controller/process/QUIC retake passes all twelve selected
checks, including queued-repair cancellation with no stale registration.

The v0.6.8 candidate repeated the held-file/capacity timeouts seen on v0.6.7.
Its saved held fixture contains 102 valid laptop objects but 36 backup
objects; no absent copy counted as protection. The original logs show the
disk-wait headline, but contain no pressure readings and do not establish
the cause. The follow-up records the observed Linux pressure average and
pending content/child counts when a high-pressure reading or outstanding-work
state changes. Sampling frequency, the 10% threshold, unknown-reading fallback,
back-off and progress floors are unchanged. These diagnostics do not establish
per-device pressure, read-latency feedback or a measured failure-rate reduction.

After integrating the version-only v0.6.8 main commit, the combined priority
and serving-repair source passed all 771 parallel workspace checks (two
skipped). The earlier pre-serving run passed 770. The pressure diagnostics
were added after the 771-check run; native S2 then passed all 11 checks and
S3 all 14 using that source, with TLC accepting their filesystem and roster
streams. These finite scenarios do not close the measured M6 budgets.
The first combined all-target check found the external transport fixture
naming an indirect dependency; its constructor now infers the default native
environment without adding a dependency.

Final source, including the pressure diagnostics, passes all-target workspace
Clippy with the lab, trace and console features and the repository structural,
ADR-index and spec-reference checks. Exact-head CI still precedes merging.
