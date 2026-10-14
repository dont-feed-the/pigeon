# 2. Guarantees

> Mission: state what pigeon promises, under which assumptions, what it does not
> promise, and which invariants of the other files carry each promise. Taken from
> the proposal (§3), strengthened by ADR 0034.

## 2.1 Definitions

- A version V is **protected** when custodians in at least K distinct failure
  domains hold copies of its key that were verified when placed and, since, by
  every scrub that covered them (ADR 0002), each on its disk, and each of them
  also holds the commit that introduced V (ADR 0046). A **failure domain** is
  a set of custodians that share a disk, a machine or a declared `domain`
  (ADR 0049); with `min_sites` ≥ 2, the copies also span that many declared
  sites. Those custodians then hold everything needed
  to recover V on a fresh machine ([09 §9.9](09-network-and-joining.md)).
- A **repair epoch** for V starts when V's verified custody copies span fewer
  than K failure domains after having spanned K, and ends when they span K
  again.
- A version is **captured** when pigeon has added its content to the local annex
  and recorded it in a signed commit, accepted or proposal ([07](07-capture-and-install.md)).
- A **saved file** is the content a person or their application left at a path
  of the physical tree.

## 2.2 Assumptions

- **F (failures):** within any repair epoch of V, fewer than K of the failure
  domains that hold V lose or corrupt it. Custodians are independent only as
  far as pigeon can see or the group declares (ADR 0049): a fire, flood or
  theft that takes a whole place is inside F only if the group declares
  sites and sets `min_sites` ≥ 2.
- **Members** are honest but fallible: they make mistakes, run old versions,
  delete by accident, but they do not attack the group. **Keys** are not stolen.
- **Networks** partition, reorder, duplicate and drop messages; **devices** crash
  at any instant, including during a write, and restart.
- **Clocks** may be wrong by minutes. Correctness never depends on them; only
  displayed times, thresholds and the end of a retention period do.
- **Disks** behave as POSIX file systems on the supported list
  ([14](14-platforms-and-packaging.md)): `rename` is atomic, `fsync` is durable.

## 2.3 The six guarantees

**P1 Survival.** If V is protected and F holds, no sequence of syncs, evictions,
laptop edits, crashes or partitions destroys V. Only a purge whose retention
period has ended, or a Drop, can. *Survival is not reachability:* during a
partition V may survive but be out of reach. Nor does survival need any
laptop: after the loss of every laptop and of pigeon's local state, a fresh
machine recovers V, its name and its history from the custodians alone
(ADRs 0046, 0047, [09 §9.9](09-network-and-joining.md)).

**P2 Capture, and no overwrite.** A captured version is never overwritten: its
laptop keeps it as an immutable copy until it is protected (ADR 0041).
pigeon never replaces, moves aside or removes content that a person saved,
unless that person asks (ADR 0034). Installing incoming content is
crash-recoverable and skips every path whose file diverges. Install moves a file
away from its name only to prove it is pigeon's own, a save that races it wins
the name, and a write that escapes the proof is kept as a recovered version
(ADR 0040). Writes made between
two captures are protected on a best-effort basis only.

**P3 Reversible deletion.** Unlinking, including a mass deletion, never
releases protection. Until purge, every deleted protected version can be
restored. A device returning from offline cannot resurrect a deleted file or
bypass a purge decision: what a purge covers stays released, and a version
the purge never covered, such as one saved offline meanwhile, is kept and shown
to its deciders (ADR 0042).

**P4 Authority.** The accepted catalogue is the set of commits valid under the
roster in force at their parents. Validation is deterministic, so all devices
accept the same catalogue from the same commits. A release that does not know
the group's `rules` or a document's `format` judges the commits concerned
*unknown* and waits: it never accepts what a newer release rejects
(ADR 0048). Commits mixing authorities are
split at capture or rejected at acceptance, and merges are checked against their
parents.

**P5 Convergence.** Identical accepted history gives an identical logical
catalogue (paths, versions, states). Physical trees differ only by the folders
each device keeps and by unresolved divergences (ADR 0034). Names follow one
portable rule set: Unicode NFC, case-fold collisions, Windows reserved names,
deterministic conflict names.

**P6 Honest status.** Each version's state is shown as *observed at time T* or
as *uncertain*. A partitioned device says "not checked since T"; it never claims
knowledge it cannot have. A state that is not *Safe* comes with its reason and
what happens next (ADR 0027).

## 2.4 Not guaranteed

- Versions that are *local* or *published* but not yet *protected*: they are
  always shown as such (P6).
- History beyond a purge's retention period.
- Writes between two captures (P2), and an application's unsaved data.
- Protection without custodial capacity: a group with no custodian, or with less
  capacity than K × (collection + retained history + suggestions not erased),
  gets no protection for the versions that do not fit, and is told how much is
  missing and who can act (ADRs 0003, 0050).
- Independence pigeon cannot see: virtual machines on one host with virtual
  disks, or machines in one place, count as separate failure domains unless
  the roster declares a `domain` or `site` (ADR 0049). pigeon never infers
  places from the number of machines.
- Malicious members, stolen keys, a stolen unlocked laptop before an admin
  removes it (ADR 0007).
- Confidentiality from the group's own members: every member can read every
  file of the group. Confidentiality in transit is QUIC's (TLS 1.3); relays see
  metadata (ADR 0016).

## 2.5 Invariants that carry the guarantees

Each invariant names where it is enforced and how it is checked
([13](13-verification.md)). "Model" means a TLA+ model, "law" a property-based
test (ADR 0012), "lab" a git-annex experiment.

| Id | Invariant | Carries | Enforced in | Checked by |
|---|---|---|---|---|
| INV-K | a custodian's copy is dropped only by its own `pigeond`, and only if K other verified custody copies exist in K failure domains, or by a purge after retention | P1 | [08 §8.4–8.6](08-custody-loops.md), ADR 0014 | E1, E5, E9, E13; model (planned) |
| INV-Named | a custodian counts toward K for a version only if it also has the commit that introduced it, and only for copies synced to its disk | P1, P6 | [06 §6.2](06-status.md), [09 §9.4](09-network-and-joining.md), ADR 0046 | E21 (asserting); status and filter laws |
| INV-NoCount | a laptop copy never counts toward K | P1 | [03 §3.3](03-repository-and-roles.md) (`untrust`) | E8, E9 |
| INV-Domain | copies count once per failure domain: a version is protected only if its counted custodians span K domains (and `min_sites` sites), and every verified drop lets git-annex lock only one copy per domain | P1, P6 | [06 §6.2](06-status.md), [08 §8.3, §8.10](08-custody-loops.md), ADR 0049 | E22; status and drop-choice laws |
| INV-Settings | drops are suspended while git-annex's shared settings disagree with the roster | P1 | [03 §3.5](03-repository-and-roles.md) | law; lab |
| INV-History | custodians want every version not purged, not only the current tree | P1, P3 | [03 §3.4](03-repository-and-roles.md) | E9, E10c |
| INV-Capture | every saved file reaches a signed commit (accepted or proposal) before any install touches its path | P2 | [07 §7.3](07-capture-and-install.md) | `CaptureInstall` model; fault injection |
| INV-NoOverwrite | install never renames over a name, and replaces a file only after proving it is what pigeon last installed and that nobody writes it | P2 | [07 §7.5](07-capture-and-install.md) | E19 (asserting); `CaptureInstall` model |
| INV-Journal | after a crash at any point, replaying the install journal leaves every path at its old content, its new content or the person's | P2 | [07 §7.5](07-capture-and-install.md), [11](11-local-state.md) | law (replay idempotent); E19 `kill -9` at every step (asserting); `CaptureInstall` model (two crashes, one during replay) |
| INV-Unlink | an unlink changes no preferred content of custodians | P3 | [03 §3.4](03-repository-and-roles.md), ADR 0018 | E2, E4 |
| INV-Purge | a key is dropped by a purge only if every reference of it known to a custodian when it sealed is covered by an active purge decision, every custodian not `dead` has sealed it, and no current version uses it | P3 | [08 §8.6](08-custody-loops.md), ADR 0042 | `Purge` model (KeepWins, WaitingPeriod, NoResurrection); purge laws; E14 |
| INV-Valid | a commit is accepted iff it is valid under the rosters of its parents and all its ancestors are accepted | P4 | [04 §4.4](04-roster-and-authority.md) | laws (homomorphism, determinism) |
| INV-Merge | the tree of a merge is the catalogue of its history: a function of the contributions and of which saw which, never of merge commits (ADR 0052) | P5 | [05 §5.2](05-catalogue-and-names.md) | laws (commutative, associative, idempotent); the reference model `models/catalogue` |
| INV-Names | portable names are injective within a folder and stable for non-colliding files | P5 | [05 §5.4](05-catalogue-and-names.md) | laws |
| INV-Observed | a state "protected" is shown only with the time of the observations it rests on; observations older than the staleness threshold give "uncertain" | P6 | [06 §6.2](06-status.md) | laws (monotone, join) |

## 2.6 Escapes and their records

Force and Drop are the only ways around these guarantees ([01 §1.7](01-concepts.md)).
Both are signed and recorded in the catalogue
([04 §4.6](04-roster-and-authority.md), [08 §8.7](08-custody-loops.md)), both ask
before acting and need `--yes` without a terminal (ADR 0024). With Force, every
invariant but INV-Valid's folder-rule part still holds; with Drop, every
invariant but INV-K still holds for other versions.
