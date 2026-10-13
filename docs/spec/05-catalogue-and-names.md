# 5. The catalogue and its names

> Mission: define the logical catalogue that every device computes from accepted
> history, the deterministic merge that keeps it identical everywhere (P5), the
> names of concurrent versions, and the portable-name rules.

## 5.1 The logical catalogue

The **logical catalogue** is a pure function of the set of accepted commits
(ADR 0012), never of their arrival order. It gives:

| Function | Value |
|---|---|
| `current(p)` | the key at path *p* in the tree of the catalogue, or none |
| `versions(p)` | the accepted changes of *p*: commit, key or deletion, person, device, recorded time |
| `deleted()` | the paths whose last change is a deletion, with their last key (Deleted files, ux.md §6) |
| `proposals()` | the proposals and their states ([04 §4.5](04-roster-and-authority.md)) |
| `keys()` | every key that an accepted or proposal commit introduces at a path (a *reference*) and that no purge released: what custodians keep ([08 §8.6](08-custody-loops.md), ADR 0042) |

A **change** of *p* is an accepted commit whose tree differs at *p* from the
tree of at least one of its parents. The changes of *p* form a DAG through the
commit graph; its maximal elements are the **heads** of *p*.

**Restoring** a version (ux.md §8.7) or a deleted file (§8.6) is a new change
that sets *p* to the old key; it is checked by the authority rule like any change
(restoring a deleted file of an add-only folder is an *add*; restoring in
someone else's folder is a proposal). Nothing is re-transferred: the key is
already known to custodians, unless a purge erased it ([08 §8.6](08-custody-loops.md)).

## 5.2 The deterministic merge

`merge` builds the tree of the catalogue path by path, from the heads:

1. *p* has one head: its key, or no file if it is a deletion.
2. Several heads that agree (same key, or all deletions): that.
3. Heads that disagree: deletions are dropped, **a modification survives a
   concurrent deletion** (P3: nothing a person saved disappears by a race).
4. Several distinct keys remain: a **concurrent edit**. Each distinct key gets a
   representative: the earliest non-merge commit that introduced it at *p*,
   ordered by (recorded author time, device ID, commit ID). The first keeps *p*;
   each other key goes to its **conflict name** (§5.3).

**Packages** (ADR 0044, [07 §7.9](07-capture-and-install.md)). The merge treats
a package as one path, whose value is its whole subtree. A change of any file
inside is a change of the package. Concurrent edits keep each side whole: the
first representative keeps the package's name, and each other side goes to
the package's conflict name. A modification survives a concurrent deletion of
the whole package. No merged package mixes files of two saves.

Because the result depends only on the set of accepted commits, `merge` is
commutative, associative and idempotent, and conflict names do not depend on
arrival order (laws of ADR 0012). Time only orders; a wrong clock changes which
version keeps the name, never whether a version is kept.

**Merge commits.** `pigeond` writes the merged tree with `git merge-tree
--write-tree` for clean paths and its own resolution for the others (ADR 0010),
then a signed merge commit. Two devices that merge the same heads write
different commits with the same tree. To avoid an endless exchange of such
merges, a device merges a peer's tip only if it brings an accepted non-merge
commit that this device lacks; otherwise it records the tip as equivalent
**[no ADR]**. Devices' `main` branches may therefore differ by merge commits only,
which changes nothing in the logical catalogue (P5).

**The roster.** `.pigeon/roster.toml` is not merged by heads. Its value in a
merged tree is the fold of the roster changes of the accepted history
([04 §4.8](04-roster-and-authority.md), ADR 0051): concurrent changes that
conflict are skipped and shown to the admins, and device removals always
apply.

## 5.3 Conflict names

The conflict name of key *k* at path `dir/stem.ext` is

```text
dir/stem (Person, 2026-10-13 14h02).ext
```

- *Person* is the display name of the representative commit's person, made
  portable (§5.4); the time is its recorded author time, in the time-zone offset
  recorded in that commit, so every device computes the same name.
- If that name is taken, ` 2`, ` 3`, … are appended inside the parentheses, in
  representative order.
- The extension is kept, so the file still opens in the same application
  (ux.md §8.9). A name without extension gets the suffix at its end.
- A package gets the suffix on its folder: `Rapport (Paul, 2026-10-13
  14h02).pages/`, with every file inside under it (ADR 0044).

ux.md §8.9 shows `budget (Paul, 13 Oct 14:02).xlsx`. A colon is not portable and
an English month would make names depend on the device's language, which breaks
P5; this specification uses the form above **[no ADR]**.

**People see** "Paul and you edited *budget.xlsx* at the same time; both versions
are kept", with *Keep mine*, *Keep Paul's*, *Keep both* (ux.md §8.9). Each choice
is an ordinary signed change: *Keep both* accepts the names; *Keep mine* and
*Keep Paul's* set *p* to the chosen key and delete the conflict name. Whoever
chooses first decides for the group; the notice disappears everywhere once a
later change of *p* supersedes the merge. The notice is computed from the
catalogue, never stored (ADR 0036).

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
