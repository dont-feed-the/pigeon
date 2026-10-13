# pigeon — related work

Short survey for the spec discussion. Each entry: what pigeon shares with it, and how it differs.

## Closest systems

| System | Shares with pigeon | Key difference |
|---|---|---|
| **Syncthing** (Block Exchange Protocol) | P2P sync, LAN discovery + optional global discovery/relays, per-file version vectors, conflict copies, *send-only / receive-only* folders, file versioning (trash) | Assumes every device holds the whole folder: it does not track *where* content lives, so no safe unsync and no K-copies guarantee. Permissions are per device/folder, not per user. Deletes propagate (no frozen default). No proposal/approval workflow. |
| **git-annex** | *Closest on durability.* Tracks the location of every file's content; `numcopies`/`mincopies`; refuses to drop unless enough other copies are verified, locking content on the remote to survive concurrent drops; trust levels: *trusted* ≈ diogene, *dead* ≈ forget node, *untrusted* ≈ portable drive | Verifies by **locking** a copy that must be online at drop time; pigeon **transfers custody** (tokens) instead. Its docs note that with remotes that cannot lock, concurrent drops can break `numcopies` (hence the `mincopies` floor). Metadata lives in git; files are symlinks/pointers; sync is mostly explicit (the assistant automates it). No ownership. |
| **IPFS + IPFS Cluster** | Content addressing; **pinning** (same word, same idea); Cluster's `replication_factor_min/max` ≈ our K | A global content network, not a shared directory tree; Cluster keeps the pinset with consensus (Raft or CRDT) and allocates pins centrally. No ownership, no conflicts. |
| **Bayou** (Terry et al., SOSP 1995) | Weakly connected replicas, anti-entropy, *tentative* writes that a *primary* later commits ≈ proposals accepted by the owner | One primary for the whole database, application-specific merge procedures; pigeon has one "primary" (owner) per folder. |
| **Hypercore / Hyperdrive** (Dat) | Signed append-only logs, P2P, a drive writable only by its owner ≈ `alice_/` | No frozen semantics, no custody/replication guarantee; multi-writer is an add-on (Autobase). |
| **Unison** (Balasubramaniam & Pierce 1998; Pierce & Vouillon 2004) | A *formally specified* synchronizer: what "safe" sync means | Two replicas, run on demand; pigeon is n-way and continuous. |
| **Coda, Ficus** | Disconnected operation, optimistic replication; Coda's *hoarding* ≈ partial sync | Client–server (Coda); no ownership model. |

## Building blocks and theory

- **Optimistic replication survey** — Saito & Shapiro, ACM Computing Surveys 2005. The vocabulary for §1 Ideas 1 and 3 (version vectors, anti-entropy, conflict handling).
- **CRDTs** — Shapiro et al. 2011. "Sync by union of a grow-only set" (Idea 1) is the simplest state-based CRDT; convergence comes for free.
- **Local-first software** — Kleppmann et al., Onward! 2019. Same goals (offline, P2P, user ownership of data).
- **Replicated tree moves** — Kleppmann et al., "A highly-available move operation for replicated trees", IEEE TPDS 2022. Explains why folder moves with identity are hard (cycles); pigeon side-steps it with move = delete + create.
- **Tra** — Cox & Josephson, "File synchronization with vector time pairs", MIT 2005. Compact metadata that handles deletions and *partial* sync; candidate for the metadata encoding.
- **git / Mercurial** — content-addressed immutable DAG of versions and merges (Idea 1), but manual commits.
- **Upspin** (Google) — names rooted at a user (`ann@example.com/…`), owner-controlled writes ≈ `alice_/`.
- **Tahoe-LAFS** — K-of-N erasure-coded durability over storage servers ("servers of happiness").
- **Distributed reference counting** — weighted reference counting (Bevan 1987; Watson & Watson 1987) conserves weights to avoid message-reordering races, as our tokens conserve custody; the bug TLC found (S-10) is the classic **ABA** problem.
- **Testing sync engines** — Dropbox, "Testing sync at Dropbox" (Nucleus, 2020): deterministic randomized testing (CanopyCheck, Trinity). A model for §6 scenario tests.
- **Formal methods in industry** — Newcombe et al., "How Amazon Web Services uses formal methods", CACM 2015 (TLA+).
- **Placeholders** — Dropbox Smart Sync, OneDrive Files On-Demand: prior art for files that are known but not present (S-7).

## What seems new in pigeon's combination

1. **Durability is a checked guarantee of an automatic sync tool**: Syncthing-style continuous sync
   plus git-annex-style knowledge of where content lives, with a *lock-free* custody hand-off
   (no waiting, so no deadlock). Like git-annex, the receiving node must be online to answer; the
   difference is that custody is explicit and countable rather than inferred from location logs.
2. **Per-user ownership with proposals and no server**: Bayou's primary commit, per folder.
3. **Append-only by default** (frozen folders): most sync tools propagate deletes; closer to
   WORM archives.
4. **Unsafe actions are recorded escapes** (Force, Drop) inside the data model, not configuration
   switches.
5. **Spec and model checking first.**

## Worth borrowing

- git-annex: trust levels, `numcopies` vs `mincopies`, its analyses of concurrent drops.
- Syncthing: block-level transfer, conflict naming, discovery/relay design, its history of
  data-loss bugs (case-insensitive filesystems, ignore patterns, mass deletes) as test scenarios.
- Dropbox: deterministic simulation testing.
- Tra / Kleppmann: metadata encoding and, if ever needed, folder moves with identity.

## Choosing a base (12 Oct)

Checked 12 Oct 2026: iroh 1.3.0 (MIT/Apache-2.0), iroh-blobs 0.103, iroh-docs 0.101, iroh-mdns-address-lookup 0.6 (n0), notify 8.2, libp2p 0.57; Syncthing MPL-2.0; git-annex AGPL-3.0-only.

| Candidate | Verdict |
|---|---|
| Fork **Syncthing** (Go) | Most mature disk and network code, but its model (full replicas, per-file version vectors, deletes propagate) *is* what pigeon replaces: a fork would fight upstream. Use as a reference and a source of test scenarios; reuse its libraries only if we pick Go. |
| Build on **git-annex** (Haskell) | **Revised 12 Oct, see GIT-ANNEX.md:** experiments show it already solves custody (race-safe drops), partial content, purge, conflicts, scrubbing, and has an iroh transport. Recommended v1 base, with pigeon as a policy + UX layer; costs: pointer files, 2× disk for unlocked files without reflinks, Windows beta. |
| **IPFS / Hypercore** | Wrong shape: a global content network / a JS runtime. |
| **Rust + iroh** (own engine) | Library, not framework: QUIC connections dialed by public key (= `node_id`), hole punching + relays, mDNS lookup, BLAKE3 content-addressed verified and resumable blob transfer (= our blobs). Pre-1.0 blobs/docs crates ⇒ API churn; iroh-docs' semantics do not match ours, so we write our own version-set reconciliation (small: it is a grow-only set). |

Proposed layering: **core** (version set, authorization, view, tokens) as a pure library with no I/O,
mirroring the TLA+ models and driven by a deterministic simulator · **disk door** (watcher via
`notify`, scanner, renderer), with Syncthing as the reference for filesystem quirks ·
**network** (iroh + iroh-blobs).
