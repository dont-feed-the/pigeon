# M5 implementation progress

M5 is not complete or tagged. Its branch leaves the published version unchanged. Erase is implemented in the working CLI and
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
- Initial rules-3 enablement needs every participant's signed capability
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

The signed Git integration suite has 21 tests at this checkpoint. The physical purge
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

- The deciders' Needs you state for unavailable restored content, complete
  cancellation/readiness recovery, and broader interface usability tests.
- Post-seal partition/restore/drop races and implementation traces. The
  waiting-period live offline-return check below is complete.
- Wider refounded archive and initial-preparation recovery conformance.
- Initial upgrade contention: different concurrent requests may split
  immutable choices and remain blocked. No retry may silently change a
  vote. Explicit refounding creates a fresh authority domain; the finite
  model and signed-history fixture below check this separation; actual
  network partition recovery and broader implementation traces still need evidence.
- Linux release validation and both platforms’ archive builds, followed by
  publication; macOS tests supply periodic background evidence. This checkpoint supplies no longitudinal test claim.

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

Refounding now starts a new authority domain with the rules needed by its folder settings, preserving old votes
and decisions in its signed archive. It carries completed erasure forward
only after verifying the original full signed epoch proofs with the old
group's judge. Missing or substituted archive anchors pause content, even
after a successful cached check. Current files have new reference births.
This design is recorded in proposed [ADR 0079](../adr/0079-refounding-keeps-completed-erasure-in-its-signed-archive.md).

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

A signed-history fixture now drives three independent production receipt
stores through competing accepted snapshots. Two devices choose one request
and the third another. Reopening the stores and merging both histories keeps
those choices and refuses activation. Mere extra requests do not produce a
split warning; independently verified conflicting signed acknowledgements
do. The CLI and browser explain that waiting alone cannot resolve it and
describe explicit refounding, without offering a misleading retry button.

The fixture then creates a real signed refounding genesis and new roster
additions. Old reports cannot activate that group. Each original receipt
store can acknowledge a fresh request under the new group while its old
choice remains unchanged; all new signed acknowledgements permit activation.
This is a production-adapter test with serial metadata snapshots and explicit
signed roster additions. It is not a live network partition or a replacement
for the separate real CLI refound/rejoin scenario.

The v0.5.8 Linux S2 run recorded one immediate status assertion failure
after the visible file disappeared; its eviction and roster traces were
accepted by TLC. File absence can precede completion of the asynchronous
journal/index transition. S2 now waits, with the existing 120-second bound,
for its same final Protected condition. It still requires the unprotected
file to remain visible, then disappear without a pointer after protection.
The immutable v0.5.8 tag is unchanged. Its targeted rerun passed and
[v0.5.8 was published](https://github.com/dont-feed-the/pigeon/releases/tag/v0.5.8)
at 14:30:46 UTC with both platform archives. The first failure is retained
as evidence; the bounded completion check is included in the rebased v0.5.9 PR.


## Restore while erasure remains pending

A returning source-blind participant used the normal `70e68bc` build in
a fresh three-peer fixture. After all participants prepared erasure, Bob
went offline and saved a private binary edit. Alice requested erasure; the
request stayed pending under the actual month-long policy. The participant
restored the deleted file and then version 1 through the browser, both on
the first attempt, without cancelling or changing the request.

Independent byte comparisons verified the exact original binary contents
before and after Bob restarted. Bob retained his exact private edit and
the ordinary Suggestions command exposed his own change for the same path.
The participant independently saw that suggestion, safe restored versions
and the unchanged pending request. No suggestion was accepted or declined.
The browser and all owned fixture daemons were closed and reaped.

The [participant report](m5-usability-offline-return.md) rates ease 6/7 but
confidence about eventual retention under the pending request only 3/7.
The interface did not explain why restoring and leaving an erasure pending
are compatible. English and French restore receipts and pending-request
text now explain that earlier requests do not cover later saved or restored
versions, and that restoring does not cancel a request. A [focused retake](m5-usability-offline-return-retake.md) on a fresh normal
`2291757` build completed both restores on the first attempt and retained
the request. Scope comprehension was rated 7/7 and retention confidence
6/7, up from 3/7; independent exact-byte and offline-return checks passed
again. This is one returning participant’s subjective evaluation, not a
new first-use population result.

This is a live offline-return check during the waiting period, with real
normal-build peers and signed suggestions. It does not exercise post-seal
physical-drop races, a month of elapsed waiting or arbitrary partitions.
History now labels a confirmed released key Erased instead of trusting old
copy logs; signed-history and actual-annex regression checks also require
that a pending request alone does not get this label and a fresh uncovered
reference removes it.

## Restoring absent contents

An explicit file or folder restore now retains a new ordinary signed main
version or authorized suggestion before fetching absent contents. Suggestions
start at current accepted main so an older already-covered suggestion cannot
silently supply the new reference birth. No transport bypass, released bit,
new replicated format or exception to the reference algebra is introduced.
Cold reference scanning runs outside the worktree lock; after reacquiring it,
the request checks that refounding has not replaced its group. Installation
uses the current index baseline and keeps the existing inode/content guards.

If no copy arrives, `restore-waiting` explains that the request is retained
and can be retried. Main/proposal refs themselves are the durable intent;
the existing visible file and index are left unchanged. A suggestion is
bound to actual installed bytes only after installation succeeds. A crash
before that binding can leave an extra ordinary suggestion, rather than
claiming private bytes match another version. Existing suggestions remain
retained; a later erasure must explicitly review their scope.

Real normal daemon/CLI regressions cover an admin's absent file, a folder
snapshot, and an ordinary member's signed suggestion. They check new refs,
main authority, exact visible and unrelated bytes, index preservation and
restart. Supplying an exact copy later permits an explicit admin retry.
Fixture-only forced removals create the unavailable-copy branch; this is
not yet an end-to-end post-seal physical-drop race. That evidence and the
deciders' missing-restoration alert remain required before M5 release.

## Current-main integration checkpoint

The M5 branch includes the automatic updater, per-person folder defaults,
file manager and incremental peer-history acceptance. The combined M5
signed-history, install and metadata-fetch checks pass all 47 tests; the
interface, protocol and language checks pass all 79 selected tests. The
file manager retains erasure review links for files, folders, deleted files
and suggestions, the request list, and the existing confirmation controls.
These local checks do not complete the remaining purge conformance work.

The file manager's accepted ADR uses 0077. The still-Proposed receiver-wait
decision is now 0080; accepted ADRs 0075, 0076 and 0077 remain byte-identical
to their source. On 16 October 2026, the user resolved the retention question:
declined suggestions stay retained until explicit erasure, with no automatic
expiry setting.
