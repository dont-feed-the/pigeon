# M6 resource-control progress

M6 is not complete or released. This branch contains controller components,
configuration validation and native content shaping. No resource budget
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
slices, with quiet recovery. Mid-file child pausing and own-read latency
measurement are still outstanding; this is not the full disk controller.

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

## File scheduling continuation (not yet released)

The native background capture adapter uses monotonic elapsed time, two equal
stat observations and the size-dependent 2–60 second quiet period. Seven
distinct captures in an hour space later captures of that path by ten minutes;
an hour without changes cools it. One earliest deadline wakes the normal round,
without a sleeping task per file. Explicit capture requests remain immediate.
Actual successful proposals and concurrent saves are recorded separately from
failed attempts; a remembered near-install classification survives settling.

Nine pure laws run 1000 cases each. The native same-size edit fixture initially
failed because Git status invoked a clean filter before settling. Background
candidate discovery now uses the existing index parser and stat observations,
plus a names-only untracked query, without that filter. The complete local
install target and those laws pass: 58 checks, including index formats 2/3/4,
conflict, crash recovery, seven actual hot-file captures, unrelated-file progress
and the same-size pre-ingestion regression. This does not establish whole-round
quiet behavior: status/install callers still have their older observations,
and the full daemon round, oldest-first order and resource budgets need work.

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
unused imports under macOS Clippy. The separate periodic mass-deletion
protection timeout is still open; neither change establishes a passing
macOS runtime suite.


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
