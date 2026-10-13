# 0042. A purge releases the references its deciders saw; content goes only when no other reference holds it and every custodian has sealed

- **Status:** Proposed, 13 October 2026, by the agent, for issues #4 ("Make
  purge reference-aware before forcibly deleting deduplicated annex keys") and
  #5 ("Define purge semantics for concurrent restores and returning offline
  peers") of Guillaume's review, on the evidence of the `Purge` TLA+ model.
  Replaces the mechanism of ADR 0018 (released-key metadata); its decision
  stands. Replaces ADR 0012's laws for released keys.

## Context

ADR 0018 turns a purge decision into git-annex metadata `pigeon-released` on
keys, then drops those keys with `--force`. A version and a key are different
units: identical files in two folders, a restored old version and a
suggestion all share one key. Releasing by key can destroy content that
another path still needs (#4). ADR 0012 calls the released keys a grow-only
set, which cannot express a new obligation: a restore during the waiting
period, or a peer that returns from weeks offline with a copy it re-added.
And a custodian cannot know a restore that reached only another custodian, so
dropping when its own clock passes `retention_end` can lose a restore that
the UI promised to keep (#5). The `Purge` model finds that loss.

git-annex metadata is also the wrong place for the answer: any device writes
the git-annex branch without a signature, and its values replay by
timestamp. Checking E14's assumptions showed two more facts (git-annex
10.20261006): `get --auto` and `drop --auto` refuse `--all` and `--key`, and
`satisfy --all` drops content from remotes. Custodians cannot use them as
spec 08 assumed.

## Decision

1. **References.** A *reference* of key k is a pair (c, p): c is an accepted
   commit or a proposal commit whose tree has k at p, and none of its parents
   has k at p. Adds, edits, restores, renames and conflict copies make
   references; an unchanged entry inherited by later commits does not.
2. **Scope.** A purge decision names `paths` (prefixes), optionally `keys`
   (some versions only), `proposals` (suggestions it erases) and `base`, the
   `main` its decider saw. It **covers** (c, p) when p is under one of its paths,
   c is `base` or an ancestor of it, and k is in `keys` if they are given; a
   proposal's references are covered only if the decision names it. A request
   for a current file first deletes it, in the same commit. The decision is
   *active* while its file is in the tree of `main`; deleting the file
   cancels it.
3. **Seals.** A custodian **seals** an active decision when two things hold
   on its own clock: `retention_end` has passed, and `retention_days` have
   elapsed since it first received the decision (kept in its local state; a
   lost state restarts the count). The seal goes into every later status
   report of the custodian while the decision stays active: `{decision, main,
   held}`. `main` is its `main` at that moment. `held` lists the covered keys
   that it keeps for a reference outside `main`, such as a suggestion.
4. **Release.** A device holds key k as **released** when all of these hold:
   - every reference of k that it knows is covered by an active decision;
   - each such decision has a seal from every custodian of the roster not
     marked `dead`, and the device's `main` contains each seal's `main`;
   - no seal lists k in `held`;
   - k is not in the current tree of `main`.

   This is a pure function of `main`, the known proposals and the seals,
   computed at each merge and never stored. It sets no git-annex metadata.
5. **Drop.** A device drops each released key it holds, locally, with
   `git annex drop --force --key`. Custodians do it first; laptops do it the
   same way when they learn it, for example on returning from offline. This
   is still the only forced drop pigeon performs outside Drop. Remote
   `REMOVE` stays refused (ADR 0014).
6. **Transfers per key.** `pigeond` chooses every transfer and drop itself, key
   by key. For each key of `keys()` (05 §5.1, which excludes released keys) that the
   device's `wanted` expression matches (`git annex matchexpression --key`),
   it runs `git annex get --key` or `copy --to <custodian> --key`; for each key
   it holds and no longer wants, it runs a verified `git annex drop --key`. It
   never runs `get --auto`, `drop --auto`, `satisfy` or `sync --content`, which
   drop from peers.
7. **New references win.** A reference that no decision covers holds its key
   like any version. This includes a restore, a copy in another folder, a
   suggestion, or a version saved offline before the decision arrived. If
   the content was already dropped, a device that still has it uploads it
   again: a new **retention generation**, which only a new purge releases.
   If no copy remains, the version is *missing* with reason `erased`. The
   deciders see *Needs you* ("Erased content came back"), and can erase it
   again.
8. **Promises.** During the waiting period the UI says: "Until 12 November
   anyone can still restore them; erasing finishes after that, once every
   backup machine has confirmed". A restore or a cancel that reaches **any**
   custodian before that custodian seals, so within `retention_days` of the
   decision whatever the clocks, keeps the content. This holds in every
   message order, across partitions and restarts. Later ones are kept only
   if a copy is still left. The UI offers no restore of a version whose erase
   has finished.
9. **Release is not erasure.** Erasing releases pigeon's custody: every device
   of the group drops its copies when it next runs. It cannot reach copies
   made outside pigeon, a device that never returns, or another backup of
   a laptop. The names, authors and times of the erased versions also stay
   in the signed history. `pigeon erase` says so before asking.
10. **Laws** (replacing ADR 0012's row; hegel properties):
    - *isolation*: releasing one reference never releases a key that another
      reference holds; adding an uncovered reference removes its key from
      the released set;
    - *seal safety*: a key is dropped only if every reference known to every
      custodian at its seal is covered;
    - *monotone*: for fixed history and proposals, the released set grows
      with the seals;
    - *order independence*: the released set is a function of sets, not of
      the order in which they arrived;
    - *current*: no key of the current tree is ever released.

## Consequences

- Purging one photo never touches its identical twin in another folder, an
  accepted version, or a suggestion that shares its key.
- A purge completes only when every live custodian is reachable and has
  waited its full period. An unreachable custodian holds back every purge,
  and the status says so ("Erasing waits for Paul's NAS"). Admins unblock it
  by marking the machine `dead` if it is gone, which also starts repair.
- A purge is not a security boundary against members: a member can keep a
  copy anyway, so a key ban would cost legitimate reuse and buy nothing.
  Rejected alternatives:
  - a permanent ban on the key: it would refuse new saves by content and
    make acceptance depend on keys rather than authority (P4);
  - a grow-only released set in git-annex metadata: unsigned, and it cannot
    express a new obligation;
  - dropping at `retention_end` without seals: the `Purge` model loses a
    restore that way;
  - waiting for every device: one lost laptop would block purges forever.
- The `Purge` model checks KeepWins, WaitingPeriod and NoResurrection for
  this protocol. Two negative configurations lose a cancel or a restore:
  dropping when one's own period ends, and accepting a seal without merging
  the sealer's `main`.
- E14 asserts the mechanism with git and git-annex 10.20261006:
  - issue #4's three cases keep their key, the suggestion through `held`;
  - a restore that reached one custodian before its seal holds the purge
    back at another until it merges that `main`; trusting the seals alone
    would release the restored key;
  - per-key transfers never fetch a released key, although git-annex's own
    `wanted` still matches it; after `drop --force --key` no copy is left;
  - a returning laptop drops its covered copy, and its offline re-add comes
    back to both custodians;
  - `matchexpression --key` agrees with git-annex's own choices (`balanced`
    with `satisfy`, `sizebalanced` with `find --want-get`, which lists
    nothing in a bare repository).
- If accepted: ADR 0018's status gains "mechanism replaced by ADR 0042",
  and ADR 0012's gains "purge laws replaced by ADR 0042".
