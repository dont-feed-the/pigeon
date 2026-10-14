# 0040. Install proves a file is its own before replacing it, and a racing save always wins

- **Status:** Proposed, 13 October 2026, by the agent, for issue #1
  (Guillaume's review: "Define an install protocol that preserves application
  saves racing with incoming updates"), on the evidence of experiment E19;
  revised 14 October 2026 on the evidence of the `CaptureInstall` model
  (issue #10), absorbing the former ADR 0055 (recovery rolls forward from a
  journalled proof). Refines the mechanism of ADR 0034; its rule stands.

## Context

ADR 0034 says pigeon never replaces, moves aside or removes what a person
saved, unless they ask. Install skips a path whose file differs from what
pigeon last installed. Checking first and then renaming the new version over
the name leaves a race: an application can save between the check and the
rename, and the rename then destroys that save without a trace. A quiet period
and repeated `stat` calls narrow the window but do not close it, because
applications do not synchronise with pigeon. POSIX has no "rename over the
name only if it is still this inode". Writes through a handle opened before
the rename are a second problem: they reach the old inode wherever it is.

Options were:

- check, then rename over: the race above;
- swap the two files atomically (`RENAME_EXCHANGE`): the new version is shown
  before the proof, and undoing a failed proof means swapping back a file an
  application may already have opened;
- leases or advisory locks: Linux only, or unused by most applications;
- a best-effort guarantee: a lost save leaves no trace, so P2 would not hold.

## Decision

- **Move aside, then prove.** Install renames the file at *p* to
  `.git/pigeon/aside/<seq>` (same file system), so no name refers to it any
  more. It then proves the file is its own. No process may hold it open for
  writing or map it shared and writable. This is the **writer scan**: on
  Linux, `/proc/*/fd` with the access mode in `fdinfo`, and `/proc/*/maps`;
  on macOS, `libproc` (`proc_pidinfo`, `proc_pidfdinfo`). Its `(dev, ino,
  size, mtime)` must equal the index entry, and its link count must be the
  expected one: 1, or 2 when it is the object's inode on a thin laptop. A
  higher count is a person's hard link; ADR 0044 (Proposed) lets the proof
  pass and releases such an aside without hashing. The scan comes first, so no write
  can slip between the check and the next step.
- **A racing save wins.** If the proof fails, the file goes back to *p* with
  `RENAME_NOREPLACE` (Linux `renameat2`, macOS `renamex_np(RENAME_EXCL)`),
  and the path waits for capture. If an application recreated *p*
  meanwhile, the moved file is captured as a *recovered* version (a
  divergence, spec 07 §7.6). The new version enters with `RENAME_NOREPLACE`.
  If an application created *p* since the move, its file stays, and the path
  becomes a divergence based on the version pigeon last installed. pigeon
  never renames over a name.
- **Busy files wait.** A failed scan, or an application lock file next to *p*
  (`~$name`, `.~lock.name#`, `.#name`, `.name.swp`), defers the path with
  reason `in-use`. The path is retried when the lock file goes, and at every
  scan.
- **No temporary names in the folder.** The new file is written under
  `.git/pigeon/stage/` (ADR 0039's location) and fsynced before the journal
  entry.
- **pigeon never writes stat data into the index.** It sets the entry with
  `update-index --cacheinfo` and no stat, then `update-index --refresh`. git
  records stat data only if the content hashes to the key, by its racy-git
  rule and the annex clean filter. A racily clean entry (mtime not older
  than the index file) defers the path.
- **Journal order:** temporary file fsynced; `begin` (with the expected stat
  and the temporary inode) fsynced; move aside; scan and check; `proved` for
  the entries of the batch that passed, fsynced, before any rename in; rename
  in; fsync both directories; index; end record. A creation, where no file is
  expected at *p*, is proved when *p* is found free. Each entry ends with
  exactly one of `done`, `skip`, `raced` or `recovered`, appended before its
  staged file is removed; the aside's removal is logged as `released`.
- **Recovery rolls forward what was proved, and back what was not.** What is
  at *p* alone cannot tell whether the proof had passed (see Consequences).
  - *Proved:* if the new file is still at its stage name, rename it in with
    `RENAME_NOREPLACE`; if *p* exists, the entry ends `raced`. Then redo the
    index (git records the stat only if the file at *p* is the new key) and
    end `done`. Whatever the person did at *p* after the rename is an
    ordinary save on the new version, captured next.
  - *Not proved:* if *p* is absent, the aside goes back with
    `RENAME_NOREPLACE`. If *p* is another file, the aside is kept and hashed
    at release (`raced` if it holds the old key, `recovered` otherwise).
    Otherwise the entry ends `skip`.
  - Only the staged files of ended entries are removed, so replaying twice
    equals replaying once.
- **Kept aside.** The moved file is kept until the first full scan at least
  10 minutes after `done` (default). It is unlinked then only if no process
  writes it and its content still hashes to the old key; otherwise it is
  captured as a recovered version. Writers that escape the scan are assumed
  to close their handle within this keep period (07 §7.5).
- **Explicit resolution** (*Take the group's version*) runs the same
  sequence. The proof compares the file with the person's version recorded
  in the divergence table when they chose (`mine_stat`), which capture has
  already committed from a quiet file (07 §7.2). A `mine_stat` that is racily
  clean defers the resolution until the next capture. A save after the choice
  fails the proof: nothing is replaced,
  the save extends their proposal, and they are told once. *Keep mine as a
  copy* copies from the captured key, never from the live file.

## Consequences

- **The guarantee, narrowed and exact.** pigeon destroys no saved content.
  It replaces the file at a name only after proving the file is the one it
  installed, or the one the person chose to discard, and that nobody writes
  it. A write through a handle opened before the move that escapes the scan
  lands in the kept aside. Such writers are processes of another user, an
  `open` in flight during the move, and mappings that macOS cannot see. The
  aside is then captured as a recovered version: the content is kept, but
  not at its name.
- **The name is absent for an instant** between the two renames. E19
  measured a median of 0.55 ms and a maximum of 6.6 ms (Linux, ext4). In that
  instant an application opening *p* without creating it gets "not found";
  one that creates it wins the name.
- A save made after the install, by an application that still holds the old
  content in memory, counts as an edit of the new version. The new version
  stays in the history but is no longer current. Lock files cover office
  suites and editors; the rest is an open question (spec 16 §16.4).
- `RENAME_NOREPLACE` must be supported. ext4, btrfs, xfs and APFS support
  it. A file system that answers `EINVAL` gets no installs, with a Problem
  ([14 §14.2](../spec/14-platforms-and-packaging.md)).
- The writer scan costs one pass over the processes' open files per install
  batch. Kept asides cost a copy of each replaced file for about 10 minutes
  on laptops that are not thin; this counts as temporary install space
  (issue #13).
- **Evidence:** E19 (`lab/git-annex/ga-e19-install-races.py`, asserting)
  runs the sequence against git-annex 10.20261006, thin and not thin.
  - Seven application actions at each of seven boundaries: write in place,
    atomic replace, create, delete, a handle opened earlier (closed or
    held), and a shared mapping.
  - 21 `kill -9` points, each recovered twice.
  - A fuzzing writer.
  - No save was lost, git never called a changed file clean, and recovery
    was idempotent. With the scan, no aside changed after its check. Without
    it, 9 of 49 writes landed in the aside and were kept as recovered
    versions.
- **The `CaptureInstall` model** ([13 §13.2](../spec/13-verification.md))
  has one action per step and runs the person acting while `pigeond` is down.
  A recovery that decided from *p* alone, as the first version of this ADR
  did, brought a finished deletion back (`CaptureInstall_rec07del`,
  `Journal`) and undid a deletion the person made after the rename in
  (`CaptureInstall_rec07`, `DeletionKept`). Deciding from whether the staged
  file was still at its stage name failed the model's double replay. A
  racily clean `mine_stat` let a resolution pass over a save not yet captured
  (`CaptureInstall_quiet`, `CaptureFirst`), and a release after an escaping
  writer's late write lost it (`CaptureInstall_keep`, `NoLoss`). With the
  journalled proof, after any crash each path ends at its old content, the
  new one or the person's, and the index agrees with it
  (`CaptureInstall_own`, `CaptureInstall_crash2`). The journal recovery
  function gets property tests (idempotence, every path ends old, new or the
  person's).
- Install costs one more journal record and fsync per batch. Rolling forward
  may run the rename in at start, after `pigeond` was down for a long time,
  so the proof can be old: a write through a handle opened before the move
  still lands in the aside, which is hashed at release.
- If this ADR is accepted, ADR 0034's status line gains "mechanism refined
  by ADR 0040".

## Alternatives considered

- *Check, then rename over*; *swap atomically* (`RENAME_EXCHANGE`); *leases
  or advisory locks*; *best effort*: see Context.
- *Decide recovery from the staged file and the index, with no new record.*
  Deletions have no staged file, and the index can already hold the new key
  before the index step (a rejected suggestion of a new file resolved by
  taking the group's deletion).
- *Always roll back.* Undoing a finished install means two more renames at a
  name the person may be using, and discards any change made after the
  rename in.
