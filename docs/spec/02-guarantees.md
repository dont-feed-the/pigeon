# 2. Guarantees

> Mission: the promises P1–P6 (ADR 0034), their assumptions, what is not
> promised, and the invariants that carry them with what checks each.

## 2.1 Definitions

- A version V is **protected** when at least K distinct custodians, each
  counted once by its device, hold on disk a copy of its key verified when
  placed and by every scrub since (ADR 0002), and the commit that introduced
  V (ADR 0046). They then hold all a fresh machine needs to recover V
  ([09 §9.9](09-network-and-joining.md)).
- A **repair epoch** for V runs while fewer than K custodians of the roster
  hold intact copies of V. A copy that rots opens it when it rots, not when a
  scrub finds it; a version that never reached K is in one (ADR 0046).
- A version is **captured** when its content is in the local annex and in a
  signed commit, accepted or proposal ([07](07-capture-and-install.md)).
- A **saved file** is the content a person or an application left at a path.

## 2.2 Assumptions

- **F (failures):** within a repair epoch of V, fewer than K of the custodians
  that hold V lose or corrupt it. A fault that takes several custodians (one
  disk, machine or house) counts once per custodian it takes (ADR 0046).
- **Members** are honest but fallible; **keys** are not stolen.
- **Networks** partition, reorder, duplicate and drop messages; **devices**
  crash at any instant, including during a write, and restart.
- **Clocks** may be wrong by minutes. Only displayed times, thresholds and
  the end of a retention period depend on them, never correctness.
- **Disks** are POSIX file systems of the supported list
  ([14](14-platforms-and-packaging.md)): `rename` is atomic, `fsync` durable.

## 2.3 The six guarantees

**P1 Survival.** If V is protected and F holds, no sequence of syncs,
evictions, laptop edits, crashes or partitions destroys V; only a purge after
its retention period, or a Drop, can. Survival is not reachability: in a
partition V may be out of reach. Survival needs no laptop: after the loss of
every laptop and of pigeon's local state, a fresh machine recovers V, its name
and its history from the custodians alone (ADR 0047, [09 §9.9](09-network-and-joining.md)).

**P2 Capture, and no overwrite.** A captured version is kept by its laptop as
an immutable copy until protected ([07 §7.8](07-capture-and-install.md)).
pigeon never replaces, moves aside or removes content a person saved unless
that person asks (ADR 0034). Install is crash-recoverable, skips diverging
paths and moves a file from its name only to prove it is pigeon's own; a
racing save wins the name, and a write that escapes the proof is kept as a
recovered version ([07 §7.5](07-capture-and-install.md)).

**P3 Reversible deletion.** Unlinking, even in bulk, never releases
protection; until purge, every deleted protected version can be restored. A
returning device can neither resurrect a deleted file nor bypass a purge:
what a purge covers stays released, and a version it never covered (one saved
offline meanwhile) is kept and shown to its deciders ([08 §8.6](08-custody-loops.md)).

**P4 Authority.** The accepted catalogue is the set of commits valid under the
roster in force at their parents. Validation is deterministic, so all devices
accept the same catalogue from the same commits. A release that does not know
the group's `rules` or a document's `format` judges the commits concerned
*unknown* and waits; it never accepts what a newer release rejects (ADR 0048).
Commits mixing authorities are split at capture or rejected at acceptance;
merges are checked against their parents.

**P5 Convergence.** Identical accepted history gives an identical logical
catalogue. Physical trees differ only by the folders kept and by unresolved
divergences (ADR 0034). Names follow one portable rule set: Unicode NFC,
case-fold collisions, Windows reserved names, deterministic conflict names.

**P6 Honest status.** Each state is shown as *observed at time T* or as
*uncertain*; a partitioned device says "not checked since T" and never claims
knowledge it cannot have. A state that is not *Safe* comes with its reason and
what happens next (ADR 0027).

## 2.4 Not guaranteed

- Versions not yet *protected*: always shown as such (P6).
- History beyond a purge's retention period; writes between two captures
  (best effort only); an application's unsaved data.
- Protection beyond custodial capacity: versions that do not fit in
  K × (collection + retained history + suggestions not erased) are not
  protected, and the group is told how much is missing and who can act
  (ADR 0003, [08 §8.9](08-custody-loops.md)).
- Independence: two custodians on one machine count as two (ADR 0046).
  Groups are advised to keep custodians in separate places.
- Malicious members, stolen keys, a stolen unlocked laptop before an admin
  removes it (ADR 0007).
- Confidentiality from the group's members. In transit it is QUIC's
  (TLS 1.3); relays see metadata (ADR 0016).

## 2.5 Invariants that carry the guarantees

A cited TLA+ invariant (`Module.tla` `Name`) or `pigeon-core` law
([13 §13.13](13-verification.md)) is normative. E*n* are the experiments of
`lab/git-annex/`.

| Id | Invariant | Carries | Enforced in | Checked by |
|---|---|---|---|---|
| INV-K | a custodian's copy is dropped only by its own `pigeond`, and only if K other verified custody copies exist, or by a purge after retention | P1 | [08 §8.4–8.6](08-custody-loops.md), ADR 0014 | `AnnexDrop.tla` `INV_K`, `LockedStays`; `Repair.tla` `INV_K`; E1, E5, E9, E13, E23 |
| INV-Named | a custodian counts for a version only if it has the commit that introduced it, and only for copies synced to its disk | P1, P6 | [06 §6.2](06-status.md), [09 §9.4](09-network-and-joining.md), ADR 0046 | `state` (`protected_when_k_distinct_custodians_count`); `filter` (`custodian_answers_only_after_sync`); E21 |
| INV-NoCount | a laptop copy never counts toward K | P1 | [03 §3.3](03-repository-and-roles.md) (`untrust`) | E8, E9 |
| INV-Settings | drops are suspended while git-annex's shared settings disagree with the roster | P1 | [03 §3.5](03-repository-and-roles.md) | lab; not yet coded |
| INV-History | custodians want every version not purged, not only the current tree | P1, P3 | [03 §3.4](03-repository-and-roles.md) | E9, E10c |
| INV-Capture | every saved file reaches a signed commit before any install touches its path | P2 | [07 §7.3](07-capture-and-install.md) | `CaptureInstall.tla` `CaptureFirst`, `SavesCaptured` |
| INV-NoOverwrite | install never renames over a name, and replaces a file only after proving it is what pigeon last installed and that nobody writes it | P2 | [07 §7.5](07-capture-and-install.md) | `CaptureInstall.tla` `NoOverwrite`, `DeletionKept`; E19 |
| INV-Journal | after a crash at any point, recovery leaves every path at its old content, its new content or the person's, and the index agrees | P2 | [07 §7.5](07-capture-and-install.md), [11](11-local-state.md) | `journal` (all three laws); `CaptureInstall.tla` `NoLoss`, `Journal`, `IndexHonest`; E19 |
| INV-Evict | `free` removes only working copies of protected versions, never a saved file, and leaves no pointer file | P1, P2 | [08 §8.5](08-custody-loops.md), ADR 0031 | `Evict.tla` `DropsProtected`, `NoLoss`, `NoPointer`, `NoDeletion` |
| INV-Unlink | an unlink changes no custodian's preferred content | P3 | [03 §3.4](03-repository-and-roles.md), ADR 0018 | E2, E4 |
| INV-Purge | a purge drops a key only if every reference a custodian knew when it sealed is covered by an active purge decision, every custodian not `dead` has sealed, and no current version uses it | P3 | [08 §8.6](08-custody-loops.md) | `Purge.tla` `KeepWins`, `WaitingPeriod`, `NoResurrection`; E14 |
| INV-Valid | a commit is accepted iff a parent is accepted and it is valid under the rosters of its accepted parents (a proposal parent only through rule 7) | P4 | [04 §4.4](04-roster-and-authority.md) | `Authority.tla` `AuthorisedContent`, `DecisionsByDeciders`, `AdoptionHoldsContent`; `roster` (fold laws) |
| INV-Revoke | once a history holds a device's removal, nothing that device signed unseen by the removal, nor any key it added, counts | P4 | [04 §4.4](04-roster-and-authority.md), [09 §9.7](09-network-and-joining.md), ADR 0051 | `Authority.tla` `RemovedSilenced`, `RemovalRevokesAdded`; `Roster.tla` `RemovalWins`; `roster` (`every_fold_is_valid_and_removals_win`) |
| INV-Merge | a merge's tree is the catalogue of its history: a function of the contributions and of which saw which, never of merge commits | P5 | [05 §5.2](05-catalogue-and-names.md) | `models/catalogue/catalogue.py` |
| INV-Names | portable names are injective within a folder and stable for files that do not collide | P5 | [05 §5.4](05-catalogue-and-names.md) | not yet coded |
| INV-Observed | *protected* is shown only with the time of its observations; observations older than the staleness threshold give *uncertain* | P6 | [06 §6.2](06-status.md) | `state` (`more_presence_never_lowers_the_state`, `protected_is_observed_within_offline_after`); `headline` |

## 2.6 Escapes and their records

Force and Drop are the only ways around these guarantees ([01 §1.7](01-concepts.md)),
signed and recorded ([04 §4.6](04-roster-and-authority.md), [08 §8.7](08-custody-loops.md));
both ask first, or need `--yes` without a terminal (ADR 0024). With Force every invariant but the folder-rule part of
INV-Valid holds; with Drop every invariant but INV-K holds for other versions.
