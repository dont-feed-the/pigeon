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
- Refounded archive and pending-erasure recovery semantics.
- Initial upgrade contention: different concurrent requests may split
  immutable choices and remain blocked. No retry may silently change a
  vote; a safe recovery protocol needs separate modelling and evidence.
- Release validation on both supported runners, followed by the tag and
  publication gates. This checkpoint supplies no longitudinal test claim.
