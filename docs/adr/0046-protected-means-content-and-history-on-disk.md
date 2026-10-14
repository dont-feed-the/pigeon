# 0046. Protected means K custodians in K failure domains hold the content and the commit that names it, on disk

- **Status:** Proposed, 14 October 2026, by the agent, for issues #11
  (Guillaume's review: "Specify and test recovery onto fresh machines from
  surviving custodians") and #12 ("Distinguish independent failure domains
  from distinct custody repositories"), on the evidence of experiments E21 and
  E22 and of the `Repair` model. Revised 14 October 2026 at Guillaume's
  request (chat), absorbing the former ADR 0049 (failure domains). Refines how
  ADR 0002 counts its K custody copies.

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
  machine; a reinitialized repository has a new UUID while its old one still
  lists the same objects. E22 shows git-annex counting two repositories on
  one disk as two copies, and a verified drop that relies on them leaving one
  physical copy.

Changing the meaning of *protected* later changes, after the fact, which
versions people were told were safe, and what every device was allowed to
drop. This is why it is recorded here.

## Decision

**A version V is protected when the custodians that count for it span K
failure domains** (and at least `min_sites` declared sites, by default 1). A
custodian *x* counts for V only if all of these hold:

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

**Failure domains.** Two custodians are in one domain when their storage
fingerprints intersect (a disk or network share holds both), when their host
fingerprints are equal (one machine), or when the roster gives them the same
`domain` label (hardware pigeon cannot see, such as virtual machines on one
host). A domain is a connected component of that relation, computed by
`pigeon-core` from the roster and the status reports. Detection and
declarations only merge domains, never split them; an unknown fingerprint is
reported as unknown, never guessed. Sites are declared, never inferred.

**Drops and repair follow the same count.** Every verified drop locks one
copy in each of K distinct domains, by naming the K remotes and ignoring the
others; if no such choice exists, the drop waits. When the custodians that
can take a key span fewer than K domains, custodians of other domains with
room want it too, chosen by a deterministic rank, so protection can recover.

**What a recovery needs.** Once its versions are protected, a group needs only
what its custodians hold: content, `main` and proposals. The git-annex branch,
status reports and every device's local state are rebuilt or not needed;
authority comes from ADR 0047.

The specification carries the mechanism: the count H and its reasons
([06 §6.2–6.3](../spec/06-status.md)), the fingerprints
([06 §6.3](../spec/06-status.md)), the configuration
([03 §3.4](../spec/03-repository-and-roles.md)), the filter's sync and
lock ([09 §9.4](../spec/09-network-and-joining.md)), drops and domain repair
([08 §8.3–8.5](../spec/08-custody-loops.md)), the UUID rule
([04 §4.2](../spec/04-roster-and-authority.md)), recovery
([09 §9.9](../spec/09-network-and-joining.md)) and assumption F restated
over failure domains ([02 §2.2](../spec/02-guarantees.md)).

## Consequences

- Fewer versions are *Safe* where custodians share hardware, and the reason
  says why (`same-domain`, `same-site`, `independence-unknown`,
  `repository-lost`). This is P6: the count was optimistic before.
- *Safe* waits for one fetch by each counted custodian: seconds, when it is
  reachable.
- A group whose custodians all run on one host cannot reach K ≥ 2, and is told
  so. Virtual machines on one host look independent unless someone declares a
  `domain`; the notice and `pigeon doctor` say so.
- fsync costs a few milliseconds per object and per fetch on custodians, and
  one flush per capture batch on laptops, within ADR 0037's budgets. The
  filter emits a sync effect before any positive presence answer.
- Domain repair may store a key beyond `sizebalanced`'s share; capacity
  ([08 §8.9](../spec/08-custody-loops.md)) counts it.
- Laws (06 §6.2, 13 §13.4): adding a fingerprint, a declaration or a missing
  commit never raises a state; with one custodian per domain the state equals
  ADR 0002's; a drop's K locked remotes are in K domains; domain repair is
  deterministic.

## Alternatives considered

- **Count the UUIDs that hold the key**, as git-annex does and as ADR 0002
  first read. Simplest, and what git-annex users expect. Rejected: E21 and E22
  each show a *Safe* version lost by one disk or one power cut.
- **Protect content and history separately**, saying *Safe* once the content
  is there. Earlier *Safe*; rejected because a key without its commit is
  unusable after the laptop is lost.
- **Trust the location log without syncing.** Cheaper on custodians; rejected
  because a drop would rest on copies in a page cache.
- **Untrust all but one custodian per domain.** Trust is per UUID, so keys held
  only by the others would never count.
- **Raise `--numcopies` to the worst case** instead of choosing the remotes: a
  holder that appears during the drop breaks the bound.
- **A placement planner aware of domains.** Out of scope (ADR 0003); domain
  repair is the minimum that lets protection recover.
- **Guess sites from network addresses.** Often right, sometimes not; never
  counted.
