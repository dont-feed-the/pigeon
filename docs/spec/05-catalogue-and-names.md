# 5. The catalogue and its names

> Mission: the logical catalogue every device computes from accepted history,
> the deterministic merge that keeps it identical everywhere (P5), conflict
> names, and portable names. Normative model: `models/catalogue/catalogue.py`
> (mode `new`) and its laws.

## 5.1 The logical catalogue

The logical catalogue is a pure function of a set of accepted commits
(ADR 0012), never of their arrival order nor of merge commits (§5.2):

| Function | Value |
|---|---|
| `current(p)` | the key at path *p* in the catalogue's tree, or none |
| `versions(p)` | the contributions that wrote *p*: commit, key or deletion, person, device, recorded time |
| `deleted()` | paths whose last change is a deletion, with their last key (ux.md §6) |
| `conflicts()` | open conflicts and their versions (§5.3) |
| `proposals()` | proposals and their states ([04 §4.5](04-roster-and-authority.md)) |
| `keys` | every key an accepted or proposal commit introduces at a path (a *reference*) that no purge released: what custodians keep ([08 §8.6](08-custody-loops.md)) |

An accepted commit *c* has base `B(c)`, the catalogue of its accepted
ancestors, and `Δ(c) = diff(B(c), tree(c))` ([04 §4.4](04-roster-and-authority.md)).
It is a **contribution** when Δ(c) is not empty or it carries the trailer
`Pigeon-Resolved: <path>` (*Keep both*, §5.3). Saves (capture, restore,
resolution, Force) and decision merges (the decider's contribution) are
contributions; clean merges are not, so who wrote them, and how many, never
matters; proposals and local divergences count for nothing.

**Slots.** Paths are compared after portable folding (§5.4): `Photo.jpg` and
`photo.jpg` are one slot. A package ([07 §7.9](07-capture-and-install.md)) is
one slot whose value is its whole subtree. A contribution writes the slots of
its Δ.

**Restoring** a version or a deleted file (ux.md §8.6–8.7) is a new change
setting *p* to the old key, checked by the authority rule like any change; no
content moves, unless a purge erased it ([08 §8.6](08-custody-loops.md)).

## 5.2 The deterministic merge

`catalogue(S)`, for an ancestor-closed set S of accepted commits, is built
from **events**: the contributions of S not revoked ([04 §4.4](04-roster-and-authority.md))
and the conflict events they froze.

1. **Order.** An event comes after its ancestors and the events it froze.
2. **Heads** of a slot: the events that wrote it with no later writer of it.
3. **Versions:** each distinct content among the heads' values. A deletion is
   no version, so a modification survives a concurrent deletion (P3).
4. **Representative** of a version: its earliest head by (author time, device
   ID, that device's earlier contributions, commit ID); an adoption counts
   with its proposal's author and time.
5. **Placement**, in representative order: a folder takes its first version's
   spelling; each slot's first version takes its own name, unless a folder of
   that folded name is there; every other version takes its first free
   conflict name (§5.3).
6. **Freezing.** A slot with several versions, or a version under another
   name, is unsettled. Every contribution freezes each unsettled slot of its
   base into a **conflict event** that writes each version where B(c) showed
   it, so a later save of *p* keeps the copies its person saw.

| Case | Result |
|---|---|
| edit and edit, add and add | the first representative keeps *p*; the others go to conflict names |
| edit and delete | the edit stays; the deletion is in `versions(p)` |
| rename and edit | a rename is a deletion and an add: *p* holds the edit, *q* the renamed file |
| case or Unicode collision | one slot: the later goes to `photo (Bob, 2026-10-13 14h02).jpg`; one content under both names is one version |
| package | two edits keep two whole packages, never a mix |
| *Keep mine* racing an edit | a new conflict, both kept |
| *Keep both* racing an edit of a copy | the edited copy stays; the conflict is closed |
| `.pigeon/` | never a conflict name: the roster is the fold of [04 §4.8](04-roster-and-authority.md); another file keeps its first version, the others stay in history |

**Laws** (`catalogue.py`, every schedule of three devices for each case,
[13 §13.4](13-verification.md)): `order` (the catalogue depends only on the
contributions and on which saw which: merge is commutative, associative,
idempotent), `tree` (`tree(c) = catalogue(anc*(c))`), `kept` (a key leaves only
through a contribution whose base showed it), `seen` (no unseen version comes
back), `names` (portable, injective after folding), `authority` (clean
merges change nothing; each copy is allowed to its representative's person).
Time only orders: a wrong clock changes which version keeps the name, never
whether one is kept. Not yet coded in `pigeon-core`.

**Merge commits.** `pigeond` writes the tree of a merge *M* as
`catalogue(anc*(M))` with git plumbing; `git merge-tree --write-tree` may serve
as a checked fast path (ADR 0010). A device merges a peer's tip only if it
brings a contribution it lacks, otherwise it records the tip as equivalent,
so `main` branches may differ by merge commits only (P5). A clean merge may be
signed by a device of any parent's roster; a conflict event is no commit; a
decision merge is judged as the decider's change ([04 §4.4](04-roster-and-authority.md)).

## 5.3 Conflict names

```text
dir/stem (Person, 2026-10-13 14h02).ext
```

- *Person* is the representative's person's display name, made portable; the
  time is its author time in the offset recorded in that commit, so every
  device computes the same name. A colon or a month name would break P5, so
  this differs from ux.md §8.9 **[no ADR]**.
- A taken name gets ` 2`, ` 3`, … inside the parentheses, in representative
  order. The extension is kept; a package gets the suffix on its folder.

A slot is **in conflict** while it has several versions or one under its
conflict name; the conflict is **open** until a later contribution writes the
slot or one of its names, or carries `Pigeon-Resolved: p`.

People see "Paul and you edited *budget.xlsx* at the same time; both versions
are kept", with *Keep mine*, *Keep Paul's*, *Keep both* (ux.md §8.9). Each is
a signed change: the first two set *p* and delete the conflict names; *Keep
both* changes no file and counts only when its person may change *p*. The
first choice decides for the group. The notice is computed, never stored
(ADR 0036). On the device whose file lost the name, install leaves it as a
**divergence** ([07 §7.6](07-capture-and-install.md)), resolved by the same
choices (ADR 0034).

## 5.4 Portable names

`portable(path)`, a pure function of the core, holds when every component:

- is valid UTF-8 and in Unicode NFC (`core.precomposeunicode` on macOS);
- does not collide, by Unicode full case folding, with a sibling;
- is not a Windows reserved name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`,
  `LPT1`–`LPT9`, with or without extension), does not end with a dot or space,
  and holds none of `< > : " / \ | ? *` nor a control character;
- is at most 255 bytes, the path at most 1024 **[no ADR: limits]**;
- is not `.git`, nor `.pigeon` at the root unless it is pigeon's folder.

**At capture** a non-portable name is *held back* on this computer; the
headline says "Needs you: *Rapport: v2.docx* can't be shared because of its
name", with a suggested name and a rename button **[no ADR: hold back]**.
**At acceptance** it makes a commit invalid (rule 5, [04 §4.4](04-roster-and-authority.md)).
The Unicode tables are pinned in the core; changing them is a format change
(issue #15). So every platform can write every name, with no mapping in v1
(ADR 0005); a Windows-only mapping like Samba's `catia` is an open question
([16](16-open-questions.md)).

Laws, not yet coded: normalisation is idempotent; `portable` accepts every
conflict name of §5.3; the suggested name is portable and differs from every
sibling; names that do not collide never change.
