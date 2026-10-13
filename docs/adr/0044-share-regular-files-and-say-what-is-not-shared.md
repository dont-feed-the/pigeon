# 0044. pigeon shares regular files and their executable bit, and says what it does not share

- **Status:** Proposed, 14 October 2026, by the agent, for issue #14
  (Guillaume's review: "Define the supported filesystem object and metadata
  model"), on the evidence of experiment E20 and of the git-annex
  10.20261006 source. Refines the install check of ADR 0040 (Proposed).

## Context

A shared folder holds more than byte files: links, empty folders, permissions,
extended attributes, macOS packages, files of other programs. The
specification said only that empty folders, symbolic links, permissions other
than the executable bit and extended attributes "are not versioned in v1".
People could not tell what is kept, changed, skipped or refused. git and
git-annex decide some of it silently:

- `git status` does not list FIFOs, sockets or devices, and `git annex add`
  skips them without a message (`Command/Add.hs`, `start`: not a regular file
  and not a link, then `stop`).
- `git annex add` stages a symbolic link as a git link (mode 120000). A link
  whose target is in `.git/annex/objects` is taken for a locked annexed file
  (`Command/Add.hs`, `fixuplink`).
- A directory holding `.git` is shown as one entry; git never lists its files.
- git keeps one permission bit, the owner's execute bit (`Command/Add.hs`,
  `addFile`; `Annex/Content/LowLevel.hs`: "git doesn't support file modes
  beyond execute"). Yet `git annex add` stages an unlocked executable file as
  100644, and does not stage a change of mode alone (E20).
  `ingestAdd'` (`Annex/Ingest.hs`) reads the mode of a content file that
  `cleanCruft` has already removed, so `stagePointerFile` (`Annex/Link.hs`)
  gets no mode. `git update-index --chmod=+x` sets the bit on the pointer.
- `git annex add` stages a path it is given even when `git status` never lists
  it, such as a file inside a nested repository (E20).
- With `annex.thin`, git-annex hard-links an object to a work-tree file only
  if the object has no other name, and copies otherwise
  (`Annex/Content/LowLevel.hs`, `linkOrCopy'`). The specification's install
  hard-linked the object to every path of that key on thin laptops: an
  in-place save of one path would then change the other path and the object.
- Nothing checked what a peer's commit holds: a link, a submodule entry, a
  plain blob, or a pointer to a key that cannot be verified (`WORM`).
- A macOS package (*Rapport.pages*) is a folder that people see as one
  document. Merging it file by file can produce a mix that no application
  ever saved.

Options were: full fidelity (links, modes, attributes, packages as archives),
which git does not carry and which Windows could not follow later; refusing
everything outside plain files with an error, which blocks folders people
already have; or a small declared subset that says what it leaves out.
The issue accepts "an explicit, small supported subset".

## Decision

- **Shared:** regular files, by their bytes and the owner's execute bit.
  Folders exist through their files. Nothing else of a file is shared or
  kept: other mode bits, owner, times, ACLs, extended attributes, resource
  forks, Finder tags. Install writes files with mode 0644 or 0755 under the
  umask, at the time of the install.
- **A valid tree** (rule 6 of the authority rule, spec 04 §4.4). It holds
  only folders and files of mode 100644 or 100755. Outside `.pigeon/` and the
  root `.gitattributes`, every file is a git-annex pointer to a `SHA256E` key
  with its size. `.pigeon/` and the root `.gitattributes` hold plain blobs. A
  commit that breaks rule 6 is malformed: it is neither accepted nor kept as a
  proposal, and its keys are not references (ADR 0042). Force does not waive
  it. Install therefore never meets a link or a key it cannot verify.
- **The execute bit is staged by capture.** After `git annex add`, capture sets
  each added path's bit with `git update-index --chmod=+x|-x`, from its
  `lstat`. It does the same for a path whose only change is its mode. The fix
  of `ingestAdd'` is offered upstream (ADR 0004).
- **Not shared, and said so.** Capture takes its paths from `git status` only,
  never from watcher events, and checks each one with `lstat` before
  `git annex add`; only regular files reach git-annex, and pigeon never
  follows a link, so it never reads outside the folder. These kinds stay on
  this computer. They are listed under *Not shared* in the folder view and in
  `pigeon status --not-shared`, and they are never captured as deletions:
  - `link`: a symbolic link, whatever its target;
  - `repository`: a folder holding `.git`, with everything in it;
  - `library`: a package that is a live database: `.photoslibrary`,
    `.musiclibrary`, `.tvlibrary`, `.imovielibrary`, `.fcpbundle`,
    `.sparsebundle`;
  - `unreadable`: a file or folder pigeon may not read; it is neither
    captured nor installed until it is readable again.

  The first time a path of these kinds is seen, a calm notification names it
  (ADR 0029). The folder row says "2 items not shared", and a *Safe* headline
  adds "2 items of your folder aren't shared", so nobody reads *Safe* as "all
  of it". FIFOs, sockets, devices, empty folders and ignored names hold no
  data and are not listed.
- **A shared file that becomes one of these kinds** (replaced by a link, for
  instance) is a held change (spec 07 §7.4): *Delete it for everyone*, or
  *Bring it back*.
- **Hard links are not shared.** Each name is shared as its own file. Install
  accepts a file with more links than its own: one, or two when the file is
  the object's inode on a thin laptop. It does so when everything else
  matches and no writer is found. The aside of such a file is released by
  unlinking, without hashing, because its content lives on under its other
  name. On thin laptops, install hard-links an object to at most one path, as
  git-annex does; every other path of that key gets a copy, a reflink where
  the file system can.
- **Packages are one document.** `package(p)` is the outermost folder above
  *p* whose name ends, compared case-insensitively, with `.app`, `.band`,
  `.bundle`, `.framework`, `.key`, `.logicx`, `.numbers`, `.pages`,
  `.playground`, `.rtfd`, `.scriv`, `.textbundle`, `.xcodeproj` or
  `.xcworkspace`. It is a pure function of the name, so Linux and macOS agree.
  - Capture waits until no file of a package has changed for the quiet
    period, then puts all its changes in one commit or proposal.
  - The mass-deletion guard counts a package as one file.
  - The merge treats a package as one path: concurrent edits keep each side
    whole, the second under the package's conflict name
    (*Rapport (Paul, 2026-10-13 14h02).pages*). A modification survives a
    concurrent deletion of the whole package.
  - Install writes a package in one batch, and only once all its paths are
    clean and none is in use. A reader may see a mix for the duration of the
    batch; no atomic install is promised. The divergence table keeps one
    entry per package.
- **No other multi-file consistency.** Files saved together outside a package
  share a commit only if they settle within one capture batch.
- **Names.** The rules of spec 05 §5.4 hold, plus valid UTF-8. The Unicode
  tables used for NFC and case folding are pinned in the core, and changing
  them is a format change (issue #15). Because every name of the catalogue is
  portable, every supported platform can represent it, and no mapping is
  needed in v1. The Windows mapping stays an open question.
- **Ignore list.** A fixed list in the core, the same on every device of a
  format version, matched on each path component. It applies to names not in
  the catalogue, as in git. An ignored name is never captured, so never
  captured as deleted. A catalogue path is captured and installed whatever its
  name. The list adds `Icon\r` (macOS folder icons), `.localized`,
  `.Spotlight-V100`, `.Trashes`, `.fseventsd`, `.TemporaryItems`,
  `.DocumentRevisions-V100`, `.Trash-*`, `~*.tmp` and `.goutputstream-*`.
  Other short-lived temporary names never outlast the quiet period.
- **A file that changes while it is added** is refused by git-annex, which
  compares the file's inode cache before and after hashing and copying
  (`Command/Add.hs`, `changed`; `Annex/Content.hs`, `checksrcunchanged`).
  Capture retries it after its next quiet period. A captured object always
  hashes to its key (E20).

## Consequences

- People can read, in one table of spec 07 §7.9, what is kept, changed,
  skipped or refused.
- A folder full of links or a project with its own `.git` is not backed up
  by pigeon. It says so once and keeps a line in the folder view, but it does
  not nag.
- Finder tags, colours and other attributes are lost on the devices that
  receive a file, and on the saving device once a newer version is
  installed. Old Mac files whose content lives in a resource fork arrive
  empty of it.
- Hard links inside the folder break at the first install of either name;
  nothing is lost.
- Two people editing one package keep two whole packages, never a mix. A
  document format that spreads one save over several folders outside a
  package gets no such care.
- Rule 6 and the package rule are pure functions of the tree, with laws
  (ADR 0012). Every tree that capture makes passes rule 6, and `package` is
  stable under renaming of other paths.
- Spec 07 §7.5 changes in steps 2, 6 and 9, and ADR 0040's link-count check
  reads as above.
- On a thin laptop, `git annex fsck` does not notice an object changed through
  its work-tree link: `checkBackend` (`Command/Fsck.hs`) skips the check for
  an unlocked thin file on purpose (E20). The one-link rule above is
  therefore what protects the object. A laptop's copies never count (ADR
  0002), and a custodian verifies what it receives.
- **Evidence:** E20 (`lab/git-annex/ga-e20-filesystem-objects.py`) runs on
  Linux and on macOS in CI (`lab.yml`), 53 checks each.
