# 7. Capture and install: the disk door

> Mission: the only two ways pigeon meets the shared folder on a laptop.
> **Capture** turns saves into signed commits; **install** writes what the
> group accepted. They carry P2 and part of P3. Custodians run neither.

The `CaptureInstall.tla` invariants and `journal` laws cited are normative.

## 7.1 Principles

- People write the folder; pigeon reads it at capture and writes it only at
  install. The git index always equals `main`'s tree, with the stat of the
  file pigeon last left at each path; a path differing from it is a save
  (capture's job) or a divergence (§7.6).
- pigeon never replaces, moves aside or removes what a person saved unless
  they ask (ADR 0034); install moves a file away only to prove it is its
  own. A crash leaves each path at its old or new content (ADR 0036).

## 7.2 Noticing changes (ADRs 0013, 0037)

- `notify` events are hints; a full scan (`git status --porcelain=v2 -z`) at
  start, every 10 minutes and on watcher overflow is the truth.
- A dirty path is captured after its **quiet period** (2 s plus 1 s per
  100 MB, at most 60 s) and two equal `stat` (size, mtime, inode); more than
  6 captures in an hour slow it to one per 10 minutes. A package (§7.9) is
  quiet when all its files are, and is captured together.
- The **ignore list**, fixed in the core and matched per path component:
  `.DS_Store`, `._*`, `Icon\r`, `.localized`, `.Spotlight-V100`, `.Trashes`,
  `.fseventsd`, `.TemporaryItems`, `.DocumentRevisions-V100`, `.Trash-*`,
  `Thumbs.db`, `desktop.ini`, `~$*`, `.~lock.*#`, `~*.tmp`, `*.swp`,
  `.goutputstream-*`, `*.part`, `*.crdownload`, pigeon's temporary names. It
  applies only to names not in the catalogue, and an ignored name is never
  captured as a deletion. In-tree `.gitignore` and personal excludes have no
  effect ([03 §3.7](03-repository-and-roles.md)); `.pigeon/ignore` is open
  ([16](16-open-questions.md)).

## 7.3 Capture

A capture **batch** handles the stable dirty paths of one repository:

1. **Root guard.** The root, its `.git` and `.pigeon/roster.toml` exist, on
   the device of the start. Otherwise capture and install stop ("Needs you:
   pigeon can't find your folder"); nothing is captured as a deletion.
2. **Changes** come only from `git status --porcelain=v2 -z -uall
   --ignored=traditional`, never from events: `git annex add` stages any
   path given, even in a nested repository (E20). Skipped: paths the install
   journal is installing, held paths (§7.4), ignored names. Only regular
   files (`lstat`) go on; other kinds are *not shared* (§7.9).
3. **Mass-deletion guard.** Deletions of the batch, with those already held,
   beyond 100 files (a package counts as one) or removing a top-level folder
   are all **held** (§7.4); the rest goes on.
4. **Names.** A non-portable name is held
   ([05 §5.4](05-catalogue-and-names.md)).
5. **Classify** by the authority rule under `main`'s roster
   ([04 §4.3](04-roster-and-authority.md)): *allowed*, *not allowed* (per set
   of deciders) or *diverging* (§7.6).
6. **Ingest** with `git -c annex.thin=false annex add --no-check-gitignore`;
   a file changing meanwhile is retried after its next quiet period. Then
   the execute bit is set from `lstat` (`update-index --chmod`; `annex add`
   stages 100644, E20). The object is a copy (reflink where possible).
   Content beyond the pending budget waits (§7.8).
7. **Commit.** Allowed: one signed commit, then `update-ref main` from the
   expected old value. Not allowed: one proposal ref per set of deciders, on
   `main` or the path's pending proposal ([04 §4.5](04-roster-and-authority.md));
   the index keeps `main`'s version; the path becomes a *suggestion* (§7.6).
8. **Trace** each commit and proposal (ADR 0008, [12](12-observability.md)).

A crash anywhere loses nothing: the next scan finds the files and redoes the
batch. INV-Capture (`CaptureFirst`): what install moves away for good is in
a commit, since install touches only clean paths.

## 7.4 Held changes

A **held** change stays on this computer: the file is gone (or misnamed) on
disk, `main` still has it, and install leaves the path alone. It is *local*
with reason `held` ([06 §6.4](06-status.md)) and a *Needs you* item until the
person chooses (ux.md §8.6):

| Held because | Choice | Then |
|---|---|---|
| mass deletion | *Delete them for everyone* / *Bring them back* | captured as usual / install restores them from `main` |
| non-portable name | *Rename to the suggested name* / rename it themselves | pigeon renames it and capture adds it / an ordinary capture |
| a shared file became a link or a repository (§7.9) | *Delete it for everyone* / *Bring it back* | the deletion is captured / pigeon renames the link or folder to a free name beside it, and install restores the file |

## 7.5 Install

Install runs when `main` moves and when a folder is kept again
([08 §8.5](08-custody-loops.md)). It moves each **clean** path *p* of a kept
folder (no change after `update-index --refresh`, not racily clean) from its
index version to `current(p)`; hidden folders change in the index only
(sparse checkout, E15). Other paths are left to capture (§7.6). Capture and
install of a repository never run at once. A package (§7.9) is installed in
one batch once all its paths are clean and none is in use. A path is
deferred `in-use` ([06 §6.4](06-status.md)) while a lock file sits next to
it (`~$name`, `.~lock.name#`, `.#name`, `.name.swp`) or step 5 finds a
writer, and retried at every scan.

Per path, in order (a batch shares its fsyncs and one writer scan):

1. **Content:** `git annex get --key` fetches and verifies the new key;
   without a peer that has it, the path waits ("receiving").
2. **Stage** the new file at `.git/pigeon/stage/<seq>` with its mode,
   fsynced: on thin laptops a hard link to an object with no other name,
   else a copy (as git-annex `linkOrCopy'`). No temporary name enters the
   folder.
3. **Journal** `begin` (path, both keys, the index entry's `(dev, ino, size,
   mtime)`, staged file and inode, aside) in `.git/pigeon/install.ndjson`,
   fsynced ([11](11-local-state.md)).
4. **Move aside:** `rename(p, .git/pigeon/aside/<seq>)`. *p* gone: `skip`.
5. **Writer scan:** no process holds the aside open for writing or maps it
   writable (Linux `/proc/*/fd`, `fdinfo`, `maps`; macOS `libproc`). If some
   processes cannot be inspected, it passes only if the file is not group-
   or other-writable; otherwise `in-use`.
6. **Check:** the aside's `(dev, ino, size, mtime)` are the journalled ones
   and its link count is 1 (2 for the object's inode); a higher count marks
   it *linked* (§7.9). If step 5 or 6 fails, the aside goes back
   (`RENAME_NOREPLACE`, macOS `RENAME_EXCL`): `skip`; if *p* was recreated
   meanwhile, the aside is captured as a recovered version (§7.6). Then
   `proved` is journalled for the batch's passing entries. A creation is
   proved when *p* is free.
7. **Rename in** with `RENAME_NOREPLACE`. If *p* exists, the application's
   file stays (`raced`) and is captured as a concurrent edit (§7.6). Fsync
   both directories.
8. **Index:** `update-index --cacheinfo` without stat, then `--refresh`
   (`git annex restage`): git records the stat only if the file holds the
   new key, so a later save shows as modified. pigeon never writes stat
   data (E19).
9. **Done.** The aside is kept until the first full scan 10 minutes later,
   then `released` if no process writes it and it hashes to the old key (a
   *linked* aside at once); otherwise it is captured as a recovered version.

A deletion skips steps 1, 2 and 7. The journal is truncated once every
entry has ended and its aside is released.

**Recovery** at start, before capture: an entry with `proved` rolls forward
(step 7, `raced` if *p* exists, then step 8); any other rolls back (the
aside goes back if *p* is absent, or is kept and hashed at release if *p* is
another file). Staged files no entry names are removed. INV-Journal is
`CaptureInstall.tla` `Journal` and the `journal` laws
(`recovery_ends_every_entry_and_a_second_does_nothing`,
`a_crash_during_recovery_ends_as_one_recovery`,
`nothing_the_person_wrote_is_lost`).

INV-NoOverwrite (`NoOverwrite`, ADR 0034): pigeon never renames over a name
and moves a file away only to prove it is its own. A write through a handle
opened before step 4 that escapes the scan (another user's process, an
`open` in flight, a macOS mapping before `msync`) lands in the kept aside
and is captured at step 9; after the keep period it is lost. The name is absent for
milliseconds (E19).

## 7.6 Divergences and conflicts (ADRs 0034, 0035)

A **divergence** is a path the person saved, so its file differs from the
catalogue's version and from pigeon's last install. The **divergence table**
(`.git/pigeon/conflicts.json`, ADR 0036) keeps per path: kind, the person's
key and file `stat` at capture, their proposal chain, the catalogue key it
diverged from, the key last installed, state and since when.

| Kind | Starts when | State |
|---|---|---|
| `suggestion` | capture made a proposal for the path | *waiting* ("waiting for Paul") |
| `concurrent` | a merge gave the path to another person's version ([05 §5.3](05-catalogue-and-names.md)), or a save won the name during install (§7.5 step 7) | *conflict* |
| `recovered` | install moved aside content not its own and could not put it back, or a kept aside changed (§7.5 steps 6 and 9) | *conflict* |

*waiting* ends when the proposal is accepted (only the index changes) and
becomes *conflict* when it is rejected or `current(p)` moves: the person is
told **once**, and Home keeps a *Needs you* item ("Paul declined your change
to *tarte.docx*; your version is still on your computer"). A new save of a
diverging path extends the person's proposal chain, or makes a proposal of
their own, so it is replicated without changing the catalogue **[no ADR]**.
Install never writes a diverging path. A divergence changes no safety state:
the person's version is a proposal's content, *protected* once K custodians
hold it ([06 §6.2](06-status.md)).

Resolutions (`pigeon resolve PATH …`, or ux.md §8.8):

| Choice | Effect |
|---|---|
| *Take the group's version* (`--take-group`) | install writes `current(p)` because the person asked; a pending proposal is withdrawn ([04 §4.5](04-roster-and-authority.md)); their version stays in history |
| *Keep mine as a copy* (`--keep-copy DEST`) | the captured key (never the live file) is copied to DEST, in a folder they may change, and captured; then as above |
| *Suggest again* (`--suggest-again`) | a new proposal from their file on the current `main`: *waiting* |
| *Keep mine*, *Keep Paul's*, *Keep both* | the signed changes of [05 §5.3](05-catalogue-and-names.md) |

*Take the group's version* runs install with step 6 checking the person's
key and `stat` from the table. A save after the choice fails the check (a
racily clean one defers it to the next capture): nothing is replaced, the
save extends their proposal, and they are told once.

**The group's version** (ADR 0035): *Open Paul's version* or `pigeon view
PATH --group` (`--version N`) writes a read-only copy outside the folder, in
pigeon's cache, named `tarte (Paul's version).docx` in the reader's
language, and opens it; it goes at resolution or after seven days.

## 7.7 What the models must show

`models/tla/CaptureInstall.tla` ([13 §13.2](13-verification.md)) models
every kind of save (in place, replace, create, delete, through an earlier
handle or mapping), each step of §7.3–§7.5, crashes and recovery. Invariants:
`NoLoss` (every saved content is captured, at a name pigeon keeps, or
discarded by the person), `Journal`, `IndexHonest` (E19), `CaptureFirst`,
`NoOverwrite`, `DeletionKept`; liveness `SavesCaptured`, `Converges`. Each
safeguard has a failing configuration in `models/tla/expected.tsv`; E19 runs
the steps against git-annex with an adversary at each boundary (ADR 0008).

## 7.8 Pending history and disk use

A version captured on a laptop stays there as an immutable copy until it is
*protected* ([06 §6.2](06-status.md)), accepted or suggested: its **pending
history**.

- **Budget:** `pending_max` (10 % of the file system) and
  `annex.diskreserve` (max(1 GB, 1 %), [08 §8.9](08-custody-loops.md)),
  which pigeon checks itself. Beyond either, capture of new content pauses
  (`history-full`, [06 §6.4](06-status.md)); deletions and renames go on.
- **Once protected**, old versions are dropped by key (verified,
  [08 §8.3](08-custody-loops.md)), never a key a kept path still has
  ([08 §8.5](08-custody-loops.md) step 4; `Evict.tla` `NoPointer`). Without
  copy-on-write a file at the protected version is re-thinned by installing
  the same key.
- **Disk use** is about one copy of what is kept, plus the pending history
  (changed blocks only on APFS and btrfs), kept asides and metadata (E18).

Content received from others is a hard link (thin): an in-place save may
destroy it, so it never counts toward K; its origin keeps an immutable copy.

## 7.9 What the folder may hold

pigeon shares **regular files**: their bytes and the owner's execute bit.
Folders exist through their files. Everything else is listed, so that nobody
takes an incomplete capture for a safe one.

| Object | What pigeon does |
|---|---|
| regular file | shared (`SHA256E`, execute bit); installed 0644 or 0755 under the umask at install time |
| other mode bits, owner, times, ACLs, extended attributes, resource forks, Finder tags | not shared; an install writes a new file without them |
| hard links | each name is its own file; installing one breaks the link and the other keeps its content (§7.5 step 6) |
| empty folder | not shared, not listed; install `rmdir`s a folder it emptied, which fails harmlessly if a person added something |
| symbolic link (`link`), folder holding `.git` (`repository`, with all it holds), live library (`library`: `.photoslibrary`, `.musiclibrary`, `.tvlibrary`, `.imovielibrary`, `.fcpbundle`, `.sparsebundle`), unreadable file or folder (`unreadable`, until readable; never a deletion) | *not shared*, never followed; listed, notified once |
| FIFO, socket, device | not shared, not listed (no data) |
| ignored name (§7.2) | not captured, not listed |
| non-portable name | held, *Needs you* ([05 §5.4](05-catalogue-and-names.md)) |
| package: `.app`, `.band`, `.bundle`, `.framework`, `.key`, `.logicx`, `.numbers`, `.pages`, `.playground`, `.rtfd`, `.scriv`, `.textbundle`, `.xcodeproj`, `.xcworkspace` | one document: captured together (§7.2), merged as one slot ([05 §5.2](05-catalogue-and-names.md)), installed in one batch (§7.5), one divergence entry |

`package(p)`, the outermost folder above *p* with such an extension
(case-insensitive), is a pure function of the name. Outside packages, files
saved together share a commit only if they settle in one batch. *Not shared*
items show in the folder view, in `pigeon status --not-shared`, and in a
*Safe* headline ("2 items of your folder aren't shared", [06 §6.5](06-status.md)).

The accept rule ([04 §4.4](04-roster-and-authority.md) rule 6) allows only
folders and files of mode 100644 or 100755: `SHA256E` pointers, except plain
blobs in `.pigeon/` and the root `.gitattributes`. git and git-annex run
with pigeon's configuration only ([03 §3.7](03-repository-and-roles.md)):
global settings and in-tree `.gitattributes` or `.gitignore` cannot change
keys, trees or listings.
