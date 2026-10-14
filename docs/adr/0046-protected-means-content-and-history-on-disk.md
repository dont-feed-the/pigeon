# 0046. Protected means K custodians hold the content and the commit that names it, on disk

- **Status:** Accepted, 14 October 2026, by Guillaume (chat: the question on
  the revised record, "Accepter telle quelle"); proposed by the agent for
  issues #11 (Guillaume's review: "Specify and test recovery onto fresh
  machines from surviving custodians") and #12 ("Distinguish independent
  failure domains from distinct custody repositories"), on the evidence of
  experiments E21 and E22 and of the `Repair` model, and revised twice at
  Guillaume's request (chat): first absorbing the former ADR 0049, then
  dropping failure domains ("I think 46 is overcomplicating things, we
  won't be able to keep track of failure domains in practice so we
  shouldn't try to model them"). Refines how ADR 0002 counts its K custody
  copies.

## Context

*Protected* is the word behind "Safe", the only state people are told they can
stop worrying about (P1, P6), and every verified drop relies on it. ADR 0002
made a version protected when K custodians hold its key, which is what
git-annex counts (`numCopiesCount` in `Annex/NumCopies.hs` counts UUIDs). The
experiments found that count optimistic in three ways:

- **Content without a name.** A laptop uploads content with `copy --to`,
  while custodians only fetch history (pull only). Content can reach K
  custodians before any of them has the commit that names it; if the laptop
  is lost then, nobody can tell which file, version or author it was (E21).
- **Copies not on disk.** git's default `core.fsync` hardens neither loose
  objects nor references, and git-annex 10.20261006 never calls `fsync`. A
  custodian answers a drop's `LOCKCONTENT` as soon as the object file exists,
  so one power cut in a house holding both custodians can lose a version the
  laptop has already dropped (E21, traces and strace).
- **Two repositories, one backup.** Two UUIDs can sit on one disk or one
  machine, and a reinitialized repository has a new UUID while its old one
  still lists the same objects. E22 shows git-annex counting two repositories
  on one disk, even one hard-linked inode, as two copies.

Changing the meaning of *protected* later changes, after the fact, which
versions people were told were safe, and what every device was allowed to
drop. This is why it is recorded here.

## Decision

**A version V is protected when K distinct custodians count for it**, each
counted once, by its device. A custodian *x* counts for V only if all of these
hold:

1. **It holds the content, on disk.** Its copy is verified, and synced before
   it says so: every device sets `core.fsync = objects,reference`, and a
   custodian's filter holds back the `SUCCESS` that answers `PUT`,
   `CHECKPRESENT` or `LOCKCONTENT` until `pigeond` has synced that object.
2. **It holds the commit that names V** (`has_commit`), observed from its
   status report or a fetch. Until then a version whose content is there stays
   *published*, with reason `sending`.
3. **It is one checked repository.** A UUID belongs to one device for its
   whole life and is never reused; joining always makes a new one, whose
   copies count only once a hashing `git annex fsck --all` has logged them. A
   repository that went back in time (restored from a backup) or lost its
   folder counts for nothing until checked again.

**pigeon does not model which custodians fail together.** It reads no storage
or host fingerprints and the roster declares no domains or sites: two
custodians on one machine or one NAS count as two. The documentation advises
putting custodians on separate machines, in separate places; nothing enforces
it.

**Drops and repair follow the same count.** Every verified drop names K
custodians that hold the key and ignores every other remote; without K, the
drop waits. When fewer than K custodians can take a key (a custodian at its
reserve still wants its share but cannot take it), other custodians with room
want it too, chosen by a deterministic rank, so protection can recover.

**What a recovery needs.** Once its versions are protected, a group needs only
what its custodians hold: content, `main` and proposals. The git-annex branch,
status reports and every device's local state are rebuilt or not needed;
authority comes from ADR 0047.

The specification carries the mechanism: the count H and its reasons
([06 §6.2–6.4](../spec/06-status.md)), the configuration
([03 §3.4](../spec/03-repository-and-roles.md)), the filter's sync and
lock ([09 §9.4](../spec/09-network-and-joining.md)), drops and overflow
([08 §8.3–8.4](../spec/08-custody-loops.md)), the UUID rule
([04 §4.2](../spec/04-roster-and-authority.md)) and recovery
([09 §9.9](../spec/09-network-and-joining.md)).

## Consequences

- Fewer versions are *Safe* until a synced copy and the commit reach K
  custodians, and the reason says why (`sending`, `custodian-offline`,
  `repository-lost`). This is P6: the count was optimistic before.
- *Safe* waits for one fetch by each counted custodian: seconds, when it is
  reachable.
- Custodians that share a disk, a machine or a house count separately, so one
  fault there can take several copies of a version shown *Safe*. Assumption F
  counts such a fault once per custodian it takes
  ([02 §2.2](../spec/02-guarantees.md)), and the guarantee says so
  ([02 §2.4](../spec/02-guarantees.md)).
- fsync costs a few milliseconds per object and per fetch on custodians, and
  one flush per capture batch on laptops, within ADR 0037's budgets. The
  filter emits a sync effect before any positive presence answer.
- Overflow may store a key beyond `sizebalanced`'s share; capacity
  ([08 §8.9](../spec/08-custody-loops.md)) counts it.
- Laws (`pigeon-core` `state`, 13 §13.13): a missing commit never raises a
  state; a second entry for one custodian changes nothing; a version is
  protected exactly when K distinct custodians count.

## Alternatives considered

- **Count the UUIDs that hold the key**, as git-annex does and as ADR 0002
  first read. Simplest, and what git-annex users expect. Rejected: E21 shows a
  *Safe* version lost by one power cut, and a reinitialized repository would
  count twice.
- **Failure domains**, this record's previous revision, with the former ADR
  0049: custodians whose storage fingerprints intersect, whose hosts are
  equal, or that the roster declares in one `domain` count once; `min_sites`
  asks for copies in that many declared sites; each drop locks one copy per
  domain, and domain repair spreads keys across domains. Safer where
  custodians share hardware. Rejected at Guillaume's request: groups would
  not keep the declarations current, fingerprints miss virtual machines,
  network shares and disk images, and it took roster fields, a status
  section, join fields, two notices and a placement rule.
- **Protect content and history separately**, saying *Safe* once the content
  is there. Earlier *Safe*; rejected because a key without its commit is
  unusable after the laptop is lost.
- **Trust the location log without syncing.** Cheaper on custodians; rejected
  because a drop would rest on copies in a page cache.
- **A placement planner.** Out of scope (ADR 0003); overflow is the minimum
  that lets protection recover.
