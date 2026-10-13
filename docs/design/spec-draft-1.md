# pigeon — specification (draft 1)

> **Superseded** by [docs/spec/](../spec/README.md) on 13 October 2026. Kept for the record.

> Status: draft for discussion. `TBD` = undecided, `v2` = later (v1 must not block it).
> Draft 1 rewrites draft 0 ([spec-draft-0.md](spec-draft-0.md)) for a higher power-to-complexity ratio:
> the special cases of draft 0 (conflicts, tombstones, moves, proposals, force-confirm,
> frozen edits, unsync, diogene, partitions) become ordinary cases of **four ideas**.
> Wire protocol: CBOR or Protobuf (TBD), language-neutral. Implementation language after the spec.

## 0. Goals

1. Sync files between several computers, peer-to-peer.
2. **No data loss** as long as no unsafe action is taken.
3. **Unsafe actions** are possible, warned, and still best-effort non-destructive.

## 1. The four ideas

### Idea 1 — State is a grow-only set of immutable versions

```
Version = { path, parents: set<VersionId>, blob: Hash | ⊥, author: User, node: NodeId, forced: bool }
VersionId = hash(Version)            -- signed by `node`
Blob      = file content, addressed by its hash
```

- An **edit** is a version whose parents are the versions it replaces.
- A **delete** is a version with `blob = ⊥`.
- A **move** is a delete of the old path plus a create at the new path with the *same blob*
  (content addressing ⇒ nothing is re-transferred).
- A **merge** is a version with several parents.
- Nothing is ever modified or removed from the set. Two nodes **sync by union** of their sets.
  Union is commutative, associative and idempotent, so partitions, chains, and repeated
  split/merge need no special handling: everyone who has seen the same versions has the same state.

### Idea 2 — One authorization rule

```
owner(p)   = u   if the topmost folder named `u_` on the path of p exists, else none
writers(p) = {owner(p)}  or  ∅ when owner(p) = none   (∅ = frozen)

authorized(v) =
    v.forced = false  and
    if v.parents = ∅ :  writers(v.path) ∈ { ∅, {v.author} }      -- creation
    else             :  v.author ∈ writers(v.path)                 -- modification
```

Consequences, with no extra rule:
- Folders are frozen by default; anyone may add, nobody may modify.
- Only the owner modifies `alice_/…`. Anything below `alice_/` is alice's, so "an owned folder
  cannot contain a frozen folder" and "`bob_/` inside `alice_/` means nothing" hold by construction.
- A **proposal** is simply an unauthorized version: bob editing `alice_/f`, or anyone editing a
  frozen file. Proposals sync like everything else (so the owner has them pre-downloaded).

### Idea 3 — Files on disk are a deterministic view of the set

```
visible(v)  = authorized(v) or v.forced
heads(p)    = maximal visible versions of path p        -- w.r.t. the parents order
render(p)   = the non-⊥ blobs of heads(p), mapped injectively to disk names
```

- One head → one file. Several heads → **conflict copies**. A deleted head renders nothing.
- So: concurrent edits, two nodes of one user, concurrent adds in a frozen folder, edit vs delete
  (the edit survives, since a delete renders nothing), conflicts of conflicts — all the same case.
- A stale node cannot resurrect a deleted file: its version is an ancestor, not a head.
- **Accept** a proposal x: owner writes a merge with parents `heads(p) ∪ {x}` and blob `x.blob`.
  **Reject**: the same merge with the current blob. Accept and reject are symmetric.
- **Force-confirm** (unsafe): a non-owner writes that merge with `forced = true`; it is visible to
  everyone. If the owner edited concurrently, there are two heads — an ordinary conflict.
- Unauthorized, unforced versions are listed under `.pigeon/proposals/` (name TBD), not rendered.
- The disk mapping `render` must be injective on every target OS: conflict copies, case-insensitive
  collisions (`Foo`/`foo`) and names invalid on Windows are all resolved by the same deterministic
  disambiguation function.
- **Scanner** = the inverse of `render`: a local change on disk becomes a new version whose parents
  are the heads it was rendered from. Scanner and renderer are the only two doors to the disk.

### Idea 4 — Live content survives through custody tokens

```
live blobs = blobs of current heads  ∪  blobs of pending proposals
K          = replication factor (group setting, maybe per folder; default TBD)
Token      = (blob, i), i ∈ 1..K   -- "my disk keeps this blob"
```

- Every live blob has K tokens. A node holding a token keeps the content; a node holds at most one
  token per blob, so a *fully placed* blob is on K distinct disks. Nodes without a token are
  caches: they fetch and evict freely. History (below) is never covered.
- A new version starts with all K tokens on its author's node, which places K−1 of them by
  hand-off. Until then the blob is **under-replicated**, shown as "not yet backed up (1/K)".
- **Hand-off** — the only way to give a token away:
  1. n sends `offer(token, id)` to m, with a fresh random `id`;
  2. m accepts iff it has the content, holds no token for this blob, and is not itself unsyncing it;
     it then holds the token and replies `ack(id)`; otherwise `nack(id)`;
  3. n forgets the token (and may then delete the content) only on an `ack` carrying *its current* `id`.
- No locks: a node that wants to leave never accepts, and a node that accepts never waits, so the
  dining-philosophers deadlock cannot occur. Verified in §6 against duplicated, reordered, delayed
  and lost messages and users changing their minds.
- **Unsync** a folder = hand off each token, then delete. Safe. If nobody can accept (last copies,
  holders offline): unsafe **Drop**, with a warning showing the known risk (which nodes were last
  seen holding the blob, and when).
- **Disk death** is a fault, not an action. Guarantee: a fully placed live blob survives any K−1 disk
  deaths. With K = 1 it survives none (unsync is still safe). Under-replicated blobs are flagged.
- **Forget node** (declared dead): its tokens are re-minted on nodes holding the content. If the node
  was only offline, there are extra tokens — over-replication, which is safe (see Q-2).
- A superseded blob's tokens dissolve once every version superseding it is fully placed, so the last
  fully placed state of every file survives.
- **Sync policy** per folder: `keep: bool` (false = unsync: hand off tokens, stay a cache),
  `priority: int` (0 = **paused**).
- **Diogene** (v2) = a node that accepts every token and never unsyncs; it makes hand-offs always
  possible. **Stop being diogene** = hand off its tokens — an ordinary unsync, not an unsafe action.

**History.** When a node's file is replaced by a new head, the old blob is moved to
`.pigeon/versions/` (name TBD) according to the node's **retention** setting:
`all` · `external` (only when overwritten by a change from another node) · `none`.
Cleanup is manual (or by that setting). History never syncs and never holds tokens.

## 2. Unsafe actions

Only two primitive escapes remain; every unsafe action of draft 0 is one of them.

| Escape | What it does | Covers |
|---|---|---|
| **Force** | write a version with `forced = true` | force-confirm a proposal; edit/move/delete in a frozen folder |
| **Drop** | destroy a token without hand-off | unsync when nobody can accept |

Both are warned (Drop with the known risk, §1 Idea 4), and both are recorded in the version set (forced versions are versions; drops are
token records), so they are auditable and, for Force, reversible (the old version stays an ancestor).
Running several nodes as one user is no longer unsafe: it only produces conflicts (Idea 3).

## 3. Glossary

| Term | Definition |
|---|---|
| **Group / group_key** | Nodes sharing one version set; the key grants membership. v2: invites, revocable keys. |
| **Node** | One pigeon install, with a key pair `node_id`. Declares the user it acts for. |
| **User** | Short string (`alice`). Owns `alice_/` folders. |
| **Version, Blob** | Idea 1. |
| **Frozen / owned folder** | `writers = ∅` / `writers = {u}` (Idea 2). |
| **Proposal** | An unauthorized, unforced version (Idea 2). |
| **Head, conflict copy** | Idea 3. |
| **Token, K, hand-off, under-replicated, forget node, diogene** | Idea 4. |
| **Force, Drop** | The two unsafe escapes (§2). |
| **Protocol version** | Exchanged at handshake; incompatible ⇒ no sync. v2: signed self-distribution of binaries. |

Discovery: mDNS on the LAN, optional rendezvous/relay server; both may coexist.

## 4. Invariants

Safety (with no Force/Drop):
- **INV-Immutable** Versions and blobs never change (they are hashes, signed by their node).
- **INV-Converge** Two nodes with the same version set render the same tree (render is a function
  of the set only; local policy affects which content is present, never names or heads).
- **INV-NoLoss** Every token of a live blob always has a holder that has the content (no disk
  deaths). Hence a fully placed live blob survives any K−1 disk deaths. *Model-checked.*
- **INV-Distinct** A node holds at most one token per blob. *Model-checked.*
- **INV-Authorized** Every rendered file comes from an authorized version.
- **INV-NoSilentOverwrite** The renderer never overwrites a disk file whose content is not yet
  ingested by the scanner (scan-before-write), and writes atomically (temp + rename).
- **INV-Membership/Compat** No version or blob is accepted from a node without a valid key or with
  an incompatible protocol version.

With Force/Drop: everything except INV-Authorized (Force) and INV-NoLoss (Drop) must still hold.

Liveness (fair gossip and fetch):
- **LIV-Converge** Connected nodes (directly or transitively) eventually have the same version set.
- **LIV-Content** A kept, unpaused folder eventually holds every head blob, if a holder is reachable.
- **LIV-Placement** A blob eventually becomes fully placed if K willing nodes stay reachable.
- **LIV-Unsync** A node's unsync eventually succeeds if a willing node stays reachable. (not yet checked)

## 5. Scenario catalogue

Each scenario names the idea that handles it. Most are now *ordinary cases*; they remain tests.
Nodes A–F; users alice, bob, carol.

### 5.1 Version set (Idea 1)
- **B-1** A adds `docs/f` → B gets it.
- **B-4** alice edits `alice_/f` → everyone gets it.
- **B-5** alice deletes `alice_/f`; B offline holds old f → reconnect → stays deleted.
- **B-6** alice moves `alice_/f` → `alice_/g/f` → no content re-transferred.
- **P-1** A B C ‖ D E F add files → reconnect → union everywhere.
- **P-2** chain A–B–C, A never meets C → C gets A's versions via B (B may relay blobs it holds no token for).
- **P-5** repeated, varying splits and merges → converges.

### 5.2 Authorization (Idea 2)
- **O-1** bob edits `alice_/f` → proposal → alice accepts → everyone sees bob's content.
- **O-2** … alice rejects → f unchanged, bob's version stays in history.
- **O-3** bob and carol propose concurrently → two proposals; accepting one leaves the other pending.
- **O-6** bob creates `alice_/x` → proposal (creation under foreign owner).
- **O-7** bob creates `alice_/bob_/y` → still alice's (topmost owner).
- **O-8** someone renames frozen `x/` to `bob_/` → that is a frozen delete ⇒ needs Force.

### 5.3 View (Idea 3)
- **B-2** A and B add frozen `docs/f` concurrently, different content → two heads → conflict copy.
- **B-3** … same content → same blob, single file.
- **M-1** alice on A and A' edits `alice_/f` offline → two heads → conflict copy.
- **M-2** alice deletes f on A, edits on A' → edit survives.
- **P-3** alice edits f on one side, bob proposes on the other → proposal's parent is not a head; alice can still accept (merge).
- **O-5** carol force-confirms while alice edits offline → two heads → conflict.
- **C-1** conflict copy edited concurrently → more heads of the same path; names stay bounded.
- **C-2** "stop & fix": user writes a merge of all heads (ordinary merge).
- **E-2** `Foo.txt` + `foo.txt` to macOS/Windows → disambiguated, both kept.
- **E-3** `a:b`, `CON` to Windows → disambiguated, kept.
- **E-1** file changes while being hashed → scanner retries, no version for a torn read.
- **E-4** crash mid-write → temp file discarded, rescan.
- **E-6** disk full → render stops, nothing ingested as a delete.
- **E-7** `.pigeon/` deleted → rescan; files on disk become versions with no parents ⇒ duplicates, not deletions (TBD: match by blob hash to avoid conflict copies).
- **E-8** synced root deleted or drive unmounted → deletes are versions, so recoverable from `.pigeon/versions/` per retention; confirmation above a threshold (TBD).

### 5.4 Tokens (Idea 4)
- **S-1** A unsyncs `photos/`, B keeps it → hand-off to B → A deletes.
- **S-2** A holds the last copy of `p.jpg` → nobody accepts → warning with risk ("last seen on B, 3 days ago"); Drop only if confirmed.
- **S-2b** B was seen holding `p.jpg` but is offline now → no hand-off → same warning (a stale sighting is not a proof).
- **S-3** A and B, the only two holders, unsync concurrently → neither accepts the other's token → both stay. *Model-checked.*
- **S-3b** naive "confirm a copy exists" → both delete; "+ promise not to delete meanwhile" → deadlock. *TLC counterexamples.*
- **S-10** token goes A→B→A while A's first ack is delayed; replies without offer id → both delete (ABA). *TLC counterexample; fixed by ids.*
- **S-11** B re-acks an offer for a token it already holds; A and B swap while acks are in flight → both delete. *TLC counterexample; fixed by nack.*
- **S-12** K = 2: one holder's disk dies → blob survives, flagged under-replicated; "forget node" → re-placed.
- **S-13** K = 2, the forgotten node comes back → extra token, nothing lost.
- **S-14** author's disk dies before any hand-off → lost; the UI had shown "not yet backed up" (documented limit).
- **S-5** A pauses `videos/` (priority 0), resumes → catches up.
- **S-6** priority on `urgent/` during a large transfer → goes first.
- **S-8** A keeps a folder whose only holder is offline → waits, reports.
- **S-9** retention `external`: B overwrites A's `f` → old f in A's `.pigeon/versions/`; A's own edit → not kept.
- **D-1** (v2) diogene D stops → ordinary hand-offs; its last copies need Drop.

### 5.5 Membership & versions
- **V-1** incompatible protocol versions → refuse, both report.
- **K-1** wrong group_key on the LAN → rejected.
- **K-2** (v2) revoked key → node excluded; its versions remain.

## 6. Verification plan

Done ([`models/tla/`](../../models/tla/), run with `java -cp tla2tools.jar tlc2.TLC -config <cfg> <module>`):
- `CustodyNaive.tla` — confirm-only (`Naive_FALSE`): **NoLoss violated**; confirm + lock (`Naive_TRUE`): **deadlock**.
- `CustodyTokens.tla` — hand-off of §1 Idea 4. Messages never consumed (so duplicated, reordered,
  delayed, or never delivered = lost), users change their minds, caches fetch and evict.
  | Model | cfg | Result |
  |---|---|---|
  | N=3 K=1, replies without id | `T_nononce` | TokensNeverLost violated (S-10) |
  | N=3 K=1, re-ack held token | `T_idem` | TokensNeverLost violated (S-11) |
  | N=3 K=1, ids, ≤ 5 offers | `T_n31o5` | no error (5.1 M states) |
  | N=3 K=2, ids, ≤ 5 offers | `T_n32o5` | no error (8.0 M states) |
  | N=4 K=2, ids, ≤ 4 offers | `T_n42o4` | no error (65.9 M states) |

To do:
- Token placement for new versions, forget node, superseded-blob dissolution; liveness (LIV-Unsync).
- **Whole-system model** (3 nodes, 2 users, 2 paths): version sets, authorization, view, tokens.
  Actions: `Write, Force, Gossip(union), Fetch, Offer, Receive, Reply, Drop, Crash, Connect, Disconnect`.
- **Disk door** model: scanner and renderer racing with user edits (INV-NoSilentOverwrite).
- Scenario tests §5 on a deterministic simulated network, then real processes; random op sequences +
  partitions checking INV-NoLoss and INV-Converge.

## 7. Open questions

Decided 12 Oct (Laurent): unproven unsync is unsafe with a risk warning; proven (hand-off) is safe ·
history in `.pigeon/versions/`, manual cleanup, retention `all|external|none`, not guaranteed ·
unsync is a token hand-off (model-checked) · disk death is a fault: K copies survive K−1 deaths.

- **Q-1** Default K, and can it differ per folder?
- **Q-2** Forget node: who may declare a node dead, and how are extra tokens retired when it returns?
- **Q-3** Disambiguation function for disk names (conflicts, case, invalid names).
- **Q-4** Empty folders, symlinks, permissions, xattrs: versions of their own or ignored?
- **Q-5** Users are claimed by nodes, not proven: acceptable for v1?
- **Q-6** Renaming a user (`alice` → `alicia`)?
- **Q-7** Default retention setting, and the mass-deletion threshold (E-8).
- **Q-8** CBOR or Protobuf.

## 8. v2 backlog
Invites & revocable keys · signed self-update · diogene · internet rendezvous/relay.
