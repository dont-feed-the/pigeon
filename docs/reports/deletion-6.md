# M5 implementation progress

The working version is 0.6.0. M5 is not complete or tagged. Erase is implemented in the working CLI and
web interface; destructive protocol conformance and the remaining recovery
semantics still need release evidence.

## Implemented and checked locally

- Reference births include all parents, copies, restores, suggestions and
  retained archives. Release requires every retaining reference to be
  covered, and excludes current and independently held keys.
- Receiver waits use durably checkpointed monotonic uptime as well as the
  declared UTC deadline. Restarting does not count downtime as waiting.
- Full signed immutable seals and completion witnesses survive replaced
  reports and receipt loss. Missing report objects wait provisionally;
  fetching them permits history admission to resume.
- Completion evidence survives adding and removing backups and restarting.
  Metadata fetching includes the full reports during join and recovery.
- Initial rules-2 enablement needs every participant's signed capability
  acknowledgement and a nonempty custody seed. A roster edit alone cannot
  assert readiness. New backups remain pending until the existing quorum
  and the joining backup acknowledge their actual introduction.
- An external fsynced receipt ledger remembers critical accepted heads and
  the first upgrade choice. Restoring an older repository pauses content.
  Full signed reports recover a missing receipt file; unknown files remain
  intact and stop new acknowledgements. Recovery runs before networking.
- Content gates cover both directions, including an older peer's active
  stream. Placement, safe-copy observations and health exclude pending
  custody independently of stale reports.
- The 30-day scrub runs independently of worktree requests, with Linux idle
  I/O and CPU priority. It checkpoints progress and reports actual failures;
  a real corrupted annex object was moved aside while healthy and private
  bytes remained exact. Unknown scrub ledgers are preserved.

The CLI and browser share typed read-only previews, explicit protocol
preparation, exact-history confirmation and cancellation. Both languages
explain waiting machines, permanent loss and shared bytes. GET makes no
write request, failed CSRF checks make no backend call, and the reviewed
history token is carried unchanged to the daemon. No readiness update
submits an erasure automatically.

Signed-history preview tests check refs/index preservation, fresh consent
after an arriving suggestion, explicit suggestion-only scope, shared bytes
and orphaned append-only cache entries. A real-file confirmation regression
first exposed unrelated private index removal by a full install; erasure
now installs only its commit's changed paths and the regression passes.
Actual SIGKILL immediately before and after the main CAS replays through a
fresh Group, preserving dirty and staged bytes; unknown/torn markers stop
capture without being replaced. These are process-crash tests, not power-loss
observations.

The signed Git integration suite has 20 tests at this checkpoint. The physical purge
test drops an actual historical annex object only after signed completion,
and preserves current objects, private staged entries and working bytes.
It also removes the provider pool name, keeps another group's hard link,
and repeats pool cleanup when the annex name is already absent. Pool reads,
hydration and compaction obey the content gate. The current replaceable
status report stops carrying completed seals only after full signed epoch
proofs have been retained.
Protocol, reference, format, filtering and scrub laws use 1000 generated
cases. Full workspace Clippy and repository structural checks pass.

Historical seals and zero-day fixtures test lifecycle ordering; they do
not claim that a month elapsed. The membership model covers one pending
admission with one or two original custodians, including a partitioned
decider. Its unfenced variants demonstrate safety failures. It does not
yet prove initial upgrade agreement, arbitrary churn or daemon conformance.

Home shows the signed integrity-pass completion time and findings for each
backup, preserving the report’s age. The local backup also exposes the
remaining durable inventory and current findings without starting work.
Doctor shows local progress and signed remote pass results. An unreadable
local inventory produces `scrub-ledger`, keeps the original record, and does
not invent a pass time. The health framing law exercises every new optional
field in 1000 cases; English/French catalogue and Home/live-value tests pass.

The real three-peer CLI lifecycle passes with actual signed capability
acknowledgements: two laptops and one backup prepare the protocol, show
readiness without submitting an erasure, refuse unconfirmed noninteractive
use, confirm the scoped deletion, name the waiting backup, cancel, and
restore the exact retained binary bytes. A concurrent final preparation
merge can invalidate a preview; this is reported as `erase-stale` and the
test explicitly performs a fresh review/confirmation. The test does not
skip the configured month or claim physical purge completion.

## Remaining before release

- Erased history/restore states, complete cancellation and readiness recovery
  scenarios, and broader interface usability tests.
- Real partition/restore/offline-return races and implementation traces.
- Wider refounded archive and initial-preparation recovery conformance.
- Initial upgrade contention: different concurrent requests may split
  immutable choices and remain blocked. No retry may silently change a
  vote. Explicit refounding creates a fresh authority domain; the finite
  model below checks this separation, while a real split-vote recovery
  scenario and broader implementation traces still need evidence.
- Release validation on both supported runners, followed by the tag and
  publication gates. This checkpoint supplies no longitudinal test claim.

## Browser usability checkpoint

Two independent source-blind agent participants, accepted by the user for
usability evaluation, used normal builds of `32486cb`. Each started at Home,
requested erasure of a file's two observed versions, explained shared bytes
and the receiver waiting periods, cancelled the request, and explicitly
restored version 1. Neither needed assistance. Their original reports are
[English](m5-usability-english.md) and [French](m5-usability-french.md); each
rated the overall workflow 5/7. These are agent evaluations, not measured
human task times.

An independent byte comparison after each session confirmed the exact
original contents, including binary bytes and line endings. Each retained
three historical versions and no accepted erasure request. Both browsers
and all six fixture daemons were stopped and reaped afterward.

The English participant encountered a stale review after preparation and
a transient `receiving` failure restoring an already local version. Both
participants found the terminal command in the browser receipt confusing,
and inferred rather than immediately understood that restoring older bytes
creates a new current version. The French participant could not verify
contents through “Open a copy”, which actually initiates a browser download.

Corrections keep the fresh-consent check and explain arriving preparation
acknowledgements, show readiness explicitly, link successful receipts to
Erasure requests, explain the new current version, and label downloads as
downloads in both languages. Local deduplication now skips pool publication
while content is fenced, without refusing an already local restore. Network
hydration and export still obey the fence. The real three-peer lifecycle
now requires the first restore attempt after cancellation to succeed;
deduplication tests require that no fenced pool name is published. These
corrections were evaluated in the focused browser retakes below.

The [English retake](m5-usability-english-retake.md) and
[French retake](m5-usability-french-retake.md), using a fresh normal build
of `242146a`, both completed every step on the first attempt without help.
Both rated the workflow 6/7. They found the direct receipt link and the
restoration explanation clear. Browser downloads completed; the French
participant compared versions 3 and 1 byte for byte. Independent checking
confirmed both restored files and all three downloaded copies matched the
exact original binary bytes. Each session retained three history entries
and no accepted erasure request. All browsers and fixture peers were then
stopped and reaped. Headless transport limitations are recorded separately.

## Refounding checkpoint

Refounding now starts a new rules-1 authority domain, preserving old votes
and decisions in its signed archive. It carries completed erasure forward
only after verifying the original full signed epoch proofs with the old
group's judge. Missing or substituted archive anchors pause content, even
after a successful cached check. Current files have new reference births.
This design is recorded in proposed [ADR 0077](../adr/0077-refounding-keeps-completed-erasure-in-its-signed-archive.md).

The physical-purge fixture closes a real signed epoch, refounds, restarts
and verifies that the old key stays released while a current alias, another
group's pool link, private bytes and staged entries survive. Removing its
verified canonical archive then stops content. This uses historical signed
fixtures; it does not claim a month of observation.

A real two-peer CLI scenario prepares erasure, submits a pending request,
refounds and checks that the request remains archived rather than being
reissued or cancelled in the new group. The first explicit restore recovers
the exact binary bytes. The original backup rejoins in place, safe copies
return, and a fresh preparation completes without importing old votes or
submitting any erasure. Broader partition and initial-upgrade contention
evidence remains required before release.

The wider refound scenario initially failed twice before refounding: Bob
accepted the second main but could not receive its bytes. Routine fetches
were invalidating the content reading despite writing only quarantined
peer/report refs. The first correction still missed the report namespace;
the same failure exposed it. Explicit empty Git refmaps now prevent any
configured opportunistic branch updates, and only complete, constrained
quarantine fetches avoid invalidation. Validated adoption still takes the
retained-ref mutation lease and invalidates the reading before changing it.
A real Git regression verifies this even with a remote configured to map
incoming heads onto local heads, including during the erasure lease.

With that correction, the original full refound scenario passed in 47.45
seconds, including a participant's declined suggestion and fresh peers
reading old bytes across two signed refoundings. The backup-history test,
all three public erasure lifecycle tests and all three mutation-gate tests
also passed. This supplies recovery evidence, not arbitrary membership
churn or a completed longitudinal release gate.

## Initial-preparation model

`PurgeUpgrade` models two competing readiness requests, durable per-device
choices, a restored local repository and an explicit new genesis with
rejoining participants. A split can stay blocked indefinitely; retry never
changes an old vote. Each domain's activation needs its own complete current
participant consent and a nonempty backup quorum. The two-participant
configuration passes; mutations that forget a choice, accept a partial
quorum or copy old-domain votes produce the expected safety counterexamples.
The three-participant configuration checks the same rules with another
possible conflicting voter. This is finite design evidence; it does not
establish arbitrary churn, automatic recovery or daemon conformance.
