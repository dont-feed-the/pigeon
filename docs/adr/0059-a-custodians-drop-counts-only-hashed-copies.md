# 0059. A custodian's drop counts only copies hashed for it, and repair counts only members that can take a key

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Add models and trace vocabulary for capture/install,
  authorization, purge, and repair"), on the evidence of the `Repair` model.
  Refines ADRs 0049 and 0054, and the definitions of 02 §2.1–2.2.

## Context

The `Repair` TLA+ model (spec 13 §13.2) runs placement, repair and
rebalancing on custodians against lost disks, removed custodians, silently
corrupted copies, restored disks, crashes and partitions. Assumption F
(02 §2.2) bounds the faults: within a repair epoch of a version, fewer than K
failure domains lose or corrupt it. The model finds two flaws.

- **A rotten copy proves a drop.** A verified drop locks K copies
  (ADR 0054). In git-annex 10.20261006, `lockContentShared`
  (`Annex/Content.hs`) checks that the object is present and locks it; it
  never checks that the object is intact. Only a scrub
  (`git annex fsck`, `Command/Fsck.hs`) hashes it, every 30 days by default
  (08 §8.8). Suppose custodian 1's copy of *a* rots while custodian 2 holds
  another and the joiner 3 holds a third. Custodian 2 then no longer wants
  *a* and drops it, locking 1 and 3. Now 3's disk dies. That is the only
  fault since the epoch began, so F holds, and *a* has no intact copy left
  (`Repair_lockonly`, invariant `Survival`, 6 steps).
- **Domain repair waits for a member that cannot take the key.** 08 §8.3
  starts domain repair when the custodians that hold or *want* a key span
  fewer than K domains. sizebalanced's want ignores real free space: `maxsize`
  is advisory (ADR 0050), and a custodian at its reserve still wants its
  share. Suppose that custodian is in a domain the key lacks. The span then
  looks complete, no other custodian is chosen, and git-annex refuses the
  download (`checkDiskSpace'` in `Annex/Content/LowLevel.hs`). The key stays
  endangered forever, even though another domain has room
  (`Repair_wantall`, property `Restores`).

The model also shows that the definition of a repair epoch must count
corruption when it happens. 02 §2.1 starts the epoch when the *verified*
copies span fewer than K domains. A copy that rots stays verified until a
scrub finds it. So a rot before the epoch, followed by one loss within it,
destroys the version while F, as written, still holds. That is true even with
no action of pigeon's.

## Decision

- **The holder hashes a copy before it proves a custodian's drop.** On a
  custodian, when the peer is another custodian (its device ID in the
  roster), the filter ([09 §9.4](../spec/09-network-and-joining.md)) answers
  `LOCKCONTENT k` in three steps:
  1. it takes git-annex's lock;
  2. while holding the lock, it hashes the object (`git annex contentlocation`,
     then SHA-256 against the `SHA256E` key);
  3. it answers `SUCCESS` only if the hash matches.

  The probe that answers `CHECKPRESENT k` (ADR 0054) does the same. On a
  mismatch the filter releases the lock, answers `FAILURE`, and queues
  `git annex fsck --key k`, which moves the object out and starts repair
  (08 §8.4).
- **Laptops' locks are not hashed.** A laptop's copy never counts (ADR 0002),
  so its drop leaves the custody copies as they were. The hash would cost a
  read of every key on K custodians, once for each laptop that frees it.
- **A restored disk still counts only after its fsck** (09 §9.9). Without
  that check, a laptop's unhashed drop can lean on a bad restored copy: in
  `Repair_nofsck`, the rot of the only other copy then destroys the version
  within F.
- **Domain repair counts only members that can take the key.** The span
  counts custodians that hold the key, are downloading it, or want it and
  have room above their reserve for it, as their status reports say. The best-ranked
  member with room in each missing domain is then chosen
  ([08 §8.3](../spec/08-custody-loops.md)).
- **An epoch is counted on intact copies** (02 §2.1). A repair epoch for V
  starts when V's intact custody copies span fewer than K domains, from the
  moment a copy rots, not when a scrub finds it. A version not yet protected
  is in an epoch too. F is unchanged in its words; with this epoch it counts
  every corruption.

## Consequences

- With these rules the model passes:
  - every invariant (Survival, ReserveKept, EndangeredFirst, and INV-K as a
    property of steps) in `Repair_design`, `Repair_disk` and the full set;
  - `Restores` under fairness in `Repair_live`, `Repair_live4` and
    `Repair_live2`.
- Each safeguard has a negative configuration that fails:
  - `Repair_lockonly`: no hash;
  - `Repair_nosync`: copies count before their sync (ADR 0046);
  - `Repair_nofsck`: a restored disk counts before its fsck;
  - `Repair_noprio`: no endangered-first admission (ADR 0050);
  - `Repair_inflight`: the reserve check ignores the downloads in progress,
    which git-annex counts (`sizeOfDownloadsInProgress`);
  - `Repair_wantall`: the span counts members that cannot take the key.
- A rebalancing drop reads each of its K proofs in full on their holders.
  Rebalancing runs last in the queue (ADR 0037) and only after shares move,
  so the reads are rare. A large key's drop waits for the hash, as git-annex's
  client waits for the answer to `LOCKCONTENT`.
- A rotten copy can still make *protected* overstate until a scrub finds it,
  at most the scrub period (08 §8.8). Under the corrected epoch that time
  counts against F; 16 §16.4 records the limit.
- 02 §2.1 changes its definition of a repair epoch; 08 §8.3 and 09 §9.4 apply
  the rules. If this ADR is accepted, ADR 0054's status line gains "locks for
  a custodian's drop hashed by ADR 0059", and ADR 0049's gains "the domain
  repair span refined by ADR 0059".

## Alternatives considered

- *Scrub more often.* A shorter period narrows the window but never closes it,
  and every scrub reads every key.
- *Hash for laptops' drops too.* This closes no hole that P1 covers, and it
  reads the whole collection again on K custodians for each laptop.
- *Never drop on custodians.* Rebalancing would stop, and the custodians that
  hold the most would stay full after a new one joins (ADR 0003).
- *Let the dropper verify by downloading.* That moves K copies over the network
  for each drop, where the holder reads only its own disk.
