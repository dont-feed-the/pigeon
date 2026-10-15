# 8. Custody loops

> Mission: the loops that move history and content and keep K custody
> copies, carrying P1 and P3; git-annex does the verified drops (ADR 0001).
> The `Repair`, `AnnexDrop`, `Evict` and `Purge` invariants and the `queue`
> laws cited below are normative.

## 8.1 The loops of `pigeond`

| Loop | Runs on | Triggered by |
|---|---|---|
| accept: fetch, validate, merge | every device | a peer's change notice (remotedaemon), a reconnection, every 5 minutes **[no ADR]** |
| install | laptops | `main` moved; a folder kept again ([07 §7.5](07-capture-and-install.md)) |
| place | every device | new keys in `main`; a custodian came online |
| repair | custodians | a key below K: a custodian removed, a scrub failure |
| free and keep | laptops | the person's choice per folder |
| scrub | custodians | the nightly window (ADR 0037) |
| purge | custodians, then laptops | a purge decision sealed by every custodian (§8.6) |
| settings check | every device | every merge ([03 §3.5](03-repository-and-roles.md)) |

All work goes through one queue ordered by `queue`
([15 §15.2](15-resource-use.md)): capture, history, protect (uploads,
downloads toward protection, repair), download, scrub, rebalance. ADR 0037
lists repair after downloads; issue #13 moved it into protect **[no ADR]**
(16 §16.3).

## 8.2 Exchanging history (accept)

For each reachable peer *d*:

| # | Step | Rule |
|---|---|---|
| 1 | fetch | the refspecs of [03 §3.6](03-repository-and-roles.md): *d*'s `main` into `refs/pigeon/peers/<d>/main`, its git-annex branch, the proposals and status reports it relays; a P2P fetch can hang (16 §16.4), so it has a deadline, after which its process group is killed; a remotedaemon stuck in `SYNCING` is restarted (E12) |
| 2 | annex merge | the git-annex branch is a union |
| 3 | settings check | [03 §3.5](03-repository-and-roles.md) |
| 4 | validate | the new commits in topological order, by the authority rule ([04 §4.4](04-roster-and-authority.md)), verdicts memoised in memory only; each rejected commit without a rejected ancestor is kept as `refs/pigeon/proposals/<signer>/<commit>`; an *unknown* commit (ADR 0048) stops that branch, and the headline says *Needs you*: update pigeon (`update-needed`) |
| 5 | merge | the maximal accepted commits into `main`, by fast-forward or a signed merge ([05 §5.2](05-catalogue-and-names.md)), only if they bring a contribution this device lacks ([05 §5.1](05-catalogue-and-names.md)) |
| 6 | reports | accept newer, validly signed status reports ([06 §6.3](06-status.md)) |
| 7 | install | on laptops ([07 §7.5](07-capture-and-install.md)) |

History travels transitively: `main` carries every commit it merged, and
proposals and reports relay through any peer, so devices that never meet
converge. Fetching is idempotent and order-independent; the accepted set is
a join (ADR 0012; `Authority.tla` `Converge`).

## 8.3 Placement: from published to protected

The driver enumerates pointer blobs from every tree reachable from the
accepted head and retained proposal heads (`pigeond::retention`), not just
their current trees. The same set feeds placement and repair. Purge is not
implemented yet, so every such historical reference remains retained.
The whole-group status includes retained keys outside the current tree;
they cannot be hidden behind a safe current version. Current files and
retained history use one snapshot of the Git refs, so a capture between
reads cannot turn a new current file into an unlisted historical version.
Counting their metadata conservatively waits for every retained head on
each counted custodian **[no ADR]**.

- A **laptop** uploads each key of `keys()`
  ([05 §5.1](05-catalogue-and-names.md)) that a custodian wants (its
  `sizebalanced` share, [03 §3.4](03-repository-and-roles.md)) and lacks, in
  queue order. It drops what it does not want (old versions, freed folders)
  with a verified drop ([09 §9.4](09-network-and-joining.md)); its own copy
  never counts (ADR 0002).
- A **custodian** gets from any peer each key of `keys()` it lacks, history
  included (E9), that its `wanted` matches or overflow gives it, and syncs
  each batch to disk (ADR 0046). Transfers go key by key: `satisfy --all`
  would drop from peers.
- **Every verified drop names K custodians**, reachable first, and counts no
  other remote (ADR 0046); without K it waits.
- **A copy counts only if it can be locked**: a custodian's filter answers
  `CHECKPRESENT` with a lock ([09 §9.4](09-network-and-joining.md)), or two
  custodians dropping at once over P2P count each other (E23);
  `AnnexDrop.tla` `INV_K`, `LockedStays`.
- **Overflow:** when fewer than K custodians hold a key, are getting it, or
  want it with room above their reserve, the others with room (above
  `annex.diskreserve`, within `maxsize`) rank themselves by SHA-256 of key
  and device ID, and the best want it too until K can take it; all compute
  the same choice. Laptops re-thin their captured current files
  ([07 §7.8](07-capture-and-install.md)), staying at about one copy.

## 8.4 Repair

A key falls below K when a custodian is removed (`dead`,
[03 §3.4](03-repository-and-roles.md)) or a scrub finds it bad. The others'
shares and overflow then include it, and their gets restore K within their
capacity (E9, E10), before any new key ([15 §15.2](15-resource-use.md)).
Until every key has K copies (a **repair epoch**, "repairing", ux.md §8.11)
such a key is **endangered**: a custodian with less free space above its
reserve than the endangered keys it wants defers other downloads and reports
`admits = "endangered"` ([06 §6.3](06-status.md)). A custodian unheard of for
7 days is never removed automatically: its owner and the admins see *Needs
you* ([06 §6.5](06-status.md)). A replacement joins as a new device; a
reused disk (new UUID) or a restored backup counts only after a hashing
fsck of everything ([09 §9.9](09-network-and-joining.md), ADR 0046).
`Repair.tla` checks repair, placement and rebalancing under faults:
`Survival`, `INV_K`, `ReserveKept`, `EndangeredFirst`, `Restores`.

## 8.5 Freeing and keeping a folder (ADR 0031)

Per top-level folder F of a laptop, `Evict.tla` checks that a file of F is
complete or absent, never a pointer file, even after a crash (`NoPointer`,
E15b); no save is lost (`NoLoss`); hidden files are never captured as
deletions (`NoDeletion`, `CaptureOnlySaves`); a key is dropped only while K
custodians hold it (`DropsProtected`); both transitions finish (`FreeLive`,
`KeepLive`). Each holds F's lock (capture skips F), records its target in
`wanted` first ([03 §3.4](03-repository-and-roles.md)) and is idempotent, so
a restart reruns it before capture.

| Free F (`pigeon free F`) | Model |
|---|---|
| 1. remove F from `wanted` | `ChooseFree` |
| 2. refuse while F has an unresolved divergence ([07 §7.6](07-capture-and-install.md)) | `ChooseFree` |
| 3. **hide** each *protected* file as an install of its absence ([07 §7.5](07-capture-and-install.md) steps 3–6, the aside kept and released as there), then set sparse-checkout patterns that exclude F and re-include its unprotected files, so git unlinks nothing (git's unlink loses writes through open handles: E24, `Evict_git`) | `Hide`, `HideFinish`, `Release` |
| 4. **drop** each hidden key, verified ([09 §9.4](09-network-and-joining.md)), only if every path whose index entry has it is hidden, absent and in a folder to free: git-annex makes any other file of the key a pointer (E24); a key's drop and its install, staging or capture run one at a time; a refused drop is retried once the key is protected again | `DropFree` |
| 5. unprotected files stay visible (ux.md §8.5) until protected, then go through steps 3–4; F stays listed; undo is `pigeon keep F` | |

| Keep F (`pigeon keep F`) | Model |
|---|---|
| 1. add F to `wanted`; get every current key of F; F stays hidden until all are present | `ChooseKeep`, `Get` |
| 2. **stage** them under `.git/pigeon/stage/F` with their index modes, linked or copied as in [07 §7.9](07-capture-and-install.md) | `Stage`, `Unstage` |
| 3. **move in** without replacing; if F exists on disk, install file by file ([07 §7.5](07-capture-and-install.md)), collisions becoming divergences | `MoveIn` |
| 4. **unhide without a checkout**: a moved-in file whose key `main` passed meanwhile, unchanged and without a writer, gets the current key (`Evict_stale`); then the patterns and skip-worktree bits are cleared and the index refreshed | `Restage`, `Unhide` |

The sparse-checkout file and `wanted` record kept folders; `pigeond`
restores both at start. Path commands skip hidden folders (E15), so pigeon
handles them by key. A stale `index.lock` or `info/sparse-checkout.lock`
left by a killed git of `pigeond`, the only writer, is removed at start
(`Evict_lockfile`).

## 8.6 Purge: erase forever (ADR 0018)

Not yet coded; `Purge.tla` checks the protocol and E14 asserts it with
git-annex. A purge releases **references**, not keys. A reference of key
*k* is a pair (*c*, *p*): an accepted or proposal commit *c* whose tree has
*k* at *p* while none of its parents does. A key is dropped only once all
its references are released.

`pigeon erase PATH` (any member) commits `.pigeon/decisions/purge-<id>.toml`,
deleting PATH if current: a decision from an admin, else a proposal to them
([04 §4.3](04-roster-and-authority.md)).

```toml
format = 1
kind = "purge"
paths = ["Photos/old-scans/"]       # every version of these paths …
base = "<commit id>"                # … in this main and its ancestors: what the decider saw
keys = []                           # optional: only these versions
proposals = []                      # optional: suggestions erased too
requested_by = "ines"
retention_end = 2026-11-12T00:00:00Z  # ≥ the commit's recorded time + retention_days
```

It **covers** (*c*, *p*) when *p* is under `paths`, *c* is `base` or its
ancestor, and *k* is in `keys` if given; suggestions only if `proposals`
names them. A `retention_end` before the commit's recorded time plus
`retention_days` makes the commit invalid (P4). The CLI and web first list
what is covered and what shares a key but stays.

| Phase | Rule | Model |
|---|---|---|
| waiting | while the decision is in `main`, anyone may restore a covered version (a new reference) and an admin may cancel by deleting the file (ux.md §7); a restore or cancel reaching **any** custodian before it seals keeps the content, in any message order, across partitions and restarts; nothing is dropped before `retention_days` after the decision, whatever the clocks | `KeepWins`, `WaitingPeriod` |
| seal | a custodian seals once, on its own clock, `retention_end` has passed and `retention_days` have elapsed since it received the decision (`purges.toml`, [11 §11.3](11-local-state.md)); its status reports carry the seal ([06 §6.3](06-status.md)): the decision, its `main` then, and `held`, the covered keys it keeps for a reference outside `main` | |
| release | a device holds *k* **released** when every reference of *k* it knows is covered by an active decision, every custodian not `dead` has sealed it, `main` contains each seal's `main`, no seal holds *k*, and *k* is not in the current tree; a pure function of `main`, the known proposals and the seals, recomputed at each merge and never stored (INV-Purge); pigeon writes no git-annex metadata for it | |
| drop | each device drops its released keys with a forced drop by key, the only one outside Drop, logged and traced; custodians first, laptops when they learn of it; remote `REMOVE` stays refused (ADR 0014); released keys are never transferred again | |
| new references | a version saved offline before the decision, a copy elsewhere or a restore after the seals keeps its key like any version; a device that still has dropped content uploads it again; if none has it, it is *missing* (`erased`, [06 §6.4](06-status.md)) and the deciders see *Needs you* and may erase it again | `NoResurrection` |

A purge completes only when every custodian not `dead` has sealed, and says
whom it waits for. It ends pigeon's custody only: copies outside pigeon or
on devices that never return remain, and names, authors and times stay in
the signed history; `pigeon erase` says so before asking.

## 8.7 Drop: remove without checking

`pigeon free --without-checking PATH` (CLI only, ux.md §2.2) is the Drop
escape: it removes this device's copy of PATH's content even if it is the
last. It shows the known risk ("last seen on Paul's NAS, 3 days ago"; "no
other copy is known"), asks (or needs `--yes`), drops with `--force`, and
commits `.pigeon/records/drop-<id>.toml` (device, paths, keys, risk shown,
time) **[no ADR: command and record]**.

## 8.8 Scrubs

Custodians run an incremental, time-limited `git annex fsck` in the nightly
window (ADR 0037), a full pass every 30 days by default **[no ADR]**.
git-annex moves bad content out and logs it absent, which starts repair
(§8.4). The last full pass and the failures go into the custodian's status
report ([06 §6.3](06-status.md)).

## 8.9 Capacity (ADR 0003)

- **Need**, in parts, each K × the size of its distinct keys: *current*,
  *history* not purged, *suggestions* (open, declined or withdrawn) until
  erased, overflow copies (§8.3). A custodian also needs its git repository,
  transfers in progress and its reserve; a laptop, what it keeps, its
  pending history and asides ([07 §7.8](07-capture-and-install.md)).
  **Capacity** is the sum over custodians of min(`maxsize`, one copy of all).
- **Reserve:** `annex.diskreserve` is max(1 GB, 1 %) of the file system on
  laptops, max(5 GB, 2 %) on custodians; git-annex refuses a get or incoming
  copy crossing it (E22), and so does capture (`ReserveKept`). `maxsize`, an
  advisory `sizebalanced` share, is never read as free space; at joining it
  is free space minus 10 %, at most free space minus the reserve
  ([09 §9.6](09-network-and-joining.md)).
- **Disk full** pauses every writer on that file system; serving and reports
  go on, with *Needs you* (`disk-full`). Work resumes once free space
  exceeds the reserve by 1 GB (checked every minute); a restart replays the
  install journal and retries the batch (E22). Scrubs wait for the reserve.
  `pigeond` never parses git-annex errors; the receiver's status report says
  why a transfer failed.
- **Admission:** new content waits in view, never silently: on a laptop with
  `history-full` ([07 §7.8](07-capture-and-install.md)); a key no custodian
  can take stays *published* with `no-capacity(missing_bytes, machines,
  who)`, and admins and the full custodians' owners see *Needs you*: GB
  missing, who can free space where, what declined suggestions take. An
  endangered key's room never goes to another (`EndangeredFirst`).
  A custodian plans the keys it wants and lacks in queue order
  ([15 §15.2](15-resource-use.md)) with its *room*, the free space above
  its reserve less the downloads in progress, which git-annex counts too:
  an endangered key is admitted if it fits in what is left, any other key
  only if the endangered keys not yet admitted still fit after it. A
  deferred key says the bytes it misses: the custodian's `doctor` and
  `status` say `no-capacity` with the files and MB missing. Repair records
  the keys a removed custodian held as it marks it dead; they are
  endangered until K custodians hold them. Normative: `pigeon-core::capacity`
  and its laws (`tests/capacity.rs`); `Admission.tla` checks the plan with
  sizes (`ReserveKept`, `EndangeredFirst`, `DeferredStays`), and
  `AdmitTrace` checks the code's plans against it.
- Admins and custodian owners are warned when capacity < 1.25 × need
  (`sizebalanced` is heuristic, E10). `pigeon status --capacity` and the web
  *Space* page show the parts of the need, each custodian's offered, used
  and free space, and declined suggestions by size and age, which `pigeon
  erase --suggestion ID` (§8.6) erases. Nothing expires by itself.

## 8.10 Rebalancing

At the lowest priority, a custodian drops, verified (§8.3), each key it
holds that its `wanted` no longer matches and overflow does not give it;
each of the K other copies is locked by its holder after hashing it for
this drop ([09 §9.4](09-network-and-joining.md); ADRs 0014, 0046). pigeon
never runs `--rebalance` against a remote. A placement planner may replace
`sizebalanced` later (ADR 0003).
