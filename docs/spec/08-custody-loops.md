# 8. Custody loops

> Mission: specify the loops that move history and content between devices and
> keep K custody copies: exchanging history (accept), placement and repair,
> freeing and keeping folders, scrubs, purge, Drop. They carry P1 and P3, with
> git-annex doing the verified drops (ADR 0001).

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

All work goes through one queue, ordered by the priorities of ADR 0037
([15](15-resource-use.md)): capture, history, uploads toward protection,
downloads, repair, scrubs, rebalancing.

## 8.2 Exchanging history (accept)

For each reachable peer *d*:

1. `git fetch <remote of d>` with the refspecs of [03 §3.6](03-repository-and-roles.md):
   its `main` into `refs/pigeon/peers/<d>/main`, its git-annex branch, the
   proposals it knows, the status reports it relays. Over P2P a fetch can
   wait forever (16 §16.4), so it has a deadline, after which its process
   group is killed and the next notice or period fetches again; a
   remotedaemon whose fetch began (`SYNCING`) and never ended is restarted,
   since it hears no more notices from that peer (E12).
2. `git annex merge` (the git-annex branch is a union; git-annex merges it).
3. Settings check ([03 §3.5](03-repository-and-roles.md)).
4. **Validate** the new commits of the peer's `main` in topological order with
   the authority rule ([04 §4.4](04-roster-and-authority.md)). Verdicts are
   memoised in memory only (rebuildable, ADR 0036). Each rejected commit that
   has no rejected ancestor is kept as
   `refs/pigeon/proposals/<signer>/<commit>`. An *unknown* commit (ADR 0048)
   stops validation of that branch: it stays in `refs/pigeon/peers/<d>/main`,
   and the headline says *Needs you*: update pigeon (`update-needed`).
5. **Merge** the maximal accepted commits into `main`: fast-forward if possible,
   else a signed merge commit ([05 §5.2](05-catalogue-and-names.md)), only if
   they bring a contribution this device lacks ([05 §5.1](05-catalogue-and-names.md)).
6. Accept the peer's status reports that are newer and validly signed
   ([06 §6.3](06-status.md)).
7. Laptops: install ([07 §7.5](07-capture-and-install.md)).

History travels transitively: a device's `main` carries every accepted commit
it merged, so devices that never meet still converge through those in between,
and proposals and reports relay through any peer. Fetching is idempotent and
order-independent; the accepted set is a join (ADR 0012).

## 8.3 Placement: from published to protected

- A **laptop** uploads, with `git annex copy --to <custodian> --key`, every key
  of `keys()` ([05 §5.1](05-catalogue-and-names.md)) that custodian wants (its `sizebalanced` share, [03 §3.4](03-repository-and-roles.md))
  and does not hold yet, in the queue order of the protect class
  ([15 §15.2](15-resource-use.md), §8.9).
- A **custodian** downloads what it wants from any peer that has it: for each
  key of `keys()` that it lacks, history included (E9), and that its `wanted`
  expression matches (`git annex matchexpression --key`) or overflow
  gives it (below), `git annex get --key`. pigeon chooses every transfer key by key (§8.6): git-annex
  refuses `get --auto` and `drop --auto` with `--all` or `--key`, and
  `satisfy --all` would drop content from peers. After each batch it syncs
  what arrived to disk (ADR 0046): git-annex never does.
- Both directions run, so protection proceeds whichever side reaches the other.
  git-annex verifies content on arrival; the location log records it.
- A laptop then drops what it does not want (old versions, folders not kept)
  with a verified drop for each such key it holds ([09 §9.4](09-network-and-joining.md)): git-annex locks
  and checks K custody copies on custodians first (E1, E5, which ran over
  local paths; over P2P see below), and the laptop's own copy never counts
  (ADR 0002).
- **Every verified drop names K custodians** (ADR 0046). `pigeond` chooses K
  custodians holding the key, reachable ones first, and runs
  `git -c remote.<other>.annex-ignore=true … annex drop --key <key>
  --numcopies K --mincopies K`, ignoring every other remote (git-annex
  verifies only remotes that are not ignored, `verifiableCopies` in
  `Annex/NumCopies.hs`). Without K such custodians, the drop waits.
- **A copy counts only if it can be locked** ([09 §9.4](09-network-and-joining.md)). A custodian that
  drops its own copy (rebalancing, §8.10) holds its removal lock, and git-annex
  then also counts a copy it could not lock but saw present, assuming that a
  removal lock keeps every other drop from using the copy. Over a local path
  that holds (E1); over P2P `CHECKPRESENT` ignores the removal lock, so two
  custodians dropping at once each count the other and both drop (E23), and
  `--mincopies` does not help. Each custodian's filter therefore answers
  `CHECKPRESENT` with a lock ([09 §9.4](09-network-and-joining.md)); the
  `AnnexDrop` model shows the drop safe with it and unsafe without
  ([13 §13.2](13-verification.md)).
- **Overflow.** When fewer than K custodians hold a key, are downloading it,
  or want it and have room for it above their reserve (a custodian at its
  reserve still wants its share but cannot take it), the other custodians
  with room for it (free space above `annex.diskreserve`, within `maxsize`)
  rank themselves by SHA-256 of the key and their device ID, and the best
  ranked want it too, until K can take it. Every custodian computes the same
  choice. git-annex refuses
  `--auto` with `--all` (E18). The laptop also re-thins the current files it
  captured ([07 §7.8](07-capture-and-install.md), [07 §7.8](07-capture-and-install.md)). This keeps laptops
  at about one copy of what they keep.

## 8.4 Repair

A key falls below K custody copies when a custodian is removed from the roster
(marked `dead`, [03 §3.4](03-repository-and-roles.md)), when a scrub finds it
bad, or when a custodian's disk dies and its operator removes it. The other
custodians' `sizebalanced` share and overflow then include it, and their
per-key downloads (§8.3) restore K copies within their capacity (E9, E10),
before any new key (§8.9, [15 §15.2](15-resource-use.md)). While a key is
in a repair epoch it is **endangered**: a custodian whose free space above its
reserve is smaller than the endangered keys it wants defers its own downloads
of other keys and reports `admits = "endangered"` ([06 §6.3](06-status.md)),
so laptops send it nothing else and new captures do not take the space a
repair needs. A **repair epoch** lasts until every
key has K copies again; its progress (keys and GB left) is in the status
("repairing", ux.md §8.11). A custodian that stops being heard of is not
removed automatically: after 7 days its owner and the admins see *Needs you*
([06 §6.5](06-status.md)), and they decide. A replacement joins as a new
device; a surviving disk is reused under a new UUID and counts only after a
hashing `git annex fsck --all`, and a repository restored from a backup is
not counted until the same check ends ([09 §9.9](09-network-and-joining.md),
ADR 0046). The `Repair` model ([13 §13.2](13-verification.md)) checks this
loop, placement and rebalancing against lost disks, removed custodians,
rotten and restored copies, crashes and partitions.

## 8.5 Freeing and keeping a folder (ADR 0031)

Per top-level folder F, on a laptop. A file of F is always complete or absent,
never a pointer file, including after a crash at any instant (,
experiment E15b). Freeing and keeping hold F's lock: capture does not scan F
during a transition. The target is recorded first, in this device's `wanted`
expression ([03 §3.4](03-repository-and-roles.md)), and both sequences are
idempotent, so a restart reruns the transition toward the target before
capture resumes.

**Free F** (*Keep on this computer* switched off, `pigeon free F`):
1. Remove F from this device's `wanted`.
2. Refuse while F has an unresolved divergence ([07 §7.6](07-capture-and-install.md)):
   "Resolve the conflict on *tarte.docx* first".
3. **Hide** the files of F whose versions are *protected*, each as an install
   of its absence ([07 §7.5](07-capture-and-install.md)): journal
   `begin`, move aside, writer scan, check against the index entry, `proved`.
   Then `git sparse-checkout` non-cone patterns that exclude F and re-include
   its files not protected yet set the bits, with the files already absent,
   so git unlinks nothing. The aside is kept for the keep period and released
   with a hash. A writer, or a file that is not the expected one (someone
   saved it), sends the aside back, and the file is captured like any save;
   freeing resumes after. If the file was made again meanwhile, the aside is
   kept as a recovered version. Capture skips a path while its file is aside.
   Hiding with git's own unlink would lose a write through a handle opened
   before it (E24, `Evict_git`).
4. **Drop** the keys of the hidden files with a verified `git annex drop --key`,
   which locks K custody copies when it runs ([09 §9.4](09-network-and-joining.md)), whatever the
   status said at the hide. A key is dropped only if every path whose index
   entry has it is hidden, absent on disk, and in a folder whose target is
   free. A kept or visible path, or a staged or moved-in file of a folder
   being kept, keeps its key: git-annex's drop turns every unmodified file of
   the key into a pointer file with its old mtime, and `git status` stays
   clean (`removeAnnex` in `Annex/Content.hs`; E24). The drop of a key and
   any install, staging or capture of that key run one at a time. A drop that
   refuses leaves the file hidden, with its content here, and is retried once
   the key is protected again.
5. Files not protected yet keep their space and stay visible; the row says "6
   files are not safe yet; their space is freed once they are" (ux.md §8.5).
   When one becomes protected, steps 3 and 4 run for it.
6. F stays listed in pigeon's Files page and in `pigeon status`, with the space
   freed. Undo: `pigeon keep F`.

**Keep F** (switched on, `pigeon keep F`):
1. Add F to `wanted`; `git annex get --key` every current key of F. Until all
   are present, F stays hidden and its row says how many files are still being
   received (all or nothing per folder).
2. **Stage** the complete files under `.git/pigeon/stage/F`, with the modes of
   their index entries (on thin laptops a hard link to the object if it has no
   other name, else a copy; at most one path per object, [07 §7.9](07-capture-and-install.md)).
3. **Move in** the staged folder with one `rename` that refuses to replace
   (`RENAME_NOREPLACE`, `RENAME_EXCL`). If F exists on disk (the person made a
   folder of that name meanwhile), install the files one by one instead
   ([07 §7.5](07-capture-and-install.md)); colliding files become divergences.
4. **Unhide without a checkout:** compare each moved-in file's staged key with
   its index entry first. A file that main has passed since it was staged
   (the accept loop changes hidden paths in the index), unchanged and without
   a writer, is pigeon's own: it is removed and the current key is staged
   (`Evict_stale`). Then update the sparse patterns, clear the
   skip-worktree bits of F's index entries (`git update-index
   --no-skip-worktree`, then `--refresh`), and run `git annex restage`.

Hidden files are never read as deletions: `git status` skips skip-worktree
entries, and the staging directory is inside `.git`. Kept folders are recorded
by git's own sparse-checkout file and the device's `wanted` expression;
`pigeond` restores both at start (ADR 0031). Path commands skip hidden folders
(E15), so pigeon handles hidden folders by key. A stale `index.lock` or
`info/sparse-checkout.lock` left by a killed git process of `pigeond` is
removed at start, since `pigeond` is the only writer of the repository: a
`kill -9` inside `git sparse-checkout set` leaves the latter, and every later
free fails until it is gone (E15b; without the removal a freed
folder is never freed, `Evict_lockfile`).

## 8.6 Purge: erase forever (ADR 0018)

A purge releases **references**, not keys. A reference of key *k* is
a pair (*c*, *p*): an accepted or proposal commit *c* whose tree has *k* at *p*
while none of its parents does (an add, edit, restore, rename or conflict
copy). A key is dropped only once every reference to it is released, so
identical files in two folders, an accepted version and a suggestion that
share a key never lose each other's content.

1. **Request.** `pigeon erase PATH` (any member) writes
   `.pigeon/decisions/purge-<id>.toml`, in a commit that also deletes PATH if
   it is current. From an admin it is a decision; from anyone else it is a
   proposal to the admins ([04 §4.3](04-roster-and-authority.md)).

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

   The decision **covers** (*c*, *p*) when *p* is under one of `paths`, *c* is
   `base` or an ancestor of it, and *k* is in `keys` if they are given; a
   suggestion's references are covered only if `proposals` names it. A
   `retention_end` earlier than the commit's recorded time plus
   `retention_days` makes the commit invalid (P4). Before asking, the CLI and
   web list what is covered, and what shares a key but stays ("*IMG_0042.jpg*
   in *Photos/2019* has the same content and is kept").
2. **Waiting period.** While the decision file is in `main`, anyone can restore
   a covered version, and an admin cancels the purge by deleting the file. A
   restore is a new reference, which the decision does not cover. The CLI and
   web say: "Until 12 November 2026 anyone can still restore them; erasing
   finishes after that, once every backup machine has confirmed" (ux.md §7).
   A restore or cancel that reaches **any** backup machine before that machine
   seals keeps the content, in every order of messages, across partitions
   and restarts. A machine never seals sooner than `retention_days` after it
   received the decision, whatever the clocks.
3. **Seal.** A custodian seals the decision when, on its own clock,
   `retention_end` has passed and `retention_days` have elapsed since it first
   received the decision (`purges.toml`, [11 §11.3](11-local-state.md)). Its
   status reports then carry the seal ([06 §6.3](06-status.md)): the decision,
   its `main` at that moment, and `held`, the covered keys it keeps for a
   reference outside `main` (a suggestion).
4. **Release.** A device holds *k* as **released** when all of these hold:
   - every reference of *k* that it knows is covered by an active decision;
   - every custodian of the roster not marked `dead` has sealed that
     decision, and the device's `main` contains each seal's `main`;
   - no seal holds *k*;
   - *k* is not in the current tree.

   This is a pure function of `main`, the known proposals and the seals. It is
   recomputed at each merge and never stored (INV-Purge), and pigeon writes no
   git-annex metadata for it.
5. **Drop.** Each device drops the released keys it holds, locally, with
   `git annex drop --force --key`. This is the only forced drop pigeon performs
   outside Drop, and it is recorded in the log and as a trace event.
   Custodians drop first. Laptops drop when they learn of it, including on
   their return from offline. Remote `REMOVE` stays refused (ADR 0014).
   Released keys are never transferred again (§8.3).
6. **New references.** A reference that no decision covers keeps its key like
   any version. Examples: a version saved offline before the decision
   arrived, a copy in another folder, a restore after the seals. If the
   content was dropped, a device that still has it uploads it again. This is
   a new retention generation, which only a new purge releases. If no device
   has it, the version is *missing* with reason `erased`
   ([06 §6.4](06-status.md)). The deciders see *Needs you* ("Erased content
   came back: Martine's laptop had *scan-12.pdf*") and can erase it again.

A purge completes only when every custodian not marked `dead` has sealed. An
unreachable custodian holds it back, and the purge says so ("Erasing waits for
Paul's NAS"). Erasing releases pigeon's custody of the content. It does not
reach copies made outside pigeon, or devices that never return. The names,
authors and times of the erased versions stay in the signed history.
`pigeon erase` says both before asking. The `Purge` model checks this
protocol ([13 §13.2](13-verification.md)); E14 asserts it with git-annex.

## 8.7 Drop: remove without checking

`pigeon free --without-checking PATH` (CLI only, ux.md §2.2) is the Drop escape:
it removes this device's copy of PATH's content even if it is the last one. It
first shows the known risk ("last seen on Paul's NAS, 3 days ago"; "no other
copy is known"), asks (or needs `--yes`), then runs `git annex drop --force` and
commits `.pigeon/records/drop-<id>.toml` (device, paths, keys, risk shown, time)
**[no ADR: command and record]**.

## 8.8 Scrubs

Custodians check their content with `git annex fsck --incremental
--time-limit=<slice>` in the nightly window (ADR 0037), completing a full pass
every 30 days by default **[no ADR]**. Bad content is moved out and logged
absent by git-annex, which starts repair (§8.4). The last completed pass and the
failures go into the custodian's status report ([06 §6.3](06-status.md)).

## 8.9 Capacity (ADR 0003)

- **Need,** in parts, each K × the size of its distinct keys: *current*
  versions; *history* not purged; *suggestions*, open, declined or withdrawn,
  until erased; and the copies overflow adds (§8.3). Each custodian
  also needs its git repository and git-annex branch, transfers in progress
  and its reserve; a laptop, what it keeps, its pending history and its
  install asides ([07 §7.8](07-capture-and-install.md)).
- **Capacity:** the sum over custodians of min(`maxsize`, one copy of
  everything), since a custodian is useful only up to one copy.
- **Reserve.** pigeon sets `annex.diskreserve` to max(1 GB, 1 %) of the file
  system on laptops and max(5 GB, 2 %) on custodians. git-annex refuses a
  download or an incoming copy that would cross it (`checkDiskSpace` in
  `Annex/Content/LowLevel.hs`, E22), and capture refuses to cross it.
  `maxsize` is a share for `sizebalanced`, advisory, never read as free space;
  at joining it is the free space minus 10 %, at most the free space minus the
  reserve ([09 §9.6](09-network-and-joining.md)).
- **Disk full.** A write that fails with `ENOSPC` or `EDQUOT` pauses every
  writer on that file system (capture, downloads, installs, merges); serving
  and status reports go on. *Needs you*: "This computer's disk is full: free
  3 GB to go on" (`disk-full`). Work resumes when the free space exceeds the
  reserve by 1 GB, checked every minute; a restart replays the install
  journal and retries the failed batch. E22 shows that a get failing with
  `ENOSPC` logs nothing and keeps no object (its partial in `annex/tmp` is
  resumed), that a `git fetch` fails and leaves `main` whole, and that both
  succeed when retried. `git annex fsck` fails on a full disk, so scrubbing
  waits for the reserve. `pigeond` never parses git-annex's error messages to
  tell why a transfer failed (a copy refused by the receiver's reserve reads
  "transfer already in progress"); the receiver's status report says why.
- **Admission.** New content waits in view, never silently. A laptop captures
  within `pending_max` and the reserve, then waits with `history-full`
  ([07 §7.8](07-capture-and-install.md)). A key that no custodian can take stays *published* with
  `no-capacity(missing_bytes, machines, who)`. The admins and the owners of
  the full custodians see *Needs you*: how many GB are missing, who can free
  space where, and how much declined suggestions take ("erasing them frees
  24 GB of backup space").
- pigeon warns admins and custodian owners when capacity < need × 1.25 (the
  25 % headroom, because `sizebalanced` is heuristic, E10).
- `pigeon status --capacity` and the web's *Space* page show the parts of the
  need, each custodian's offered, used and free space, and the declined
  suggestions by size and age. An admin erases these with `pigeon erase
  --suggestion ID` (§8.6); nothing expires by itself.

## 8.10 Rebalancing

At the lowest priority, a custodian drops each key it holds that its `wanted`
expression no longer matches after the shares moved (a custodian joined or
changed `maxsize`), and that overflow does not give it, with a verified
drop (§8.3): git-annex checks K other custody copies first, each
by a lock on a copy its holder has hashed for this drop
([09 §9.4](09-network-and-joining.md); ADRs 0014 and 0046, [09 §9.4](09-network-and-joining.md)). pigeon
never runs `--rebalance` against a remote. A placement planner may replace
`sizebalanced` later (ADR 0003).
