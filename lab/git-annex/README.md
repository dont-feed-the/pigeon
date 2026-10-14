# git-annex experiments

Shell scripts run against git-annex 10.20261006 (standalone build in
`~/git-annex.linux`), each in its own lab directory under `~`. Results are
summarised in [`docs/design/GIT-ANNEX.md`](../../docs/design/GIT-ANNEX.md) and
in the proposal, §5.

| Script | Experiments | Asserts? |
|---|---|---|
| `ga-setup.sh` | lab with repositories A, B, C for E1–E6 | no |
| `ga-e1.sh` | E1 concurrent drops of the last copies | no, prints |
| `ga-e2.sh` | E2 deletion keeps content, E3 concurrent edits, E6 absent content | no, prints |
| `ga-e3.sh` | E6' fresh clone, E4 mass deletion | no, prints |
| `ga-e7.sh`, `pigeon-pull.py` | E7 proposals filtered at pull | no, prints |
| `ga-e8-thin.sh` | E8 thin laptops | no, prints |
| `ga-e9-custody.sh` | E9 roles: untrusted laptops, custodians, K = 2 | no, prints |
| `ga-e10-partial-custodians.sh`, `ga-e10c-partial-history.sh` | E10 partial custodians | no, prints |
| `ga-e14-purge.py` | E14 purge by references and seals: shared keys, a restore against the seals, per-key transfers, the forced drop, a returning laptop and a new generation; git-annex contract checks (ADR 0042, issues #4 and #5); about 25 seconds | **yes** |
| `ga-e15-sparse-folders.sh` | E15 freeing space per folder with sparse checkout (ADR 0031) | **yes**, exits non-zero on failure |
| `ga-e15b-pointer-free-transitions.sh` | E15b no pointer file during freeing and keeping, observed continuously, with `kill -9` trials (ADR 0039, issue #2) | **yes** |
| `ga-e18-unprotected-versions.sh` | E18 offline in-place saves against `annex.thin`, failed uploads, reconnection, re-thinning (ADR 0041, issue #3) | **yes** |
| `ga-e12-e13-p2p-boundary.py` | E12 and E13 over git-annex's generic P2P transport (a lab network): `remotedaemon` only fetches; pushed refs; remote `REMOVE`; append-only `git-annex-shell`; pigeon's filter with random read sizes; copy counts from `.gitattributes`, the branch and the command line; the hooks git-annex installs (ADR 0043, issue #9); about 90 seconds | **yes** |
| `ga-e19-install-races.py` | E19 install racing application saves and `kill -9`, thin and not thin, with a fuzzing writer (ADR 0040, issue #1); Linux, about 3 minutes | **yes** |
| `ga-e20-filesystem-objects.py` | E20 the file-system objects a shared folder may hold: regular files and the execute bit, empty directories, symlinks, hard links under `annex.thin`, xattrs, FIFOs and sockets, nested repositories, unreadable paths, in-tree `.gitignore` and `.gitattributes`, files rewritten during add, awkward names (ADR 0044, issue #14); Linux and macOS, run by the `lab` workflow; about 20 seconds | **yes** |
| `ga-e21-disaster-drill.py` | E21 the disaster drill of issue #11: every laptop and its key lost, a fresh machine rebuilds names, hashes, deleted history, the suggestion and every signature from two bare custodians; content on K custodians without its commit is lost to names (the old *protected* rule); a recovery sheet adds a device of its person and nothing else; plain git-annex commands export complete files; a copied repository counts only under a new UUID after a hashing fsck; git's default `core.fsync` and git-annex leave received objects unsynced; Linux and macOS, run by the `lab` workflow; about 15 seconds | **yes** |
| `ga-e22-domains-and-capacity.py` | E22 failure domains and capacity limits of issues #12 and #13: storage fingerprints (mountinfo and `lsblk -s`, `/dev/disk/by-uuid`, the mount source; `diskutil` on macOS) and host fingerprints (`/etc/machine-id`, IOPlatformUUID) agree for two custodians on one volume, follow a loop mount or disk image to the disk holding its file, and differ on a tmpfs; git-annex counts two UUIDs on one disk, even one hard-linked inode, as two copies; a drop that ignores all but one remote per domain refuses until another domain holds the key; `annex.diskreserve` refuses gets and incoming copies over a local path and p2pstdio; `maxsize` with `sizebalanced` leaves keys with one custody copy; a real full tmpfs (ENOSPC); a custodian killed or deleted mid-transfer; Linux (sudo) and macOS, run by the `lab` workflow | **yes** |
| `ga-e23-p2p-drop.py` | E23 git-annex's verified drop over its P2P protocol (issue #10, ADR 0054): three custodians served by `remotedaemon` on a lab network; a drop counts a copy its owner holds locked for removal, since the server answers `CHECKPRESENT` with `inAnnex`, while over a local path it refuses; two concurrent drops leave no copy (numcopies 1) or one of three (numcopies = mincopies = 2); behind a filter that answers `CHECKPRESENT` with `LOCKCONTENT` both races keep every copy and a lone drop succeeds; a stale log is not counted unless its custodian is `trust`ed; a `dead` one never counts (asserts, 12 checks, about 15 s) |

The lab joins CI once its scripts assert, as E12/E13, E14, E15, E15b, E18, E19,
E20, E21, E22 and E23 do (ADR 0026); the `lab` workflow runs E20, E21, E22 and E23 on Linux and macOS so far. Pending:
E11 native Windows.
