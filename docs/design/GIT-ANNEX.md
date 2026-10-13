# git-annex as pigeon's base — a closer look (12 Oct 2026)

Guillaume challenged my earlier verdict ("good for prototyping, not as the product"). I went back to
the docs and ran experiments with **git-annex 10.20261006** (standalone build, git 2.55) on local
repositories. Scripts: [`lab/git-annex/`](../../lab/git-annex/).

## Revised verdict

**I was too dismissive.** git-annex already implements the hardest and riskiest half of pigeon:
custody of copies with race-safe drops, partial content, history with explicit purge, conflict
copies, scrubbing, trust levels, and even an iroh transport. These are exactly the parts my review
flagged as unsound or reinvented (S2, S5, S7, N1, N3). What it lacks (owned and frozen folders,
proposals, a zero-config group, a friendly UX) is a **policy and UX layer**. A 60-line proof of
concept shows the authorization rule fits on top of git (E7).

Recommendation: **pigeon v1 = a policy + UX layer over git-annex**, keeping the spec engine-neutral
so that a custom engine stays possible. Only write our own engine if the prototype hits a real
blocker: Windows, pointer-file UX, disk usage, or scale.

## What I verified by experiment

| # | Scenario | Result |
|---|---|---|
| E1 | A and B hold the only 2 copies (numcopies 1) and drop **concurrently**, 40 trials | **40/40: exactly one drop succeeded, no loss.** git-annex locks the remote copy during the drop. This is our S-3 race, solved. |
| E5 | numcopies 2, a stale location log claims 3 copies | drop **refused**: git-annex verifies live copies instead of trusting logs |
| E2 | A deletes a file; B syncs with `--content` | file leaves B's tree, **content stays on disk**, listed by `git annex unused` until an explicit `dropunused` (= the *purge* proposed in REVIEW S2) |
| E4 | B does `rm -rf docs`, runs `git annex assist` | the deletion propagates to A's tree, but content stays; **`git revert` restores everything** |
| E3 | A and B edit the same file concurrently | both kept: `s.variant-9685.txt`, `s.variant-df47.txt` (conflict copies) |
| E6 | a clone that never fetched content | files are 102-byte **pointer files** (`/annex/objects/SHA256E-…`); `git annex adjust --hide-missing` hides them instead |
| E8 | `annex.thin` (hard links): one app edits a file **in place**, an editor **writes-and-renames** another; then B unsyncs both, relying on A | 1× disk (2.3 MB for 2 MB). In-place edit corrupts A's old copy, **but git-annex notices: B's drop is refused**, so nothing is lost. Write-and-rename leaves A's old copy intact, and B's drop succeeds. |
| E7 | pigeon's authorization rule as a pull filter (`pigeon-pull.py`) | bob's addition in frozen `docs/` merged; bob's edit of `alice_/notes.txt` kept as `refs/proposals/B/…`; alice accepts it by cherry-pick; content follows via git-annex |

## pigeon spec → git-annex

| pigeon | git-annex | Status |
|---|---|---|
| sync agent | `git annex assistant` (daemon), `git annex assist` (one-shot) | ✓ |
| full tree known, partial content | git tree + unlocked pointer files; `adjust --hide-missing` | ✓ (UX ◐) |
| per-folder sync/unsync | preferred content: `git annex wanted . "include=photos/*"` | ✓ |
| pause / priority | stop daemon, `annex-ignore`; no priority | ◐ / ✗ |
| custody, K copies | `numcopies` / `mincopies`, verified drops with content locking | ✓ (E1, E5) |
| diogene | standard group `backup` (wants even deleted content), `required`, `trust` | ✓ |
| forget node | `git annex dead`, `untrust` | ✓ |
| scrubbing (S7) | `git annex fsck` (also `--from` remotes) | ✓ |
| conflict copies | `.variant-XXXX` (`annex.resolvemerge`) | ✓ (E3) |
| immutable versions, history | git commits (content-addressed keys) | ✓ |
| purge (S2 fix) | `unused` / `dropunused` | ✓ (E2) |
| mass-delete guard | none, but recoverable by revert | ◐ → pigeon layer (E4) |
| metadata compaction (S5) | `git annex forget` for location logs; git history stays (small) | ◐ |
| users bound to keys (S1) | signed commits (SSH/GPG) + `git verify-commit` | building block |
| owned / frozen folders | — | ✗ → pigeon layer (E7) |
| proposals, accept/reject | git branches and refs, cherry-pick / merge | ✗ → pigeon layer, natural in git (E7) |
| Force / Drop | a commit marked forced / `git annex drop --force` | ◐ / ✓ |
| P2P, NAT traversal | `git annex p2p` over Tor; Magic Wormhole pairing; **`git-annex-p2p-iroh`** helper (iroh `dumbpipe`, by Joey Hess, 2025) | ✓ |
| LAN group discovery | assistant *local pairing* (multicast UDP + shared secret), pair by pair | ◐ (no group-wide key) |
| protocol compatibility | repository versions (v10) | ✓ |

## Disk usage: why 2×, and how to avoid it (incl. Windows)

**Why.** git-annex keeps content in an immutable, hash-addressed store (`.git/annex/objects`). A
*locked* file is a symlink into the store (1 copy, read-only). An *unlocked* file must be a normal,
writable file. If it shared storage with the store, editing it would silently change "immutable"
content, so by default git-annex keeps a second copy. That copy is what preserves the previous
version when you edit locally.

**Options.**
| Option | Disk | Trade-off | Windows |
|---|---|---|---|
| default (copy) | 2× | safe; local old versions kept | yes |
| `annex.thin` (hard link) | 1× | an *in-place* edit destroys the local old version; git-annex notices and refuses to count that copy when another node drops (E8). Editors that write a temp file and rename it keep the old version. | **yes on NTFS** (not FAT/exFAT) |
| reflink / copy-on-write | 1× | safe, both versions | documented for btrfs and xfs; Windows ReFS / Dev Drive block cloning **untested** |
| locked files (symlinks) | 1× | read-only, edit needs `unlock` | no: git-annex sets Windows repos to unlocked |

**For pigeon.** Use `annex.thin` on laptops, NTFS included, and let one *backup* node (git-annex's
`backup` group ≈ diogene) keep history. That is Syncthing's footprint and matches the spec's
retention `external`. Old versions are best-effort history in the spec anyway, and E8 shows the
current-version guarantee survives in-place edits. FAT/exFAT USB drives should be backup or
transfer remotes, not working trees.

## Real costs and risks (to test in the prototype)

1. **Disk usage**: 2× by default for unlocked files; 1× with `annex.thin` (see the section above, E8).
2. **Pointer files**: a file whose content is absent is a small text file. Users may open, copy or
   edit it. `--hide-missing` helps but changes the tree people see.
3. **Windows**: the docs call the port beta. The command line is "reasonably good", the assistant is
   usable, local pairing has known issues and Tor is unsupported. macOS and Linux are mature.
4. **No block-level deltas**: editing a 4 GB file transfers the whole new version (Syncthing does
   block deltas).
5. **Scale**: hundreds of thousands of files are fine; beyond that, git slows down.
6. **Where the policy layer plugs in**: `git annex sync` and the assistant merge automatically, so
   pigeon must own the git-level fetch/validate/merge and leave git-annex the git-annex branch and
   content (`sync --only-annex`, `satisfy`). E7 does exactly this; the assistant's auto-commit needs
   `annex.autocommit=false`, or pigeon's own watcher.
7. **People**: Haskell code and essentially one maintainer (Joey Hess), but 15 years of releases and
   still active (Oct 2026 build). AGPL-3.0 applies to git-annex itself; a separate wrapper calling the
   binary can have its own licence.
8. **Trust model**: any member can rewrite git-annex-branch settings (numcopies, trust). Same as
   pigeon today ("all key holders are trusted"), but it should be stated.

## What pigeon still needs to build

- **Policy layer** (E7, grown up): signed commits; validation of incoming commits against
  owned/frozen rules; proposals as refs, with accept/reject UI; Force as a marked commit; mass-delete
  guard and root marker before committing local changes.
- **Group UX**: one invite code → pairing with every member (wrapping `p2p --pair` / iroh helper),
  sensible defaults (`annex.addunlocked`, numcopies = K, preferred content per folder).
- **Front end**: tray app / status page — what is backed up (`find --lackingcopies`), proposals,
  warnings.

## Prototype plan (1–2 weeks)

1. Three machines (Linux, macOS, Windows), unlocked files, numcopies 2, iroh helper.
2. pigeon daemon in any language: watcher → mass-delete guard → `git annex add` → signed commit;
   periodic `pigeon-pull` of each peer + `git annex satisfy`.
3. Run the spec's §5 scenarios on it; measure disk usage, pointer-file confusion, Windows behaviour.
4. Decide: keep git-annex as the engine, or write ours with the lessons learned.
