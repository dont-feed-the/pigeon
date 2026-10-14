# 5. The catalogue and its names

> Mission: define the logical catalogue that every device computes from accepted
> history, the deterministic merge that keeps it identical everywhere (P5), the
> names of concurrent versions, and the portable-name rules.

## 5.1 The logical catalogue

The **logical catalogue** is a pure function of a set of accepted commits
(ADR 0012), never of their arrival order nor of the merge commits that joined
them (ADR 0052). It gives:

| Function | Value |
|---|---|
| `current(p)` | the key at path *p* in the tree of the catalogue, or none |
| `versions(p)` | the contributions that wrote *p*: commit, key or deletion, person, device, recorded time |
| `deleted()` | the paths whose last change is a deletion, with their last key (Deleted files, ux.md §6) |
| `conflicts()` | the open conflicts and their versions (§5.3) |
| `proposals()` | the proposals and their states ([04 §4.5](04-roster-and-authority.md)) |
| `keys()` | every key that an accepted or proposal commit introduces at a path (a *reference*) and that no purge released: what custodians keep ([08 §8.6](08-custody-loops.md), ADR 0042) |

**Contributions.** The **base** `B(c)` of an accepted commit *c* is the
catalogue of its accepted ancestors, and `Δ(c) = diff(B(c), tree(c))`
([04 §4.4](04-roster-and-authority.md)). *c* is a **contribution** when Δ(c)
is not empty, or when it carries the trailer `Pigeon-Resolved: <path>`
(*Keep both*, §5.3). Four kinds of record are kept apart:

| Record | Δ | In the catalogue |
|---|---|---|
| a save: capture, restore, resolution, Force | the person's changes | a contribution |
| a clean merge, written by `pigeond` (§5.2) | ∅ | not a contribution: it joins histories; which device wrote it, and how many did, never matters |
| a decision merge (adoption, [04 §4.5](04-roster-and-authority.md)) | the adopted change | a contribution of the decider; the proposal stays outside |
| a proposal; a local divergence ([07 §7.6](07-capture-and-install.md)) | — | nothing: a proposal is not accepted, and a divergence is no commit until capture makes one |

**Slots.** Paths are compared as **slots**: a path after portable folding
(§5.4), so `Photo.jpg` and `photo.jpg` are one slot. A package (ADR 0044,
[07 §7.9](07-capture-and-install.md)) is one slot whose value is its whole
subtree; a change of any file inside is a change of the package. The value of
a slot in a tree is an exact name with its content (a key, or a package's
files), or nothing. A contribution **writes** the slots of its Δ.

**Restoring** a version (ux.md §8.7) or a deleted file (§8.6) is a new change
that sets *p* to the old key; it is checked by the authority rule like any change
(restoring a deleted file of an add-only folder is an *add*; restoring in
someone else's folder is a proposal). Nothing is re-transferred: the key is
already known to custodians, unless a purge erased it ([08 §8.6](08-custody-loops.md)).

## 5.2 The deterministic merge (ADR 0052)

`catalogue(S)`, for an ancestor-closed set S of accepted commits, is built
from **events**: the contributions of S and the conflict events they froze.

1. **Order.** A contribution comes after its ancestors and after the conflict
   events it froze; a conflict event comes after the heads it froze.
2. **Heads.** The heads of a slot are the events that wrote it and that no
   later writer of the same slot follows.
3. **Versions.** Each distinct content among the heads' values is one
   **version**. A deletion is no version, so **a modification survives a
   concurrent deletion** (P3: nothing a person saved disappears by a race).
   One content under names that differ only after folding is one version.
4. **Representatives.** A version's representative is its earliest head, by
   (recorded author time, device ID, number of that device's earlier
   contributions, commit ID). A device's own contributions follow each
   other, so the commit ID decides only for a faulty device. A conflict event
   keeps the representatives of the versions it froze; an adoption counts
   with its proposal's author and time.
5. **Placement.** All versions are placed in representative order:
   - a folder takes the spelling of its first version, so `Straße/` and
     `STRASSE/` created concurrently are one folder;
   - first, each slot's first version takes its own name, unless it is a
     file where a folder of the same folded name is;
   - then each other version takes its **conflict name** (§5.3), the first
     that is free after folding.

   The tree of the catalogue is every version at its place.
6. **Freezing.** A slot is **unsettled** when it has several versions, or a
   version shown under another name. Every contribution *c* freezes each
   unsettled slot of its base into a **conflict event**. The event writes
   each version of the slot where B(c) showed it, and nothing at the slot
   itself when no version stayed there. Two contributions that froze the same
   heads at the same names froze the same event.

Freezing keeps what a person saw. A later save of *p* replaces *p* and leaves
in place the copies it saw; *Keep mine* deletes them knowingly. Without it, a
copy shown in a save's tree would leave the catalogue of that very save as
soon as the save is the only head.

| Case | Result |
|---|---|
| edit and edit, add and add | the first representative keeps *p*; each other version goes to its conflict name |
| edit and delete | the edit stays at *p*; the deletion is in `versions(p)` |
| rename and edit | renames are not tracked: a rename is a deletion and an add. A renames *p* to *q* while B edits *p*: *p* holds B's edit, *q* holds A's file. A concurrent add of *q* conflicts with A's at *q* |
| case or Unicode collision | `Photo.jpg` and `photo.jpg`, or `Straße.txt` and `STRASSE.txt`, added concurrently are one slot: the later representative goes to `photo (Bob, 2026-10-13 14h02).jpg`, and one content under both names is one version. A file named like a concurrent folder goes to its conflict name |
| package | two edits keep two whole packages, the later under the package's conflict name, never a mix; an edit survives a deletion of the whole package |
| a save after a conflict | it replaces *p*; the copies it saw stay |
| *Keep mine* racing an edit of *p* | both write *p*: a new conflict, both kept |
| *Keep both* racing an edit of a copy | the edited copy stays; the conflict is closed |
| decision merge | a contribution of the decider. If *p* changed meanwhile, the decider's tree keeps both: the proposal's version under its conflict name, with the proposal's author and time |
| `.pigeon/` | never a conflict name: the roster is a fold (below); for another file, the first version keeps the slot and the others stay in history |

**Laws** (ADR 0012). The reference model `models/catalogue/catalogue.py`
checks them on every reachable state of three devices, for each case above
([13 §13.4](13-verification.md)):

- the catalogue depends only on the contributions and on which saw which,
  never on the order of contacts, on duplicate deliveries or on the merge
  commits; merging is commutative, associative and idempotent;
- `tree(c) = catalogue(anc*(c))` for every accepted commit *c*;
- a key leaves the catalogue only through a contribution whose base showed it;
- a version that none of the latest contributions holds never comes back;
- every catalogue is portable, so conflict names are injective after folding;
- every copy is a name that its representative's person may add.

Time only orders: a wrong clock changes which version keeps the name, never
whether a version is kept.

**Merge commits.** The tree of a merge commit *M* is `catalogue(anc*(M))`.
`pigeond` computes it from the history and writes it with git's plumbing;
`git merge-tree --write-tree` may serve as a fast path, whose result pigeon
checks (ADR 0010). Two devices that merge the same heads write different
commits with the same tree, and neither counts. To avoid an endless exchange
of such merges, a device merges a peer's tip only if it brings a contribution
that this device lacks; otherwise it records the tip as equivalent (ADR
0052). Devices' `main` branches may therefore differ by merge commits only,
which changes nothing in the logical catalogue (P5).

**Authority of merges** (ADR 0052). A clean merge changes nothing, so a
device of any parent's roster may sign it, whoever made the changes it joins
([04 §4.4](04-roster-and-authority.md)). A conflict event is not a commit and
has no signature. Each copy it writes is the version of its representative,
at a new name in the same folder, and whoever may write a path of a folder
may add to it ([04 §4.3](04-roster-and-authority.md)). A decision merge is
judged as the decider's change, with the copy it writes.

**The roster.** `.pigeon/roster.toml` is not merged by heads. Its value in a
merged tree is the fold of the roster changes of the accepted history
([04 §4.8](04-roster-and-authority.md), ADR 0051): concurrent changes that
conflict are skipped and shown to the admins, and device removals always
apply.

## 5.3 Conflict names

The conflict name of a version at path `dir/stem.ext` is

```text
dir/stem (Person, 2026-10-13 14h02).ext
```

- *Person* is the display name of the representative's person, made portable
  (§5.4); the time is its recorded author time, in the time-zone offset
  recorded in that commit, so every device computes the same name.
- If that name is taken after folding, by a version placed before it or by a
  folder, ` 2`, ` 3`, … are appended inside the parentheses. Versions are
  placed in representative order (§5.2), so the number never depends on
  arrival order.
- The extension is kept, so the file still opens in the same application
  (ux.md §8.9). A name without extension gets the suffix at its end.
- A package gets the suffix on its folder: `Rapport (Paul, 2026-10-13
  14h02).pages/`, with every file inside under it (ADR 0044).

ux.md §8.9 shows `budget (Paul, 13 Oct 14:02).xlsx`. A colon is not portable and
an English month would make names depend on the device's language, which breaks
P5; this specification uses the form above **[no ADR]**.

**Conflicts.** A slot is **in conflict** when it has several versions, or a
version under its conflict name. The conflict is **open** while no
contribution has frozen it; once frozen, it stays open until a later
contribution writes the slot or one of the event's names, or carries
`Pigeon-Resolved: p` (ADR 0052).

**People see** "Paul and you edited *budget.xlsx* at the same time; both versions
are kept", with *Keep mine*, *Keep Paul's*, *Keep both* (ux.md §8.9). Each choice
is an ordinary signed change. *Keep mine* and *Keep Paul's* set *p* to the
chosen key and delete the conflict names. *Keep both* changes no file: its
commit carries the trailer `Pigeon-Resolved: p`, and counts only when its
person may change *p*. Whoever chooses first decides for the group, and a
choice races later saves like any change (§5.2). The notice disappears
everywhere once the conflict is closed. It is computed from the catalogue,
never stored (ADR 0036).

On the device whose saved file lost the name, install does not replace that
file: it is a **divergence** ([07 §7.6](07-capture-and-install.md)), and the
same three choices resolve it (ADR 0034).

## 5.4 Portable names

Every path of the catalogue must be **portable**, so that Linux, macOS and later
Windows can all hold it under the same name (ADR 0005). `portable(path)` is a pure
function of the core; a path is portable if every component:

- is valid **UTF-8** (Linux allows any bytes; ADR 0044);
- is in Unicode **NFC** (`core.precomposeunicode=true` on macOS);
- does not collide, by Unicode full case folding, with a sibling of the
  catalogue;
- is not a Windows reserved name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`,
  `LPT1`–`LPT9`, with or without an extension), does not end with a dot or a
  space, and contains none of `< > : " / \ | ? *` nor a control character;
- is at most 255 bytes in UTF-8, the whole path at most 1024 bytes **[no ADR:
  limits]**;
- is not `.git`, and is not `.pigeon` at the root unless it is pigeon's folder.

**At capture** a file whose name is not portable is *held back*: it stays only on
this computer, and the headline says "Needs you: *Rapport: v2.docx* can't be
shared because of its name", with a suggested portable name and a button to
rename it (the person asks, so ADR 0034 allows it) **[no ADR: hold back]**.
**At acceptance** a commit that introduces a non-portable path is invalid (rule 5
of the authority rule, added to [04 §4.4](04-roster-and-authority.md)).
Concurrent saves can still bring names that collide; the merge then treats
them as one slot (§5.2).

The Unicode tables for NFC and case folding are pinned in the core. Changing
them changes which commits are valid, so it is a format change (issue #15,
ADR 0044). Because the catalogue only ever holds portable names, every
supported platform can write every name, and v1 needs no mapping between a
stored name and a name on disk.

Laws (ADR 0012): normalisation is idempotent; `portable` accepts every conflict
name that §5.3 generates; the suggested name is portable and differs from every
sibling; names of files that do not collide are never changed.

The alternative of mapping reserved characters on Windows only, as Samba's
`catia` does, would let v1 groups keep such names; it is listed in
[16](16-open-questions.md).
