# 0039. Freeing and keeping a folder never show a pointer file: hide before dropping, stage before showing

- **Status:** Proposed, 13 October 2026, by the agent, for issue #2
  (Guillaume's review: "Prevent pointer files from becoming visible during
  folder eviction and restoration"), on the evidence of experiment E15b;
  revised 14 October 2026 for issue #10 on the evidence of the `Evict` model
  and E24, absorbing the former ADR 0061. Replaces the order of steps of
  ADR 0031; its other decisions stand. Adds the guard of the drop of old
  versions of ADR 0041.

## Context

ADR 0031 promises that a folder's files are complete or absent, never shown
as git-annex pointer files. Its order of steps breaks that promise between
steps. Freeing drops the content while the folder is still checked out, and
git-annex then replaces each unlocked file with its pointer until the folder
is hidden. Keeping shows the folder before `git annex smudge --update` fills
it, and git's checkout writes pointers first. E15b
(`lab/git-annex/ga-e15b-pointer-free-transitions.sh`) scans the folder
continuously during each sequence: with 100 files, ADR 0031's freeing showed
44 887 pointer observations, and its keeping 1 306. A `kill -9` inside
`git sparse-checkout set` leaves a lock that blocks every later hide
(`Evict_lockfile`).

The `Evict` model ([13 §13.2](../spec/13-verification.md)) then ran hide
before drop, stage before show against a person who keeps saving, a group
that keeps changing the folder, custodians that lose copies, and crashes;
E24 checks its assumptions with real git and git-annex. It found:

- **A hide loses a write through a handle opened before it.**
  `git sparse-checkout set` unlinks a hidden file that matches the index, so
  an editor's later write lands in no file, or, with `annex.thin`, inside the
  annex object, which git-annex then calls absent (`inAnnex'` in
  `Annex/Content/Presence.hs`) (`Evict_git`, `NoLoss`).
- **A drop turns other files of its key into pointer files.** git-annex's
  drop replaces every unmodified file its keys database associates with the
  key (`removeAnnex` and `resetpointer` in `Annex/Content.hs`,
  `depopulatePointerFile` in `Annex/Content/PointerFile.hs`,
  `isUnmodifiedLowLevel` in `Annex/Content/Presence/LowLevel.hs`), keeping the
  old mtime, and `git status` stays clean. A free can do this to a file of
  another kept folder (`Evict_guard`) or to the staged files of a keep under
  way (`Evict_target`), and ADR 0041's drop of old versions to a file whose
  current version is that key (`Evict_prune`).
- **A keep can show a version that `main` has already replaced**, which
  capture then commits as the person's save (`Evict_stale`,
  `CaptureOnlySaves`).

## Decision

- **Freeing folder F:** hide the files of F whose versions are protected,
  then drop their keys. A **hide is an install of the file's absence**
  (07 §7.5): journal `begin`, move aside, writer scan, check against the
  index entry, `proved`; then `git sparse-checkout set` (non-cone patterns
  that exclude F and re-include its files not protected yet) runs with the
  file already absent, so it sets the bit and unlinks nothing. The aside is
  kept for the keep period and released with a hash; a writer, or a file
  that is not the expected one, sends it back and capture takes over. While
  a file is aside, capture skips its path. A file that becomes protected
  later is hidden, then dropped, the same way.
- **No drop on a laptop takes a key that a path needs.** Whether it frees a
  folder or drops old versions (07 §7.8, ADR 0041), a drop of key *k*
  requires that every path whose index entry is *k* be hidden, absent on
  disk, and in a folder whose target (the `wanted` expression) is free.
  `pigeond` runs the drop of *k* and every step that brings *k* to a path
  (install, staging, capture) one at a time. The drop is verified when it
  runs: it locks K copies in K domains (ADRs 0049, 0054), whatever the status
  said at the hide (`Evict_lock`, `DropsProtected`); if it refuses, the file
  stays hidden and the drop is retried later.
- **Keeping folder F:** `git annex get --key` every key of F first. Then
  write the complete files under `.git/pigeon/stage/F`, with the modes of
  their index entries, and move the staged folder into place with one
  `rename` that refuses to replace (`RENAME_NOREPLACE`, `RENAME_EXCL`).
  Before clearing a moved-in file's skip-worktree bit, compare its staged key
  with the index entry: a file `main` has passed, unchanged and without a
  writer, is replaced by the current key; a file the person changed is
  theirs. Then update the sparse patterns and clear the bits
  (`git update-index --no-skip-worktree`, `--refresh`, `git annex restage`),
  without any checkout. If F exists on disk (the person made a folder of that
  name meanwhile), files are installed one by one with the install protocol,
  and colliding files become divergences. Keeping is all or nothing per
  folder: if some keys cannot be fetched yet, F stays hidden and its row says
  how many files are still being received.
- A file that git leaves in place when hiding (someone saved it) is captured
  like any save; freeing resumes after. A folder with an unresolved
  divergence is not freed until it is resolved.
- Freeing and keeping hold F's lock in `pigeond`: capture does not scan F
  during the transition. The target is recorded first, in this device's
  `wanted` expression (ADR 0031), and both sequences are idempotent, so a
  restart reruns the transition toward the target before capture resumes.
  Stale `index.lock` and `info/sparse-checkout.lock` files left by a killed
  git process of `pigeond` are removed at start.

## Consequences

- E15b shows no pointer observed in either sequence, every file complete or
  absent after `kill -9` at random instants, and a clean `git status` after a
  rerun. The `Evict` model checks, within its bounds, that no save is lost,
  no pointer is shown, capture commits only what the person wrote, a laptop
  drops only keys K custodians hold (`Evict_free`), and, under fairness, that
  a freed folder ends hidden and a kept one present (`FreeLive`, `KeepLive`).
  E24 runs in the `lab` workflow on Linux and macOS.
- Hidden files are never interpreted as deletions: `git status` skips
  skip-worktree entries, and the staging directory is inside `.git`.
- A hide costs what an install costs: two renames, a writer scan and a check
  per batch, plus an aside kept for about 10 minutes. A file kept open by an
  application is not hidden until it is closed (reason `in-use`).
- A key shared with a kept folder keeps its space on the laptop. Keeping
  needs free space for a full copy of F before it appears, and, without
  `annex.thin` or reflinks, twice F's size during the move (issue #3).
- Keep may restage files whose version changed during the transition; a
  large folder that changes often takes longer to show.
- Once accepted, ADR 0031's status gains "order of steps replaced by ADR
  0039", and ADR 0041's "guard of the drop of old versions added by ADR 0039".

## Alternatives considered

- *Keep git's unlink and scan for writers first.* A writer that opens the
  file between the scan and git's unlink is still lost; git gives no hook
  between its check and its unlink.
- *Drop with `--force` and restore pointers afterwards.* The pointer is
  visible meanwhile and keeps its old mtime, so nothing would notice it.
- *Keep `annex.thin` off for received content.* It doubles the disk use of
  kept folders (ADR 0041) and still loses the write in the unlinked inode.
- *Stop the accept loop during a keep.* A keep can wait for downloads for
  hours, and history would stall for every folder.
