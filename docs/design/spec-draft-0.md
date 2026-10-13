# pigeon — specification (skeleton, draft 0)

> **Superseded** by [docs/spec/](../spec/README.md) on 13 October 2026. Kept for the record.

> Status: draft for discussion. Edit freely. `TBD` = undecided, `v2` = later, but v1 must not block it.
> Wire protocol: language-neutral (CBOR or Protobuf, TBD), documented here. Implementation language chosen *after* the spec (Rust favoured: iroh/libp2p, mDNS, chunking).

## 0. Goals

1. Sync files between several computers, peer-to-peer.
2. **No data loss** as long as the hard rules (§3) are respected.
3. Allow **unsafe actions** (hard rules broken) that try their best not to lose data, with no guarantee, always behind an explicit warning.

Non-goals (v1): real-time collaborative editing, access control beyond "owned folders", internet-scale groups.

---

## 1. Glossary

| Term | Definition |
|---|---|
| **Group** | The set of nodes that sync one shared tree. |
| **group_key** | Shared secret granting membership of a group. v2: invite requests accepted by hand, several revocable keys. |
| **Node** | One pigeon installation on one computer. Has a stable `node_id` (key pair). |
| **User** | Short string naming a person (`alice`). Not tied to a computer. One user *should* run one node (see U-2). |
| **Tree** | The full metadata of the group's files (paths, versions, owners, tombstones). Every node knows the whole tree, even for files it does not hold. |
| **Content** | The bytes of one file version, addressed by its hash (`blob_id`). |
| **Version** | An immutable pair (path, blob_id), with author (user, node), parent version(s) and causal metadata (version vector / HLC, TBD). |
| **Frozen folder** | Folder where files can be *added* but not edited, moved or deleted. Default type of every folder. |
| **Owned folder** | Folder named `<user>_` (e.g. `alice_`). Read-write for its owner, read-only for everyone else. |
| **Owner** | The user named by an owned folder. |
| **Proposal** | A change suggested by a non-owner to an owned folder. Staged in `.pigeon/to-confirm/` (name TBD) until the owner accepts or rejects it. |
| **Force-confirm** | Unsafe: a non-owner applies a proposal without the owner. |
| **Sync state** | Per folder/file on a node: `sync` (content kept up to date) or `unsync` (content removed locally, metadata kept). |
| **Pause / priority** | Modifiers of `sync`: paused folders do not transfer; high-priority folders transfer first. |
| **Holder** | A node that has the content of a version on disk. |
| **Diogene** (v2) | A node promising never to unsync anything again (it may still be partially synced). Lets other nodes prove an unsync is safe. |
| **Unsync proof** | Evidence that a version is held elsewhere, required before dropping local content. |
| **Conflict** | Two versions of one path, neither descending from the other. Resolved by keeping both (conflict copy). |
| **Conflict copy** | A file such as `report (conflict alice@laptop 2025-01-01).txt` holding the losing side. Name format TBD. |
| **Tombstone** | Metadata recording a deletion, so that it propagates and is not resurrected. |
| **Partition** | A subset of nodes that can talk to each other but not to the rest (A B C ‖ D E F). |
| **Hard rule** | A rule whose respect makes the no-loss guarantee hold (§3). |
| **Unsafe action** | An action breaking a hard rule; allowed after a warning (§5). |
| **Validation stage** | A hold point where an unsafe change does not propagate until confirmed. |
| **Protocol version** | Version of the wire format and semantics; nodes with incompatible versions refuse to sync. |

---

## 2. Model

### 2.1 Membership & discovery
- A node joins a group by knowing `group_key`. All traffic authenticated & encrypted with keys derived from it (TBD: PAKE / noise handshake).
- Discovery: mDNS on the LAN (no server needed); optional rendezvous/relay server for the internet. Both may coexist.
- v2: per-invite keys, manual approval, revocation.

### 2.2 Versions & compatibility
- Handshake exchanges `protocol_version`; incompatible ⇒ refuse to sync, tell the user.
- v2: nodes serve their own signed binaries; a node only installs binaries signed by a trusted release key.

### 2.3 Folder types
- Every folder is **frozen** unless it is an **owned** folder `<user>_`.
- A frozen folder may contain owned folders, which override it.
- An owned folder may **not** contain a frozen folder (everything below `alice_` belongs to alice).
- TBD: may `alice_/` contain `bob_/`? (proposal: no — names below an owned folder carry no meaning.)
- TBD: who may create `alice_/`? (proposal: only alice; anyone else doing it is unsafe or refused.)

### 2.4 Operations
| Op | Where | Who | Safe? |
|---|---|---|---|
| Add file | frozen | anyone | safe |
| Add / edit / move / delete | owned | owner | safe |
| Propose change | owned | non-owner | safe (staged) |
| Accept / reject proposal | owned | owner | safe |
| Force-confirm proposal | owned | non-owner | **unsafe** |
| Edit / move / delete | frozen | anyone | **unsafe** |
| Unsync | any | node | safe only with unsync proof |
| Pause / prioritise | any | node | safe |
| Become diogene (v2) | — | node | safe |
| Stop being diogene (v2) | — | node | **unsafe**, announced |
| Run 2 nodes as one user | — | user | **unsafe** (v2: maybe safe if conflicts handled) |

### 2.5 Partial sync
- Metadata (tree) always fully replicated; content per node according to sync state.
- `unsync` deletes local content only after an **unsync proof** (§3, H-4).
- v1 without diogene: TBD (see Q-1).

### 2.6 Propagation
- P2P gossip of metadata; content fetched from any holder.
- Partitions merge by exchanging missing versions; no node is a master.
- TBD: data model — per-node append-only op log (CRDT) vs per-file version vectors.

---

## 3. Hard rules (the user's side of the contract)

- **H-1** One user ⇄ one active node.
- **H-2** Never edit, move or delete files in a frozen folder.
- **H-3** Never write in someone else's owned folder except through proposals; never force-confirm.
- **H-4** Unsync only with an unsync proof (pigeon enforces this unless overridden).
- **H-5** Do not modify files through means pigeon cannot observe while it is not running (TBD: or pigeon rescans on start and treats them as local edits).
- **H-6** (v2) Do not stop being a diogene.
- Out of scope: a node whose disk dies before its new versions reached another node (no system can save that). TBD: should a version count as "committed" only after ≥1 other holder acks?

---

## 4. Invariants

To check in TLA+ (safety: `[]P`; liveness: `<>P` / `~>`) and in tests.

### Safety
- **INV-NoLoss** For every version V ever committed and not *intentionally superseded*, some node holds V's content. (Define "intentionally superseded": owner edit/delete — TBD whether history of owned files must be kept; see Q-3.)
- **INV-NoSilentOverwrite** A node replaces local version V by remote W only if W causally descends from V. Otherwise ⇒ conflict copy.
- **INV-FrozenImmutable** Under safe ops, once a path in a frozen folder has a version, that version stays the current one forever.
- **INV-FrozenAddCollision** Two concurrent adds of the same frozen path with different content both survive (one as conflict copy); with equal content they merge.
- **INV-OwnerOnly** Under safe ops, every version in `alice_/` is authored by alice or is a proposal accepted by alice.
- **INV-NoFrozenInOwned** No frozen-semantics folder exists below an owned folder.
- **INV-UnsyncProof** A node drops content of V only if, at that moment, another node is known to hold V (v2: a diogene that held exactly V).
- **INV-LastCopy** The last known holder of V can never unsync V by a safe action.
- **INV-NoResurrection** A deleted file is not brought back by a stale node (tombstones), unless that node holds a concurrent edit (⇒ conflict, edit wins/kept).
- **INV-ProposalIsolated** A proposal never changes the owner's file until accepted (or force-confirmed).
- **INV-Compat** No content or metadata is applied from a node with incompatible protocol version.
- **INV-Membership** No data is exchanged with a node lacking a valid group key.
- **INV-AtomicWrite** A file on disk is always either a complete version or absent — never half-written (temp file + rename).
- **INV-UnsafeBestEffort** An unsafe action never immediately destroys content: superseded versions go to a trash/history kept until … (TBD).
- **INV-UnsafeAudit** Every unsafe action is warned before and logged in the tree (who, when, what).

### Liveness
- **LIV-Converge** If changes stop and the nodes are connected (directly or transitively), all nodes eventually agree on the tree.
- **LIV-Content** A synced, unpaused folder eventually holds the content of every current version, if a holder is reachable.
- **LIV-PartitionHeal** After two partitions reconnect, the union of their versions is eventually known everywhere.
- **LIV-ConflictStop** Cascading conflicts terminate once users stop editing; and a user can always "freeze" a conflict for manual fixing.

---

## 5. Unsafe actions

Each: warning text, what pigeon does to limit damage, validation stage if any.

- **U-1 Stop being diogene** (v2): broadcast "no longer diogene" with the last version vector; nodes that unsynced relying on it re-evaluate (refetch or warn).
- **U-2 Two nodes, one user**: allowed; concurrent edits ⇒ conflict copies. v2: may become safe.
- **U-3 Write in someone else's owned folder**: becomes a proposal; content pre-downloaded by the owner into `.pigeon/to-confirm/`; applied to the owner's file only on acceptance, then re-synced to everyone.
- **U-4 Force-confirm**: any user applies a proposal without the owner. If the owner edited concurrently ⇒ conflict, manual merge.
- **U-5 Edit/move/delete in frozen folder**: validation stage TBD; old version kept in history.
- **U-6 Unsync without proof**: explicit "may lose last copy" warning.

---

## 6. Scenario catalogue

Format: **ID — setup → steps → expected / invariants**. Nodes A–F; users alice, bob, carol. `f` = a file.

### 6.1 Basics
- **B-1** A adds `docs/f` (frozen) → B receives it. *LIV-Content*
- **B-2** A and B add `docs/f` concurrently, different content, offline → reconnect → both versions present, one as conflict copy. *INV-FrozenAddCollision*
- **B-3** Same as B-2 with equal content → single file, no conflict copy.
- **B-4** alice edits `alice_/f` → all get the new version.
- **B-5** alice deletes `alice_/f` → tombstone everywhere; a node offline during deletion does not resurrect it. *INV-NoResurrection*
- **B-6** alice moves `alice_/f` → `alice_/g/f` → seen as move, not delete+add (content not refetched).
- **B-7** Large file (10 GB) interrupted mid-transfer → resumes, never half-written on disk. *INV-AtomicWrite*

### 6.2 Partitions
- **P-1** A B C ‖ D E F; each side adds files → reconnect → all see the union. *LIV-PartitionHeal*
- **P-2** Chain A–B–C (A never sees C) → A's files reach C through B, even if B has the folder unsynced (TBD: does B relay content it does not keep?).
- **P-3** alice edits `alice_/f` on side 1 while bob proposes a change to `f` on side 2 → reconnect → proposal is against a stale base; alice is shown both.
- **P-4** alice deletes `alice_/f` on side 1, bob's node on side 2 holds old `f` → reconnect → deleted, no resurrection.
- **P-5** Repeated split/merge with different splits each time → still converges.

### 6.3 Owned folders & proposals
- **O-1** bob edits `alice_/f` → proposal → alice accepts → everyone gets it.
- **O-2** … alice rejects → bob's change kept where? (TBD: bob's local copy restored to alice's version, his edit saved as `f (rejected …)`?)
- **O-3** bob and carol propose concurrently → alice sees two proposals, accepts one, other becomes stale.
- **O-4** alice edits `f` while bob's proposal is pending → proposal marked stale, needs rebase/merge.
- **O-5** carol force-confirms bob's proposal while alice is offline; alice edited `f` offline → reconnect → conflict, nothing lost. *INV-NoLoss, U-4*
- **O-6** bob creates `alice_/` before alice has one → refused or proposal (TBD).
- **O-7** bob creates `bob_/` inside `alice_/` → no ownership change (per §2.3 TBD).
- **O-8** A frozen folder is renamed to `bob_` (unsafe move) → semantics change; what happens to existing files?

### 6.4 Partial sync / unsync
- **S-1** A unsyncs `photos/` while B holds all its versions → allowed, content removed on A, tree kept.
- **S-2** A unsyncs `photos/` while A is the only holder of `p.jpg` → refused. *INV-LastCopy*
- **S-3** A and B, the only two holders, unsync concurrently, each relying on the other → at most one succeeds. *INV-UnsyncProof* (key TLA+ race)
- **S-4** (v2) A unsyncs, relying on diogene D; D had an older version → refused.
- **S-5** A pauses `videos/`; others edit; A resumes → catches up.
- **S-6** A marks `urgent/` high-priority during a large transfer → it goes first.
- **S-7** User edits a local file in an unsynced folder (placeholder?) — TBD semantics.
- **S-8** A re-syncs an unsynced folder whose only holder is offline → waits, does not fail silently.

### 6.5 Same user on several nodes
- **M-1** alice on A and A' edits `alice_/f` offline on both → reconnect → conflict copy, both kept. *U-2*
- **M-2** alice deletes `f` on A, edits it on A' → edit kept (delete loses). TBD rule.

### 6.6 Frozen folder unsafe edits
- **F-1** A edits frozen `docs/f` (unsafe) → warning; old version kept in history; others get the new version (after validation stage?).
- **F-2** A deletes frozen `docs/f` while B, offline, does too / edits it → reconnect → no loss of B's version.
- **F-3** A moves frozen `docs/` → `archive/docs/` → tree consistent on all nodes.

### 6.7 Diogene (v2)
- **D-1** D becomes diogene; A unsyncs relying on D; D stops being diogene → A warned / refetches.
- **D-2** Two diogenes; one stops → proofs relying on the other remain valid.
- **D-3** Diogene's disk dies → out of scope? (document).

### 6.8 Versions & membership
- **V-1** A (proto 1) meets B (proto 2, incompatible) → refuse, both tell user. *INV-Compat*
- **V-2** A–B–C where B is incompatible with both → A and C still sync if they can meet.
- **K-1** Node with wrong group_key on the LAN → discovered but rejected. *INV-Membership*
- **K-2** (v2) Revoked key → node excluded; its past versions remain.

### 6.9 File-system edge cases
- **E-1** File modified while being hashed/uploaded → no corrupted version published.
- **E-2** `Foo.txt` and `foo.txt` added on Linux, synced to macOS/Windows (case-insensitive) → no overwrite.
- **E-3** Name invalid on Windows (`a:b`, `CON`) → kept in tree, flagged, not lost.
- **E-4** Crash / power cut during download or metadata write → restart without corruption.
- **E-5** Clock skew of hours between nodes → ordering unaffected (causal metadata, not wall clock).
- **E-6** Disk full during sync → stops cleanly, no partial file, no tombstone emitted.
- **E-7** User deletes `.pigeon/` state dir → node rescans; must not interpret it as "delete everything".
- **E-8** User deletes the whole synced root (or drive unmounted) → must not propagate mass deletion (safety threshold / validation stage).
- **E-9** Symlinks, permissions, empty folders, extended attributes — policy TBD.
- **E-10** User changes their username → owned folders? TBD.

### 6.10 Cascading conflicts
- **C-1** A conflict copy is itself edited concurrently on two nodes → conflict of conflict; names stay bounded.
- **C-2** User invokes "stop & fix": path frozen from automatic resolution until marked resolved.

---

## 7. Verification plan

- **TLA+ model** (small scope: 3 nodes, 2 users, 2–3 paths, bounded versions):
  - State: per-node tree, per-node held blobs, network (in-flight messages, partitions), diogene flags.
  - Actions: `Add, Edit, Delete, Move, Propose, Accept, Reject, ForceConfirm, Unsync, Resync, Pause, Connect, Disconnect, Gossip, Fetch, Crash, Restart, DiogeneOn, DiogeneOff`.
  - Check §4 safety invariants with TLC; liveness under fairness of `Gossip`/`Fetch`.
  - Run twice: safe actions only (all invariants must hold) and with unsafe actions (only best-effort invariants expected).
- **Scenario tests**: §6 as executable tests on a simulated network (deterministic, seeded), then on real multi-process nodes.
- **Randomised / fuzz**: random op sequences + partitions, check NoLoss & Converge at the end.

---

## 8. Open questions

- **Q-1** v1 has no diogene: how is an unsync proven safe? Options: (a) a holder ack ("I hold V and promise to keep it until you re-sync or I hand it on") — custody transfer; (b) unsync is always unsafe in v1.
- **Q-2** When is a version "committed" (counted by NoLoss)? On local write, or once ≥1 other node holds it?
- **Q-3** Does an owner's edit/delete of `alice_/f` drop the old version, or does pigeon keep history? For how long?
- **Q-4** Conflict copy naming and where they live.
- **Q-5** Delete vs concurrent edit: edit always wins?
- **Q-6** Should a non-owner node keep a copy of a *rejected* proposal?
- **Q-7** Can a node relay content it does not want to keep (P-2)?
- **Q-8** Data model: op-log CRDT vs per-file version vectors.
- **Q-9** Mass-deletion safety threshold (E-8).
- **Q-10** CBOR or Protobuf.

## 9. v2 backlog
Invites & revocable keys · signed self-update/binary serving · diogene mode · safe multi-node per user · internet rendezvous/relay.
