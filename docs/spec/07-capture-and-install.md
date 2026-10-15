# 7. Capture and install: the disk door

> Mission: the only two ways pigeon meets the shared folder on a laptop.
> **Capture** turns saves into signed commits; **install** writes what the
> group accepted (P2, part of P3). Custodians run neither.

Normative: `models/tla/CaptureInstall.tla` (the actions and invariants named
below) and the `pigeon-core::journal` laws.

## 7.1 Principles

- People write the folder; pigeon reads it at capture and writes it only at
  install. The git index equals `main`'s tree, with the stat pigeon last
  left at each path; a path that differs is a save (§7.3) or a divergence
  (§7.6).
- pigeon never replaces, moves aside or removes what a person saved unless
  they ask (ADR 0034); install moves a file away only to prove it is its
  own. A crash leaves each path at its old or new content (ADR 0036).

## 7.2 Noticing changes (ADRs 0013, 0037)

- Watcher events are hints; a full scan at start, every 10 minutes and on
  watcher overflow is the truth.
- A dirty path is captured after its **quiet period** (2 s plus 1 s per
  100 MB, at most 60 s) and two equal stats (size, mtime, inode); more than
  6 captures in an hour slow it to one per 10 minutes. A package (§7.9) is
  quiet when all its files are, and is captured together.
- The **ignore list** (§7.9) is fixed in the core, matched per path
  component, and applies only to names not in the catalogue; an ignored name
  is never captured as a deletion. In-tree `.gitignore` has no effect
  ([03 §3.7](03-repository-and-roles.md)); `.pigeon/ignore` is open
  ([16](16-open-questions.md)).

## 7.3 Capture

A capture **batch** (`Capture`) handles the stable dirty paths of one
repository, in this order:

| # | Step | Rule |
|---|---|---|
| 1 | root guard | the root, its `.git` and `.pigeon/roster.toml` exist on the device of the start; otherwise capture and install stop (*Needs you*: "pigeon can't find your folder") and nothing becomes a deletion |
| 2 | changes | from `git status` alone, never from events (E20); skipped: paths install is installing, held paths (§7.4), ignored names; only regular files go on, other kinds are *not shared* (§7.9) |
| 3 | mass deletion | deletions beyond 100 files with those already held (a package counts as one), or removing a top-level folder, are all **held** (§7.4) |
| 4 | names | a non-portable name is held ([05 §5.4](05-catalogue-and-names.md)) |
| 5 | classify | under `main`'s roster ([04 §4.3](04-roster-and-authority.md)): *allowed*, *not allowed* (per set of deciders) or *diverging* (§7.6) |
| 6 | ingest | a copy (reflink where possible) into the annex, with the execute bit from `lstat` (E20); a file changing meanwhile waits for its next quiet period; content beyond the pending budget waits (§7.8) |
| 7 | commit | allowed paths: one signed commit, `main` moved from its expected old value, with a `Pigeon-Renamed: "old" "new"` trailer for each deleted file paired with a new one by content, else, up to 1 MiB, by git's similarity estimate (ADR 0072, [05 §5.2](05-catalogue-and-names.md)); the others: one proposal per set of deciders ([04 §4.5](04-roster-and-authority.md)), the index keeps `main`'s version, the path becomes a *suggestion* (§7.6) |
| 8 | trace | each commit and proposal (ADR 0008, [12](12-observability.md)) |

A crash anywhere loses nothing: the next scan redoes the batch. Before it
stages a path, capture records each path's index entry in `capture.json`
([11 §11.3](11-local-state.md)), and a capture that finds the file puts
them back first **[no ADR]**: the index alone cannot tell a save staged
before a crash from a version install has not written yet.
`CaptureFirst`: what install moves away for good is already in a commit.

## 7.4 Held changes

A **held** change stays on this computer: the file is gone (or misnamed),
`main` still has it, install leaves the path alone; it is *local* with
reason `held` ([06 §6.4](06-status.md)) and a *Needs you* item until the
person chooses (ux.md §8.6):

| Held because | Choice | Then |
|---|---|---|
| mass deletion | *Delete them for everyone* / *Bring them back* | captured as usual / install restores them from `main` |
| non-portable name | *Rename to the suggested name* / rename it themselves | pigeon renames it and capture adds it / an ordinary capture |
| a shared file became a link or a repository (§7.9) | *Delete it for everyone* / *Bring it back* | the deletion is captured / pigeon renames the link or folder to a free name beside it, and install restores the file |

Coded so far (`pigeon-core::held`, laws in `tests/held.rs`;
`pigeond/src/held.rs`, `held_file.rs`, `tests/held.rs`): mass deletions.
A deletion paired by content with a new path of the same round is a rename
and goes through; the others of the round are held together when, with
those already held, they are more than 100, or when one is in a top-level
folder gone from the disk. They wait in `.git/pigeon/held.json`
([11 §11.3](11-local-state.md)); the headline is *Needs you*. `pigeon held`
lists the batches; `held ID --share` releases one, deleted at the next
capture whatever its size; `held ID --bring-back` takes its paths out of
the index, so that install writes `main`'s versions again. A held file
that comes back on its own leaves its batch. Not coded: a package counting
as one, held names and links, and the web's buttons.

## 7.5 Install

Install runs when `main` moves and when a folder is kept again
([08 §8.5](08-custody-loops.md)). It moves each **clean** path *p* of a kept
folder (unchanged since the index, not racily clean) from its index version
to `current(p)`; hidden folders change in the index only (E15). Capture and
install of a repository never run at once. A package is installed in one
batch once all its paths are clean and none is in use. A path is deferred
`in-use` ([06 §6.4](06-status.md)) while a lock file sits beside it (`~$name`,
`.~lock.name#`, `.#name`, `.name.swp`) or step 5 finds a writer, and is
retried at every scan. A batch shares its fsyncs and one writer scan.

| # | Step | Rule | Model |
|---|---|---|---|
| 1 | content | fetch and verify the new key; without a peer that has it, the path waits ("receiving") | `Start` |
| 2 | stage | the new file with its mode under `.git/pigeon/stage/`, fsynced: a hard link to an object with no other name on thin laptops, else a copy; no temporary name enters the folder | `Start` |
| 3 | journal | `begin` (both keys, the index entry's identity and stat, staged file, aside), fsynced ([11 §11.3](11-local-state.md)) | `Start` |
| 4 | move aside | *p* into `.git/pigeon/aside/`; *p* gone: skip | `MoveAside` |
| 5 | writer scan | no process holds the aside open for writing or maps it writable; if some cannot be inspected, it passes only if the file is not group- or other-writable; otherwise `in-use` | `Scan` |
| 6 | check | the aside's identity and stat are the journalled ones, link count 1 (2 for an object's inode; higher marks it *linked*, §7.9); if 5 or 6 fails, the aside goes back without replacing anything (skip), or is captured as a recovered version (§7.6) if *p* was recreated; then `proved` is journalled; a creation is proved when *p* is free | `Check` |
| 7 | rename in | without replacing; if *p* exists, the application's file stays (`raced`) and is captured as a concurrent edit (§7.6); fsync both directories | `RenameIn` |
| 8 | index | git records the stat only if the file holds the new key, so a later save shows as modified; pigeon never writes stat data (E19) | `Index`, `IndexHonest` |
| 9 | done | the aside is kept until the first full scan 10 minutes later, then released if no process writes it and it hashes to the old key (a *linked* aside at once), else captured as a recovered version | `Done`, `Release` |

A deletion skips steps 1, 2 and 7. The journal is truncated once every
entry has ended and its aside is released.

The driver (`pigeond::asides`) applies this release pass in ordinary rounds
and after recovery. A changed aside is snapshotted into the annex and
captured under a deterministic recovered conflict name; the ordinary
authority split makes it an accepted contribution or a proposal. Its
preserved file is never renamed over another recovery's file. All capture
and recovery failures leave their intent or journal entry pending.

**Recovery** (`Recover`, `RecoverEnd`) runs at start, before capture, and
again at the start of a round after a step failed (a git call that found
`index.lock`, say), which leaves its entry as a crash would: capture,
install, free, keep and re-thinning wait while an entry has no end, so a
file moved aside never reads as the person's deletion. An
entry with `proved` rolls forward (step 7, `raced` if *p* exists, then step
8); any other rolls back (the aside goes back if *p* is absent, or is kept
and hashed at release if *p* is another file). Staged files no entry names
are removed. It satisfies `Journal` and the `journal` laws
(`recovery_ends_every_entry_and_a_second_does_nothing`,
`a_crash_during_recovery_ends_as_one_recovery`,
`nothing_the_person_wrote_is_lost`).

`NoOverwrite` (ADR 0034): pigeon never renames over a name. A write through
a handle opened before step 4 that escapes the scan lands in the kept aside
and is captured at step 9; after the keep period it is lost. The name is
absent for milliseconds (E19).

**Saves soon after an install** (ADR 0071): an application that kept a
file's old content saves it over the new version. So capture puts a save
whose modification time is within 10 seconds after install wrote another
version of that path (2 seconds before it allowed) beside the installed
version: at its first free conflict name ([05 §5.3](05-catalogue-and-names.md)),
with the saver's name and the time of the save, when the person may add
that name; the installed version keeps the path, and the laptop's file is a
`concurrent` divergence (§7.6), never overwritten. Install keeps when it
wrote each path in memory, so a restart forgets them. `pigeond/tests/install.rs`
checks it (`a_save_soon_after_an_install_is_kept_beside_it`).

## 7.6 Divergences and conflicts (ADRs 0034, 0035)

A **divergence** is a path the person saved, so its file differs from the
catalogue's version and from pigeon's last install. The **divergence table**
(`conflicts.json`, [11 §11.3](11-local-state.md)) records each one.

| Kind | Starts when | State |
|---|---|---|
| `suggestion` | capture made a proposal for the path | *waiting* ("waiting for Paul") |
| `concurrent` | a merge gave the path to another person's version ([05 §5.3](05-catalogue-and-names.md)), a save won the name at §7.5 step 7, or a save soon after an install went beside it (§7.5, ADR 0071) | *conflict* |
| `recovered` | install moved aside content not its own and could not put it back, or a kept aside changed (§7.5 steps 6 and 9) | *conflict* |

*waiting* ends when the proposal is accepted (only the index changes,
`Decide`) and becomes *conflict* when it is rejected or `current(p)` moves:
the person is told **once**, and Home keeps a *Needs you* item. It also
ends when the file is clean again, the person having put a version back;
the proposal stays for its deciders **[no ADR]**. Capture leaves a
suggested path alone while its file keeps the stat the table recorded. A new save
of a diverging path extends the person's proposal chain, or makes a proposal
of their own, so it is replicated without changing the catalogue
**[no ADR]**. Install never writes a diverging path. A divergence changes no
safety state: the person's version is a proposal's content, *protected* once
K custodians hold it ([06 §6.2](06-status.md)).

Resolutions (`pigeon resolve PATH …`, or ux.md §8.8; the model's
`TakeGroup` and `ResolveMine`):

| Choice | Effect |
|---|---|
| *Take the group's version* (`--take-group`) | install writes `current(p)`, step 6 checking the person's key and stat from the table; a pending proposal is withdrawn ([04 §4.5](04-roster-and-authority.md)); their version stays in history |
| *Keep mine as a copy* (`--keep-copy DEST`) | the captured key (never the live file) is copied to DEST, in a folder they may change, and captured; then as above |
| *Suggest again* (`--suggest-again`) | a new proposal from their file on the current `main`: *waiting* |
| *Keep mine*, *Keep Paul's*, *Keep both* | the signed changes of [05 §5.3](05-catalogue-and-names.md) |

A save after the choice fails the check: nothing is replaced
(`changed-since`), the save extends their proposal, and they are told
once. `pigeon conflicts` lists the table; `pigeond::resolve` codes the
first three choices (`tests/resolve.rs`), the withdrawal being the decision
`withdrawn` signed by the author's device. *Open Paul's version* and
`pigeon view PATH --group` (`--version N`) show the group's version as
ADR 0035 says: read-only, outside the folder, for seven days at most.

## 7.7 What the models must show

`CaptureInstall.tla` ([13 §13.2](13-verification.md)) models every kind of
save (in place, replace, create, delete, through an earlier handle or
mapping), each step of §7.3–§7.5, crashes and recovery, and checks `NoLoss`
(every saved content is captured, at a name pigeon keeps, or discarded by
the person), `Journal`, `IndexHonest`, `CaptureFirst`, `NoOverwrite`,
`DeletionKept`, `SavesCaptured` and `Converges`. Each safeguard has a
failing configuration in `models/tla/expected.tsv`; E19 runs the steps
against git-annex with an adversary at each boundary (ADR 0008).

## 7.8 Pending history and disk use

A version captured on a laptop stays there as an immutable copy, its
**pending history**, until it is *protected* ([06 §6.2](06-status.md)),
accepted or suggested. Its budget is `pending_max` (10 % of the file system)
and `annex.diskreserve` (max(1 GB, 1 %), [08 §8.9](08-custody-loops.md)),
which pigeon checks itself; beyond either, capture of new content pauses
(`history-full`, [06 §6.4](06-status.md)), while deletions and renames go on.
Once protected, old versions are dropped by key (verified,
[08 §8.3](08-custody-loops.md)), never a key a kept path still has
(`Evict.tla` `NoPointer`); without copy-on-write, a file at the protected
version is re-thinned by installing the same key. Disk use is about one copy
of what is kept, plus pending history (changed blocks only on APFS and
btrfs), kept asides and metadata (E18). Content received from others is a
hard link (thin): an in-place save may destroy it, so it never counts
toward K; its origin keeps an immutable copy.

## 7.9 What the folder may hold

pigeon shares **regular files**: their bytes and the owner's execute bit.
Folders exist through their files. Everything else is listed, so that
nobody takes an incomplete capture for a safe one.

| Object | What pigeon does |
|---|---|
| regular file | shared (`SHA256E`, execute bit); installed 0644 or 0755 under the umask at install time |
| other mode bits, owner, times, ACLs, extended attributes, resource forks, Finder tags | not shared; an install writes a new file without them |
| hard links | each name is its own file; installing one breaks the link and the other keeps its content (§7.5 step 6) |
| empty folder | not shared, not listed; install removes a folder it emptied, which fails harmlessly if a person added something |
| symbolic link (`link`), folder holding `.git` (`repository`, with all it holds), live library (`library`: `.photoslibrary`, `.musiclibrary`, `.tvlibrary`, `.imovielibrary`, `.fcpbundle`, `.sparsebundle`), unreadable file or folder (`unreadable`, until readable; never a deletion) | *not shared*, never followed; listed, notified once |
| FIFO, socket, device | not shared, not listed (no data) |
| ignored name (§7.2): `.DS_Store`, `._*`, `Icon\r`, `.localized`, `.Spotlight-V100`, `.Trashes`, `.fseventsd`, `.TemporaryItems`, `.DocumentRevisions-V100`, `.Trash-*`, `Thumbs.db`, `desktop.ini`, `~$*`, `.~lock.*#`, `~*.tmp`, `*.swp`, `.goutputstream-*`, `*.part`, `*.crdownload`, pigeon's temporary names | not captured, not listed |
| non-portable name | held, *Needs you* ([05 §5.4](05-catalogue-and-names.md)) |
| package: `.app`, `.band`, `.bundle`, `.framework`, `.key`, `.logicx`, `.numbers`, `.pages`, `.playground`, `.rtfd`, `.scriv`, `.textbundle`, `.xcodeproj`, `.xcworkspace` | one document: captured together (§7.2), merged as one slot ([05 §5.2](05-catalogue-and-names.md)), installed in one batch (§7.5), one divergence entry |

`package(p)` is the outermost folder above *p* with such an extension
(case-insensitive), a pure function of the name. Outside packages, files
saved together share a commit only if they settle in one batch. *Not
shared* items show in the folder view, in `pigeon status --not-shared` and
in a *Safe* headline ([06 §6.5](06-status.md)).

The accept rule ([04 §4.4](04-roster-and-authority.md) rule 6) allows only
folders and files of mode 100644 or 100755 that are `SHA256E` pointers,
except plain blobs in `.pigeon/` and the root `.gitattributes`. git and
git-annex run with pigeon's configuration only
([03 §3.7](03-repository-and-roles.md)): global settings and in-tree
`.gitattributes` or `.gitignore` cannot change keys, trees or listings.
