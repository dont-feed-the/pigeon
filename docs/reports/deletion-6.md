# M5 implementation progress

The working version is 0.6.0. M5 is not complete or tagged. Erase remains
unavailable through the command line and web interface in this checkpoint.

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

The signed Git integration suite has 17 passing tests. The physical purge
test drops an actual historical annex object only after signed completion,
and preserves current objects, private staged entries and working bytes.
Protocol, reference, format, filtering and scrub laws use 1000 generated
cases. Full workspace Clippy and repository structural checks pass.

Historical seals and zero-day fixtures test lifecycle ordering; they do
not claim that a month elapsed. The membership model covers one pending
admission with one or two original custodians, including a partitioned
decider. Its unfenced variants demonstrate safety failures. It does not
yet prove initial upgrade agreement, arbitrary churn or daemon conformance.

## Remaining before release

- Erase preview, confirmation, cancellation, waiting names and erased
  history states in both interfaces.
- Real partition/restore/offline-return races and implementation traces.
- Refounded archive and pending-erasure recovery semantics.
- Scrub progress and errors in the interface and problem catalogue.
- Initial upgrade contention: different concurrent requests may split
  immutable choices and remain blocked. No retry may silently change a
  vote; a safe recovery protocol needs separate modelling and evidence.
- Release validation on both supported runners, followed by the tag and
  publication gates. This checkpoint supplies no longitudinal test claim.
