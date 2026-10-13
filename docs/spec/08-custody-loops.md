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
   proposals it knows, the status reports it relays.
2. `git annex merge` (the git-annex branch is a union; git-annex merges it).
3. Settings check ([03 §3.5](03-repository-and-roles.md)).
4. **Validate** the new commits of the peer's `main` in topological order with
   the authority rule ([04 §4.4](04-roster-and-authority.md)). Verdicts are
   memoised in memory only (rebuildable, ADR 0036). Each rejected commit that
   has no rejected ancestor is kept as
   `refs/pigeon/proposals/<signer>/<commit>`.
5. **Merge** the maximal accepted commits into `main`: fast-forward if possible,
   else a signed merge commit ([05 §5.2](05-catalogue-and-names.md)), only if
   they bring an accepted non-merge commit this device lacks.
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
  and does not hold yet, in queue order: fewest custody copies first, then
  newest, then smallest (ADR 0037).
- A **custodian** downloads what it wants from any peer that has it: for each
  key of `keys()` that it lacks, history included (E9), and that its `wanted`
  expression matches (`git annex matchexpression --key`), `git annex get
  --key`. pigeon chooses every transfer key by key (ADR 0042): git-annex
  refuses `get --auto` and `drop --auto` with `--all` or `--key`, and
  `satisfy --all` would drop content from peers.
- Both directions run, so protection proceeds whichever side reaches the other.
  git-annex verifies content on arrival; the location log records it.
- A laptop then drops what it does not want (old versions, folders not kept)
  with `git annex drop --key` for each such key it holds. This is a verified
  drop: git-annex locks and checks K custody copies on custodians first (E1,
  E5), and the laptop's own copy never counts (ADR 0002). git-annex refuses
  `--auto` with `--all` (E18). The laptop also re-thins the current files it
  captured ([07 §7.8](07-capture-and-install.md), ADR 0041). This keeps laptops
  at about one copy of what they keep.

## 8.4 Repair

A key falls below K custody copies when a custodian is removed from the roster
(marked `dead`, [03 §3.4](03-repository-and-roles.md)), when a scrub finds it
bad, or when a custodian's disk dies and its operator removes it. The other
custodians' `sizebalanced` share then includes it, and their per-key downloads
(§8.3) restore K within their capacity (E9, E10). A **repair epoch** lasts until every
key has K copies again; its progress (keys and GB left) is in the status
("repairing", ux.md §8.11). A custodian that stops being heard of is not
removed automatically: after 7 days its owner and the admins see *Needs you*
([06 §6.5](06-status.md)), and they decide.

## 8.5 Freeing and keeping a folder (ADRs 0031, 0039)

Per top-level folder F, on a laptop. A file of F is always complete or absent,
never a pointer file, including after a crash at any instant (ADR 0039,
experiment E15b). Freeing and keeping hold F's lock: capture does not scan F
during a transition. The target is recorded first, in this device's `wanted`
expression ([03 §3.4](03-repository-and-roles.md)), and both sequences are
idempotent, so a restart reruns the transition toward the target before
capture resumes.

**Free F** (*Keep on this computer* switched off, `pigeon free F`):
1. Remove F from this device's `wanted`.
2. Refuse while F has an unresolved divergence ([07 §7.6](07-capture-and-install.md)):
   "Resolve the conflict on *tarte.docx* first".
3. **Hide** the files of F whose versions are *protected*, with
   `git sparse-checkout` non-cone patterns that exclude F and re-include its
   files not protected yet. Each file goes from complete to absent. A file that
   git leaves in place (it differs from the index: someone saved it) is
   captured like any save, and freeing resumes after.
4. **Drop** the keys of the hidden files with a verified `git annex drop --key`,
   except keys that a kept path still uses.
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
   their index entries (a hard link to the object on thin laptops, else a copy).
3. **Move in** the staged folder with one `rename` that refuses to replace
   (`RENAME_NOREPLACE`, `RENAME_EXCL`). If F exists on disk (the person made a
   folder of that name meanwhile), install the files one by one instead
   ([07 §7.5](07-capture-and-install.md)); colliding files become divergences.
4. **Unhide without a checkout:** update the sparse patterns, clear the
   skip-worktree bits of F's index entries (`git update-index
   --no-skip-worktree`, then `--refresh`), and run `git annex restage`.

Hidden files are never read as deletions: `git status` skips skip-worktree
entries, and the staging directory is inside `.git`. Kept folders are recorded
by git's own sparse-checkout file and the device's `wanted` expression;
`pigeond` restores both at start (ADR 0031). Path commands skip hidden folders
(E15), so pigeon handles hidden folders by key. A stale `index.lock` left by a
killed git process of `pigeond` is removed at start, since `pigeond` is the
only writer of the repository.

## 8.6 Purge: erase forever (ADRs 0018, 0042)

A purge releases **references**, not keys (ADR 0042). A reference of key *k* is
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
protocol ([13 §13.2](13-verification.md)); E14 is to test it with git-annex.

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

- **Need:** K × (the size of `keys()`: current versions, history and proposals
  not purged).
- **Capacity:** the sum over custodians of min(`maxsize`, one copy of
  everything), since a custodian is useful only up to one copy.
- pigeon warns admins and custodian owners when capacity < need × 1.25 (the
  25 % headroom, because `sizebalanced` is heuristic, E10), saying how many GB
  are missing and on which machines. Versions that do not fit stay *published*
  with reason `no-capacity`.

## 8.10 Rebalancing

At the lowest priority, a custodian drops each key it holds that its `wanted`
expression no longer matches after the shares moved (a custodian joined or
changed `maxsize`), with a verified `git annex drop --key`: git-annex checks K
other custody copies first (ADR 0014). pigeon
never runs `--rebalance` against a remote. A placement planner may replace
`sizebalanced` later (ADR 0003).
