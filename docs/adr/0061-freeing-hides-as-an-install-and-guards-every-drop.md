# 0061. Freeing hides a file as an install of its absence, and no drop takes a key that a kept path uses

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Add models and trace vocabulary for capture/install,
  authorization, purge, and repair"), on the evidence of the `Evict` model
  and E24. Refines ADR 0039 (how a file is hidden, which keys a drop keeps)
  and ADR 0041 (the drop of old versions).

## Context

ADR 0039 frees a folder by hiding its protected files with
`git sparse-checkout`, then dropping their keys with a verified
`git annex drop --key`, "keeping keys that a kept path still uses". Keeping
a folder stages complete files, moves them in, and clears the skip-worktree
bits without a checkout. E15b showed that this order shows no pointer file.

The `Evict` TLA+ model (spec 13 §13.2) runs these steps against a person who
keeps saving, a group that keeps changing the folder, custodians that lose
copies, and crashes. E24 checks its assumptions with real git and
git-annex. They find four gaps:

- **A hide loses a write through a handle opened before it.**
  `git sparse-checkout set` unlinks a hidden file that matches the index. An
  editor that holds the file open then writes into an inode no name leads to,
  and the save is lost. E24 shows that with `annex.thin=false` no file holds
  it afterwards. With `annex.thin=true` the write lands in the annex object,
  which git-annex then calls absent (`inAnnex'` in
  `Annex/Content/Presence.hs`), so it stays inside `.git/annex` where the
  person never finds it. Install guards against exactly this case with its
  aside and writer scan (ADR 0040), but a hide does not (`Evict_git`,
  invariant `NoLoss`).
- **A drop turns other files of its key into pointer files.** git-annex's
  drop replaces every file that its keys database associates with the key,
  if unmodified, by a pointer file. "Unmodified" is checked by inode cache,
  else by a hash, so a fresh copy also counts (`removeAnnex` and
  `resetpointer` in `Annex/Content.hs`, `depopulatePointerFile` in
  `Annex/Content/PointerFile.hs`, `isUnmodifiedLowLevel` in
  `Annex/Content/Presence/LowLevel.hs`). The pointer keeps the old mtime, and
  E24 shows that `git status` stays clean, so capture never notices. Three
  drops can do this:
  - a free that drops a key that a file of another, kept folder also has
    (`Evict_guard`);
  - a free while a keep of the same folder is under way, since its staged
    and moved-in files still have their bits set. "A kept path" read as a
    visible one misses them (`Evict_target`; E24 turns such a file into a
    pointer);
  - the drop of old versions of ADR 0041, which has no guard at all: an old
    version of one file can be the current version of another
    (`Evict_prune`).
- **A keep can show a version that main has already replaced.** The accept
  loop changes the index of a hidden path while a keep is under way
  (07 §7.5). A file staged before that change is moved in, and its bit is
  cleared. Capture then sees a file that differs from the index and commits
  the old version as the person's save, undoing the group's change
  (`Evict_stale`, invariant `CaptureOnlySaves`).
- **A crash inside `git sparse-checkout` blocks every later hide** until its
  lock file is removed (E15b). 08 §8.5 already removes it at start;
  `Evict_lockfile` shows that, without the removal, a freed folder is never
  freed.

## Decision

- **A hide is an install of the file's absence.** It takes install's steps
  for a deletion (07 §7.5): journal `begin`, move aside, writer scan, check
  against the index entry, `proved`. Then `git sparse-checkout set` runs with
  the file already absent, so it sets the bit and unlinks nothing. The aside
  is kept for the keep period and released with a hash. A writer, or a file
  that is not the expected one, sends the aside back, and capture takes over.
  If the person made the file again meanwhile, the aside is kept as a
  recovered version. While a file is aside, capture skips its path, as it
  skips every path of the install journal (`Evict_journal` shows the
  deletion it would otherwise capture).
- **No drop on a laptop takes a key that a path needs.** Whether it frees a
  folder (08 §8.5) or drops old versions (07 §7.8, ADR 0041), a drop of key
  *k* requires that every path whose index entry is *k* be hidden, absent on
  disk, and in a folder whose target (the `wanted` expression) is free.
  Staged and moved-in files of a folder being kept therefore count, and so
  do visible files. `pigeond` runs the drop of *k* and every step that brings
  *k* to a path (install, staging, capture) one at a time, so the check still
  holds when git-annex removes the object.
- **The drop is verified when it runs.** It locks K copies in K domains
  (ADRs 0049, 0054), whatever the status said when the file was hidden
  (`Evict_lock`, invariant `DropsProtected`). If it refuses, the file stays
  hidden and its content stays here, and the drop is retried once the key
  is protected again.
- **Keep shows only current files.** Before it clears a moved-in file's bit,
  keep compares the staged key with the index entry. A file that main has
  passed, and that is unchanged and has no writer, is pigeon's own: it is
  removed and the current key is staged. A file the person changed is shown
  and captured as theirs.

## Consequences

- No save is lost while a folder is freed, and no folder shows a pointer
  file. Capture commits only what the person wrote, and a laptop drops only
  keys that K custodians hold. The model checks this, within its bounds,
  for every interleaving of saves (in place through an earlier handle, or by atomic replace), group
  changes, lost custody copies, status reports, crashes and switches between
  kept and freed (spec 13 §13.2).
- Under fairness, a folder that stays freed ends with its files hidden and
  their keys gone, unless a kept folder uses them. A folder that stays kept
  ends with every file present (`FreeLive`, `KeepLive`).
- A hide costs what an install costs: two renames, a writer scan and a check
  per batch, plus an aside kept for about 10 minutes. A file kept open by an
  application is not hidden until it is closed (reason `in-use`).
- A key shared with a kept folder keeps its space on the laptop: freeing the
  folder does not free it.
- Keep may restage files whose version changed during the transition; a
  large folder that changes often takes longer to show.
- E24 runs in the `lab` workflow on Linux and macOS.
- If this ADR is accepted, the status lines gain: ADR 0039 "hide and drop
  guard refined by ADR 0061"; ADR 0041 "guard of the drop of old versions
  added by ADR 0061".

## Alternatives considered

- *Keep git's unlink and scan for writers first.* A writer that opens the
  file between the scan and git's unlink is still lost, and git gives no
  hook between its check and its unlink. Rejected for the aside.
- *Hide only folders that no application holds open.* It needs the same
  writer scan, and it still loses a handle opened just after it. Rejected.
- *Drop with `--force` and restore pointers afterwards.* The pointer is
  visible meanwhile and keeps its old mtime, so nothing would notice it.
  Rejected.
- *Keep `annex.thin` off for received content*, so that a write after a hide
  lands outside the object. It doubles the disk use of kept folders (ADR
  0041) and still loses the write in the unlinked inode. Rejected.
- *Stop the accept loop during a keep.* A keep can wait for downloads for
  hours, and history would stall for every folder. Rejected for the
  comparison before the bit is cleared.
