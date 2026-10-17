# M5 implementation progress

Erase and the periodic integrity scrub shipped in v0.6.1. This report
records the implementation evidence, the failed v0.6.0 candidate and its
corrected publication. The
release workflow requires the tag’s complete Linux CI, lab and slice and
both platforms’ archives before actual publication.

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

## Publication

[v0.6.1](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.1)
was published on 17 October at 09:07:42 UTC from
`be2e4ffa3be8e651becf711842f460a0ff15aac2`. Its complete
[Linux CI](https://github.com/dont-feed-the/pigeon/actions/runs/39018960470),
[lab](https://github.com/dont-feed-the/pigeon/actions/runs/39018960488),
[slice and crash schedules](https://github.com/dont-feed-the/pigeon/actions/runs/39018960452),
and [release](https://github.com/dont-feed-the/pigeon/actions/runs/39018960498)
passed. The separate finite
[models-full](https://github.com/dont-feed-the/pigeon/actions/runs/39018960486)
run also passed. No longitudinal test claim is made at this milestone.

All six published assets, four archive checksums and asset digests were
verified after publication. The native Linux installer reported 0.6.1,
installed the pinned `10.20261006-pigeon1` annex, and generated completions
for bash, zsh and fish. The public latest-release download redirected to
v0.6.1. Both application archives contain executable programs. macOS tests
remain periodic background evidence; its archive build passed.

## Known limits of this evidence

Initial preparation can wait indefinitely if competing requests split the
participants' immutable choices. No retry silently changes a vote. Explicit
refounding creates a fresh authority domain; finite models, signed-history
fixtures, and the normal three-peer CLI recovery check cover that boundary.
Arbitrary churn and full network-partition conformance remain broader
verification work. The new Purge traces cover fixed membership, not every
membership interleaving. Process-crash tests do not prove power-loss safety.

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


## Usability follow-up and ready-install integration

The branch includes rollup `0340319`, preserving main `0ac8465`'s per-file
suggestion decisions, truthful status, free-space confirmations and pause
banner alongside the file manager and system computer names. Erasure review
links remain on the new Suggestions page. History keeps the verified archive
and erasure-aware version listing while explaining a differing local copy.
The early-install regression runs with the M5 daemon's real receipt store.

All-target Clippy with lab and trace features passes. The combined checks
pass 100 interface/protocol/language tests (one existing ignored test), all
178 core tests, and 59 daemon integration tests covering signed history,
physical purge, restore, restart, refounding and incoming installation.
The catalogue oracle is included. These results do not complete the missing
restoration alert, post-seal partition scenarios or Purge implementation
trace conformance listed above.

The proposed completion-epoch ADR is now 0081 because the rollup uses 0078
for per-file suggestion decisions. Proposed ADRs 0079 and 0080 keep their
numbers. Accepted ADRs 0075, 0076 and 0077 retain their exact source contents.


## Unavailable current contents after earlier erasure

Current files with no known copy now explain an earlier erasure to the
people who may decide their paths and raise *Needs you*. The classifier
replays the existing reference/seal algebra against the new reference's
parents, with their actual membership and current keys. It retains unrelated
uncovered histories as conservative blockers. Different annex extensions
are matched by their physical content hash; a retained alias blocks the
inference that all those bytes qualified for release. Unknown parent
metadata suppresses this diagnosis and never alters the release policy.
The bound diagnostic cache is rebuilt; no persistent released-key bit exists.

The missing-restoration regression failed before this change: *Missing*,
*Not safe*, no explanation. It now passes before and after reopening the
group. Deciders see the actionable alert, other members keep the missing
state without that decision level, and counts continue to classify the
contents as missing. A pending request and an alias retained before sealing
do not get blamed on an erasure. Another extension gets the same correct
explanation. These fixtures use the stated zero-day signed lifecycle and
actual absent contents; they do not claim a month elapsed or a multi-host
physical-drop race.

All 233 selected interface, daemon, signed-history and physical-install
checks passed (one existing ignored test). Nine subsequent focused checks
also passed, including the extension/retained-alias cases and the new
1000-case DAG ancestry/descendant law. Wider archive/proposal cases and
Purge implementation-trace conformance remain in the release checklist.


## Missing-content wording review

A returning English agent participant reviewed only the new status text,
without source access. They understood that an erased historical version
was requested again, no copy was currently known, and permanent loss was
not established. They initially read “restored” as implying the PDF was
ready and found “bring back a computer” ambiguous between reconnecting and
rejoining. The text now explicitly says restoration could not finish and
suggests another saved version or a separate copy, followed by an explicit
retry. A text-only retake correctly identified the PDF as unavailable and
loss as uncertain. It still asked how to supply a separate copy and what
“request is kept” meant; the latter now says the restore request remains
recorded. English and French share this meaning.

This is a wording review by an existing participant, not an additional
full workflow session. It makes no promise that an offline machine will
retain bytes indefinitely, or that supplying a copy automatically retries
the restore. The existing 18 language checks passed; wider recovery and
trace evidence remains required before release.


## Physical erasure interleavings checked against Purge

Four native-driver fixtures now use one laptop and two real bare git-annex
repositories, whose UUIDs are recorded before genesis. Signed rules-3
preparation is fixture setup, using the existing verified report/witness
readers; this does not claim a runtime consent negotiation. The production
purge driver makes the seals and physical drops. Reports use the production
builder and codec and are signed with the fixture keys. Explicit file-remote
transfers control partitions and deliver signed histories separately from
reports; they are not a three-host P2P test.

The observed cases are a restore reaching an unsealed backup, cancellation
reaching an unsealed backup, a late restore after both backup copies drop,
and a late restore after all three annex copies drop. A report delivered
before its restoring history cannot authorize a drop. The normal placement
code does not fetch released history; a new signed reference permits the
surviving laptop's exact binary bytes to return to both backups. When no
annex copy remains, the new request survives reopening, placement does not
invent contents, and the decider sees *Needs you* with a missing file.

That last case initially exposed a status defect: the headline explained
missing restored contents, but the row said “capturing” because the incoming
main led the index. The pending scan excludes absent index-only lag while
preserving actual private files, including staged saves. The regression
checks the missing row and both private saves, all visible without modifying
private bytes.

`scripts/check-purge-traces` generates all four runs using nextest and
checks their 58 snapshots against `PurgeTrace`, which uses the original
`Purge` transition relation and all three safety invariants. Observations
come from admitted main ancestry, actual authenticated report objects, and
annex object presence with exact-byte checks. TLC derives time, causal and
safety ghost state. Six semantic corruptions are rejected: dropping before
adopting a sealer's history, omitting its restored reference, losing a
received seal, an unexplained physical disappearance, refetching without a
fresh reference, and forgetting cancellation. Missing fields, empty output
and an absent required case are also rejected. Linux CI gates this check.

Retention is explicitly zero days in these fixtures, testing order without
pretending that a month elapsed. Receiver elapsed-time laws and crash tests
remain separate evidence. These traces cover fixed membership; they do not
prove arbitrary churn, initial-preparation liveness, power-loss durability,
or every implementation interleaving.


The same missing-content diagnostic now crosses a verified refounding
archive. A new signed reference in the new authority domain can explain
completed erasure in its old signed archive. The regression failed with
only “too few backups” before the change and now raises the missing-content
alert before and after reopening. Historical parent reasoning incorporates
only archive roots verified for that authority domain's genesis; older
parents cannot borrow future links. Removing the signed archive anchor
suspends content and suppresses the cached erasure diagnosis. This changes
status explanation only, never the release policy.


The full workspace run exposed a moving-head preview race during initial
preparation: the preview took one retention snapshot but archive traversal
reread a later main, reporting an unverified archive when the two simply
differed. Traversal now starts at the snapshot's immutable main. The final
fresh-ref check still rejects a changed review as stale; no archive check
or confirmation token was weakened.


## Combined validation checkpoint

The full workspace baseline ran 642 tests: 640 passed, two failed, and two
existing tests were skipped. The failures were the moving-head preview
race described above and the slice's obsolete expectation that erase was
unimplemented. With those corrected, all 22 selected signed-history,
physical-erasure and restore checks pass, including acceptance of a missing
restoration suggestion and its decider-only alert before and after reopening.
All 12 normal CLI/status/archive checks pass, including both prior failures,
member/folder restoration and S2's free/keep/offline-return path. All four
Purge traces and their rejection checks pass, as do 25 CI-selection checks.
All-target Clippy, workflow lint and repository structural checks pass.
These local results are not a substitute for current-main CI or publication.


## French workflow retake on the conformance commit

The returning French agent participant used the normal, non-lab build of
`36cce03f6e235980f4c39f87da03edcc30e5fe1a`, without source access or
assistance. This is a retake by an existing participant, not a new person.
The three-peer fixture used the actual 30-day retention setting. Its
preparation and invitation setup happened before the participant began;
two harness setup errors are not counted as participant errors.

The participant found the deleted file and its history, reviewed the
correct scope (one current file, two versions and 79 distinct bytes),
prepared erasure separately from requesting it, identified the named
backup being waited for, cancelled, restored the current file, and then
explicitly restored version 1. There were no wrong targets, failed clicks
or requests for help. The final history showed version 3 alongside
versions 2 and 1, and no accepted erasure request remained.

Their explanations distinguished the earliest retention end from a
completion promise, and explained that every backup waits at least
30 days after receipt. They understood that another current file using
the same bytes prevents their release; 79 bytes is not guaranteed freed
space. Preparation does not delete or schedule anything. Newer saves and
restores remain retained without cancelling an older request. Cancellation
cannot recreate destroyed bytes or automatically restore the current
file. Permanent erasure in Pigeon is not a promise of secure disk wiping
or deletion of separate copies.

Independent checks after the session found no active decisions, three
saved versions, and exact binary equality of the current file to the
original version 1, including its NUL, 0xff and CRLF bytes. Its SHA-256 was
`43c3705c99546b9a86985a0d7b1aa5d394f244d23e8f0ccb5dc2ac36cb114337`.
The duplicate file's original bytes were unchanged. All fixture peers
were stopped and reaped after these checks.

Ease ratings on a 1–7 scale were 7 for finding the controls, 6 for
understanding/requesting erasure, 7 for cancelling, 5 for restoring, and
6 overall. Remaining concerns were the vague “Retour” after preparation,
the long path from a deleted file to an older version, long differing
receipt/request identifiers, and history not naming version 1 as the
source of version 3. A restore was not visible in the first snapshot;
the participant waited without clicking again and it then appeared.
This session did not measure response latency. A transient inventory
verification warning around confirmations cleared on later navigation,
but was alarming and unexplained. It remains an observation to investigate,
not evidence that any content was lost. No month-long wait or secure-wipe
test is claimed by this session.


## A private staged copy can reuse erased history

A physical-drop regression reproduced loss of a real annex object still
named by an unpublished index entry whose working file was absent. Other
private-save fixtures used different bytes and did not cover this case.
Physical erasure now reads local staged pointers before dropping anything,
keeps their exact keys and shared content hashes, and leaves the index
unchanged. Bare backup repositories have no private working index to pin.
The read inspects only bounded pointer blobs; an incomplete or unreadable
object response prevents physical deletion.

The regression now passes with both the same annex key and a different
extension sharing its pool hash. It verifies exact annex and provider-pool
bytes and the unchanged staged entry while no working file exists. Removing
the private entry lets the same qualified erasure drop those bytes. The
signed reference policy still marks the old history released: the private
entry neither publishes a reference nor enables fetching erased history.


## Current-main integration and verification wording

The branch integrates v0.5.16 and PR #66's resumable join, ordinary-failure
cleanup, verified-welcome serving and old-format leave. A release-tag CI
miss in the old-roster fixture held `index.lock` beyond the existing bounded
retry, even though the daemon had already been stopped. The fixture now
constructs its historical roster in an isolated tree instead of editing the
working index. The production retry remains bounded and never removes locks.

Status used the same warning for a deliberate history-verification barrier
and an unreadable inventory. The barrier now has a distinct status message
explaining that copying waits
for verification and suggesting `pigeon doctor` if it persists. The level
remains *Needs you*, inventory counts remain unavailable, and status cannot
lift the barrier or claim that the files are safe. Actual unreadable history
still gets its separate warning. This wording correction has not had another
full workflow retake.

On the integrated source, all 30 selected join/resume, retry, leave, health,
signed-history and physical-erasure checks pass, as do all 18 language
checks and the four native Purge traces with their rejection checks.
All-target Clippy and the mandatory repository checks pass. Full Linux CI
and release publication remain pending.

The first PR CI attempt reached the new native Purge step without
`cargo-nextest` installed in that job. The earlier trace and mutation checks
had passed; this was missing runner setup, not a Purge verdict. The job now
uses the repository's pinned nextest action before generating the traces.


## Exact-head PR validation for v0.6.0

PR #68's integrated head `d211890edcfc15348064d0338a6cfbe1fdd6409d` passed its complete selected
[Linux CI](https://github.com/dont-feed-the/pigeon/actions/runs/38992529939),
[lab](https://github.com/dont-feed-the/pigeon/actions/runs/38992529910), and
[real Nextcloud bridge](https://github.com/dont-feed-the/pigeon/actions/runs/38992529922)
checks before merging. The selection-verification checks also passed.
The version is set only in this release commit; the final tag workflows
remain publication gates. These are finite run results, not elapsed
longitudinal evidence.


## v0.6.0 publication stopped by diagnostic checks

The immutable v0.6.0 candidate passed its full Linux CI, lab checks, both
platform archive builds, S2 and all crash schedules. The remaining slice
job failed S12 and S13: the CLI did not expose `no-capacity` or
`commit-unknown-signer`. The actual backup logs recorded capacity deferral
and refusal of the unknown signing key. This is a diagnostic regression,
not evidence that the invalid commit was accepted. S13 also reproduced
with the unchanged candidate on the local real-daemon executor.

Doctor's newly added scrub observations were terminal `Said` responses.
The CLI correctly stops at the first non-`Progress` answer, so those rows
hid later findings. They now use continuing progress responses; actual
problems and the final all-clear retain their existing meanings. The failed
tag remains unchanged and unpublished. A corrected candidate needs its own
complete release gates.


The corrected diagnostic path then reached S13's later retention assertion.
The laptop with a deliberately unaccepted local head now fails closed on
membership, so it can report the previous file as Published while the real
backups still retain it. S13 now checks both backups' actual annex objects,
the laptop's exact original text, and that the save after the forgery never
becomes Protected for the full existing 20-second observation. The original
pre-forgery Protected check and both backups' signing-key refusals remain.
The admission and transfer barriers are unchanged; the fixture does not
require trusting an unaccepted local head to assess current membership.


The first complete S13 trace attempt exposed an observer error after the
content barrier closed: the filesystem observer was asking the production
transfer-retention API for permission to read its metadata. That API correctly
refused. The observer now counts only actual, locally present annex objects
with hashes matching their keys while the barrier is closed. References
alone do not prove stored bytes, and the observation cannot publish policy,
fetch contents or permit deletion. The repeated native S13 and its real
filesystem and roster traces now pass. The 20-second retention observation
still runs in full; the original unsuccessful observations remain recorded.
