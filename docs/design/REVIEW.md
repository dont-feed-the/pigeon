# pigeon spec — critical review (of draft 1, 12 Oct)

Reviewer's note: most of draft 1 was written by the same agent that reviews it here. Errors found in
my own earlier claims are marked **[self-correction]**.

## Verdict

The core is sound and small: immutable versions merged by union, one authorization function, files
on disk as a pure view, two unsafe escapes, and a model-checked custody hand-off. The main risks are
**not** where the spec spends its words. Real file loss comes from deletions that were never
intended, from the disk door (scanner/renderer), and from silent corruption, and the spec covers
these least. Ideas 1 and 3 reinvent textbook optimistic replication and should adopt its standard
pieces. Several features (proposals, diogene, retention modes, re-minting) add states that existing
primitives already express.

---

## 1. Soundness

Ranked by severity.

### High

**S1. Ownership by naming convention, and users are unauthenticated.**
- `owner(p)` is read from a folder name `u_`. If `old_/` exists and someone later picks the user
  name `old`, they gain ownership of existing folders: authorization changes *retroactively*, and the
  rendered tree changes without any new version. INV-Converge holds only if the set of users is
  itself part of the replicated state, and INV-Authorized holds only if users cannot be added after
  the fact.
- A node *declares* its user, so any member can claim to be `alice`. Owned folders protect against
  mistakes, not against members. The spec should say so explicitly (threat model: all key holders
  are trusted) or bind users to keys.
- Case and Unicode: `Alice_` vs `alice_`, NFC vs NFD (macOS). The owner check needs a canonical path
  form, which the spec does not define.
- *Fix:* ownership as an explicit attribute recorded in a version (a folder's creation version names
  its owner), user names registered in the version set, canonical paths (NFC + case-folding rule).

**S2. Unintended deletions are the main real-world loss path, and the guarantee excludes them.**
- The scanner turns "file absent" into a delete version. Absent can mean an unmounted drive, a
  permission error, a lost `.pigeon/` state, or `rm -rf` in the wrong terminal.
- Per Idea 4, the deleted blob stops being live, its tokens dissolve, and with retention `none` the
  content may be gone everywhere. That is "no loss" in the spec's sense and total loss in the user's.
- E-8 lists a mass-deletion threshold as "TBD". It should be mandatory.
- *Fix:* tokens of deleted or superseded blobs are kept until an explicit **purge** (by the owner,
  or by policy after a delay). Add a root marker file (as Syncthing's `.stfolder`), so a missing root
  is never read as a mass delete.

**S3. The disk door is stated as an invariant that POSIX cannot provide.**
- INV-NoSilentOverwrite asks for scan-before-write. There is no atomic "rename if unchanged", so a
  user write between the last check and the `rename` can be lost. Syncthing has the same race.
  The spec also does not say what a node shows while a new head's blob is still downloading.
- *Fix:* never unlink or overwrite in place. Move the current file aside (into `.pigeon/versions/`)
  and then rename the new one in: a concurrent write lands on the moved-aside inode and survives.
  Replace a file only once the new blob is fully present; until then keep showing the old content.
  Model this race separately, as §6 already plans.

### Medium

**S4. The model-checking claims are narrower than the prose suggests.** [self-correction]
- Checked: one blob, ≤ 4 nodes, ≤ 5 offers, durable and atomic node state, safety only.
- Not checked: crashes between steps (the token must be fsynced *before* the ack); the placement
  phase, where the author holds all K tokens and so breaks "≤ 1 per node"; dissolution of superseded
  blobs; re-minting after "forget node"; liveness.
- Bounded model checking is evidence, not proof. An inductive invariant (TLC or Apalache) or a TLAPS
  proof of `TokensNeverLost` would close the gap cheaply, since the protocol is tiny.

**S5. Metadata grows forever.**
- The version set is grow-only: every edit and every tombstone, forever. Compacting it needs causal
  stability, and a laptop that stays offline for a year never reaches it.
- *Fix:* see N1 (per-file version vectors). History then becomes a matter of blob retention, not of
  metadata.

**S6. Frozen is weaker than advertised.**
- A version with no parents is a "creation", and creation in a frozen folder is authorized. So
  anyone can add a competing version of an existing frozen file, or a parentless tombstone. Nothing
  is lost (it renders as a conflict copy), but "frozen" really means "nobody can supersede", not
  "nobody can change what is shown".
- *Fix:* forbid `blob = ⊥` in creations. Document the remaining loophole: other nodes cannot tell a
  concurrent add from a deliberate parallel one.

**S7. K copies is not K independent copies.**
- Two nodes can share a disk (dual boot, VM), a house, or a cloud account.
- Silent corruption: nothing re-checks hashes, so K tokens may guard K corrupted copies.
- *Fix:* periodic scrubbing (like `git annex fsck`); optional failure-domain labels on nodes.

### Low

- **S8. Moves as delete + create.** A concurrent edit on another node brings the old path back,
  with one file in it. Moving a 10 000-file folder writes 20 000 versions. Proposals against a moved
  file go stale silently.
- **S9.** [self-correction] RELATED.md said the hand-off needs no remote online. That was wrong: the
  receiver must be online to answer. It is now corrected.
- **S10. Proposals are replicated to everyone and hold tokens** (they count as live), so every node
  stores every pending suggestion.

---

## 2. NIH: reinvented where a standard exists

| # | Spec | Existing practice | Recommendation |
|---|---|---|---|
| N1 | Explicit parent DAG per version, kept forever | Per-file **(dotted) version vectors**: Syncthing, Riak, Coda/Ficus; Preguiça et al. | Use dotted version vectors for causality; keep parents only for merges. Solves S5. |
| N2 | "Sync by union" with no algorithm | **Range-based set reconciliation** (Meyer 2023, used by iroh), Merkle search trees, Syncthing's index sequence numbers | Pick one; do not invent. |
| N3 | Custody tokens | **Replica maintenance**: git-annex `numcopies`/`mincopies` + content locking + trust levels; Carbonite (Chun et al., NSDI 2006) for re-replication | Keep the protocol (small, checked), but adopt git-annex vocabulary (numcopies, trusted/untrusted/dead) and Carbonite's re-replication ideas. Do not claim novelty. |
| N4 | `group_key`, membership | Noise / QUIC-TLS with node keys; PAKE (CPace, SPAKE2) or signed invites | Delegate to iroh + a PAKE; specify only the threat model. |
| N5 | Blob transfer, chunking | BLAKE3/bao verified streaming (iroh-blobs); content-defined chunking (restic, casync) | Reuse. |
| N6 | Conflict-copy naming, ignore rules, FS quirks | Syncthing conventions and its bug history | Copy them. |
| N7 | The whole engine | **git-annex assistant** already gives automatic sync, location tracking, partial content, numcopies, full history and LAN pairing | Before writing an engine, spend 1–2 weeks prototyping pigeon's policies on git-annex (or on Syncthing with receive-only folders). Only ownership, proposals and frozen-by-default are truly missing. *(Follow-up: GIT-ANNEX.md tests this; the case for git-annex is stronger than stated here.)* If the prototype feels right, the new engine is justified by UX and licence (AGPL), not by the protocol. |

What is genuinely specific to pigeon, and fine to design: the authorization rule (Idea 2), frozen by
default, the two unsafe escapes, and the custody guarantee as a product promise.

---

## 3. Unnecessary complications

1. **Proposals as a separate state** (unauthorized and unforced, a proposals directory, accept and
   reject merges, tokens for pending proposals). They can be expressed with what already exists: a
   non-owner's edit is saved as a *new frozen file* in an inbox next to the owner's folder
   (`alice_.inbox/f (bob, 12 Oct).txt`). Accepting means the owner copies it in. No new version
   kind and no "visible ≠ authorized" split, except for Force.
2. **Diogene.** Fully subsumed by "a node that keeps everything and accepts every token" (= git-annex
   *trusted*). Drop the concept; keep the word as a UI label if it is liked.
3. **Two mechanisms for old versions**: tokens for history ("last fully placed state survives") plus
   `.pigeon/versions/` retention `all | external | none`. Keep one: old blobs keep their tokens until
   **purge**, and retention becomes the purge policy. This also fixes S2.
4. **The token rule set has grown**: placement phase, ≤ 1 per node, dissolution, forget node,
   re-minting, retiring extra tokens, per-folder K. For v1: K per group, dissolution only by purge,
   manual "forget node" without re-minting (warn instead). Defer the rest.
5. **Ownership by name** (S1). An explicit owner attribute is both simpler and sound.
6. **Version DAG** (N1). Version vectors are less to store, less to reconcile and less to garbage-collect.
7. **Self-update with signed binaries, and the "refuse incompatible versions" machinery.** Use
   forward-compatible CBOR (ignore unknown fields) and one protocol version number; defer the rest.

## 4. Suggested next steps

1. Decide S1 (explicit owner attribute and authenticated users?) and S2 (purge instead of automatic
   dissolution). Both change the data model.
2. Replace parents with dotted version vectors; pick a set-reconciliation algorithm.
3. Extend the TLA+ model: crashes with fsync ordering, the placement phase, purge; plus an inductive
   invariant.
4. Model the disk door: move-aside rendering, and the scanner racing user writes.
5. Time-boxed git-annex prototype of the policies (N7).
