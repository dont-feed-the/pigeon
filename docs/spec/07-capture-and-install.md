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
  the person asks (ADR 0034). Install moves a file away from its name only to
  prove it is pigeon's own, and a save that races it wins the name (§7.5,
  §7.5).
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
- Names matching the **ignore list** are never captured (§7.9). The list
  is fixed in the core and matched on each path component: `.DS_Store`, `._*`,
  `Icon\r`, `.localized`, `.Spotlight-V100`, `.Trashes`, `.fseventsd`,
  `.TemporaryItems`, `.DocumentRevisions-V100`, `.Trash-*`, `Thumbs.db`,
  `desktop.ini`, `~$*`, `.~lock.*#` and `~*.tmp` (office files), `*.swp`,
  `.goutputstream-*`, `*.part`, `*.crdownload`, and pigeon's own temporary
  names. It applies to names not in the catalogue, as in git: a catalogue path
  is captured and installed whatever its name, and an ignored name is never
  captured as a deletion. In-tree `.gitignore` files and the person's git
  excludes have no effect ([03 §3.7](03-repository-and-roles.md)). A per-group `.pigeon/ignore` is an open
  question ([16](16-open-questions.md)).
- A **package** (§7.9) is quiet when none of its files has changed for the
  quiet period; its changes are captured together.

## 7.3 Capture

One capture **batch** handles the stable dirty paths of one repository:

1. **Root guard.** The folder's root, its `.git` and `.pigeon/roster.toml` exist,
   and the root is on the same file system as at start (device number). If not
   (drive unplugged, folder moved or deleted), capture and install stop, and the
   headline says "Needs you: pigeon can't find your folder *Famille Geiger*".
   Nothing is ever captured as a deletion because the root vanished.
2. **Changes.** `git status --porcelain=v2 -z -uall --ignored=traditional`
   lists the batch's added, modified, deleted and renamed paths, including
   those an in-tree `.gitignore` names ([03 §3.7](03-repository-and-roles.md)). Paths of the install journal
   whose content is the one being installed, held paths (§7.4) and ignored
   paths are skipped. Capture takes paths from this listing only, never from
   watcher events, since `git annex add` stages any path it is given, a file
   of a nested repository included (E20). Each remaining path is checked with `lstat`: only regular
   files go on; the other kinds are *not shared* (§7.9), and pigeon never
   follows a link.
3. **Mass-deletion guard.** If the deletions of the batch, with those already
   held, exceed 100 files or remove a whole top-level folder, all of them are
   **held** (§7.4). A package counts as one file. The rest of the batch goes
   on.
4. **Names.** A path whose name is not portable is held
   ([05 §5.4](05-catalogue-and-names.md)).
5. **Classify** every change with the authority rule under the roster of `main`
   ([04 §4.3](04-roster-and-authority.md)): *allowed*; *not allowed*, grouped by
   set of deciders; or *diverging* (the path is in the divergence table, §7.6).
6. **Ingest** the content of added and modified files with `git -c annex.thin=false
   annex add --no-check-gitignore` (batch). A file that changes while it is
   added is refused by git-annex and retried after its next quiet period
   (§7.9, E20). `git annex add` stages every file as 100644, so capture
   then sets the execute bit of each added path, and of each path whose only
   change is its mode, with `git update-index --chmod=+x|-x` from its `lstat`
   (E20, §7.9). The object is a copy (copy-on-write where the file system
   can), so a later in-place save cannot destroy the captured version (§7.8,
   §7.8). Re-adding content already in the annex is cheap, so a batch
   interrupted here is simply redone. Content beyond the pending budget waits
   (§7.8).
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
| a shared file became a link or a repository (§7.9) | *Delete it for everyone* | the deletion is captured |
| | *Bring it back* | pigeon renames the link or folder to a free name beside it, then install restores the file from `main` (the person asked) |

Held changes are *local* with reason `held` ([06 §6.4](06-status.md)) and a
*Needs you* item until decided.

## 7.5 Install

Install runs when `main` moves (a merge from the accept loop, a decision, a
restore) and when a folder is kept again ([08 §8.5](08-custody-loops.md)). It
moves each **clean** path *p* of a kept folder from its index version to
`current(p)`. A path is clean when git reports no change after
`update-index --refresh` and its index entry is not racily clean (its mtime is
older than the index file's). Hidden folders change in the index only (sparse
checkout, E15). A path that is not clean is skipped: capture handles it first,
then it may become a divergence (§7.6). Capture and install of one repository
never run at the same time. The paths of a package (§7.9) are installed in one
batch, and only once all of them are clean and none is in use; otherwise the
whole package waits. A reader may see a mix of versions while the batch runs.

A path is deferred with reason `in-use` (06 §6.4) while an application lock file
sits next to it (`~$name`, `.~lock.name#`, `.#name`, `.name.swp`) or the writer
scan of step 5 finds a writer. It is retried when the lock file goes and at every
scan.

For each path, in this order (a batch shares its fsyncs and its one writer scan):

1. **Content.** `git annex get --key` brings and verifies the new key. If no peer
   has it yet, the path waits; the status says "receiving".
2. **New file.** Write the new content at `.git/pigeon/stage/<seq>`, set the
   mode of the index entry, fsync it. On thin laptops it is a hard link to the
   object if the object has no other name; otherwise, and on laptops that are
   not thin, it is a copy (a reflink where the file system can). An object is
   thus linked to at most one path, as git-annex does
   (`Annex/Content/LowLevel.hs`, `linkOrCopy'`; §7.9). No temporary name
   ever appears in the folder.
3. **Journal.** Append `begin` with the path, both keys, the index entry's
   `(dev, ino, size, mtime)`, the temporary file and its inode, and the aside, to
   `.git/pigeon/install.ndjson`; fsync ([11](11-local-state.md)).
4. **Move aside.** `rename(p, aside)`, with *aside* =
   `.git/pigeon/aside/<seq>` on the same file system. No name now leads to the
   old file. If *p* is gone, the person deleted it: `skip`.
5. **Writer scan.** No process holds the aside open for writing or maps it
   shared and writable (Linux `/proc/*/fd`, `fdinfo` and `maps`; macOS
   `libproc`: descriptors with `FWRITE`, and every writable file-backed
   region, private ones included, since the share mode does not tell them
   apart). A scan that could not inspect some processes counts as clear only
   if the file gives no write permission to group or others; otherwise the
   path waits (`in-use`). It runs before the check, so no write slips in after it.
   On macOS a write through a shared mapping leaves the stat unchanged until
   `msync` (E19): capture sees such a save only once it is synced, and the
   kept aside (step 9) still keeps it.
6. **Check.** The aside's `(dev, ino, size, mtime)` equal the journalled ones.
   Its link count is 1, or 2 if it is the object's inode. A higher count
   means a person's hard link: the check still passes, and the aside is marked
   *linked* (§7.9).
   - If step 5 or 6 fails, a person saved or is saving *p*. The aside goes back
     with `renameat2(RENAME_NOREPLACE)` (Linux) or `renamex_np(RENAME_EXCL)`
     (macOS), the temporary file is dropped, and the entry ends `skip`;
     capture takes over.
   - If *p* was recreated meanwhile, the aside is captured as a recovered
     version (§7.6), never deleted: `recovered`.

   Once the checks of the batch are done, append `proved` for the entries that
   passed, and fsync. A creation is proved when *p* is found free.
7. **Rename in.** `renameat2(tmp, p, RENAME_NOREPLACE)` (or `RENAME_EXCL`). If an
   application created *p* since step 4, the call fails. The application's
   file stays, the temporary file is dropped, and the entry ends `raced`. The
   save is captured with the version pigeon last installed as its base, so it
   is a concurrent edit (§7.6). Then fsync both directories.
8. **Index.** `update-index --cacheinfo` with the new pointer and no stat data,
   then `update-index --refresh` (`git annex restage` per batch). git records
   the stat only if the file's content is the new key. A save landing after
   step 7 therefore shows as modified and is captured; pigeon never writes stat
   data itself (E19).
9. **Done.** Append `done`. The aside is kept until the first full scan at least
   10 minutes later (default). It is unlinked then (`released`) only
   if no process writes it and its content still hashes to the old key, or, for
   a *linked* aside, at once without hashing: its content lives on under its
   other name. Otherwise it is captured as a recovered version.

A deletion is the same without steps 1, 2 and 7. A released aside is
journalled `released`. When every entry has ended and its aside is released,
the journal is truncated. A staged file that no entry names (a crash between
steps 2 and 3) is removed at recovery: no name in the folder leads to it
(13 §13.13).

**Recovery at start**, before any capture, for each `begin` without an end
. An entry with `proved` rolls forward; any other rolls back.

- *Proved:* if the new file is still at its stage name, do step 7
  (`NOREPLACE`; if *p* exists, `raced`). Then redo step 8 and append `done`.
  Whatever the person did at *p* after step 7 is an ordinary save on the new
  version.
- *Not proved,* *p* absent and the aside present: put the aside back
  (`NOREPLACE`), then `skip`.
- *Not proved,* *p* another file and the aside present: keep the aside and
  hash it at release: `raced` if it holds the old key, otherwise a recovered
  version.
- Otherwise: `skip`.

An entry ends before its staged file is removed, and only the staged files of
ended entries are removed. Each path ends at its old content, the new one or
the person's, and the index agrees with what pigeon left (INV-Journal).
Replaying twice equals replaying once (a law, ADR 0036). The `CaptureInstall`
model found that deciding from *p* alone, as this table did before the journal recorded the proof,
brings back a finished deletion and undoes a deletion the person made after
step 7.

INV-NoOverwrite: pigeon never renames over a name, and moves a file away from
its name only to prove it is its own. If the proof fails, the file returns
within milliseconds, or is captured. A write through a handle opened before
step 4 that escapes the scan lands in the kept aside and is captured at step 9.
Such writers are a process of another user, an `open` in flight across the
rename, or a mapping macOS cannot see. Its content is kept, but not at its name
, if it lands before the aside is released: a write through such a
handle after the keep period goes to a file with no name, and is lost
. The name is absent between steps 4 and 7: E19 measured a median of
0.55 ms and a maximum of 6.6 ms.

## 7.6 Divergences and conflicts (ADRs 0034, 0035)

A **divergence** is a path whose file differs from the catalogue's version and
from what pigeon last installed there, because the person saved it. The
**divergence table** (`.git/pigeon/conflicts.json`, ADR 0036) keeps, per path:
its kind, the person's key, the `stat` of their file when that key was captured,
and their proposal chain; the catalogue key it diverged from, the key pigeon last
installed, its state and since when.

| Kind | Starts when | State |
|---|---|---|
| `suggestion` | capture made a proposal for the path | *waiting* ("Your suggestion, waiting for Paul") |
| `concurrent` | a merge gave the path to another person's version ([05 §5.3](05-catalogue-and-names.md)), or a save won the name during an install (§7.5 step 7) | *conflict* |
| `recovered` | install moved aside content that was not its own and could not put it back, or a kept aside changed (§7.5 steps 6 and 9) | *conflict* |

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
  the person's version is kept as a proposal's content and has that version's
  state, *protected* only once K custodians hold it (ADR 0034, [06 §6.2](06-status.md)).

Resolutions (`pigeon resolve PATH …`, or the buttons of ux.md §8.8):

| Choice | Effect |
|---|---|
| *Take the group's version* (`--take-group`) | install writes `current(p)` over the file, because the person asked; a pending proposal is withdrawn ([04 §4.5](04-roster-and-authority.md)); the person's version stays in its history |
| *Keep mine as a copy* (`--keep-copy DEST`) | the person's captured key is copied to DEST (never the live file), in a folder the person may change, and captured there; then as above |
| *Suggest again* (`--suggest-again`) | a new proposal from the person's file on top of the current `main`; state *waiting* |
| *Keep mine*, *Keep Paul's*, *Keep both* (concurrent edits) | the signed changes of [05 §5.3](05-catalogue-and-names.md) |

**A save after the choice** (§7.5). *Take the group's version* runs install
(§7.5) with one change: the check of step 6 compares the file with the person's
key and `stat` in the divergence table, which capture has already committed,
instead of what pigeon last installed. That `stat` comes from a quiet file
(§7.2), so a later save changes it; one that is racily clean (its mtime not
older than its capture) defers the resolution until the next capture
(§7.5). A save made after the person chose fails the check. Nothing is
replaced, the save extends their proposal, and they are told once: "You saved
*tarte.docx* again after choosing; nothing was replaced."

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
time: in place, by atomic replace, by creating or deleting the name, and through a
handle or mapping opened earlier), `Scan`, `Capture`, `InstallStep` (one per step
of §7.5, the writer scan included), `Crash`, `Recover`, `Release`, `Decide`,
`Resolve`. Invariants: INV-Capture, INV-NoOverwrite (no behaviour renames over a
name or installs over a diverging path, ADR 0034), INV-Journal, and NoLoss (every
saved content is at its name, kept aside or captured, §7.5). The model
exists (`models/tla/CaptureInstall.tla`, 13 §13.2); one negative configuration
per safeguard shows what fails without it. E19 runs the same steps against
git-annex with an adversary at each boundary. Every fault-injection run is
trace-validated against the model (ADR 0008).

## 7.8 Pending history and disk use

A version captured on a laptop is kept there as an immutable copy until it is
*protected* ([06 §6.2](06-status.md)). This holds for accepted versions and
suggestions alike. The unprotected versions this laptop captured form its
**pending history**.

- **Budget.** Pending history may use up to `pending_max`, 10 % of the file
  system by default, and capture never goes below
  `annex.diskreserve`, max(1 GB, 1 %) on laptops ([08 §8.9](08-custody-loops.md)), which pigeon
  checks itself because `git annex add` does not. Beyond either, capture of new content pauses with reason
  `history-full` ([06 §6.4](06-status.md)). The files stay as saved, and
  deletions and renames are still captured.
- **Once protected**, the laptop drops the old versions it holds by key
  (`git annex drop --key`, verified; [08 §8.3](08-custody-loops.md)), with the
  guard of [08 §8.5](08-custody-loops.md) step 4: never a key that a path of
  a kept folder still has, which git-annex would turn into a pointer file
  ([08 §8.5](08-custody-loops.md), E24, `Evict_prune`). If the
  file system has no copy-on-write and the file is still the protected
  version, the laptop re-thins it: an install (§7.5) of the same key, with the
  object as the new file.
- **Disk use** of a laptop:
  - one copy of what it keeps, once protected (E18: 5.2 MB for a 4 MB file);
  - plus the pending history (E18 on ext4: five offline versions of 4 MB took
    25 MB; on APFS and btrfs only the changed blocks, to be measured);
  - plus the kept asides of install (about 10 minutes, §7.5) and metadata.

  "About one copy" is the steady state while custodians are reachable.

Content received from others is installed by hard link (thin): an in-place
save may destroy that local copy, which never counts toward K, while its origin
keeps an immutable one.

## 7.9 What the folder may hold

pigeon shares **regular files**, by their bytes and the owner's execute bit.
Folders exist through their files. The rest is listed here, so that nobody
takes an incomplete capture for a safe one.

| Object | What pigeon does |
|---|---|
| regular file | shared: content (`SHA256E`) and execute bit; installed with mode 0644 or 0755 under the umask, at the time of the install |
| other mode bits, owner, times, ACLs, extended attributes, resource forks, Finder tags | not shared, and not kept by an install, which writes a new file |
| hard links | each name shared as its own file; an install of either name breaks the link, and the other name keeps its content (§7.5 step 6) |
| empty folder | not shared, not listed; install removes a folder it emptied, with `rmdir`, which fails harmlessly if a person added something |
| symbolic link | *not shared* (`link`), never followed; listed, notified once |
| folder holding `.git` | *not shared* (`repository`), with everything in it; listed, notified once |
| live library: `.photoslibrary`, `.musiclibrary`, `.tvlibrary`, `.imovielibrary`, `.fcpbundle`, `.sparsebundle` | *not shared* (`library`); listed, notified once |
| file or folder pigeon may not read | *not shared* (`unreadable`) until readable: neither captured nor installed, never a deletion; listed, notified once |
| FIFO, socket, device | not shared, not listed: they hold no data (`git status` does not show them) |
| ignored name (§7.2) | not captured, not listed |
| non-portable name | held, *Needs you* ([05 §5.4](05-catalogue-and-names.md)) |
| package: `.app`, `.band`, `.bundle`, `.framework`, `.key`, `.logicx`, `.numbers`, `.pages`, `.playground`, `.rtfd`, `.scriv`, `.textbundle`, `.xcodeproj`, `.xcworkspace` | shared as one document: captured together when quiet (§7.2), merged as one slot ([05 §5.2](05-catalogue-and-names.md)), installed in one batch (§7.5); one divergence entry |

`package(p)`, the outermost folder above *p* with one of these extensions,
compared case-insensitively, is a pure function of the name, so every platform
agrees. Outside packages, files saved together share a commit only if they
settle in one capture batch; no other multi-file consistency is promised.

*Not shared* items appear in the folder view ("2 items not shared") and in
`pigeon status --not-shared`. A *Safe* headline then adds "2 items of your
folder aren't shared" ([06 §6.5](06-status.md)).

The accept rule refuses trees that hold anything else. Rule 6 of
[04 §4.4](04-roster-and-authority.md) allows only folders and files of mode
100644 or 100755. Outside `.pigeon/` and the root `.gitattributes`, each file
must be a pointer to a `SHA256E` key; inside them, a plain blob.

git and git-annex run with pigeon's configuration only ([03 §3.7](03-repository-and-roles.md),
[03 §3.7](03-repository-and-roles.md)). The person's global settings, in-tree
`.gitattributes` and in-tree `.gitignore` files cannot change keys, trees or
listings.
