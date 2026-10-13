# 7. Capture and install: the disk door

> Mission: specify the only two ways pigeon meets the shared folder on a laptop:
> **capture** turns what people save into signed commits, **install** writes what
> the group accepted. Together they carry P2 (capture, no overwrite) and part of
> P3 (the mass-deletion guard). Custodians have no working tree and run neither.

## 7.1 Principles

- People and their applications write the folder; pigeon reads it at capture and
  writes it only at install. No other pigeon code touches it.
- **The index records what pigeon last captured or installed** at every clean
  path: the git index always equals `main`'s tree, with the stat data of the file
  pigeon left there. A path whose file differs from the index (`git status`) is
  either a new save (capture's job) or a divergence (§7.6).
- pigeon never replaces, moves aside or removes content a person saved, unless
  the person asks (ADR 0034). Install proves a file is pigeon's own before
  touching it; where POSIX gives no atomic check, it moves the file aside and
  keeps it until it is sure (§7.5).
- Every step is crash-recoverable: a crash at any instant leaves each path at its
  old or its new content, and the next start resumes (ADR 0036 journal).

## 7.2 Noticing changes (ADRs 0013, 0037)

- The `notify` watcher marks paths dirty; events are hints only.
- A dirty path is captured after its **quiet period** (2 s, plus 1 s per 100 MB,
  at most 60 s) and two consecutive equal `stat` results (size, mtime, inode).
- A path captured more than 6 times in an hour is captured at most every 10
  minutes (hot files, ADR 0037).
- A full scan (`git status --porcelain=v2 -z`) runs at start, every 10 minutes,
  and at once on a watcher overflow; it is the truth.
- Files matching the **default ignore list** are never captured: `.DS_Store`,
  `._*`, `Thumbs.db`, `desktop.ini`, `~$*` and `.~lock.*#` (office lock files),
  `*.swp`, `*.part`, `*.crdownload`, and pigeon's own temporary names
  **[no ADR: list]**. A per-group `.pigeon/ignore` is an open question
  ([16](16-open-questions.md)).

## 7.3 Capture

One capture **batch** handles the stable dirty paths of one repository:

1. **Root guard.** The folder's root, its `.git` and `.pigeon/roster.toml` exist,
   and the root is on the same file system as at start (device number). If not
   (drive unplugged, folder moved or deleted), capture and install stop, and the
   headline says "Needs you: pigeon can't find your folder *Famille Geiger*".
   Nothing is ever captured as a deletion because the root vanished.
2. **Changes.** `git status` lists the batch's added, modified, deleted and
   renamed paths. Paths of the install journal whose content is the one being
   installed, held paths (§7.4) and ignored paths are skipped.
3. **Mass-deletion guard.** If the deletions of the batch, with those already
   held, exceed 100 files or remove a whole top-level folder, all of them are
   **held** (§7.4). The rest of the batch goes on.
4. **Names.** A path whose name is not portable is held
   ([05 §5.4](05-catalogue-and-names.md)).
5. **Classify** every change with the authority rule under the roster of `main`
   ([04 §4.3](04-roster-and-authority.md)): *allowed*; *not allowed*, grouped by
   set of deciders; or *diverging* (the path is in the divergence table, §7.6).
6. **Ingest** the content of added and modified files (`git annex add`, batch;
   thin hard links on laptops). Re-adding content already in the annex is cheap,
   so a batch interrupted here is simply redone.
7. **Commit.** Allowed changes: one commit, parent `main`, signed, then
   `update-ref main` from the expected old value; the index then matches.
   Not allowed: one proposal per set of deciders, parent `main` (or the pending
   proposal of the same path, [04 §4.5](04-roster-and-authority.md)), stored as
   its ref; the index keeps `main`'s version of those paths, and each path enters
   the divergence table as a *suggestion*. Diverging paths: §7.6.
8. **Trace.** Each commit and proposal emits its trace event (ADR 0008,
   [12](12-observability.md)).

A crash before step 7 loses nothing: the files are still on disk and the next
scan finds them. A crash between the commit and `update-ref` leaves an
unreferenced commit, which the next batch recreates. INV-Capture: a saved file
reaches a commit (accepted or proposal) before install touches its path, because
install only touches clean paths (§7.5).

## 7.4 Held changes

A **held** change stays on this computer only: the file is deleted (or
misnamed) on disk, `main` still has it, and install leaves the path alone.

| Held because | The person chooses | Then |
|---|---|---|
| mass deletion (ux.md §8.6) | *Delete them for everyone* | the deletions are captured as usual |
| | *Bring them back* | install restores them from `main` (the person asked) |
| name not portable | *Rename to the suggested name* | pigeon renames the file (the person asked); capture adds it |
| | rename it themselves | an ordinary capture |

Held changes are *local* with reason `held` ([06 §6.4](06-status.md)) and a
*Needs you* item until decided.

## 7.5 Install

Install runs when `main` moves (a merge from the accept loop, a decision, a
restore) and when a folder is kept again ([08 §8.5](08-custody-loops.md)). It
moves each **clean** path *p* (file equal to its index entry) of a kept folder
from its index version to `current(p)`. Hidden folders change in the index only
(sparse checkout, E15). A path that is not clean is skipped: capture handles it
first, then it may become a divergence (§7.6).

For each path, in this order:

1. **Content.** `git annex get --key` brings and verifies the new key. If no peer
   has it yet, the path waits; the status says "receiving".
2. **Journal.** Append `{path, old_key, new_key, tmp, aside}` to
   `.git/pigeon/install.ndjson` and fsync ([11](11-local-state.md)).
3. **New file.** Write the new content at a temporary name in the same folder
   (a hard link to the object on thin laptops, else a copy), set its mode,
   fsync it.
4. **Move aside.** `rename(p, aside)`, with *aside* under `.git/pigeon/aside/`
   on the same file system. Atomic: from here pigeon owns the old inode.
5. **Check.** If the aside's `stat` differs from the index entry, a person saved
   *p* in between: put it back with `renameat2(RENAME_NOREPLACE)` (Linux) or
   `renamex_np(RENAME_EXCL)` (macOS), drop the temporary file, and stop for this
   path; capture takes over. If *p* was meanwhile recreated, the aside is
   captured as a recovered version in the divergence table, never deleted.
6. **Rename in.** `renameat2(tmp, p, RENAME_NOREPLACE)` (or `RENAME_EXCL`). If an
   application created *p* since step 4, the call fails: the application's file
   stays, the temporary file is dropped, and *p* becomes a divergence.
7. **Index.** Update *p*'s index entry to the new pointer and stat
   (`update-index`, then `git annex restage` per batch), fsync the folder.
8. **Done.** Append the completion to the journal. The aside is kept for one hour
   (default **[no ADR]**); if its `stat` is unchanged then, it is unlinked;
   if an application wrote to it through a handle it still held, it is captured
   as a recovered version.

A deletion is the same without steps 1, 3 and 6. When every entry is done, the
journal is truncated.

**Recovery at start.** For each begun entry: if *p* holds the new key, mark it
done; else if *p* is missing and the aside exists, put the aside back
(`NOREPLACE`); else drop the temporary file. Each path ends at its old or its new
content (INV-Journal), and replaying twice equals replaying once (a law,
ADR 0036).

INV-NoOverwrite: no step writes a name that holds content pigeon has not
proven to be its own. The remaining window, an application writing through a
handle opened before step 4, is covered by keeping the aside (step 8).

## 7.6 Divergences and conflicts (ADRs 0034, 0035)

A **divergence** is a path whose file differs from the catalogue's version and
from what pigeon last installed there, because the person saved it. The
**divergence table** (`.git/pigeon/conflicts.json`, ADR 0036) keeps, per path:
its kind, the person's key and proposal chain, the catalogue key it diverged
from, the key pigeon last installed, its state and since when.

| Kind | Starts when | State |
|---|---|---|
| `suggestion` | capture made a proposal for the path | *waiting* ("Your suggestion, waiting for Paul") |
| `concurrent` | a merge gave the path to another person's version ([05 §5.3](05-catalogue-and-names.md)) | *conflict* |
| `recovered` | install found content it could not prove was its own (§7.5) | *conflict* |

Transitions:

- *waiting* → **ended** when the proposal is accepted: the person's key is now
  `current(p)`; only the index changes.
- *waiting* → *conflict* when the proposal is rejected, or `current(p)` changes
  to another key meanwhile. The person is told **once** (notification), and Home
  keeps a *Needs you* item: "Paul declined your change to *tarte.docx*; your
  version is still on your computer."
- A new save of a diverging path updates the person's side: the proposal chain
  for a suggestion; a proposal of their own for the other kinds, so that it is
  captured and replicated without changing the catalogue **[no ADR]**.
- While a path diverges, install never writes it, and newer accepted versions
  of it are not installed on this disk. A divergence changes no safety state:
  the person's version is safe as a proposal's content (ADR 0034).

Resolutions (`pigeon resolve PATH …`, or the buttons of ux.md §8.8):

| Choice | Effect |
|---|---|
| *Take the group's version* (`--take-group`) | install writes `current(p)` over the file, because the person asked; a pending proposal is withdrawn ([04 §4.5](04-roster-and-authority.md)); the person's version stays in its history |
| *Keep mine as a copy* (`--keep-copy DEST`) | the file is copied to DEST, in a folder the person may change, and captured there; then as above |
| *Suggest again* (`--suggest-again`) | a new proposal from the person's file on top of the current `main`; state *waiting* |
| *Keep mine*, *Keep Paul's*, *Keep both* (concurrent edits) | the signed changes of [05 §5.3](05-catalogue-and-names.md) |

**Seeing the group's version** (ADR 0035): *Open Paul's version* or
`pigeon view PATH --group` writes a read-only copy in pigeon's cache directory,
outside the shared folder, named `tarte (Paul's version).docx` in the reader's
language, and opens it with the default application; `--version N` does the same
for any version. These copies are removed when the conflict is resolved, or after
seven days. The shared folder only ever holds what the person saved and the
accepted catalogue.

## 7.7 What the models must show

The capture and install protocol gets its TLA+ specification
([13 §13.2](13-verification.md)): actions `Save` (a person or application, at any
time, including through an old handle), `Scan`, `Capture`, `InstallStep` (one per
step of §7.5), `Crash`, `Recover`, `Decide`, `Resolve`. Invariants: INV-Capture,
INV-NoOverwrite (no behaviour installs over a diverging path, ADR 0034),
INV-Journal. Every fault-injection run is trace-validated against it (ADR 0008).
