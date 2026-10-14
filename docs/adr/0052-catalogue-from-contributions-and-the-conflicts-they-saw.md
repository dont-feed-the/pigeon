# 0052. The catalogue is built from contributions and the conflicts they saw; merge commits never count

- **Status:** Proposed, 14 October 2026, by the agent, for issue #8 (Guillaume's review: "Specify a catalogue merge algorithm that satisfies the stated convergence laws").

## Context

ADR 0012 requires the catalogue merge to be commutative, associative and
idempotent, with conflict names independent of arrival order. Spec 05 §5.2
built the catalogue from the *heads* of each path: the latest accepted
commits whose tree differs there from a parent's. Merge commits counted as
changes, and conflict copies were derived from the heads. A reference model,
`models/catalogue/catalogue.py`, runs every schedule of three devices on
eleven cases. It found four weaknesses in that rule.

- **(a) Merge identities enter the heads.** B deletes `a.txt` and edits
  `b.txt`; Paul's C, made after B, deletes `b.txt`; A edits `a.txt`. A merge
  of A and B is a change of `b.txt` that C never saw. So
  `merge(merge(A, B), C)` brings `b.txt` back, and `merge(A, merge(B, C))`
  does not. Two devices that merge the same heads write two merge commits; a
  later save supersedes only the one it saw. Bob's copy then comes back next
  to his own *Keep mine*, on some schedules only.
- **(b) Copies vanish.** Counting only the non-merge commits fixes (a), but a
  derived copy disappears as soon as a later save is the only head. Alice
  sees `a.txt` and Bob's copy, and saves `a.txt`. Her commit's tree holds
  Bob's copy; the catalogue of her commit's own history does not. Bob's
  version leaves the catalogue, though nobody removed it.
- **(c) Pairwise merges invent copies.** A, B and C edit `a.txt`. D, made
  after A and B, replaces it. If Paul's device first merged A with C, the
  final catalogue shows A's version again: D replaced it, and no last save
  holds it. Without that merge, it does not come back.
- **(d) Gaps.** Names equal after case folding (`Photo.jpg`, `photo.jpg`)
  were kept side by side, which no Mac can hold. The ` 2` of two copies of
  one minute depended on which merge came first. Rename and edit, packages,
  `.pigeon/` and the authority of a merge joining several owners' changes
  were not defined.

## Decision

The catalogue of an ancestor-closed set S of accepted commits is defined as
follows (spec 05 §5.1–5.3).

1. **Contributions.** For an accepted commit *c*, its **base** B(c) is the
   catalogue of its accepted ancestors, and Δ(c) = diff(B(c), tree(c)). *c* is
   a **contribution** when Δ(c) is not empty, or when it carries the trailer
   `Pigeon-Resolved: <path>` (*Keep both*). A clean merge has Δ = ∅: it joins
   histories and is otherwise ignored, whoever wrote it and however often. A
   decision merge is a contribution of the decider. A proposal is not
   accepted, and a local divergence is not a commit.
2. **Slots.** Paths are compared after portable folding (spec 05 §5.4). A
   package (ADR 0044) is one slot, whose value is its whole subtree. The value
   of a slot is an exact name and a content, or nothing.
3. **Events and heads.** A contribution **writes** the slots of Δ(c). The
   heads of a slot are its latest writers among the contributions and the
   conflict events (4) of S. Each distinct content among the heads is one
   **version**; deletions are dropped, so a modification survives a
   concurrent deletion. A version's **representative** is its earliest head,
   by (recorded author time, device ID, that device's number of earlier
   contributions, commit ID); an adoption counts with its proposal's author
   and time.
4. **Conflicts are frozen when seen.** A slot is **unsettled** when it has
   several versions, or a version shown under another name. Each
   contribution *c* freezes every unsettled slot of B(c) into a **conflict
   event**, which writes each version where B(c) showed it. The event comes
   after the slot's heads and before *c*. It is identified by those heads and
   names, so two saves that saw the same conflict froze the same event.
5. **Placement.** The versions of all slots are placed in representative
   order. A folder takes the spelling of its first version. First, each
   slot's first version takes its own name, unless it is a file where a
   folder of the same folded name is. Then every other version takes its
   conflict name (spec 05 §5.3), the first one that is free after folding.
6. **Merge commits.** The tree of a merge commit is the catalogue of its
   history. A device merges a peer's tip only if the tip brings a
   contribution that this device lacks; otherwise it records the tip as
   equivalent. `git merge-tree` is at most a fast path whose result pigeon
   checks (ADR 0010).
7. **Authority.** A clean merge changes nothing, so a device of any parent's
   roster may sign it, whoever made the changes it joins (spec 04 §4.4). A
   conflict event is not a commit and has no signature. Each copy it writes
   is the version of its representative, at a new name in the same folder,
   and whoever may write a path of a folder may add to it (spec 04 §4.3). A
   decision merge is judged as the decider's change, including the copy it
   writes when the path changed meanwhile.
8. **Resolutions.** *Keep mine* and *Keep Paul's* set the slot and delete the
   conflict names. *Keep both* changes nothing and carries the trailer; it
   counts only when its person may change the path. A conflict stays open
   until a later contribution writes its slot or one of its names, or
   resolves it.
9. **Renames are not tracked.** A rename is a deletion and an add. A rename
   racing an edit leaves the edit at the old name and the renamed file at the
   new one.
10. **`.pigeon/`** never gets conflict names. `roster.toml` is the fold of
    spec 04 §4.8 (ADR 0051). For any other file under `.pigeon/`, the first
    version keeps the slot; the others stay in history.

## Consequences

- These are laws of the reference model, checked in CI on every reachable
  state of three devices, for eleven cases (spec 13 §13.4). The catalogue
  depends only on the contributions and on which saw which, never on the
  order of contacts or on merge identities. tree(c) = catalogue(anc\*(c)) for
  every accepted commit, and a device shows the tree of its `main`. A key
  leaves the catalogue only through a contribution whose base showed it. A
  version that none of the latest contributions holds never comes back.
  Every catalogue is portable, so conflict names are injective after
  folding. Merging is idempotent, commutative and associative. Every copy is
  allowed to its author.
- The same model runs the rule of spec 05 §5.2 before this ADR (`old`) and
  the repair of (a) alone (`naive`). Both fail as described above; CI
  expects these failures, so a change to the model that hides them is
  caught.
- Every contribution's base is computed anyway, for Δ (spec 04 §4.4). The
  conflict events are a pure function of the history: computed, never
  stored.
- An edit made after seeing a conflict replaces the name and keeps the
  copies it saw. Nobody has to resolve a conflict for its versions to stay.
- One content saved under two spellings of one name is one version, under
  the first spelling. Two folders whose names differ only by case become one.
- *Keep both* is a contribution with an empty Δ: a new kind of commit to sign
  and judge.
- The model is a reference for the hegel-rust properties of the core (ADR
  0012), not a replacement: the core must agree with it on the same cases.

## Alternatives considered

- **Count only non-merge commits, derive copies from the heads.** Fixes (a)
  and loses copies (b).
- **Make every conflict copy a write of the merge that shows it.** A merge
  commit would then carry changes, and two devices' merges of the same heads
  would differ in identity and order: (a) again.
- **Materialise copies in the save that ends a conflict.** Simpler to state,
  but a save that merely saw a conflict would still depend on later context
  (a freed ` 2` name, a folder spelling), breaking tree(c) =
  catalogue(anc\*(c)). And Bob's edit of his copy, racing *Keep both*,
  would conflict with the unchanged copy that *Keep both* wrote.
- **Track renames** with git's similarity detection. It guesses from
  content, and fails on small files; left open (spec 16 §16.4).
