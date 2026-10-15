# 5. The catalogue and its names

> Mission: the catalogue every device computes from accepted history, its
> deterministic merge (P5), conflict names and portable names. Normative:
> `models/catalogue/catalogue.py` (mode `new`), its cases and laws.

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
It is a **contribution** when Δ(c) is not empty or it carries
`Pigeon-Resolved: <path>` (*Keep both*, §5.3): saves (capture, restore,
resolution, Force) and decision merges are; clean merges are not, so who
wrote them never matters; proposals and local divergences count for nothing.

**Slots.** Paths are compared after portable folding (§5.4), so `Photo.jpg`
and `photo.jpg` are one slot; a package ([07 §7.9](07-capture-and-install.md))
is one slot holding its subtree. **Restoring** (ux.md §8.6–8.7) is a new
change setting *p* to the old key, checked like any change; no content moves
unless a purge erased it ([08 §8.6](08-custody-loops.md)).

## 5.2 The deterministic merge

`catalogue(S)`, for an ancestor-closed set S of accepted commits, is built
from **events**: the contributions of S not revoked ([04 §4.4](04-roster-and-authority.md))
and the conflict events they froze.

| # | Step |
|---|---|
| 1 | **Order:** an event comes after its ancestors and the events it froze |
| 2 | **Heads** of a slot: the events that wrote it with no later writer of it |
| 3 | **Versions:** each distinct content among the heads' values; a deletion is none, so a modification survives a concurrent deletion (P3) |
| 4 | **Representative** of a version: its earliest head by (author time, device ID, that device's earlier contributions, commit ID); an adoption counts with its proposal's author and time |
| 5 | **Placement**, in representative order: a folder takes its first version's spelling; each slot's first version its own name, unless a folder of that folded name is there; every other version its first free conflict name (§5.3) |
| 6 | **Freezing:** a slot with several versions, or a version under another name, is unsettled; every contribution freezes each unsettled slot of its base into a **conflict event** writing each version where B(c) showed it, so a later save keeps the copies its person saw |

| Case (`catalogue.py`) | Result |
|---|---|
| `edit-edit`, `add-add` | the first representative keeps *p*; the others go to conflict names |
| `edit-delete` | the edit stays; the deletion is in `versions(p)` |
| `rename-edit` | a rename is a deletion and an add: *p* holds the edit, *q* the renamed file |
| `case`, `unicode` | one slot: the later goes to `photo (Bob, 2026-10-13 14h02).jpg`; one content under both names is one version |
| `package` | two edits keep two whole packages, never a mix |
| `keep-mine` racing an edit | a new conflict, both kept |
| `keep-both` racing an edit of a copy | the edited copy stays; the conflict is closed |
| `supersede` | an edit made after seeing a conflict replaces the name and keeps the copies it saw |
| `decision` | an adoption racing an edit: both kept, the adoption with its proposal's author and time |
| `.pigeon/` (no case) | never a conflict name: the roster is the fold of [04 §4.8](04-roster-and-authority.md); another file keeps its first version, the others stay in history |

**Laws** (`catalogue.py`, every schedule of three devices per case,
[13 §13.4](13-verification.md)): `order` (only the contributions and who saw
which matter), `tree` (`tree(c) = catalogue(anc*(c))`), `kept` (a key leaves
only through a contribution whose base showed it), `seen` (no unseen version
returns), `names` (portable, injective after folding), `authority` (clean
merges change nothing; each copy is allowed to its representative's
person). A wrong clock changes which version keeps the name, never whether
one is kept. `pigeon-core::catalogue` codes the merge (`Engine`): its laws
are hegel properties on random schedules (`tests/catalogue.rs`, with
judging in another order for `order`), every commit the oracle reaches gets
its verdict, tree and open conflicts (`tests/catalogue_oracle.rs`), and
traces validate against `Catalogue.tla` ([13 §13.3](13-verification.md)).
`pigeond` does not use it yet (M2).

**Merge commits.** `pigeond` writes a merge *M*'s tree as
`catalogue(anc*(M))` with plumbing (`merge-tree --write-tree` as a checked
fast path, ADR 0010). It merges a peer's tip only if it brings a missing
contribution, else records it as equivalent, so `main` branches differ by
merge commits only (P5). A conflict event is no commit; a decision merge is
judged as the decider's change ([04 §4.4](04-roster-and-authority.md)).

## 5.3 Conflict names

```text
dir/stem (Person, 2026-10-13 14h02).ext
```

- *Person*: the representative's display name, made portable (NFC, each
  forbidden or control character and each parenthesis a `-`); the time: its
  author time in the offset recorded in that commit, so every device computes
  the same name. The stem is shortened, at a character boundary, so that the
  name fits in 255 bytes. A colon or a month name would break P5: this differs from
  ux.md §8.9 **[no ADR]**.
- A taken name gets ` 2`, ` 3`, … inside the parentheses, in representative
  order; the extension is kept; a package takes the suffix on its folder.

A slot is **in conflict** while it has several versions or one under its
conflict name, **open** until a contribution writes the slot or one of its
names, or carries `Pigeon-Resolved: p`. *Keep mine*, *Keep Paul's* and *Keep
both* (ux.md §8.9) are signed changes: the first two set *p* and delete the
conflict names; *Keep both* changes no file and counts only when its person
may change *p*. The first choice decides for the group; the notice is
computed (ADR 0036). The device whose file lost the name keeps it as a
**divergence** ([07 §7.6](07-capture-and-install.md), ADR 0034).

## 5.4 Portable names

`portable(path)`, a pure function of the core, holds when every component:

- is valid UTF-8 and in Unicode NFC (`core.precomposeunicode` on macOS);
- does not collide, by Unicode full case folding, with a sibling;
- is not a Windows reserved name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`,
  `LPT1`–`LPT9`, with or without extension), does not end with a dot or space,
  and holds none of `< > : " / \ | ? *` nor a control character;
- is at most 255 bytes, the path at most 1024 **[no ADR: limits]**;
- is not `.git`, nor `.pigeon` at the root unless it is pigeon's folder.

**At capture** a non-portable name is *held back* here, with *Needs you*, a
suggested name and a rename button **[no ADR: hold back]**; **at acceptance**
it makes a commit invalid (rule 5, [04 §4.4](04-roster-and-authority.md)).
The Unicode tables are pinned in the core; changing them is a format change
(issue #15): Unicode 16.0, from `caseless` 0.2.2 (full case folding) and
`unicode-normalization` 0.1.24 (NFC). Every platform can write every name,
with no mapping in v1 (ADR 0005; [16 §16.4](16-open-questions.md)). The
suggested name replaces each offending character by `-`, prefixes a
reserved name with `_`, drops a trailing dot or space, and adds ` 2`,
` 3`, … before the extension until no sibling folds alike.

Laws (`pigeon-core::names`, `tests/names.rs`): normalisation is idempotent;
`portable` accepts every conflict name of §5.3; the suggested name is
portable and differs from every sibling; names that do not collide never
change.
