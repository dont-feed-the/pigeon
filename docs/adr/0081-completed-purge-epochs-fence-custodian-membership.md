# 0081. Completed purge epochs fence custodian membership

- **Status:** Proposed, 16 October 2026, for M5.

## Context

The purge model assumes a fixed custodian set. Requiring a newly joined
backup's fresh seal for an already completed decision makes its old keys
wanted again, violating NoResurrection. Freezing the roster at the decision
without fencing admission is also unsafe: a restore could reach a newly
admitted, still-unsealed custodian while every older custodian drops.
Removing a custodian changes the same quorum. A completion bit cached on
one device cannot establish the causal order or survive recovery safely.

## Decision

- A new backup starts pending. It does not count for safe copies, receive
  retained content or act as an unsealed custodian until its admission is
  complete. Every existing custodian must acknowledge the pending
  introduction in a signed report. Knowing that introduction prevents it
  sealing decisions made under the older roster until membership settles.
  Activation merges every acknowledgement's accepted main, so a decision
  sealed before the acknowledgement cannot be hidden by a partition.
- A completion epoch is replicated, immutable proof metadata under
  `.pigeon/purge-epochs/<decision>/<document-hash>.toml`. It names the exact decision identity, an accepted
  closing head, and the complete signed status report objects of every
  custodian in that head's roster. The closing head must contain each
  seal's main. Its roster must be nonempty. Identity, signature, declared
  deadline and immutable held sets are checked, not copied as assertions.
- Any admitted device may add a fully verified epoch witness. This grants
  no discretionary permission: an invalid, unavailable or edited witness
  cannot become accepted metadata. The witness is evidence, not a stored
  released-key list. Retain and fetch its signed report objects as well.
- Before changing the custodian set while an active purge remains, the
  accepted membership commit must observe a valid closing epoch for that
  decision. Otherwise an admin must cancel it first. A valid witness may
  be added atomically with the membership change. Missing proofs wait;
  they never imply an empty set of released keys.
- The acknowledgement fence also applies to concurrent, previously unseen
  old-roster decisions. A custodian that acknowledged pending admission
  cannot later seal one without settling its membership epoch. If such a
  decision cannot close, cancel and reissue it under the new roster. A
  completion certificate alone, without this admission handshake, does not
  protect a restore reaching a newly admitted backup during a partition.
- Once the epoch is closed, its original custodian quorum determines that
  decision's seal completion. Later devices inherit the completed epoch;
  they do not reopen its waiting period. Current and uncovered reference
  births, cancellation and immutable held sets still prevent key release.
  A restored or copied version has a new birth even for the same key.
- Enable these authority rules before accepting the first purge (rules 3).
  That first upgrade needs signed acknowledgements from every current
  non-recovery device, with a nonempty custodian quorum. Each device binds
  its acknowledgement to one upgrade request; the final accepted head
  contains their acknowledged histories. This also fences a legacy backup
  addition concurrent with enabling erasure. Older readers must report
  update-needed rather than silently accepting erasure metadata under their
  earlier authority rules. No erasure is submitted on the person's behalf
  while this readiness step is incomplete.
- Concurrent initial requests can split those immutable choices. Ordinary
  retry or local repository recovery must preserve them, even if preparation
  then waits indefinitely. Explicit refounding creates a different authority
  domain with fresh preparation after rejoining; it never edits old votes or
  treats their signatures as the new participants' consent (proposed ADR
  0079). The old group's pending activity can still finish there.
- Pending membership is a derived admission phase, not an arbitrary change
  to an existing device's identity, role or annex UUID. A configured backup
  without its required activation witness is shown as checking, never
  counted as a safe copy, and serves metadata while content waits. Its own
  signed acknowledgement establishes that it implements this protocol;
  old custodians' acknowledgements establish the membership fence.
- A private fsynced ledger outside the repository stores the upgrade choice
  and the last critical accepted head. Before acknowledging, sealing or
  erasing, the local repository must contain that head. Advance the marker
  before discarding a completed pending fence. Restoring an older repository
  therefore waits for metadata recovery instead of forgetting an earlier
  acknowledgement. Retained signed reports recover a missing receipt file;
  an unknown or contradictory file is preserved and stops new votes.
- Refounded history must verify the old bound archive and completion
  witnesses. A new genesis's current content has fresh births. Incomplete
  old erasures require explicit cancellation/restart semantics in recovery;
  an unsupported archive or membership epoch pauses erasure and transfers.

## Consequences

Completed erasure can survive later backup additions and removals without
fetching its old references again. Changing backups during a pending purge
requires cancellation or waiting for its completion; the interface must
name the decision and offer that remedy. This is a deliberate membership
fence, not evidence that dynamic membership was proved by the old model.

The extension needs its own membership model, implementation traces and
real join/restore/restart/refound scenarios before this decision is accepted
or M5 is released. The fixed-roster parameterized proof remains useful only
under its documented assumptions.
