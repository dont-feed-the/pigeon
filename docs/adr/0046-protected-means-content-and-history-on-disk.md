# 0046. Protected means K custodians hold the content and the commit that names it, on disk

- **Status:** Proposed, 14 October 2026, by the agent, for issue #11
  (Guillaume's review: "Specify and test recovery onto fresh machines from
  surviving custodians"), on the evidence of experiment E21.

## Context

Issue #11 asks what must survive for a group to be used again after every
laptop and all of pigeon's local state are lost, while at least one custodian
keeps its data, and how the durability of those items relates to *protected*.
Four gaps stood in the way:

- **A protected version could have no name.** Spec 06 §6.2 counted a custodian
  toward K as soon as it held the key. A laptop uploads with `copy --to`
  ([08 §8.3](../spec/08-custody-loops.md)), while custodians only fetch
  history (pull only, ADR 0043). So content can reach K custodians before any
  of them has the commit that names it. If the laptop is lost then, the
  content survives as an unused key: nobody can tell which file, folder,
  version or author it was. The status had said *Safe*. E21 shows it.
- **What was counted was not on disk.** git's default `core.fsync` hardens
  packs but neither loose objects nor references (`FSYNC_COMPONENTS_DEFAULT`
  in git 2.55.0's `write-or-die.h`). A fetch of fewer than
  `transfer.unpackLimit` (100) objects writes loose objects, so after a power
  cut `main` can name commits that are not there. git-annex 10.20261006
  never calls `fsync`: no call in its sources, where only the assistant sets
  `core.fsyncobjectfiles`, for removable drives
  (`Assistant/WebApp/Configurators/Local.hs`). E21 measures both: a
  `git fetch` of three commits makes no fsync with git's defaults (trace2
  counters), and `git annex get` makes no fsync, fdatasync or syncfs on an
  object (strace); its only syncs are SQLite's, on `.git/annex/keysdb`. A custodian answers the
  `LOCKCONTENT` of a verified drop as soon as the object file exists. A
  laptop can therefore drop its copy while both custodians' copies are still
  in their page caches, and one power cut in a house that holds both loses
  the version (issue #12).
- **The recovery set was implicit.** Nothing said which items a recovery needs,
  which are derived, and where each lives.
- **Duplicate repositories.** A custodian repository copied to a new disk, or
  restored from a backup, keeps its git-annex UUID. Two live repositories with
  one UUID count once, which errs on the safe side. A repository restored from
  an older backup is worse: the location log says that its UUID holds keys
  that the restored disk no longer has, and git-annex counts them.

## Decision

**The recovery set.** Once the versions it concerns are protected, this is
what a group needs and where it lives:

| Item | Needed to recover | Lives on | Replicated by |
|---|---|---|---|
| content: the objects of every version not purged, history and suggestions included | yes | K custodians per key | placement and repair (08 §8.3–8.4) |
| `main`: genesis, rosters, every accepted commit with its names, versions, authors, deletions, decisions, drop records and purge decisions | yes | every device, every custodian | fetch (08 §8.2) |
| proposals: `refs/pigeon/proposals/*` and their commits | yes | every device, every custodian | fetch |
| the git-annex branch | no: a cache. Settings are a function of the roster (03 §3.5), `dead` marks follow from removed devices, and location logs are rebuilt by a hashing `git annex fsck --all` | every device | fetch, `git annex merge` |
| status reports and seals | no: a custodian seals again (ADR 0042) | every device | fetch |
| a laptop's local state (11 §11.3) | no: every captured save is in a commit | that laptop | none |
| a custodian's `purges.toml` | no: losing it restarts each waiting period (ADR 0042) | that custodian | none |
| device keys | for authority only (ADR 0047) | each device | never |

**Protected needs the commit.** H (06 §6.2) counts a custodian *x* only if,
besides `holds(x, k)`, *x* has the commit *c* that introduced the version:
`has_commit(x, c)`. It is observed when *c* is the `main` of *x*'s latest status
report or one of its ancestors, when *c* is or precedes a proposal tip listed
in that report, or when *x* advertised such a ref to this device's own fetch.
Status reports gain `proposals`: the tips of the proposal chains the device
keeps that are not in its `main`. Until a custodian has fetched it, a version
whose content is already there stays *published*, with reason `sending`.

**Custodians harden what they count.**

- Every device sets `core.fsync = objects,reference` and
  `core.fsyncMethod = fsync`. git then flushes loose objects, packs and refs;
  on macOS it uses `F_FULLFSYNC` (`git_fsync` in `wrapper.c`).
- On custodians, pigeond's filter (ADR 0043) holds back the `SUCCESS` that
  answers `PUT`, `CHECKPRESENT` or `LOCKCONTENT` for a key until pigeond has
  synced that object to disk. The object file and its directories are found
  with `git annex contentlocation`, then flushed with `fsync`, or
  `F_FULLFSYNC` on macOS. After each batch of its own `get --key`, pigeond
  syncs what arrived.
- A verified drop rests on `LOCKCONTENT`, so no device drops on the strength of
  a copy that is not on disk. The location log can run ahead of the disk by
  one batch; that can make *Safe* seconds early, but it never leads to a drop.
- A laptop syncs the objects of a capture batch before the commit that names
  them, so a captured version is never a name without content (P2).

**One repository, one UUID, counted only once checked.**

- Roster validity (04 §4.2) gains a rule: an `annex_uuid` belongs to one device,
  and is never given to another device, even after its own was removed.
  The validator knows the UUIDs of the ancestors' rosters.
- Joining always gives the repository a new UUID. When the folder already holds
  a repository (a copied disk, a restored backup, the disk of a custodian whose
  key was lost), pigeon gives it a new UUID first: `git config --unset
  annex.uuid`, then `git annex init` (`reinit` would take an existing UUID).
  The copies count only once a hashing `git annex fsck --all` has logged them
  under that UUID; a corrupted object is not counted (E21). Until then the device is a custodian with no copies.
- **A repository that went back in time.** Before serving, a custodian compares
  its state with its own latest status report, which peers relay to it. If
  that report names a `main` that is not in its repository, or is newer than
  anything it published itself, the repository was restored or rolled back.
  The custodian then reports `checking = true`, and nobody counts it in H.
  Its filter answers `FAILURE` to `LOCKCONTENT`. A hashing `git annex fsck
  --all` under its own UUID then corrects the location log.
- A removed custodian that comes back is refused at admission, because its key
  is not in the roster. Its UUID is `dead`, so its copies do not count. A
  device key copied to a second machine is outside the assumptions (02 §2.2).

## Consequences

- *Safe* waits for one fetch by each counted custodian, which takes seconds
  after the change notice when the custodian is reachable. A custodian that
  does not fetch shows as such: `sending`, then `custodian-offline`.
- Losing every laptop loses only what was not protected (02 §2.4). With the
  devices' keys, the custodians' repositories hold everything else, without
  pigeon's local state or the git-annex branch. ADR 0047 covers losing the keys
  as well.
- fsync costs a few milliseconds per object and per fetch on custodians, and
  one flush per capture batch on laptops, within ADR 0037's budgets.
- The filter is no longer purely pass-through: a positive presence answer
  waits for a sync effect. It stays a pure state machine that emits effects,
  and its laws gain one: no positive answer for a key leaves the filter before
  that key's sync effect has completed.
- The status laws (06 §6.2) still hold: `has_commit` only grows, because `main`
  only moves forward and proposal refs are never removed.
- E21 checks the counter-example, the recovery from custodians alone, the new
  UUID with its fsck, and git's fsync counters: none with the defaults, 13 for
  the same fetch with `core.fsync = objects,reference`. Power loss itself is outside
  the lab, so the hardening rests on the sources cited above.
