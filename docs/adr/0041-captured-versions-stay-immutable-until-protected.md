# 0041. A captured version stays immutable on its laptop until it is protected; thin only after

- **Status:** Proposed, 13 October 2026, by the agent, for issue #3
  (Guillaume's review: "Reconcile captured-version guarantees with thin
  laptop copies and offline edits"), on the evidence of experiment E18.
  Refines ADR 0002's `annex.thin` for laptops; its copy classes stand.

## Context

ADR 0002 sets `annex.thin` on laptops, so a file and its annex object are one
inode, at about one copy of disk space. An application that saves in place then
rewrites the object too. ADR 0002 accepts this for copies that never count
toward K. But a version captured on an offline laptop has no other copy yet.
The issue's execution:

1. V1 is captured and committed.
2. An in-place save turns the same inode into V2.
3. V1's commit remains, but its content is gone everywhere.

E18 reproduces it. With five versions saved in place while the custodian was
away, the custodian later received 1 of the 5. Atomic saves (a new inode each
time) kept all five, even when thin. P2 says a captured version is never
overwritten, so best-effort history would void P2 for exactly the people who
work offline.

Options were:

- best effort: keep thin, and say unprotected versions may vanish;
- never thin: a full second copy of everything kept, on file systems without
  copy-on-write;
- copy until protected, then thin.

## Decision

- **Capture adds a copy.** Capture step 6 runs `git -c annex.thin=false
  annex add`, so the object is a separate file that no application has open.
  git-annex copies with copy-on-write where the file system can (APFS, btrfs,
  xfs), which E18 is still to measure there. A captured version is immutable
  on the laptop that captured it until it is protected. The laptop's
  configuration keeps `annex.thin=true`, so content received from others is
  installed by hard link (ADR 0040). Losing that copy loses nothing: its
  origin still holds an immutable one.
- **Once protected**, the laptop drops the old versions it holds, by key and
  verified (`git annex drop --key`). git-annex refuses `drop --auto --all`
  (E18). It also re-thins the current file, on file systems without
  copy-on-write, if the file is still that version. Re-thinning is an
  install of the same key with the object as the new file, so it gets
  ADR 0040's proof.
- **Three separate states**, for accepted versions and suggestions alike
  (spec 06 §6.2):
  - *captured* means in a signed commit or proposal of this laptop, with an
    immutable local copy (state *local*);
  - *published* means another device has the commit, or a custodian holds
    the key;
  - *protected* means K verified custodians hold the key.

  "Safe" is said only of protected versions, suggestions included. A version
  whose content no device is known to hold, this one included, is *missing*,
  and its reason says so.
- **Pending history** is the set of unprotected versions this laptop
  captured. It is budgeted apart from the kept folders, by `pending_max`
  (default 10 % of the file system). Capturing new content beyond it, or
  below `annex.diskreserve`, pauses with reason `history-full`. The files
  stay as saved on disk, which is the "between two captures" case of P2.
  Deletions and renames are still captured. Pending history shrinks as
  custodians receive it.

## Consequences

- P2 holds offline: no in-place save can destroy a captured version. E18
  checks this with five offline saves, failed uploads and a reconnection:
  all five versions reached the custodian, for in-place and atomic saves.
- **The disk goal, qualified.** Steady state, with custodians reachable, is
  about one copy of what a laptop keeps: E18 measured 5.2 MB for a 4 MB file
  once protected and re-thinned. Add the pending history: E18 used 25 MB
  offline for five 4 MB versions on ext4, because without copy-on-write each
  captured version is a full copy. Add the kept asides (ADR 0040) and
  metadata. On APFS and btrfs a pending version should cost only the blocks
  that changed (to be measured).
- Capture of a large file on ext4 writes a full copy. This is disk I/O
  under ADR 0037's limits, and the hot-file rule (at most every 10 minutes)
  bounds pending history for files saved often.
- Spec 08 §8.3 said laptops drop old versions with `drop --auto`. That
  command does not take `--all`, so the drop is by key (E18).
- E18 also asserts that no git-annex command run by pigeon (`fsck`,
  `restage`) re-thins a captured copy behind its back.
