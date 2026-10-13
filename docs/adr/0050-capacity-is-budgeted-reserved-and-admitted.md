# 0050. Capacity is budgeted, real free space is reserved, and endangered content goes first

- **Status:** Proposed, 14 October 2026, by the agent, for issue #13
  (Guillaume's review: "Define capacity admission, reserved space, and repair
  priorities"). Refines ADR 0003, which stays: `sizebalanced`, advisory
  `maxsize`, warnings and headroom. Replaces the place of repair in ADR
  0037's order of work.

## Context

ADR 0003 accepts heuristic placement with capacity warnings. It does not say
what happens when capture, retained history, suggestions and repair compete
for the space left, nor what a full disk does. Facts from git-annex
10.20261006:

- `annex.diskreserve` defaults to 100 MB (`Types/GitConfig.hs`).
  `checkDiskSpace` (`Annex/Content/LowLevel.hs`) refuses content that would
  leave less than the reserve, and `getViaTmp` (`Annex/Content.hs`) checks it
  before every download through `checkDiskSpaceToGet`. E22 shows the
  receiving side refusing an incoming copy the same way, over a local path
  and over `git-annex-shell p2pstdio` (`runLocal` `StoreContent` in
  `P2P/Annex.hs` calls `getViaTmp`), before any byte is kept. The sender then
  reads "transfer already in progress, or unable to take transfer lock".
- With a reserve of 0, `checkDiskSpace` still refuses a key larger than the
  free space, so git-annex alone does not reach `ENOSPC` with keys of known
  size: a full disk comes from another writer (E22). On a full disk
  `git annex fsck` fails (SQLite `ErrorBusy` on Linux, an uncaught
  exception on macOS), while `git fsck` passes.
- `git annex add` does not check the reserve: capture must check it itself.
- `maxsize` only feeds the `sizebalanced` preferred content expression. It
  never refuses a transfer. E22 shows `sizebalanced` leaving keys at one copy
  when the shares are too small, and a verified drop refusing to go lower.
- git-annex writes the git-annex branch journal, its SQLite databases and the
  transfer logs into `.git/annex/` without a space check. A full disk fails
  there first.

## Decision

**The need, broken down.** `pigeon-core` computes, from `keys()` and the
observations, the group's need in four parts, each K × its size (bytes of
distinct keys):

- *current*: keys of the current tree;
- *history*: keys of earlier versions, not purged;
- *suggestions*: keys only proposals reference: open ones, and declined or
  withdrawn ones until they are erased;
- *domain repair*: copies that ADR 0049 adds beyond `sizebalanced`.

Each custodian also needs, outside its share: the git repository and the
git-annex branch, transfers in progress, and `annex.diskreserve`. A laptop
needs one copy of what it keeps, its pending history (ADR 0041) and its
install asides. `pigeon status --capacity` and the web's *Space* page show
these parts, per custodian its offered `maxsize`, used and free space, and
for the group what is missing.

**Real space is reserved; `maxsize` is a share.** pigeon sets
`annex.diskreserve` to max(1 GB, 1 % of the file system) on laptops and
max(5 GB, 2 %) on custodians. That room holds git objects, the git-annex
journal and databases, logs, and a recovery `fsck`. git-annex refuses a
download or an incoming copy that would cross it (E22), and capture refuses to
cross it too ([07 §7.8](../spec/07-capture-and-install.md)). `maxsize`
stays what a custodian offers to `sizebalanced`. It is advisory and is never
read as free space. The default `maxsize` at joining is the free space minus
10 %, and never more than the free space minus the reserve.

**A full disk pauses writes, and nothing is lost.** When a write fails with
`ENOSPC` or `EDQUOT`, `pigeond` pauses everything that writes on that file
system: capture, downloads, installs, history merges. It keeps serving
copies and publishing its status report, which is small. The status shows
*Needs you*, "This computer's disk is full: free 3 GB to go on"
(`disk-full`). Work resumes when the free space exceeds the reserve by 1 GB,
checked every minute. After a restart, the install journal is replayed
([07 §7.5](../spec/07-capture-and-install.md)), a stale `index.lock` is
removed ([08 §8.5](../spec/08-custody-loops.md)), and the failed batch is
retried. E22 shows that a get failing with `ENOSPC` leaves no location log
entry and no object (its partial stays in `annex/tmp`, and the retry resumes
it), that a `git fetch` fails and leaves `main` whole, that `git fsck`
passes, and that the retries succeed once space is freed. `git annex fsck`
fails while the disk is full, so scrubbing and `pigeon doctor` wait for the
reserve to come back. `pigeond` never reads git-annex's error messages to
tell why a transfer failed, since they mislead (the refused copy above): the
receiver's status report (`full`, `admits`, free space against `reserve`)
says why.

**Admission: new content waits in view, never silently.**

- A laptop captures while its pending history is below `pending_max` and its
  disk above the reserve. Beyond, capture of new content waits with reason
  `history-full` (ADR 0041). The volume not yet protected is so bounded on
  each laptop.
- A key that no custodian can take (those that want it have no room above
  the reserve, or none is in a domain it lacks) stays *published* with reason
  `no-capacity(missing_bytes, machines)`. The admins and the owners of the
  full custodians see *Needs you* with who can act: "Backup machines are
  full: 40 GB are missing. Paul can free space on Paul's NAS, or an admin can
  add a backup machine." If declined suggestions take space, the message says
  so: "12 GB are declined suggestions; erasing them frees 24 GB of backup
  space."
- A custodian keeps room for repair first. A key is **endangered** while it
  is in a repair epoch (02 §2.1): it had K failure domains and has fewer.
  While a custodian's free space above the reserve is smaller than the
  endangered keys it wants, it defers its own downloads of other keys and
  says `admits = "endangered"` in its status report. Laptops then send it
  only endangered keys and show `no-capacity` for the others. A laptop with
  a stale report may still send one; git-annex accepts it within the
  reserve, so this costs space, never safety. The filter does not refuse a
  `PUT` itself: git-annex's client takes any answer but `PUT-FROM` or
  `ALREADY-HAVE` as a protocol error (`put'` in `P2P/Protocol.hs`).

**Priorities: the fewest copies first.** The protect and repair classes of
15 §15.2 become one class, **protect**, before downloads of kept folders.
Its order is the fewest failure domains holding a custody copy first, where a
laptop copy counts for ordering only. Ties go to keys that had K and lost
some (repair), then newest, then smallest. Rebalancing stays last. A key
with fewer copies never waits behind a key with more. A custodian downloads
in this order and spends its free space in it.

**Declined and withdrawn suggestions.** Their content stays until a purge
erases it, as for any version (P3, ADR 0042). It counts in the need, and the
*Space* page lists it by suggestion, with size and age. An admin erases them
with `pigeon erase --suggestion ID`, a purge decision whose `proposals` names
them ([08 §8.6](../spec/08-custody-loops.md)). Nothing expires
automatically.

**Tests.** E22 runs a nearly full custodian (the reserve refuses get and
incoming copy), too little aggregate capacity (`maxsize` with
`sizebalanced`), a custodian killed or removed during a copy, and a real
`ENOSPC` on a small file system followed by a retry. Laws: the parts of the
need sum to the total; the protect order is a total preorder in which a key
with fewer domains comes first; every key that is not protected has a reason;
admission never refuses an endangered key for a non-endangered one.

## Consequences

- A group short of space knows how much, in which part (history, declined
  suggestions), and who can act.
- Repair can no longer starve behind new captures, and new captures cannot
  fill the space a repair needs.
- The reserve costs up to 2 % of each custodian's disk.
- `disk-full` pauses writes on that file system only; the person's saves are
  not touched, and capture resumes from the files on disk.
- Laptops hold back non-endangered keys for a custodian that keeps room for
  repair, and show `no-capacity` until it has room.

## Alternatives considered

- *Treat `maxsize` as a hard limit.* git-annex does not enforce it, and the
  real limit is the file system. Rejected: the reserve is the hard limit.
- *Expire declined suggestions after a delay.* That deletes content without
  a decision, against P3. Rejected: erasing them is an admin's purge.
- *Keep repair below downloads* (ADR 0037, and 15 §15.2 before this ADR). A custodian could
  then spend its last space on a new key while an old one has a single copy.
  Rejected.
- *A capacity planner* that assigns keys to custodians. Out of scope
  (ADR 0003, issue #13).
