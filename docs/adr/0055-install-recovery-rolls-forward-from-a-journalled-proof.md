# 0055. Install recovery rolls forward from a journalled proof

- **Status:** Proposed, 14 October 2026, by the agent, for issue #1
  (Guillaume's review: "Define an install protocol that preserves application
  saves racing with incoming updates"), on the evidence of the
  `CaptureInstall` model (issue #10).

## Context

ADR 0040 fixes the steps of install and a recovery table that looks only at
what is at *p* (spec 07 §7.5). If *p* has the new file's inode, step 7
happened. If *p* is absent and the aside exists, the aside goes back. If *p*
is another file, the aside is hashed. Otherwise the entry is skipped.

The `CaptureInstall` TLA+ model (spec 13 §13.2) runs that table against a
person who keeps acting while `pigeond` is down. It finds two behaviours in
which recovery undoes a change:

- **A finished deletion comes back.** A deletion has no new file. Suppose
  `pigeond` is killed after step 8, when the index no longer has *p*, and
  before `done`. Recovery finds *p* absent and the aside present, so it puts
  the old file back. The index says the file is deleted, so the next capture
  adds it again, and this laptop undoes the group's deletion
  (`CaptureInstall_rec07del`, invariant `Journal`).
- **A person's change after step 7 is lost.** The new file is at *p*. The
  person deletes it, and `pigeond` is killed before step 8. Recovery finds *p*
  absent and the aside present, and puts the old file back: the person's
  deletion is undone (`CaptureInstall_rec07`, invariant `DeletionKept`). If
  the person instead replaced the new file atomically, recovery calls it
  `raced`, and their edit of the new version becomes a false conflict.

In both cases *p* alone cannot tell whether the proof of steps 5 and 6 had
passed. A first repair tried to decide from whether the staged file was still
at its stage name. It failed the model's double replay: a first replay
removed the staged file, and the second one then concluded that step 7 had
happened.

## Decision

- **The proof is journalled.** After the writer scan and the checks of a
  batch (steps 5 and 6), install appends `proved` for the entries that passed
  and fsyncs, before any step 7. A creation, where no file is expected at
  *p*, is proved when *p* is found free. This costs one record and one fsync
  per batch.
- **Recovery rolls forward what was proved, and back what was not.**
  - *Proved:* if the new file is still at its stage name, step 7 runs, with
    `RENAME_NOREPLACE`. If *p* exists, the entry ends `raced`. Then step 8
    runs: git records the stat only if the file at *p* is the new key. Then
    the entry ends `done`. Whatever the person did at *p* after step 7 is an
    ordinary save on the new version, captured next.
  - *Not proved:* if *p* is absent, the aside goes back with
    `RENAME_NOREPLACE`. If *p* is another file, the aside is kept and hashed at
    release (`raced` if it holds the old key, `recovered` otherwise).
    Otherwise the entry ends `skip`.
- **An entry ends before its files are cleaned up.** The end record (`done`,
  `skip`, `raced` or `recovered`) is appended before the staged file is
  removed, and only the staged files of ended entries are removed. A second
  replay therefore decides as the first did.
- **The resolution's proof needs a quiet stat.** *Take the group's version*
  proves the file against `mine_stat` (spec 07 §7.6). Capture records it only
  from a quiet file (07 §7.2), whose mtime is older than the capture. A
  `mine_stat` that is racily clean (its mtime not older than its capture)
  defers the resolution until the next capture, as a racily clean index entry
  defers install. Without the quiet period, a save in the same timestamp tick
  keeps the stat, and the proof passes over a save not yet captured
  (`CaptureInstall_quiet`, invariant `CaptureFirst`).
- **The keep period bounds the escaping writers.** A write that escapes the
  writer scan is kept only if it lands before the aside is released, about
  10 minutes after `done` (ADR 0040). The model assumes that such writers
  close their handle within the keep period. Without that assumption, release
  unlinks a file that is written afterwards, and the write is lost
  (`CaptureInstall_keep`, invariant `NoLoss`). Spec 07 §7.5 says so.

## Consequences

- After any crash, each path ends at its old content, at the new one or at
  the person's. The index then agrees with what pigeon left at the path. A
  finished deletion stays finished. A change the person made after step 7 is
  kept, and captured as an edit of the new version.
- Install costs one more journal record and fsync per batch.
- Rolling forward may run step 7 at start, after `pigeond` was down for a
  long time, so the proof can be old. A write through a handle opened before
  step 4 still lands in the aside, which is hashed at release. A new file at
  *p* makes the entry `raced`.
- The replay law of spec 11 §11.5 (replaying twice equals replaying once)
  gets the model's double replay as evidence (`CaptureInstall_crash2`, two
  crashes, one during recovery).
- If this ADR is accepted, ADR 0040's status line gains "recovery replaced
  by ADR 0055".

## Alternatives considered

- *Decide from the staged file and the index, with no new record.* Deletions
  have no staged file. The index can already hold the new key before step 8,
  for example when a rejected suggestion of a new file is resolved by taking
  the group's deletion. This rule is also idempotent only if staged files
  outlive their entry. Rejected for the explicit record.
- *Always roll back.* Undoing a finished install would mean moving the new
  file away and the old one back, two more renames at a name the person may
  be using. It would also discard any change the person made after step 7.
  Rejected.
